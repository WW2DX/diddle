use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, RwLock};
use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};
use tracing::{debug, error, info, trace, warn};

use crate::dsp::{
    MultiDecoder, RttyDemod, RttyTunable, RttyTxGenerator, Spectrum, TuningScope,
};
use crate::scp::ScpDb;
use crate::tci::protocol::Message;

// FFT size of 4096 at 48 kHz gives 11.72 Hz/bin — matches WSJT-X's
// resolution and is fine enough to clearly resolve 170 Hz RTTY shifts.
const FFT_SIZE: usize = 4096;
// 75% overlap → smooth waterfall scrolling at ~47 Hz update rate
// without sacrificing frequency resolution.
const FFT_STRIDE: usize = 1024;
const AUDIO_HEADER_BYTES: usize = 64;
const AUDIO_STREAM_TYPE: u32 = 1;
const TX_STREAM_TYPE: u32 = 2;
const BINARY_EVENT_INTERVAL_MS: u64 = 500;
const TX_SAMPLE_RATE: u32 = 48_000;
// TCI binary message type for a TXChrono request (server asks us for N
// samples). We reply with a TXAudioStream message of that size.
const TX_CHRONO_TYPE: u32 = 3;
// TCI float32 format code is 4 (per the ftl/tci reference that drives
// WSJT-X ↔ ExpertSDR). NOT 3.
const TX_FORMAT_F32: u32 = 4;
// Stay below full scale so the radio's TX chain has headroom.
const TX_AMPLITUDE: f32 = 0.6;

/// Build a TCI TXAudioStream binary message from interleaved-stereo f32
/// samples. Header is the standard 64-byte layout (7 u32 fields + reserved).
fn build_tx_audio_frame(trx: u32, sample_rate: u32, stereo_samples: &[f32]) -> Vec<u8> {
    let n = stereo_samples.len();
    let mut buf = Vec::with_capacity(AUDIO_HEADER_BYTES + n * 4);
    let header: [u32; 7] = [
        trx,
        sample_rate,
        TX_FORMAT_F32,
        0,        // codec
        0,        // crc
        n as u32, // DataLength = number of f32 values (interleaved L,R)
        TX_STREAM_TYPE,
    ];
    for w in &header {
        buf.extend_from_slice(&w.to_le_bytes());
    }
    while buf.len() < AUDIO_HEADER_BYTES {
        buf.push(0);
    }
    for s in stereo_samples {
        buf.extend_from_slice(&s.to_le_bytes());
    }
    buf
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum TciState {
    Disconnected,
    Connecting,
    Connected { url: String, ready: bool },
    Error { message: String },
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct RigState {
    pub freq: u64,
    pub mode: String,
    pub ptt: bool,
}

/// Wire-level TCI message event, emitted for the debug console.
/// Binary frames are decoded and throttled — we never spam the UI.
#[derive(Debug, Clone, Serialize)]
pub struct TciMsg {
    pub dir: &'static str,  // "rx" | "tx"
    pub kind: &'static str, // "text" | "binary"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binary: Option<BinaryFrameInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BinaryFrameInfo {
    pub bytes: usize,
    pub trx: u32,
    pub sample_rate: u32,
    pub format: u32, // 0=i16, 1=i24, 2=i32, 3=f32, 4=f64
    pub codec: u32,
    pub stream_type: u32, // 0=iq, 1=rx_audio, 2=tx_audio, 3=tx_chrono, 4=spectrum
    pub channels: u32,
    pub stream_label: String,
    pub fps: f32, // measured frame rate since last emit
}

impl BinaryFrameInfo {
    fn parse(data: &[u8]) -> Self {
        let read_u32 = |o: usize| {
            if data.len() >= o + 4 {
                u32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]])
            } else {
                0
            }
        };
        let trx = read_u32(0);
        let sample_rate = read_u32(4);
        let format = read_u32(8);
        let codec = read_u32(12);
        let stream_type = read_u32(24);
        let channels = read_u32(28);

        let stream_name = match stream_type {
            0 => "iq",
            1 => "rx_audio",
            2 => "tx_audio",
            3 => "tx_chrono",
            4 => "spectrum",
            _ => "unknown",
        };
        let fmt_name = match format {
            0 => "i16",
            1 => "i24",
            2 => "i32",
            3 => "f32",
            4 => "f64",
            _ => "?",
        };
        let stream_label = format!(
            "{} {} {} Hz x{}ch (trx {})",
            stream_name, fmt_name, sample_rate, channels, trx
        );

        Self {
            bytes: data.len(),
            trx,
            sample_rate,
            format,
            codec,
            stream_type,
            channels,
            stream_label,
            fps: 0.0,
        }
    }
}

