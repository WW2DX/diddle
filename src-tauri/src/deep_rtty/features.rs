// DeepRTTY input: the RX audio moved to where the model listens (the pair
// centred at 800 Hz, 3200 samples/s), and the normalised spectrogram it reads.
// Both follow the reference implementation (examples/python/rtty_stream.py
// in e04/DeepRTTY) number for number; the spectrogram is checked against it
// in the tests.

use std::f64::consts::TAU;
use std::sync::Arc;

use rustfft::{num_complex::Complex32, Fft, FftPlanner};

use crate::dsp::Biquad64;

/// The model listens at 3200 samples/s with the pair centred at 800 Hz
/// (a quarter of the rate, which makes the final mix-up a sign pattern).
pub const MODEL_RATE: f64 = 3200.0;

const FFT_LENGTH: usize = 128;
const WIN_LENGTH: usize = 96;
pub const HOP_LENGTH: usize = 24;
// Spectrogram band: 650–950 Hz in 25 Hz bins → bins 26..=38.
const START_BIN: usize = 26;
pub const BINS: usize = 13;
const NOISE_QUANTILE: f64 = 0.25;
const SILENT_REFERENCE_QUANTILE: f64 = 0.9;
const SILENT_RELATIVE_LEVEL: f32 = 0.01;
const RAYLEIGH_FACTOR: f32 = 0.758_527_6;
const NOISE_MIN_RELATIVE_TO_PEAK: f32 = 0.001;
const NOISE_MIN: f32 = 0.000_01;

/// Moves the RTTY pair from wherever it is in the RX audio to the model's
/// 800 Hz centre at 3200 samples/s: mix down to complex baseband, lowpass,
/// resample, mix back up to 800 Hz. Only the pair's own side of the
/// spectrum comes through — no image of the band below folds onto it.
pub struct FrontEnd {
    in_rate: f64,
    phase: f64,
    step: f64,
    lp_i: Vec<Biquad64>,
    lp_q: Vec<Biquad64>,
    // Input samples consumed, and when (in input samples) the next output
    // sample falls; outputs are linearly interpolated between inputs.
    n_in: u64,
    next_out: f64,
    out_step: f64,
    prev: (f64, f64),
    k: u64,
}

impl FrontEnd {
    pub fn new(in_rate: u32, centre_hz: f64) -> Self {
        let in_rate = in_rate as f64;
        // Keeps 800 ± 700 Hz (the model reads 650–950 Hz); everything that
        // could alias onto it after resampling is ~100 dB down.
        let lp = Biquad64::butterworth_lowpass(in_rate, 700.0, 8);
        Self {
            in_rate,
            phase: 0.0,
            step: TAU * centre_hz / in_rate,
            lp_i: lp.clone(),
            lp_q: lp,
            n_in: 0,
            next_out: 0.0,
            out_step: in_rate / MODEL_RATE,
            prev: (0.0, 0.0),
            k: 0,
        }
    }

    /// Follow a retune (AFC, a click on the waterfall).
    pub fn set_centre(&mut self, centre_hz: f64) {
        self.step = TAU * centre_hz / self.in_rate;
    }

    pub fn push(&mut self, input: &[f32], out: &mut Vec<f32>) {
        for &x in input {
            let (s, c) = self.phase.sin_cos();
            self.phase += self.step;
            if self.phase > TAU {
                self.phase -= TAU;
            }
            let mut zi = x as f64 * c;
            let mut zq = -(x as f64) * s;
            for f in self.lp_i.iter_mut() {
                zi = f.process(zi);
            }
            for f in self.lp_q.iter_mut() {
                zq = f.process(zq);
            }
            let t = self.n_in as f64;
            while self.next_out <= t {
                // Between the previous input (t − 1) and this one.
                let a = (self.next_out - (t - 1.0)).clamp(0.0, 1.0);
                let i = self.prev.0 + a * (zi - self.prev.0);
                let q = self.prev.1 + a * (zq - self.prev.1);
                // × e^{+j·2π·800·k/3200} = × j^k, real part.
                let y = match self.k % 4 {
                    0 => i,
                    1 => -q,
                    2 => -i,
                    _ => q,
                };
                out.push(y as f32);
                self.k += 1;
                self.next_out += self.out_step;
            }
            self.prev = (zi, zq);
            self.n_in += 1;
        }
    }
}

