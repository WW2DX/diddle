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
    LiveTx, MultiDecoder, RttyDemod, RttyTunable, Spectrum, TuningScope,
    LIVE_IDLE_TIMEOUT,
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
// Sample-format code in the TX audio header. The TCI spec (and ExpertSDR)
// say float32 = 3; RHR's server wants 4, as the ftl/tci Go library sends.
// See `TciFlavor`.
const TX_FORMAT_F32_SPEC: u32 = 3;
const TX_FORMAT_F32_RHR: u32 = 4;
// Unkey and report an error if the radio hasn't asked for any TX audio
// this long after we keyed up — otherwise a server that ignores our audio
// source would leave a dead carrier on the air until the 60 s timeout.
const NO_CHRONO_TIMEOUT: Duration = Duration::from_millis(2500);
// Plain mark before a macro (the far decoder settles on it) and before a
// live keyboard send (which then diddles).
const MACRO_LEAD_MS: u32 = 500;
const LIVE_LEAD_MS: u32 = 150;

/// Which TCI dialect the server speaks, from its `protocol:` announcement.
///
/// ExpertSDR (SunSDR, ColibriNANO…) follows the published TCI spec: TX
/// audio is taken from the TCI stream only when keyed with `trx:0,true,tci`
/// (anything else transmits the selected microphone), the float32 format
/// code is 3, and the header carries the channel count. RemoteHamRadio's
/// server — what Diddle was first built against — needs `vac` and format 4,
/// and is left exactly as it was. Unknown servers get the RHR behaviour.
#[derive(Debug, Clone, Copy, PartialEq)]
enum TciFlavor {
    Spec,
    Rhr,
}

impl TciFlavor {
    fn from_protocol(name: &str) -> Self {
        let n = name.to_ascii_lowercase();
        if n.starts_with("expertsdr") || n.contains("sunsdr") {
            TciFlavor::Spec
        } else {
            TciFlavor::Rhr
        }
    }

    fn ptt(self, on: bool) -> String {
        match (self, on) {
            (TciFlavor::Spec, true) => "trx:0,true,tci;".to_string(),
            (TciFlavor::Spec, false) => "trx:0,false;".to_string(),
            (TciFlavor::Rhr, on) => format!("trx:0,{on},vac;"),
        }
    }
}
// Stay below full scale so the radio's TX chain has headroom.
const TX_AMPLITUDE: f32 = 0.6;
// Sideband our AFSK RTTY tones assume. We force the radio into DIGL on
// connect and before every transmit so the operator never has to set it.
const RTTY_MODE: &str = "digl";
/// Minimum gap between automatic DIGL restores (see `mode_fix_due`).
const MODE_FIX_MIN_INTERVAL: Duration = Duration::from_secs(2);
// After PTT drops, keep the RX decoders muted this long so the level jump
// at the TX→RX transition can't pump the AGC/noise floor into junk chars.
const RX_MUTE_HOLDOFF_MS: u64 = 300;