/// In-flight transmission state. The waveform is the full mono signal
/// (lead-in + message + trail). The TXChrono handler in the run-loop reads
/// from `position` as the server requests audio.
struct TxState {
    waveform: Vec<f32>,
    position: usize,
}

pub struct TciClient {
    app: AppHandle,
    state: RwLock<TciState>,
    rig: RwLock<RigState>,
    cmd_tx: RwLock<Option<mpsc::Sender<String>>>,
    rtty: Arc<RttyTunable>,
    scp: Arc<ScpDb>,
    /// TX in flight — guards against overlapping transmissions.
    tx_busy: RwLock<bool>,
    /// Shared TX waveform + playback position, consumed by the TXChrono
    /// handler. std Mutex because access is brief and non-async.
    tx_state: std::sync::Mutex<Option<TxState>>,
    /// Set by `abort_tx` to bail out of an in-flight transmit. Reset at the
    /// start of each new transmission.
    tx_cancel: AtomicBool,
}

impl TciClient {
    pub fn new(app: AppHandle, rtty: Arc<RttyTunable>, scp: Arc<ScpDb>) -> Self {
        Self {
            app,
            state: RwLock::new(TciState::Disconnected),
            rig: RwLock::new(RigState::default()),
            cmd_tx: RwLock::new(None),
            rtty,
            scp,
            tx_busy: RwLock::new(false),
            tx_state: std::sync::Mutex::new(None),
            tx_cancel: AtomicBool::new(false),
        }
    }

    pub async fn state(&self) -> TciState {
        self.state.read().await.clone()
    }

    pub async fn rig(&self) -> RigState {
        self.rig.read().await.clone()
    }

    pub async fn connect(self: Arc<Self>, url: String) -> anyhow::Result<()> {
        {
            let s = self.state.read().await;
            if matches!(*s, TciState::Connected { .. } | TciState::Connecting) {
                return Ok(());
            }
        }

        let (cmd_tx, cmd_rx) = mpsc::channel::<String>(256);
        *self.cmd_tx.write().await = Some(cmd_tx);

        let me = self.clone();
        tokio::spawn(async move {
            me.run_loop(url, cmd_rx).await;
        });
        Ok(())
    }

    pub async fn disconnect(&self) {
        *self.cmd_tx.write().await = None;
        self.set_state(TciState::Disconnected).await;
    }

    pub async fn send(&self, raw: String) -> anyhow::Result<()> {
        let tx = self.cmd_tx.read().await.clone();
        match tx {
            Some(tx) => {
                tx.send(raw)
                    .await
                    .map_err(|e| anyhow::anyhow!("TCI command channel closed: {e}"))?;
                Ok(())
            }
            None => anyhow::bail!("TCI not connected"),
        }
    }

    /// Encode + transmit a text message as RTTY AFSK via the TCI TX audio
    /// stream. Generates the audio, frames it into TCI binary packets, and
    /// paces them at real-time. PTT is asserted before audio starts and
    /// dropped after the trailing mark/diddle.
    pub async fn transmit(&self, text: String) -> anyhow::Result<()> {
        if text.trim().is_empty() {
            // Refuse empty messages — they otherwise produce a ~700 ms mark
            // pulse (lead-in + trail) on the air, which sounds like the
            // radio is broken.
            anyhow::bail!("refusing to transmit empty text");
        }
        {
            let mut busy = self.tx_busy.write().await;
            if *busy {
                anyhow::bail!("TX already in progress");
            }
            *busy = true;
        }
        self.tx_cancel.store(false, Ordering::SeqCst);
        let result = self.transmit_inner(text).await;
        *self.tx_busy.write().await = false;
        result
    }

