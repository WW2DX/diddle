// Spectrum processor: takes incoming mono audio samples, slides them
// through an FFT with a Hann window, emits magnitude (dB) bins per frame.
//
// Supports overlap (stride < fft_size). The output bins cover
// 0..sample_rate/2 of audio frequency, which for a USB receiver maps to
// (vfo + k * sample_rate / fft_size) radio frequency.

use std::sync::Arc;

use rustfft::{num_complex::Complex32, Fft, FftPlanner};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct SpectrumFrame {
    pub mags_db: Vec<f32>,
    pub fft_size: usize,
    pub sample_rate: u32,
    pub seq: u64,
}

pub struct Spectrum {
    pub fft_size: usize,
    pub stride: usize,
    pub sample_rate: u32,
    fft: Arc<dyn Fft<f32>>,
    window: Vec<f32>,
    buf: Vec<f32>,
    scratch: Vec<Complex32>,
    seq: u64,
}

impl Spectrum {
    /// `stride` < `fft_size` enables overlapping windows. `stride == fft_size`
    /// is no overlap; `stride == fft_size / 4` is 75% overlap.
    pub fn new(fft_size: usize, stride: usize, sample_rate: u32) -> Self {
        assert!(fft_size.is_power_of_two(), "fft_size must be a power of 2");
        assert!(stride > 0 && stride <= fft_size, "stride must be in 1..=fft_size");
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(fft_size);
        let window: Vec<f32> = (0..fft_size)
            .map(|i| {
                0.5 - 0.5
                    * (2.0 * std::f32::consts::PI * i as f32 / (fft_size - 1) as f32).cos()
            })
            .collect();
        Self {
            fft_size,
            stride,
            sample_rate,
            fft,
            window,
            buf: Vec::with_capacity(fft_size * 2),
            scratch: vec![Complex32::new(0.0, 0.0); fft_size],
            seq: 0,
        }
    }

    /// Push mono samples in; returns any FFT frames that became available.
    pub fn push(&mut self, samples: &[f32]) -> Vec<SpectrumFrame> {
        self.buf.extend_from_slice(samples);
        let mut frames = Vec::new();
        while self.buf.len() >= self.fft_size {
            for i in 0..self.fft_size {
                self.scratch[i] = Complex32::new(self.buf[i] * self.window[i], 0.0);
            }
            self.fft.process(&mut self.scratch);

            let half = self.fft_size / 2;
            let norm = (self.fft_size as f32) * 0.5; // Hann coherent gain
            let eps = 1.0e-12_f32;
            let mags_db: Vec<f32> = self.scratch[..half]
                .iter()
                .map(|c| {
                    let p = (c.re * c.re + c.im * c.im).sqrt() / norm;
                    20.0 * (p + eps).log10()
                })
                .collect();

            frames.push(SpectrumFrame {
                mags_db,
                fft_size: self.fft_size,
                sample_rate: self.sample_rate,
                seq: self.seq,
            });
            self.seq = self.seq.wrapping_add(1);
            // Advance by stride; keep (fft_size - stride) samples for overlap.
            self.buf.drain(0..self.stride);
        }
        frames
    }
}
