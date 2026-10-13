// DeepRTTY: a neural-network RTTY decoder (e04/DeepRTTY, AGPL-3.0), run
// beside the classic demodulator. The model reads a 12-second spectrogram
// around the tone pair and outputs ITA2 characters (CTC); a new window is
// decoded about once a second and overlapping windows are merged, so text
// arrives ~1.5–2.5 s after it's sent. 45.45 baud / 170 Hz shift only.
//
// It costs one 12 s window per second of audio — ~40 ms of CPU each in a
// release build — on its own thread, never on the audio path. The model
// runs in pure Rust (tract), so there's no native runtime to ship.

mod features;
mod stream;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock};

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tracing::{info, warn};
use tract_onnx::prelude::*;

use crate::dsp::RttyConfig;
use features::{FrontEnd, Spectrogram, BINS, MODEL_RATE};
use stream::{ctc_tokens, Ita2, OverlapMerger, NUM_CLASSES};

static MODEL_BYTES: &[u8] = include_bytes!("../../models/deeprtty.onnx");

/// 12 s at 3200 samples/s.
const WINDOW: usize = 38_400;
/// A new window every ~1 s, on the model's output-frame grid (48 samples).
const OUTPUT_HOP: usize = 48;
const HOP: usize = 67 * OUTPUT_HOP; // 3216 samples ≈ 1.005 s
/// Tokens this close to a window's interior edge are left to its neighbour.
const EDGE_GUARD: i64 = 1600;
/// Same character in two windows: within 0.45 of a character time.
const MATCH_TOLERANCE: f64 = 0.45 * 7.5 / 45.45 * MODEL_RATE;
/// A retune bigger than this is a new station: start the windows afresh.
const NEW_STATION_HZ: f64 = 60.0;

type Model = TypedRunnableModel;

static ENABLED: AtomicBool = AtomicBool::new(false);
static MODEL: OnceLock<Result<Arc<Model>, String>> = OnceLock::new();
static STATUS: Mutex<DeepStatus> = Mutex::new(DeepStatus { state: "off", message: String::new() });

#[derive(Serialize, Clone, Debug)]
pub struct DeepStatus {
    /// "off" | "loading" | "on" | "unsupported" | "error"
    pub state: &'static str,
    pub message: String,
}

fn set_status(app: &AppHandle, state: &'static str, message: impl Into<String>) {
    let s = DeepStatus { state, message: message.into() };
    let changed = {
        let mut cur = STATUS.lock().unwrap();
        let changed = cur.state != s.state || cur.message != s.message;
        *cur = s.clone();
        changed
    };
    if changed {
        let _ = app.emit("rtty_deep:status", &s);
    }
}

pub fn status() -> DeepStatus {
    STATUS.lock().unwrap().clone()
}

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Turn the decoder on or off. Loading the model (~0.1 s) starts right away
/// so the first window isn't held up by it.
pub fn set_enabled(app: &AppHandle, on: bool) {
    ENABLED.store(on, Ordering::Relaxed);
    if !on {
        set_status(app, "off", "");
        return;
    }
    if MODEL.get().is_none() {
        set_status(app, "loading", "loading the model…");
        let app = app.clone();
        std::thread::spawn(move || match model() {
            Ok(_) => set_status(&app, "on", "listening"),
            Err(e) => set_status(&app, "error", e),
        });
    } else {
        match model() {
            Ok(_) => set_status(app, "on", "listening"),
            Err(e) => set_status(app, "error", e),
        }
    }
}

fn model() -> Result<Arc<Model>, String> {
    MODEL
        .get_or_init(|| {
            let t0 = std::time::Instant::now();
            let frames = Spectrogram::frames_for(WINDOW);
            let m = tract_onnx::onnx()
                // The file carries a symbolic batch size in its shape hints;
                // let tract work every shape out from the fixed input.
                .with_ignore_output_shapes(true)
                .with_ignore_value_info(true)
                .model_for_read(&mut std::io::Cursor::new(MODEL_BYTES))
                .and_then(|m| m.with_input_fact(0, f32::fact([1, 1, frames, BINS]).into()))
                .and_then(|m| m.into_optimized())
                .and_then(|m| m.into_runnable())
                .map_err(|e| format!("DeepRTTY model failed to load: {e}"))?;
            info!(elapsed = ?t0.elapsed(), "DeepRTTY: model ready");
            Ok(m)
        })
        .clone()
}

/// The streaming decoder: a rolling 12 s window of 3200 Hz audio, decoded
/// every hop, overlapping windows merged into final characters.
struct Decoder {
    model: Arc<Model>,
    spec: Spectrogram,
    buf: Vec<f32>,
    buf_start: i64,
    next_end: i64,
    merger: OverlapMerger,
    ita2: Ita2,
}

impl Decoder {
    fn new(model: Arc<Model>) -> Self {
        let mut d = Self {
            model,
            spec: Spectrogram::new(),
            buf: Vec::new(),
            buf_start: 0,
            next_end: 0,
            merger: OverlapMerger::new(EDGE_GUARD, MATCH_TOLERANCE),
            ita2: Ita2::default(),
        };
        d.reset();
        d
    }

