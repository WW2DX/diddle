// Plays a WAV file through the same DSP pipeline (Spectrum + RttyDemod)
// that TCI audio feeds, so the waterfall and decoder see WAV content as if
// it were live RX audio. Used for offline testing.

use std::io::Cursor;
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tracing::{info, warn};

use crate::dsp::{MultiDecoder, RttyDemod, RttyTunable, Spectrum, TuningScope};
use crate::scp::ScpDb;

const FFT_SIZE: usize = 4096;
const FFT_STRIDE: usize = 1024;
// Chunk feeding rate — small enough to keep waterfall smooth, large enough
// to amortize per-chunk overhead.
const CHUNK_SAMPLES: usize = 512;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum WavStatus {
    Idle,
    Playing {
        path: String,
        position_s: f32,
        duration_s: f32,
        sample_rate: u32,
        channels: u16,
    },
    Done {
        path: String,
    },
    Error {
        message: String,
    },
}

pub struct WavPlayer {
    app: AppHandle,
    task: RwLock<Option<JoinHandle<()>>>,
    status: RwLock<WavStatus>,
    rtty: Arc<RttyTunable>,
    scp: Arc<ScpDb>,
}

impl WavPlayer {
    pub fn new(app: AppHandle, rtty: Arc<RttyTunable>, scp: Arc<ScpDb>) -> Self {
        Self {
            app,
            task: RwLock::new(None),
            status: RwLock::new(WavStatus::Idle),
            rtty,
            scp,
        }
    }

    pub async fn status(&self) -> WavStatus {
        self.status.read().await.clone()
    }

    pub async fn stop(&self) {
        if let Some(h) = self.task.write().await.take() {
            h.abort();
        }
        self.set_status(WavStatus::Idle).await;
    }

    async fn set_status(&self, s: WavStatus) {
        *self.status.write().await = s.clone();
        let _ = self.app.emit("wav:status", &s);
    }

    pub async fn play(self: Arc<Self>, path: String) -> anyhow::Result<()> {
        // Cancel any in-flight playback first.
        if let Some(h) = self.task.write().await.take() {
            h.abort();
        }

        let bytes = tokio::fs::read(&path).await?;
        let (samples, sample_rate, channels) = parse_wav(&bytes)?;
        let duration_s = samples.len() as f32 / sample_rate as f32;
        info!(
            path = %path,
            sr = sample_rate,
            channels,
            samples = samples.len(),
            duration_s,
            "wav: parsed"
        );

        let me = self.clone();
        let path_for_task = path.clone();
        let initial_cfg = self.rtty.get().await;
        let initial_gen = self.rtty.current_gen();
        let app_for_task = me.app.clone();
        let scp_for_task = me.scp.clone();
        let handle = tokio::spawn(async move {
            let mut spectrum = Spectrum::new(FFT_SIZE, FFT_STRIDE, sample_rate);
            let mut rtty = RttyDemod::new(sample_rate, initial_cfg.clone());
            let mut scope =
                TuningScope::new(sample_rate, initial_cfg.mark_hz, initial_cfg.space_hz);
            let mut rtty_gen = initial_gen;
            let mut multi = MultiDecoder::new(sample_rate, app_for_task, scp_for_task);

            let chunk_dur =
                Duration::from_micros((CHUNK_SAMPLES as u64 * 1_000_000) / sample_rate as u64);
            let total = samples.len();
            let mut pos = 0usize;

            me.set_status(WavStatus::Playing {
                path: path_for_task.clone(),
                position_s: 0.0,
                duration_s,
                sample_rate,
                channels,
            })
            .await;

            for chunk in samples.chunks(CHUNK_SAMPLES) {
                // Hot-retune the demod if the user clicked a new mark/space.
                let cur_gen = me.rtty.current_gen();
                if cur_gen != rtty_gen {
                    let cfg = me.rtty.get().await;
                    info!(
                        mark = cfg.mark_hz,
                        space = cfg.space_hz,
                        "wav: retuning demod mid-playback"
                    );
                    rtty = RttyDemod::new(sample_rate, cfg.clone());
                    scope = TuningScope::new(sample_rate, cfg.mark_hz, cfg.space_hz);
                    rtty_gen = cur_gen;
                }

                multi.push_audio(chunk);
                for frame in spectrum.push(chunk) {
                    multi.push_spectrum(&frame.mags_db, frame.fft_size);
                    let _ = me.app.emit("spectrum", &frame);
                }
                let chars = rtty.push(chunk);
                if !chars.is_empty() {
                    let _ = me.app.emit("rtty", &chars);
                }
                for f in scope.push(chunk) {
                    let _ = me.app.emit("scope", &f);
                }
                pos += chunk.len();

                // Throttle status to ~5 fps so we don't flood the UI.
                if pos % (CHUNK_SAMPLES * 16) < CHUNK_SAMPLES {
                    me.set_status(WavStatus::Playing {
                        path: path_for_task.clone(),
                        position_s: pos as f32 / sample_rate as f32,
                        duration_s,
                        sample_rate,
                        channels,
                    })
                    .await;
                }

                tokio::time::sleep(chunk_dur).await;
                let _ = total;
            }

            me.set_status(WavStatus::Done {
                path: path_for_task,
            })
            .await;
        });

        *self.task.write().await = Some(handle);
        Ok(())
    }
}

/// Parse a WAV file's bytes; returns (mono f32 samples in [-1,1], sample_rate, channels).
fn parse_wav(bytes: &[u8]) -> anyhow::Result<(Vec<f32>, u32, u16)> {
    let cursor = Cursor::new(bytes);
    let mut reader = hound::WavReader::new(cursor)?;
    let spec = reader.spec();
    let channels = spec.channels;
    let sr = spec.sample_rate;

    let mono = match (spec.bits_per_sample, spec.sample_format) {
        (16, hound::SampleFormat::Int) => {
            let raw: Result<Vec<i16>, _> = reader.samples::<i16>().collect();
            downmix(&raw?, channels, |s| s as f32 / 32_768.0)
        }
        (24, hound::SampleFormat::Int) => {
            let raw: Result<Vec<i32>, _> = reader.samples::<i32>().collect();
            // hound returns 24-bit as i32 in low 24 bits; normalize by 2^23.
            downmix(&raw?, channels, |s| s as f32 / 8_388_608.0)
        }
        (32, hound::SampleFormat::Int) => {
            let raw: Result<Vec<i32>, _> = reader.samples::<i32>().collect();
            downmix(&raw?, channels, |s| s as f32 / 2_147_483_648.0)
        }
        (32, hound::SampleFormat::Float) => {
            let raw: Result<Vec<f32>, _> = reader.samples::<f32>().collect();
            downmix(&raw?, channels, |s| s)
        }
        (bits, fmt) => {
            warn!(bits, ?fmt, "wav: unsupported sample format");
            anyhow::bail!("unsupported WAV format ({} bit {:?})", bits, fmt);
        }
    };

    Ok((mono, sr, channels))
}

fn downmix<T: Copy>(interleaved: &[T], channels: u16, conv: impl Fn(T) -> f32) -> Vec<f32> {
    let ch = channels.max(1) as usize;
    if ch == 1 {
        return interleaved.iter().map(|&s| conv(s)).collect();
    }
    interleaved
        .chunks_exact(ch)
        .map(|frame| {
            let sum: f32 = frame.iter().map(|&s| conv(s)).sum();
            sum / ch as f32
        })
        .collect()
}
