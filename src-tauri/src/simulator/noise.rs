// HF band noise for the simulator: atmospheric (pink) noise, receiver
// hiss, a couple of wandering SSB-ish QRM tones, splatter bursts and
// static crashes. Ported from EC5W's RTTY Runner so the decoder sees the
// same "40 m at night" texture people calibrate against.

use std::f32::consts::TAU;

use rand::Rng;

pub struct HfNoise {
    sample_rate: f32,
    pink: f32,
    qrm1: f32,
    qrm2: f32,
    qrm3: f32,
    splatter_phase: f32,
    splatter_counter: u32,
    splatter_active: bool,
    splatter_len: u32,
    splatter_freq: f32,
    crack_counter: u32,
    // Gaussian white noise via Box–Muller, keeping the spare value.
    spare: Option<f32>,
}

impl HfNoise {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate: sample_rate as f32,
            pink: 0.0,
            qrm1: 0.0,
            qrm2: 0.0,
            qrm3: 0.0,
            splatter_phase: 0.0,
            splatter_counter: 0,
            splatter_active: false,
            splatter_len: 0,
            splatter_freq: 800.0,
            crack_counter: u32::MAX / 2,
            spare: None,
        }
    }

    /// Add `level` (0..1) worth of noise to every sample in `buf`.
    pub fn add(&mut self, buf: &mut [f32], level: f32) {
        if level <= 0.0 {
            return;
        }
        let mut rng = rand::thread_rng();
        for s in buf.iter_mut() {
            let mut n = 0.0;
            n += self.pink(&mut rng) * 0.4;
            n += self.gauss(&mut rng) * 0.2;
            n += self.qrm() * 0.3;
            n += self.splatter(&mut rng) * 0.25;
            n += self.crack(&mut rng) * 0.15;
            *s = (*s + n * level * 0.8).clamp(-1.0, 1.0);
        }
    }

    fn gauss(&mut self, rng: &mut impl Rng) -> f32 {
        if let Some(v) = self.spare.take() {
            return v;
        }
        let u1: f32 = 1.0 - rng.gen::<f32>();
        let u2: f32 = rng.gen::<f32>();
        let r = (-2.0 * u1.ln()).sqrt();
        self.spare = Some(r * (TAU * u2).cos());
        r * (TAU * u2).sin()
    }

    fn pink(&mut self, rng: &mut impl Rng) -> f32 {
        let white: f32 = rng.gen::<f32>() * 2.0 - 1.0;
        self.pink = 0.99 * self.pink + 0.01 * white;
        self.pink * 3.0 + white * 0.3
    }

    fn qrm(&mut self) -> f32 {
        let sr = self.sample_rate;
        let mut q = 0.0;
        let m1 = 0.5 + 0.5 * (self.qrm1 * 0.0001).sin();
        q += self.qrm1.sin() * m1 * 0.4;
        self.qrm1 += TAU * (350.0 + 100.0 * (self.qrm1 * 0.00003).sin()) / sr;
        let m2 = 0.3 + 0.4 * (self.qrm2 * 0.00015).sin();
        q += self.qrm2.sin() * m2 * 0.25;
        self.qrm2 += TAU * (700.0 + 200.0 * (self.qrm2 * 0.00005).sin()) / sr;
        if (self.qrm3 * 0.00002).sin() > 0.3 {
            q += self.qrm3.sin() * 0.3;
        }
        self.qrm3 += TAU * 1100.0 / sr;
        if self.qrm1 > 1000.0 { self.qrm1 -= 1000.0; }
        if self.qrm2 > 1000.0 { self.qrm2 -= 1000.0; }
        if self.qrm3 > 1000.0 { self.qrm3 -= 1000.0; }
        q
    }

    fn splatter(&mut self, rng: &mut impl Rng) -> f32 {
        self.splatter_counter = self.splatter_counter.saturating_add(1);
        if !self.splatter_active && rng.gen::<f32>() < 0.00005 {
            self.splatter_active = true;
            self.splatter_counter = 0;
            self.splatter_freq = 600.0 + rng.gen::<f32>() * 800.0;
            self.splatter_len = (self.sample_rate * (0.05 + rng.gen::<f32>() * 0.15)) as u32;
        }
        if !self.splatter_active {
            return 0.0;
        }
        if self.splatter_counter > self.splatter_len {
            self.splatter_active = false;
            return 0.0;
        }
        let env = (-(self.splatter_counter as f32) * 3.0 / self.splatter_len.max(1) as f32).exp();
        let p = self.splatter_phase;
        let mut v = p.sin() * 0.5 + (p * 2.1).sin() * 0.3 + (p * 3.2).sin() * 0.2;
        v += (rng.gen::<f32>() * 2.0 - 1.0) * 0.3;
        self.splatter_phase += TAU * self.splatter_freq / self.sample_rate;
        if self.splatter_phase > TAU {
            self.splatter_phase -= TAU;
        }
        v * env
    }

    fn crack(&mut self, rng: &mut impl Rng) -> f32 {
        self.crack_counter = self.crack_counter.saturating_add(1);
        if rng.gen::<f32>() < 0.00002 {
            self.crack_counter = 0;
        }
        let len = (self.sample_rate * 0.01) as u32;
        if self.crack_counter < len {
            let env = (-(self.crack_counter as f32) * 5.0 / len as f32).exp();
            (rng.gen::<f32>() * 2.0 - 1.0) * env * 2.0
        } else {
            0.0
        }
    }
}