    /// Start afresh. The window is pre-filled with silence so the first
    /// decode comes one hop after audio starts, not 12 s later; the
    /// spectrogram's noise estimate skips silent frames.
    fn reset(&mut self) {
        self.buf = vec![0.0; WINDOW - HOP];
        self.buf_start = 0;
        self.next_end = WINDOW as i64;
        self.merger = OverlapMerger::new(EDGE_GUARD, MATCH_TOLERANCE);
        self.ita2 = Ita2::default();
    }

    fn push(&mut self, audio: &[f32]) -> String {
        self.buf.extend_from_slice(audio);
        let mut text = String::new();
        while self.buf_start + self.buf.len() as i64 >= self.next_end {
            let start = self.next_end - WINDOW as i64;
            let off = (start - self.buf_start) as usize;
            match self.decode_window(&self.buf[off..off + WINDOW], start) {
                Ok(tokens) => {
                    let done = self.merger.push(start, WINDOW as i64, tokens);
                    text.push_str(&self.ita2.decode(done.iter().map(|t| t.code)));
                }
                Err(e) => warn!("DeepRTTY: window failed: {e}"),
            }
            self.next_end += HOP as i64;
            // Keep only what the next window needs.
            let keep_from = self.next_end - WINDOW as i64;
            let drop = (keep_from - self.buf_start).max(0) as usize;
            self.buf.drain(..drop.min(self.buf.len()));
            self.buf_start += drop as i64;
        }
        text
    }

    fn decode_window(&self, audio: &[f32], start: i64) -> TractResult<Vec<stream::Token>> {
        let spec = self.spec.compute(audio);
        let frames = spec.len() / BINS;
        let input: Tensor = tract_ndarray::Array4::from_shape_vec((1, 1, frames, BINS), spec)?.into();
        let out = self.model.run(tvec!(input.into()))?;
        let lp = out[0].to_plain_array_view::<f32>()?;
        let lp: Vec<f32> = lp.iter().copied().collect();
        debug_assert_eq!(lp.len() % NUM_CLASSES, 0);
        Ok(ctc_tokens(&lp, start, OUTPUT_HOP))
    }
}

enum Msg {
    Audio(Vec<f32>),
    Centre(f64),
}

/// One per audio path (TCI, simulator, WAV, sound card), beside its classic
/// demodulator: hands RX audio to a DeepRTTY worker thread while the
/// decoder is switched on and the mode is one the model knows.
pub struct DeepTap {
    app: AppHandle,
    worker: Option<(Sender<Msg>, u32)>,
    centre: f64,
    supported: bool,
}

impl DeepTap {
    pub fn new(app: AppHandle) -> Self {
        Self { app, worker: None, centre: 2210.0, supported: true }
    }

    /// Follow the RX tones (retune, AFC, shift/baud change).
    pub fn retune(&mut self, cfg: &RttyConfig) {
        self.centre = 0.5 * (cfg.mark_hz + cfg.space_hz) as f64;
        let shift = (cfg.space_hz - cfg.mark_hz).abs();
        self.supported = (cfg.baud - 45.45).abs() < 0.5 && (shift - 170.0).abs() < 10.0;
        if let Some((tx, _)) = &self.worker {
            let _ = tx.send(Msg::Centre(self.centre));
        }
    }

    pub fn push(&mut self, mono: &[f32], rate: u32) {
        if !enabled() {
            self.worker = None; // the thread ends when its channel closes
            return;
        }
        if !self.supported {
            self.worker = None;
            set_status(&self.app, "unsupported", "DeepRTTY decodes 45.45 baud, 170 Hz shift only");
            return;
        }
        if self.worker.as_ref().map(|w| w.1) != Some(rate) {
            let (tx, rx) = mpsc::channel();
            let (app, centre) = (self.app.clone(), self.centre);
            std::thread::Builder::new()
                .name("deep-rtty".into())
                .spawn(move || worker(app, rx, rate, centre))
                .ok();
            self.worker = Some((tx, rate));
        }
        if let Some((tx, _)) = &self.worker {
            let _ = tx.send(Msg::Audio(mono.to_vec()));
        }
    }
}

