// Audio-device capture: feeds any system input device (a sound card fed
// from a radio, or a virtual cable such as BlackHole / VB-Cable carrying
// another program's output — e.g. a RTTY contest simulator) through the
// same RX pipeline that TCI audio uses. Lets the decoder be exercised
// without a TCI radio or on-air activity.

use std::sync::Arc;
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, RwLock};
use tokio::task::JoinHandle;
use tracing::{info, warn};

use crate::dsp::{RttyTunable, RxPipeline};
use crate::scp::ScpDb;

/// How often the running status (with the level meter) is re-emitted.
const STATUS_INTERVAL: Duration = Duration::from_millis(200);

#[derive(Debug, Clone, Serialize)]
pub struct AudioDevice {
    pub name: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum AudioInStatus {
    Idle,
    Running {
        device: String,
        sample_rate: u32,
        channels: u16,
        /// Peak absolute sample over the last status interval, 0..1.
        peak: f32,
    },
    Error {
        message: String,
    },
}

/// Handle to a running capture: dropping/sending on `stop` ends the
/// dedicated audio thread (cpal streams are !Send, so they live there).
struct Capture {
    stop: std::sync::mpsc::Sender<()>,
    drain: JoinHandle<()>,
}

pub struct AudioInput {
    app: AppHandle,
    rtty: Arc<RttyTunable>,
    scp: Arc<ScpDb>,
    capture: RwLock<Option<Capture>>,
    status: RwLock<AudioInStatus>,
}

impl AudioInput {
    pub fn new(app: AppHandle, rtty: Arc<RttyTunable>, scp: Arc<ScpDb>) -> Self {
        Self {
            app,
            rtty,
            scp,
            capture: RwLock::new(None),
            status: RwLock::new(AudioInStatus::Idle),
        }
    }

    pub async fn status(&self) -> AudioInStatus {
        self.status.read().await.clone()
    }

    async fn set_status(&self, s: AudioInStatus) {
        *self.status.write().await = s.clone();
        let _ = self.app.emit("audio_in:status", &s);
    }

    /// Enumerate input devices on the default host. Blocking (CoreAudio /
    /// WASAPI enumeration can take tens of ms) — call via spawn_blocking.
    pub fn list_devices() -> Vec<AudioDevice> {
        let host = cpal::default_host();
        let default_name = host
            .default_input_device()
            .and_then(|d| d.name().ok())
            .unwrap_or_default();
        let mut out = Vec::new();
        match host.input_devices() {
            Ok(devs) => {
                for d in devs {
                    if let Ok(name) = d.name() {
                        // Skip devices that can't actually capture.
                        if d.default_input_config().is_err() {
                            continue;
                        }
                        out.push(AudioDevice {
                            is_default: name == default_name,
                            name,
                        });
                    }
                }
            }
            Err(e) => warn!("audio-in: device enumeration failed: {e}"),
        }
        info!(count = out.len(), default = %default_name, "audio-in: input devices");
        out
    }

    pub async fn stop(&self) {
        if let Some(c) = self.capture.write().await.take() {
            let _ = c.stop.send(());
            c.drain.abort();
        }
        self.set_status(AudioInStatus::Idle).await;
    }