    /// Abort any in-flight transmission. Drops PTT immediately and signals
    /// the transmit loop to bail. Safe to call when no TX is active.
    pub async fn abort_tx(&self) -> anyhow::Result<()> {
        let was_active = self.tx_state.lock().unwrap().is_some();
        if !was_active {
            return Ok(());
        }
        self.tx_cancel.store(true, Ordering::SeqCst);
        // Stop the chrono streamer immediately — subsequent requests get
        // None and the server stops asking for audio.
        *self.tx_state.lock().unwrap() = None;
        // Drop PTT. The waiting `transmit_inner` will also try this when
        // it sees the cancel; sending twice is harmless.
        let _ = self.send("trx:0,false,vac;".to_string()).await;
        info!("tx: aborted");
        Ok(())
    }

    async fn transmit_inner(&self, text: String) -> anyhow::Result<()> {
        let cfg = self.rtty.get().await;
        let sr = TX_SAMPLE_RATE;

        info!(
            text_len = text.len(),
            mark = cfg.mark_hz,
            space = cfg.space_hz,
            baud = cfg.baud,
            "tx: starting"
        );

        // Build the full mono waveform up-front: 500 ms mark lead-in +
        // message bits + 200 ms mark trail. The TXChrono handler in the
        // run-loop streams this out as the server requests it.
        let mut gen = RttyTxGenerator::new(sr, cfg.mark_hz, cfg.space_hz, cfg.baud);
        let lead_in_samples = (sr as usize) / 2;
        let trail_samples = (sr as usize) / 5;
        let mut waveform: Vec<f32> = Vec::new();
        // Lead-in: 0.5 s of pure mark (queue empty → generator emits mark).
        gen.next_samples(lead_in_samples, &mut waveform);
        // Message: drain ONE sample at a time so we stop the instant the bit
        // queue empties and the final bit completes. A coarse batch size here
        // overshoots by up to a full bit-period-misalignment cycle (~0.7 s of
        // trailing mark carrier — the long TX tail).
        // Enqueue with a per-character timeline so we can echo each character
        // into the decoder window exactly as its tones go on the air (rather
        // than dumping the whole message when TX completes). The message bits
        // begin playing right after the lead-in, so a character at bit index
        // `b` is heard at sample offset `lead_in_samples + b * samples_per_bit`.
        let spb = gen.samples_per_bit();
        let echo: Vec<(usize, char)> = gen
            .enqueue_with_marks(&text)
            .into_iter()
            .map(|(bit, c)| (lead_in_samples + (bit as f32 * spb) as usize, c))
            .collect();
        let mut echo_idx = 0usize;
        let cap = sr as usize * 30; // safety cap (30 s)
        let mut produced = 0usize;
        while !gen.is_idle() && produced < cap {
            gen.next_samples(1, &mut waveform);
            produced += 1;
        }
        // Short trail so the receiver sees a clean end-of-transmission.
        gen.next_samples(trail_samples, &mut waveform);
        for s in waveform.iter_mut() {
            *s *= TX_AMPLITUDE;
        }
        let total = waveform.len();
        info!(total_samples = total, "tx: waveform generated");

        // Publish the waveform for the TXChrono handler, then key up with
        // signal source = VAC (so RHR pulls TX audio from our TCI stream
        // rather than the mic). This `,vac` argument is the critical bit.
        *self.tx_state.lock().unwrap() = Some(TxState {
            waveform,
            position: 0,
        });
        self.send("trx:0,true,vac;".to_string()).await?;

        // Wait until the chrono handler has streamed the whole waveform
        // (or a generous timeout). The handler advances `position`.
        let start = std::time::Instant::now();
        let mut cancelled = false;
        loop {
            if self.tx_cancel.load(Ordering::SeqCst) {
                cancelled = true;
                break;
            }
            let position = self
                .tx_state
                .lock()
                .unwrap()
                .as_ref()
                .map(|t| t.position)
                .unwrap_or(total);
            // Echo every character whose audio has now started playing.
            while echo_idx < echo.len() && echo[echo_idx].0 <= position {
                let _ = self.app.emit("tx:echo", echo[echo_idx].1.to_string());
                echo_idx += 1;
            }
            if position >= total {
                break;
            }
            if start.elapsed() > Duration::from_secs(60) {
                warn!("tx: timed out waiting for chrono drain");
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        if cancelled {
            // abort_tx already cleared tx_state and dropped PTT — nothing
            // more to do.
            info!("tx: cancelled");
            return Ok(());
        }
        // Brief grace period so the last requested chunk plays out.
        tokio::time::sleep(Duration::from_millis(150)).await;

        self.send("trx:0,false,vac;".to_string()).await?;
        *self.tx_state.lock().unwrap() = None;
        info!("tx: done");
        Ok(())
    }

    /// Respond to a TXChrono request: hand the server the next
    /// `requested_floats` interleaved-stereo samples from the active TX
    /// waveform. Returns None if no transmission is in flight.
    fn build_chrono_response(&self, trx: u32, sample_rate: u32, requested_floats: usize) -> Option<Vec<u8>> {
        let mut guard = self.tx_state.lock().unwrap();
        let tx = guard.as_mut()?;
        let frames = requested_floats / 2;
        let mut stereo = Vec::with_capacity(requested_floats);
        for k in 0..frames {
            let s = tx.waveform.get(tx.position + k).copied().unwrap_or(0.0);
            stereo.push(s); // L
            stereo.push(s); // R
        }
        // Pad to the exact requested length (handles odd request sizes).
        while stereo.len() < requested_floats {
            stereo.push(0.0);
        }
        tx.position += frames;
        Some(build_tx_audio_frame(trx, sample_rate, &stereo))
    }

    async fn set_state(&self, s: TciState) {
        *self.state.write().await = s.clone();
        let _ = self.app.emit("tci:state", &s);
    }

    async fn emit_rig(&self) {
        let r = self.rig.read().await.clone();
        let _ = self.app.emit("tci:rig", &r);
    }

    fn emit_msg(&self, m: TciMsg) {
        let _ = self.app.emit("tci:msg", &m);
    }

    async fn run_loop(self: Arc<Self>, url: String, mut cmd_rx: mpsc::Receiver<String>) {
        self.set_state(TciState::Connecting).await;
        info!(%url, "connecting to TCI");

        let (ws, _) = match connect_async(&url).await {
            Ok(v) => v,
            Err(e) => {
                error!("TCI connect failed: {e}");
                self.set_state(TciState::Error {
                    message: e.to_string(),
                })
                .await;
                *self.cmd_tx.write().await = None;
                return;
            }
        };
        info!("TCI WebSocket connected");
        self.set_state(TciState::Connected {
            url: url.clone(),
            ready: false,
        })
        .await;

        let (mut write, mut read) = ws.split();

        if write.send(WsMessage::Text("start;".into())).await.is_err() {
            warn!("failed to send start;");
            self.set_state(TciState::Disconnected).await;
            *self.cmd_tx.write().await = None;
            return;
        }
        let _ = TX_SAMPLE_RATE; // silence unused if we somehow skip TX path

        // Per-stream-type throttle bookkeeping for binary frame events.
        let mut binary_stats: std::collections::HashMap<u32, (Instant, u32)> =
            std::collections::HashMap::new();

        // Spectrum processor — created lazily once we know the audio sample rate.
        let mut spectrum: Option<Spectrum> = None;
        // RTTY demodulator — created lazily, rebuilt when tones change.
        let mut rtty: Option<RttyDemod> = None;
        let mut rtty_gen: u64 = 0;
        // Multi-decoder — runs in parallel for spot detection.
        let mut multi: Option<MultiDecoder> = None;
        // Tuning scope — rebuilt with the demod when tones change.
        let mut scope: Option<TuningScope> = None;
        let mut audio_calibrated = false;

        loop {
            tokio::select! {
                outbound = cmd_rx.recv() => {
                    match outbound {
                        Some(line) => {
                            debug!(tx=%line, "TCI tx");
                            self.emit_msg(TciMsg {
                                dir: "tx",
                                kind: "text",
                                text: Some(line.clone()),
                                binary: None,
                            });
                            if write.send(WsMessage::Text(line.into())).await.is_err() {
                                warn!("TCI write failed");
                                break;
                            }
                        }
                        None => break,
                    }
                }
                incoming = read.next() => {
                    match incoming {
                        Some(Ok(WsMessage::Text(text))) => {
                            self.emit_msg(TciMsg {
                                dir: "rx",
                                kind: "text",
                                text: Some(text.to_string()),
                                binary: None,
                            });
                            for line in text.split(';') {
                                if let Some(m) = Message::parse(line) {
                                    self.handle_message(m).await;
                                }
                            }
                        }
                        Some(Ok(WsMessage::Binary(data))) => {
                            let mut info = BinaryFrameInfo::parse(&data);
                            trace!(stream = %info.stream_label, bytes = info.bytes, "TCI rx binary");

                            // TXChrono: server is requesting TX audio. Reply
                            // immediately (directly to the socket) with the
                            // next chunk of the active transmission waveform.
                            if info.stream_type == TX_CHRONO_TYPE {
                                // DataLength field (offset 20) = requested float count.
                                let requested = if data.len() >= 24 {
                                    u32::from_le_bytes([data[20], data[21], data[22], data[23]])
                                        as usize
                                } else {
                                    0
                                };
                                if requested > 0 {
                                    if let Some(frame) = self.build_chrono_response(
                                        info.trx,
                                        info.sample_rate,
                                        requested,
                                    ) {
                                        if write
                                            .send(WsMessage::Binary(frame.into()))
                                            .await
                                            .is_err()
                                        {
                                            warn!("TCI tx-audio write failed");
                                            break;
                                        }
                                    }
                                }
                                continue;
                            }

                            // Audio path: extract f32 stereo, downmix to mono, feed FFT.
                            if info.stream_type == AUDIO_STREAM_TYPE
                                && info.format == 3
                                && data.len() > AUDIO_HEADER_BYTES
                            {
                                let payload = &data[AUDIO_HEADER_BYTES..];
                                let mono = decode_stereo_f32_to_mono(payload, info.channels.max(1));

                                // Log peak/RMS once so we can verify the audio scale ([-1,1] vs raw int).
                                if !audio_calibrated && !mono.is_empty() {
                                    let peak = mono.iter().fold(0f32, |a, &v| a.max(v.abs()));
                                    let rms = (mono.iter().map(|&v| v * v).sum::<f32>()
                                        / mono.len() as f32)
                                        .sqrt();
                                    info!(
                                        peak = peak,
                                        rms = rms,
                                        first = mono[0],
                                        n = mono.len(),
                                        "audio scale (first frame)"
                                    );
                                    audio_calibrated = true;
                                }

                                let sp = spectrum.get_or_insert_with(|| {
                                    info!(
                                        sr = info.sample_rate,
                                        fft = FFT_SIZE,
                                        stride = FFT_STRIDE,
                                        "spectrum: starting FFT pipeline"
                                    );
                                    Spectrum::new(FFT_SIZE, FFT_STRIDE, info.sample_rate)
                                });
                                let multi_dec = multi.get_or_insert_with(|| {
                                    info!("multi-decoder: starting");
                                    MultiDecoder::new(
                                        info.sample_rate,
                                        self.app.clone(),
                                        self.scp.clone(),
                                    )
                                });
                                multi_dec.push_audio(&mono);
                                for frame in sp.push(&mono) {
                                    multi_dec.push_spectrum(&frame.mags_db, frame.fft_size);
                                    let _ = self.app.emit("spectrum", &frame);
                                }

                                // RTTY demod + tuning scope run on the same mono
                                // samples; rebuild both when the tunable changes.
                                let cur_gen = self.rtty.current_gen();
                                if rtty.is_none() || cur_gen != rtty_gen {
                                    let cfg = self.rtty.get().await;
                                    info!(
                                        sr = info.sample_rate,
                                        mark = cfg.mark_hz,
                                        space = cfg.space_hz,
                                        baud = cfg.baud,
                                        "rtty: (re)building demod"
                                    );
                                    rtty = Some(RttyDemod::new(info.sample_rate, cfg.clone()));
                                    scope = Some(TuningScope::new(
                                        info.sample_rate,
                                        cfg.mark_hz,
                                        cfg.space_hz,
                                    ));
                                    rtty_gen = cur_gen;
                                }
                                let chars = rtty.as_mut().unwrap().push(&mono);
                                if !chars.is_empty() {
                                    let _ = self.app.emit("rtty", &chars);
                                }
                                if let Some(sc) = scope.as_mut() {
                                    for f in sc.push(&mono) {
                                        let _ = self.app.emit("scope", &f);
                                    }
                                }
                            }

                            // Throttled debug event: one per stream_type per N ms.
                            let now = Instant::now();
                            let entry = binary_stats
                                .entry(info.stream_type)
                                .or_insert((now - Duration::from_millis(BINARY_EVENT_INTERVAL_MS), 0));
                            entry.1 = entry.1.saturating_add(1);
                            let elapsed = now.duration_since(entry.0);
                            if elapsed >= Duration::from_millis(BINARY_EVENT_INTERVAL_MS) {
                                let secs = elapsed.as_secs_f32().max(0.001);
                                info.fps = entry.1 as f32 / secs;
                                self.emit_msg(TciMsg {
                                    dir: "rx",
                                    kind: "binary",
                                    text: None,
                                    binary: Some(info),
                                });
                                *entry = (now, 0);
                            }
                        }
                        Some(Ok(WsMessage::Close(_))) => {
                            info!("TCI server closed connection");
                            break;
                        }
                        Some(Ok(_)) => {}
                        Some(Err(e)) => {
                            warn!("TCI read error: {e}");
                            break;
                        }
                        None => break,
                    }
                }
            }
        }

        self.set_state(TciState::Disconnected).await;
        *self.cmd_tx.write().await = None;
    }

    async fn handle_message(&self, m: Message) {
        debug!(rx=%m.name, args=?m.args, "TCI rx");
        match m.name.as_str() {
            "ready" => {
                info!("TCI server ready");
                let s = self.state.read().await.clone();
                if let TciState::Connected { url, .. } = s {
                    self.set_state(TciState::Connected { url, ready: true }).await;
                }
                // Auto-start the RX audio stream — operator doesn't need a
                // separate "Start audio" click. WavPlayer will stop this
                // explicitly when it kicks off offline playback.
                let _ = self.send("audio_start:0;".to_string()).await;
            }
            "vfo" if m.arg_u8(0) == Some(0) && m.arg_u8(1) == Some(0) => {
                if let Some(hz) = m.arg_u64(2) {
                    self.rig.write().await.freq = hz;
                    self.emit_rig().await;
                }
            }
            // Mode is reported as `modulation:trx,name` on RHR TCI v2.0.
            // Standard TCI uses `mode:trx,name` — accept both.
            "modulation" | "mode" if m.arg_u8(0) == Some(0) => {
                if let Some(mode) = m.arg_str(1) {
                    self.rig.write().await.mode = mode.to_string();
                    self.emit_rig().await;
                }
            }
            "trx" if m.arg_u8(0) == Some(0) => {
                if let Some(on) = m.arg_bool(1) {
                    self.rig.write().await.ptt = on;
                    self.emit_rig().await;
                }
            }
            _ => {}
        }
    }
}

fn decode_stereo_f32_to_mono(bytes: &[u8], channels: u32) -> Vec<f32> {
    let ch = channels.max(1) as usize;
    let bytes_per_frame = 4 * ch;
    let frames = bytes.len() / bytes_per_frame;
    let mut out = Vec::with_capacity(frames);
    for i in 0..frames {
        let base = i * bytes_per_frame;
        let mut sum = 0.0f32;
        for c in 0..ch {
            let o = base + c * 4;
            let s = f32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
            sum += s;
        }
        out.push(sum / ch as f32);
    }
    out
}