fn worker(app: AppHandle, rx: Receiver<Msg>, rate: u32, mut centre: f64) {
    let model = match model() {
        Ok(m) => m,
        Err(e) => {
            set_status(&app, "error", e);
            return;
        }
    };
    set_status(&app, "on", "listening");
    let mut front = FrontEnd::new(rate, centre);
    let mut dec = Decoder::new(model);
    let mut audio = Vec::new();
    while let Ok(msg) = rx.recv() {
        match msg {
            Msg::Centre(c) => {
                if (c - centre).abs() > NEW_STATION_HZ {
                    dec.reset();
                }
                centre = c;
                front.set_centre(c);
            }
            Msg::Audio(x) => {
                audio.clear();
                front.push(&x, &mut audio);
                let text = dec.push(&audio);
                if !text.is_empty() {
                    let _ = app.emit("rtty_deep", &text);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::RttyTxGenerator;
    use rand::{rngs::StdRng, Rng, SeedableRng};

    const TEXT: &str = "CQ TEST DE K6AC K6AC 599 05 CA TU DE W1AW 599 05 CT";

    /// RTTY at 48 kHz on the usual 2125/2295 pair in white noise at the
    /// given SNR (2500 Hz bandwidth): the audio, for both decoders.
    fn noisy(snr_db: f32, seed: u64) -> Vec<f32> {
        let sr = 48_000;
        let mut g = RttyTxGenerator::new(sr, 2125.0, 2295.0, 45.45);
        let spb = g.samples_per_bit();
        let mut w = Vec::new();
        g.next_samples((40.0 * spb) as usize, &mut w);
        g.enqueue(&format!("  {TEXT}  "));
        while !g.is_idle() {
            g.next_samples(256, &mut w);
        }
        // Mark idle afterwards so the last characters' windows complete.
        g.next_samples(4 * sr as usize, &mut w);
        let sigma = 0.05f32;
        let amp = (2.0 * sigma * sigma * (2500.0 / (sr as f32 / 2.0)) * 10f32.powf(snr_db / 10.0)).sqrt();
        let mut rng = StdRng::seed_from_u64(seed);
        w
            .iter()
            .map(|s| {
                let u1: f32 = rng.gen_range(1e-7..1.0);
                let u2: f32 = rng.gen();
                s * amp + (-2.0 * u1.ln()).sqrt() * (std::f32::consts::TAU * u2).cos() * sigma
            })
            .collect()
    }

    /// The whole DeepRTTY streaming path, as a TCI/simulator feed drives it.
    fn deep(x: &[f32]) -> String {
        let mut front = FrontEnd::new(48_000, 2210.0);
        let mut dec = Decoder::new(model().expect("model"));
        let mut out = String::new();
        let mut buf = Vec::new();
        for chunk in x.chunks(4800) {
            buf.clear();
            front.push(chunk, &mut buf);
            out.push_str(&dec.push(&buf));
        }
        out
    }

    #[test]
    fn front_end_puts_the_pair_at_800_hz() {
        // A 2125 Hz tone at 48 kHz (pair centre 2210) → 715 Hz at 3200 Hz.
        let sr = 48_000u32;
        let x: Vec<f32> = (0..sr)
            .map(|i| (std::f64::consts::TAU * 2125.0 * i as f64 / sr as f64).sin() as f32)
            .collect();
        let mut fe = FrontEnd::new(sr, 2210.0);
        let mut y = Vec::new();
        fe.push(&x, &mut y);
        assert!((y.len() as i64 - 3200).abs() <= 1, "{} samples", y.len());
        // Power at 715 Hz against its mirror about 800 Hz (where a sloppy
        // real-valued shift would also put it) and a bin well away.
        let p = |f: f64| {
            let y = &y[800..];
            let (mut re, mut im) = (0f64, 0f64);
            for (n, v) in y.iter().enumerate() {
                let a = std::f64::consts::TAU * f * n as f64 / MODEL_RATE;
                re += *v as f64 * a.cos();
                im -= *v as f64 * a.sin();
            }
            re * re + im * im
        };
        let (on, mirror, far) = (p(715.0), p(885.0), p(300.0));
        assert!(on > 1000.0 * mirror && on > 1000.0 * far, "on {on} mirror {mirror} far {far}");
    }

    fn classic(x: &[f32]) -> String {
        let cfg = RttyConfig { squelch: 0.0, ..RttyConfig::default() };
        let mut d = crate::dsp::RttyDemod::new(48_000, cfg);
        x.chunks(4800).map(|c| d.push(c)).collect()
    }

    /// Characters of TEXT copied (best-aligned window, edit distance).
    fn copy(out: &str) -> f32 {
        let (a, b): (Vec<char>, Vec<char>) = (out.replace('\n', " ").chars().collect(), TEXT.chars().collect());
        let lev = |a: &[char]| {
            let mut prev: Vec<usize> = (0..=b.len()).collect();
            for i in 1..=a.len() {
                let mut cur = vec![i; b.len() + 1];
                for j in 1..=b.len() {
                    cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + (a[i - 1] != b[j - 1]) as usize);
                }
                prev = cur;
            }
            prev[b.len()]
        };
        let mut best = lev(&a);
        for s in 0..a.len().saturating_sub(b.len()) {
            best = best.min(lev(&a[s..(s + b.len()).min(a.len())]));
        }
        1.0 - best as f32 / b.len() as f32
    }

    #[test]
    fn clean_signal_copies_exactly() {
        let out = deep(&noisy(10.0, 2));
        assert!(out.contains(TEXT), "got {out:?}");
    }

    // The point of it: well below where the classic decoder gives up.
    #[test]
    fn copies_weak_signals_the_classic_decoder_loses() {
        for (snr, seed) in [(-10.0, 1), (-12.0, 3)] {
            let x = noisy(snr, seed);
            let (d, c) = (copy(&deep(&x)), copy(&classic(&x)));
            eprintln!("{snr} dB: DeepRTTY {d:.2}, classic {c:.2}");
            assert!(d > 0.8 && d > c + 0.25, "{snr} dB: deep {d:.2} vs classic {c:.2}");
        }
    }
}