    /// Start capturing from `device` (None = system default input).
    pub async fn start(self: Arc<Self>, device: Option<String>) -> anyhow::Result<()> {
        self.stop().await;

        // Channel from the audio callback thread into the async drain task.
        // Unbounded so the realtime callback never blocks; the drain keeps up
        // easily (it's a few hundred KB/s at most).
        let (tx, mut rx) = mpsc::unbounded_channel::<Vec<f32>>();
        let (stop_tx, stop_rx) = std::sync::mpsc::channel::<()>();
        // The audio thread reports back once the stream is open (or failed).
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel::<anyhow::Result<(String, u32, u16)>>();

        std::thread::Builder::new()
            .name("audio-input".into())
            .spawn(move || {
                let opened = open_stream(device, tx);
                match opened {
                    Ok((stream, name, sr, ch)) => {
                        if let Err(e) = stream.play() {
                            let _ = ready_tx.send(Err(anyhow::anyhow!("stream start: {e}")));
                            return;
                        }
                        let _ = ready_tx.send(Ok((name, sr, ch)));
                        // Park until told to stop; dropping the stream closes it.
                        let _ = stop_rx.recv();
                        drop(stream);
                    }
                    Err(e) => {
                        let _ = ready_tx.send(Err(e));
                    }
                }
            })?;

        let (name, sample_rate, channels) = match ready_rx.await {
            Ok(Ok(v)) => v,
            Ok(Err(e)) => {
                self.set_status(AudioInStatus::Error {
                    message: e.to_string(),
                })
                .await;
                return Err(e);
            }
            Err(_) => {
                let e = anyhow::anyhow!("audio thread exited before opening the stream");
                self.set_status(AudioInStatus::Error {
                    message: e.to_string(),
                })
                .await;
                return Err(e);
            }
        };
        info!(device = %name, sr = sample_rate, channels, "audio-in: capturing");

        let me = self.clone();
        let dev_name = name.clone();
        let drain = tokio::spawn(async move {
            let mut pipeline = RxPipeline::new(
                sample_rate,
                me.app.clone(),
                me.rtty.clone(),
                me.scp.clone(),
            )
            .await;
            let mut peak = 0f32;
            let mut last_status = Instant::now();
            while let Some(chunk) = rx.recv().await {
                for &s in &chunk {
                    peak = peak.max(s.abs());
                }
                pipeline.push(&chunk, false).await;
                if last_status.elapsed() >= STATUS_INTERVAL {
                    me.set_status(AudioInStatus::Running {
                        device: dev_name.clone(),
                        sample_rate,
                        channels,
                        peak,
                    })
                    .await;
                    peak = 0.0;
                    last_status = Instant::now();
                }
            }
        });

        *self.capture.write().await = Some(Capture {
            stop: stop_tx,
            drain,
        });
        self.set_status(AudioInStatus::Running {
            device: name,
            sample_rate,
            channels,
            peak: 0.0,
        })
        .await;
        Ok(())
    }
}

/// Open `device` (or the default input) with its default config and start a
/// capture stream whose callback downmixes to mono and forwards blocks on
/// `tx`. Returns the stream plus the resolved name / rate / channel count.
fn open_stream(
    device: Option<String>,
    tx: mpsc::UnboundedSender<Vec<f32>>,
) -> anyhow::Result<(cpal::Stream, String, u32, u16)> {
    let host = cpal::default_host();
    let dev = match device.as_deref().filter(|s| !s.is_empty()) {
        Some(want) => host
            .input_devices()?
            .find(|d| d.name().map(|n| n == want).unwrap_or(false))
            .ok_or_else(|| anyhow::anyhow!("input device not found: {want}"))?,
        None => host
            .default_input_device()
            .ok_or_else(|| anyhow::anyhow!("no default input device"))?,
    };
    let name = dev.name().unwrap_or_else(|_| "?".into());
    let cfg = dev.default_input_config()?;
    let sample_rate = cfg.sample_rate().0;
    let channels = cfg.channels();
    let stream_cfg: cpal::StreamConfig = cfg.clone().into();
    let err_fn = |e| warn!("audio-in: stream error: {e}");
    let ch = channels.max(1) as usize;

    macro_rules! build {
        ($t:ty, $conv:expr) => {{
            let tx = tx.clone();
            dev.build_input_stream(
                &stream_cfg,
                move |data: &[$t], _| {
                    let conv = $conv;
                    let mono: Vec<f32> = data
                        .chunks_exact(ch)
                        .map(|f| f.iter().map(|&s| conv(s)).sum::<f32>() / ch as f32)
                        .collect();
                    let _ = tx.send(mono);
                },
                err_fn,
                None,
            )?
        }};
    }

    let stream = match cfg.sample_format() {
        cpal::SampleFormat::F32 => build!(f32, |s: f32| s),
        cpal::SampleFormat::I16 => build!(i16, |s: i16| s as f32 / 32_768.0),
        cpal::SampleFormat::U16 => build!(u16, |s: u16| (s as f32 - 32_768.0) / 32_768.0),
        cpal::SampleFormat::I32 => build!(i32, |s: i32| s as f32 / 2_147_483_648.0),
        other => anyhow::bail!("unsupported input sample format {other:?}"),
    };
    Ok((stream, name, sample_rate, channels))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Opens the first available input device and checks that the callback
    /// delivers audio blocks. Skips (passes) when the machine has no input
    /// device, e.g. a CI runner.
    #[test]
    fn capture_delivers_blocks() {
        let devs = AudioInput::list_devices();
        eprintln!("devices: {:?}", devs.iter().map(|d| &d.name).collect::<Vec<_>>());
        let Some(dev) = devs
            .iter()
            .find(|d| d.name.contains("BlackHole"))
            .or(devs.first())
        else {
            eprintln!("no input devices; skipping");
            return;
        };
        let (tx, mut rx) = mpsc::unbounded_channel::<Vec<f32>>();
        let (stream, name, sr, ch) = open_stream(Some(dev.name.clone()), tx).expect("open");
        stream.play().expect("play");
        eprintln!("capturing {name} @ {sr} Hz x{ch}");
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        let mut blocks = 0usize;
        let mut samples = 0usize;
        while std::time::Instant::now() < deadline && blocks < 10 {
            match rx.try_recv() {
                Ok(b) => {
                    blocks += 1;
                    samples += b.len();
                }
                Err(_) => std::thread::sleep(Duration::from_millis(10)),
            }
        }
        drop(stream);
        eprintln!("got {blocks} blocks, {samples} samples");
        assert!(blocks > 0, "no audio blocks arrived from {name}");
    }
}