/// k-th smallest (0-based) of a copy of `v`.
fn kth(v: &[f32], k: usize) -> f32 {
    let mut w = v.to_vec();
    let (_, x, _) = w.select_nth_unstable_by(k, |a, b| a.total_cmp(b));
    *x
}

pub struct Spectrogram {
    fft: Arc<dyn Fft<f32>>,
    window: [f32; FFT_LENGTH],
}

impl Spectrogram {
    pub fn new() -> Self {
        let mut window = [0f32; FFT_LENGTH];
        let offset = (FFT_LENGTH - WIN_LENGTH) / 2;
        for i in 0..WIN_LENGTH {
            // Periodic Hann, centred in the FFT frame (torch.stft).
            window[offset + i] =
                (0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / WIN_LENGTH as f64).cos()) as f32;
        }
        Self { fft: FftPlanner::new().plan_fft_forward(FFT_LENGTH), window }
    }

    /// Frames for `n` samples (centred STFT): n / hop + 1.
    pub fn frames_for(n: usize) -> usize {
        n / HOP_LENGTH + 1
    }

    /// Normalised log spectrogram, frames × BINS, row-major.
    pub fn compute(&self, audio: &[f32]) -> Vec<f32> {
        let n = audio.len();
        let pad = FFT_LENGTH / 2;
        // Reflect padding (numpy "reflect": the edge sample isn't repeated).
        let at = |i: isize| -> f32 {
            let mut j = i;
            if j < 0 {
                j = -j;
            }
            if j >= n as isize {
                j = 2 * (n as isize - 1) - j;
            }
            audio[j as usize]
        };
        let frames = Self::frames_for(n);
        let mut mag = vec![0f32; frames * BINS];
        let mut buf = vec![Complex32::new(0.0, 0.0); FFT_LENGTH];
        for f in 0..frames {
            let start = (f * HOP_LENGTH) as isize - pad as isize;
            for (i, b) in buf.iter_mut().enumerate() {
                *b = Complex32::new(at(start + i as isize) * self.window[i], 0.0);
            }
            self.fft.process(&mut buf);
            for k in 0..BINS {
                mag[f * BINS + k] = buf[START_BIN + k].norm();
            }
        }

        // Noise level: the 25th percentile of the band, over frames that
        // aren't digital silence (a pre-filled or paused stream).
        let frame_rank = (NOISE_QUANTILE * (BINS - 1) as f64).floor() as usize;
        let levels: Vec<f32> = mag.chunks(BINS).map(|row| kth(row, frame_rank)).collect();
        let ref_rank = (SILENT_REFERENCE_QUANTILE * (levels.len() - 1) as f64).floor() as usize;
        let reference = kth(&levels, ref_rank);
        let active: Vec<f32> = mag
            .chunks(BINS)
            .zip(&levels)
            .filter(|(_, &l)| l >= reference * SILENT_RELATIVE_LEVEL)
            .flat_map(|(row, _)| row.iter().copied())
            .collect();
        let q25 = kth(&active, (NOISE_QUANTILE * (active.len() - 1) as f64).floor() as usize);
        let peak = mag.iter().copied().fold(0f32, f32::max);
        let noise = (q25 / RAYLEIGH_FACTOR).max(peak * NOISE_MIN_RELATIVE_TO_PEAK).max(NOISE_MIN);
        for m in mag.iter_mut() {
            *m = (*m / noise).ln_1p();
        }
        mag
    }
}