/// Build a TCI TXAudioStream binary message from interleaved-stereo f32
/// samples. Header is the standard 64-byte layout (7 u32 fields + reserved).
fn build_tx_audio_frame(
    trx: u32,
    sample_rate: u32,
    format: u32,
    channels: u32,
    stereo_samples: &[f32],
) -> Vec<u8> {
    let n = stereo_samples.len();
    let mut buf = Vec::with_capacity(AUDIO_HEADER_BYTES + n * 4);
    let header: [u32; 8] = [
        trx,
        sample_rate,
        format,
        0,        // codec
        0,        // crc
        n as u32, // length = number of f32 values (all channels)
        TX_STREAM_TYPE,
        channels, // 0 for RHR (as before); 1 or 2 per the spec
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
enum TxState {
    /// Audio produced on demand as the radio asks — macros (finished at
    /// once, so chained F-keys can still join) and live keyboard sends.
    Live(LiveTx),
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
    /// While set, the RX audio path skips the RTTY demod, multi-decoder and
    /// tuning scope. The radio's monitor/loopback audio during our own TX
    /// otherwise gets decoded and interleaved with the TX echo as garbage.
    rx_mute: AtomicBool,
    /// Keeps the mute in force briefly after unkey (see RX_MUTE_HOLDOFF_MS).
    rx_resume_at: std::sync::Mutex<Option<Instant>>,
    /// When we last pushed DIGL back after the rig reported another mode.
    /// Rate-limits the guard so a radio that refuses DIGL can't ping-pong.
    last_mode_fix: std::sync::Mutex<Option<Instant>>,
    /// Server dialect, set from its `protocol:` line on connect.
    flavor: std::sync::Mutex<TciFlavor>,
    /// The radio has asked for TX audio since we last keyed up.
    chrono_seen: AtomicBool,
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
            rx_mute: AtomicBool::new(false),
            rx_resume_at: std::sync::Mutex::new(None),
            last_mode_fix: std::sync::Mutex::new(None),
            flavor: std::sync::Mutex::new(TciFlavor::Rhr),
            chrono_seen: AtomicBool::new(false),
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
        // Chained F-keys: text sent while we're already on the air joins
        // that transmission (a macro or a live keyboard send) back to back.
        // If the previous one is already in its closing mark, wait for it to
        // unkey and start a new one.
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            if self.live_push(&text) {
                return Ok(());
            }
            {
                let mut busy = self.tx_busy.write().await;
                if !*busy {
                    *busy = true;
                    break;
                }
            }
            if Instant::now() > deadline {
                anyhow::bail!("TX still busy after 60 s");
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        self.tx_cancel.store(false, Ordering::SeqCst);
        self.rx_mute.store(true, Ordering::SeqCst);
        let result = self.live_inner(text, MACRO_LEAD_MS, true).await;
        self.rx_mute.store(false, Ordering::SeqCst);
        *self.rx_resume_at.lock().unwrap() =
            Some(Instant::now() + Duration::from_millis(RX_MUTE_HOLDOFF_MS));
        *self.tx_busy.write().await = false;
        result
    }

    /// Live keyboard send: key up now and diddle until text arrives
    /// (`live_push`) or the operator finishes (`live_finish`). Resolves when
    /// the transmission is over — finished, aborted, or idle too long.
    pub async fn live_start(&self, initial: String) -> anyhow::Result<()> {
        {
            let mut busy = self.tx_busy.write().await;
            if *busy {
                anyhow::bail!("TX already in progress");
            }
            *busy = true;
        }
        self.tx_cancel.store(false, Ordering::SeqCst);
        self.rx_mute.store(true, Ordering::SeqCst);
        let result = self.live_inner(initial, LIVE_LEAD_MS, false).await;
        self.rx_mute.store(false, Ordering::SeqCst);
        *self.rx_resume_at.lock().unwrap() =
            Some(Instant::now() + Duration::from_millis(RX_MUTE_HOLDOFF_MS));
        *self.tx_busy.write().await = false;
        result
    }

    /// Run one transmission through the live engine: key up, stream audio
    /// on demand until it's done (or aborted / timed out), unkey. A macro
    /// passes `finish_now` — it ends once its text (plus anything chained
    /// onto it) is out; a keyboard send diddles until `live_finish`.
    async fn live_inner(&self, initial: String, lead_ms: u32, finish_now: bool) -> anyhow::Result<()> {
        let cfg = self.rtty.get().await;
        let (tx_mark, tx_space) = cfg.tx_tones();
        info!(mark = tx_mark, space = tx_space, "tx: live start");
        let mut live = LiveTx::with_lead(TX_SAMPLE_RATE, tx_mark, tx_space, cfg.baud, lead_ms);
        live.push(&initial);
        if finish_now {
            live.finish();
        }
        *self.tx_state.lock().unwrap() = Some(TxState::Live(live));
        self.force_rtty_mode().await;
        if let Err(e) = self.key_up().await {
            *self.tx_state.lock().unwrap() = None;
            return Err(e);
        }
        let start = Instant::now();
        loop {
            if self.tx_cancel.load(Ordering::SeqCst) {
                info!("tx: live cancelled");
                return Ok(()); // abort_tx dropped PTT and cleared state
            }
            self.check_no_chrono(start).await?;
            let (echoes, done) = {
                let mut guard = self.tx_state.lock().unwrap();
                match guard.as_mut() {
                    Some(TxState::Live(live)) => {
                        if !live.is_finishing() && live.idle_for() > LIVE_IDLE_TIMEOUT {
                            info!("tx: live idle timeout");
                            live.finish();
                        }
                        (live.take_echoes(), live.is_done())
                    }
                    _ => (Vec::new(), true),
                }
            };
            for c in echoes {
                let _ = self.app.emit("tx:echo", c.to_string());
            }
            if done {
                break;
            }
            if start.elapsed() > Duration::from_secs(600) {
                warn!("tx: live hard limit reached");
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
        self.send(self.flavor().ptt(false)).await?;
        *self.tx_state.lock().unwrap() = None;
        info!("tx: live done");
        Ok(())
    }

    /// Add typed text to the live transmission. False if none is running.
    pub fn live_push(&self, text: &str) -> bool {
        match self.tx_state.lock().unwrap().as_mut() {
            Some(TxState::Live(live)) if live.accepts_text() => {
                live.push(text);
                true
            }
            _ => false,
        }
    }

    /// Send what's queued, then unkey. False without a live transmission.
    pub fn live_finish(&self) -> bool {
        if let Some(TxState::Live(live)) = self.tx_state.lock().unwrap().as_mut() {
            live.finish();
            return true;
        }
        false
    }

    /// True while the RX decode pipeline should stay quiet: during a
    /// transmission and for a short hold-off after unkey.
    fn rx_muted(&self) -> bool {
        if self.rx_mute.load(Ordering::SeqCst) {
            return true;
        }
        let mut guard = self.rx_resume_at.lock().unwrap();
        match *guard {
            Some(t) if Instant::now() < t => true,
            Some(_) => {
                *guard = None;
                false
            }
            None => false,
        }
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
        let _ = self.send(self.flavor().ptt(false)).await;
        info!("tx: aborted");
        Ok(())
    }

    /// Respond to a TXChrono request: hand the server the next
    /// `requested_floats` interleaved-stereo samples from the active TX
    /// waveform. Returns None if no transmission is in flight.
    fn build_chrono_response(
        &self,
        trx: u32,
        sample_rate: u32,
        requested_channels: u32,
        requested_floats: usize,
    ) -> Option<Vec<u8>> {
        let flavor = self.flavor();
        let mut guard = self.tx_state.lock().unwrap();
        let tx = guard.as_mut()?;
        self.chrono_seen.store(true, Ordering::SeqCst);
        // Spec servers: answer in the channel count they asked for (we ask
        // for mono on connect — two channels in DIGL/DIGU would be taken as
        // I/Q). RHR: interleaved stereo with a zero channel field, as ever.
        let (channels, header_channels, format) = match flavor {
            TciFlavor::Spec if requested_channels == 1 => (1usize, 1u32, TX_FORMAT_F32_SPEC),
            TciFlavor::Spec => (2, 2, TX_FORMAT_F32_SPEC),
            TciFlavor::Rhr => (2, 0, TX_FORMAT_F32_RHR),
        };
        let frames = requested_floats / channels;
        let mut mono = Vec::with_capacity(frames);
        let TxState::Live(live) = tx;
        live.generate(frames, &mut mono);
        for s in mono.iter_mut() {
            *s *= TX_AMPLITUDE;
        }
        let mut out = Vec::with_capacity(requested_floats);
        for s in mono {
            for _ in 0..channels {
                out.push(s);
            }
        }
        // Pad to the exact requested length (handles odd request sizes).
        while out.len() < requested_floats {
            out.push(0.0);
        }
        Some(build_tx_audio_frame(trx, sample_rate, format, header_channels, &out))
    }

    async fn set_state(&self, s: TciState) {
        *self.state.write().await = s.clone();
        let _ = self.app.emit("tci:state", &s);
    }

    /// Force the radio into DIGL — the sideband our AFSK RTTY tones assume.
    /// Best-effort: a no-op if we're not connected. The radio echoes the
    /// change back as a `modulation:0,digl` message, which updates rig state.
    async fn force_rtty_mode(&self) {
        let _ = self.send(format!("modulation:0,{};", RTTY_MODE)).await;
    }

    fn flavor(&self) -> TciFlavor {
        *self.flavor.lock().unwrap()
    }

    /// Key up in this server's dialect and start watching for its first
    /// request for TX audio.
    async fn key_up(&self) -> anyhow::Result<()> {
        self.chrono_seen.store(false, Ordering::SeqCst);
        self.send(self.flavor().ptt(true)).await
    }

    /// Keyed up, but the radio still hasn't asked for audio after
    /// NO_CHRONO_TIMEOUT: unkey rather than sit on a dead carrier.
    async fn check_no_chrono(&self, keyed_at: Instant) -> anyhow::Result<()> {
        if self.chrono_seen.load(Ordering::SeqCst) || keyed_at.elapsed() < NO_CHRONO_TIMEOUT {
            return Ok(());
        }
        warn!("tx: radio never requested TX audio — unkeying");
        *self.tx_state.lock().unwrap() = None;
        let _ = self.send(self.flavor().ptt(false)).await;
        anyhow::bail!(
            "the radio keyed up but never asked Diddle for TX audio — check that the TCI audio stream is enabled"
        )
    }

    /// True at most once per MODE_FIX_MIN_INTERVAL, recording the attempt.
    fn mode_fix_due(&self) -> bool {
        let mut last = self.last_mode_fix.lock().unwrap();
        let now = Instant::now();
        if last.is_some_and(|t| now.duration_since(t) < MODE_FIX_MIN_INTERVAL) {
            return false;
        }
        *last = Some(now);
        true
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
        *self.flavor.lock().unwrap() = TciFlavor::Rhr;
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
                                        info.channels,
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
                                // During TX (plus a short hold-off) the RX stream
                                // carries our own monitor/loopback audio — keep the
                                // waterfall alive but don't decode or spot it.
                                let rx_muted = self.rx_muted();
                                if !rx_muted {
                                    multi_dec.push_audio(&mono);
                                }
                                for frame in sp.push(&mono) {
                                    if !rx_muted {
                                        multi_dec.push_spectrum(&frame.mags_db, frame.fft_size);
                                    }
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
                                if !rx_muted {
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
                if self.flavor() == TciFlavor::Spec {
                    // Pin the stream format our TX generator and RX decoder
                    // assume. Mono, because in DIGL/DIGU a 2-channel stream
                    // is I/Q, not left/right. (RHR is left as it was.)
                    for c in [
                        "audio_samplerate:48000;",
                        "audio_stream_sample_type:float32;",
                        "audio_stream_channels:1;",
                    ] {
                        let _ = self.send(c.to_string()).await;
                    }
                }
                let _ = self.send("audio_start:0;".to_string()).await;
                // Force DIGL so AFSK RTTY lands on the expected sideband.
                self.force_rtty_mode().await;
            }
            "protocol" => {
                let name = m.arg_str(0).unwrap_or("");
                let flavor = TciFlavor::from_protocol(name);
                info!(server = %name, version = ?m.arg_str(1), ?flavor, "TCI protocol");
                *self.flavor.lock().unwrap() = flavor;
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
                    // Something else changed the mode — the RHR Console does
                    // on spectrum clicks and band changes. USB/DIGU swaps mark
                    // and space, so the decoder prints garbage until we next
                    // key up. Put DIGL back now instead.
                    if !mode.eq_ignore_ascii_case(RTTY_MODE) && self.mode_fix_due() {
                        info!(%mode, "rig left DIGL — restoring");
                        self.force_rtty_mode().await;
                    }
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

#[cfg(test)]
mod flavor_tests {
    use super::*;

    #[test]
    fn expertsdr_gets_spec_ptt_and_rhr_is_unchanged() {
        let e = TciFlavor::from_protocol("ExpertSDR3");
        assert_eq!(e, TciFlavor::Spec);
        assert_eq!(e.ptt(true), "trx:0,true,tci;");
        assert_eq!(e.ptt(false), "trx:0,false;");
        assert_eq!(TciFlavor::from_protocol("ExpertSDR2"), TciFlavor::Spec);
        let r = TciFlavor::from_protocol("RHR");
        assert_eq!(r, TciFlavor::Rhr);
        assert_eq!(r.ptt(true), "trx:0,true,vac;");
        assert_eq!(r.ptt(false), "trx:0,false,vac;");
        assert_eq!(TciFlavor::from_protocol(""), TciFlavor::Rhr);
    }

    #[test]
    fn tx_frame_header_carries_format_and_channels() {
        let f = build_tx_audio_frame(0, 48_000, TX_FORMAT_F32_SPEC, 1, &[0.5, -0.5]);
        let u = |o: usize| u32::from_le_bytes([f[o], f[o + 1], f[o + 2], f[o + 3]]);
        assert_eq!(u(4), 48_000);
        assert_eq!(u(8), 3); // float32 per spec
        assert_eq!(u(20), 2); // length = values
        assert_eq!(u(24), TX_STREAM_TYPE);
        assert_eq!(u(28), 1); // channels
        assert_eq!(f.len(), AUDIO_HEADER_BYTES + 8);
    }
}
