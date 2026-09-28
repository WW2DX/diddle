// Standard transposed-direct-form-II biquad. Coefficient formulas from
// the RBJ audio EQ cookbook.

#[derive(Debug, Clone)]
pub struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}

impl Biquad {
    /// Bandpass with constant-skirt gain (peak gain Q).
    pub fn bandpass(sample_rate: f32, f0: f32, q: f32) -> Self {
        let w0 = 2.0 * std::f32::consts::PI * f0 / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);
        let a0 = 1.0 + alpha;
        Self {
            b0: alpha / a0,
            b1: 0.0,
            b2: -alpha / a0,
            a1: -2.0 * cos_w0 / a0,
            a2: (1.0 - alpha) / a0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }
}

/// f64 biquad for very low cutoffs relative to the sample rate (a ~160 Hz
/// channel filter at 48 kHz), where f32 coefficient/state rounding in the
/// high-Q sections would add audible-level noise and drift.
#[derive(Debug, Clone)]
pub struct Biquad64 {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    z1: f64,
    z2: f64,
}

impl Biquad64 {
    /// RBJ lowpass.
    pub fn lowpass(sample_rate: f64, f0: f64, q: f64) -> Self {
        let w0 = 2.0 * std::f64::consts::PI * f0 / sample_rate;
        let (sin_w0, cos_w0) = w0.sin_cos();
        let alpha = sin_w0 / (2.0 * q);
        let a0 = 1.0 + alpha;
        let b1 = (1.0 - cos_w0) / a0;
        Self {
            b0: b1 * 0.5,
            b1,
            b2: b1 * 0.5,
            a1: -2.0 * cos_w0 / a0,
            a2: (1.0 - alpha) / a0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    /// The sections of an order-`order` (even) Butterworth lowpass.
    pub fn butterworth_lowpass(sample_rate: f64, f0: f64, order: usize) -> Vec<Self> {
        let n = order.max(2) & !1;
        (0..n / 2)
            .map(|k| {
                let theta = std::f64::consts::PI * (2 * k + 1) as f64 / (2 * n) as f64;
                Self::lowpass(sample_rate, f0, 1.0 / (2.0 * theta.cos()))
            })
            .collect()
    }

    #[inline]
    pub fn process(&mut self, x: f64) -> f64 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }
}
