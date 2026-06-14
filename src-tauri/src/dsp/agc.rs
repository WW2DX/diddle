// Slow AGC for RTTY audio. Tracks a running mean-square level and applies a
// scalar gain so the long-term RMS sits at `target`. Time constant chosen
// long enough (default ~1 s) that bit-rate amplitude swings are not affected.
//
// Mean is in x² (power), so we square-root at gain time. Gain is clamped to
// prevent runaway during silence.

pub struct Agc {
    target: f32,
    rms2: f32,
    alpha: f32,
    max_gain: f32,
}

impl Agc {
    pub fn new(sample_rate: u32, target: f32, time_const_sec: f32) -> Self {
        // Single-pole IIR coefficient for the time constant.
        let tc_samples = time_const_sec * sample_rate as f32;
        let alpha = 1.0 - (-1.0 / tc_samples).exp();
        Self {
            target,
            rms2: 1.0e-6,
            alpha,
            max_gain: 100.0,
        }
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        self.rms2 = self.rms2 * (1.0 - self.alpha) + x * x * self.alpha;
        let rms = self.rms2.sqrt() + 1.0e-9;
        let gain = (self.target / rms).min(self.max_gain);
        x * gain
    }
}
