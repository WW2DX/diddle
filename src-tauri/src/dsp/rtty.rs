// RTTY demodulator.
//
// - Two quadrature correlators (NCOs at mark/space), matched-filter
//   integration over one bit period.
// - Slicer: bigger of mark vs. space magnitude → mark=1, space=0.
// - Async-serial state machine: detect start bit (space after mark idle),
//   sample data bits at samples-per-bit intervals at end of each bit
//   (where the matched-filter output peaks), confirm stop bit is mark.
// - ITA2 (Baudot) decode with LTRS/FIGS shift state.
//
// Defaults: 45.45 baud, 170 Hz shift, tones mark=2125, space=2295. Diddle
// forces the radio into DIGL (LSB) on connect — the usual amateur-RTTY
// sideband — so these decode upright with REVerse off.

use std::f32::consts::TAU;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::dsp::{Agc, Biquad};

const FIGS_SHIFT: u8 = 0b11011; // 27
const LTRS_SHIFT: u8 = 0b11111; // 31

// ITA2 / Baudot, US variant. Indexed by 5-bit symbol value where bit 1
// (transmitted FIRST after the start bit) goes into the LSB. This matches
// fldigi's canonical decoder table — character → symbol value:
//   E=1 LF=2 A=3 SP=4 S=5 I=6 U=7 CR=8 D=9 R=10 J=11 N=12 F=13 C=14 K=15
//   T=16 Z=17 L=18 W=19 H=20 Y=21 P=22 Q=23 O=24 B=25 G=26 FIGS=27 M=28
//   X=29 V=30 LTRS=31.
//
// We emit '\n' for both CR (8) and LF (2) to flatten line-endings. BEL/WRU
// codes are silenced ('\0' = no character emitted).
#[rustfmt::skip]
const BAUDOT_LTRS: [char; 32] = [
    '\0','E','\n','A',' ','S','I','U',
    '\n','D','R','J','N','F','C','K',
    'T','Z','L','W','H','Y','P','Q',
    'O','B','G','\0','M','X','V','\0',
];

#[rustfmt::skip]
const BAUDOT_FIGS: [char; 32] = [
    '\0','3','\n','-',' ','\'','8','7',
    '\n','$','4','\0',',','!',':','(',
    '5','"',')','2','#','6','0','1',
    '9','?','&','\0','.','/',';','\0',
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RttyConfig {
    pub mark_hz: f32,
    pub space_hz: f32,
    pub baud: f32,
}

impl Default for RttyConfig {
    fn default() -> Self {
        Self {
            mark_hz: 2125.0,
            space_hz: 2295.0,
            baud: 45.45,
        }
    }
}

/// Shared, mutable RTTY tuning. Each audio consumer (TCI run-loop, WAV
/// player) holds a clone of the Arc and watches the generation counter;
/// when it bumps, the consumer rebuilds its local RttyDemod.
pub struct RttyTunable {
    cfg: RwLock<RttyConfig>,
    gen: AtomicU64,
}

impl RttyTunable {
    pub fn new(cfg: RttyConfig) -> Self {
        Self {
            cfg: RwLock::new(cfg),
            gen: AtomicU64::new(1),
        }
    }

    pub async fn set(&self, cfg: RttyConfig) {
        *self.cfg.write().await = cfg;
        self.gen.fetch_add(1, Ordering::Release);
    }

    pub async fn get(&self) -> RttyConfig {
        self.cfg.read().await.clone()
    }

    pub fn current_gen(&self) -> u64 {
        self.gen.load(Ordering::Acquire)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum SerialState {
    Idle,
    WaitStartEnd,
    Data,
    Stop,
}

pub struct RttyDemod {
    /// Live samples-per-bit estimate (adaptive — slowly tracks the
    /// transmitter's actual baud rate based on observed transition spacing).
    samples_per_bit: f32,
    int_len: usize,

    /// Sample index counter for transition timing.
    total_samples: u64,
    /// Sample index of the previous slicer transition during the current frame.
    last_transition_sample: u64,
    /// Bit value at the previous data-sample point (for transition detection).
    prev_data_bit: bool,
    /// Whether we have a valid `last_transition_sample` to measure from.
    have_transition: bool,

    // Slow audio AGC. Normalizes long-term RMS so amplitude-relative
    // thresholds (noise floor estimate) and the bandpass operate in a
    // consistent range regardless of source level.
    agc: Agc,

    // Pre-filter: 2-pole bandpass centered between mark and space, ~2.5×
    // shift wide. Kills out-of-band noise that would otherwise leak through
    // the boxcar correlator's −13 dB sidelobes. Single biquad chosen over
    // cascaded — keeps group delay small enough not to disturb bit timing
    // on clean signals, while still rejecting big adjacent interferers.
    bp1: Biquad,

    // NCO phases / increments for mark and space.
    mark_phase: f32,
    space_phase: f32,
    mark_phase_inc: f32,
    space_phase_inc: f32,

    // Sliding boxcar matched filter — circular buffers for the four
    // correlator outputs (I/Q of mark, I/Q of space) and their running sums.
    mark_i_buf: Vec<f32>,
    mark_q_buf: Vec<f32>,
    space_i_buf: Vec<f32>,
    space_q_buf: Vec<f32>,
    mark_i_sum: f32,
    mark_q_sum: f32,
    space_i_sum: f32,
    space_q_sum: f32,
    buf_idx: usize,

    // Slicer with hysteresis. Normalized discriminator
    // (mark_mag2 - space_mag2) / (mark_mag2 + space_mag2) must cross
    // ±HYST_THRESHOLD to flip the held bit. Rejects small-margin jitter
    // around the noise floor.
    last_bit: bool,

    // Signal-presence tracking. We maintain a slowly-updated noise-floor
    // estimate (only updated when the current sample looks like noise) and
    // gate start-bit detection on current energy being well above that
    // floor. This prevents the slicer from emitting random characters
    // during silence or weak-signal gaps.
    noise_floor: f32,

    // Async-serial state machine.
    state: SerialState,
    sample_counter: f32,
    data_bits: u8,
    bits_collected: u8,

    // Baudot shift state.
    figs: bool,
}

// Lower hysteresis is OK now that AGC + bandpass clean up the noise
// floor — the discriminator is much steadier, so 6% margin is enough.
const HYST_THRESHOLD: f32 = 0.06;
// Signal must be at least GATE_SNR times the noise-floor estimate for the
// idle state to accept a start-bit transition. 6× ≈ +8 dB SNR.
const GATE_SNR: f32 = 6.0;
// Noise floor updates only when current sample is in the bottom of recent
// observations — this is the "lower envelope" we expect noise to live at.
const NOISE_FLOOR_UPDATE_RATIO: f32 = 2.0;
// IIR coefficient for noise-floor tracking. ~1000 sample time constant
// (~170 ms at 6 kHz): fast enough to adapt to noise after a transmission
// ends, slow enough that brief silences inside a transmission don't drag
// the floor up to signal level.
const NOISE_FLOOR_ALPHA: f32 = 0.001;
// Bit-clock adjustment: each observed transition during a frame updates
// the running samples-per-bit estimate with this weight. Small enough that
// noise spikes can't yank the estimate around; large enough to settle in a
// few seconds of continuous signal.
const BIT_CLOCK_ALPHA: f32 = 0.05;
// Sanity range — only update when observed bits-between-transitions is
// reasonable (1..=6 bit periods).
const BIT_CLOCK_MAX_INTERVAL_BITS: f32 = 6.0;
// Reject observed-vs-current ratios outside this window. Catches gross
// outliers from noise or framing errors.
const BIT_CLOCK_MAX_DEVIATION: f32 = 0.05;

impl RttyDemod {
    pub fn new(sample_rate: u32, cfg: RttyConfig) -> Self {
        let sr = sample_rate as f32;
        let spb = sr / cfg.baud;
        let int_len = spb.round() as usize;

        // Bandpass centered between mark and space. Bandwidth = shift + ~130 Hz
        // of margin, with a 300 Hz floor — fits both tones and their
        // baud-rate sidebands without much excess. For a typical 170 Hz
        // amateur shift this is 300 Hz wide (Q ≈ 7); for wider commercial
        // shifts the filter scales up so the signal still fits.
        let center = 0.5 * (cfg.mark_hz + cfg.space_hz);
        let shift = (cfg.space_hz - cfg.mark_hz).abs();
        let bw = (shift + 130.0).max(300.0);
        let q = center / bw;
        let bp1 = Biquad::bandpass(sr, center, q);
        let agc = Agc::new(sample_rate, 0.2, 1.0);

        Self {
            mark_phase: 0.0,
            space_phase: 0.0,
            mark_phase_inc: TAU * cfg.mark_hz / sr,
            space_phase_inc: TAU * cfg.space_hz / sr,
            samples_per_bit: spb,
            int_len,
            total_samples: 0,
            last_transition_sample: 0,
            prev_data_bit: true,
            have_transition: false,
            agc,
            bp1,
            mark_i_buf: vec![0.0; int_len],
            mark_q_buf: vec![0.0; int_len],
            space_i_buf: vec![0.0; int_len],
            space_q_buf: vec![0.0; int_len],
            mark_i_sum: 0.0,
            mark_q_sum: 0.0,
            space_i_sum: 0.0,
            space_q_sum: 0.0,
            buf_idx: 0,
            last_bit: true, // idle line is mark
            noise_floor: 0.0,
            state: SerialState::Idle,
            sample_counter: 0.0,
            data_bits: 0,
            bits_collected: 0,
            figs: false,
        }
    }

    /// Feed one block of mono audio samples; returns any decoded characters
    /// (visible Baudot chars + newlines; shift sentinels are filtered out).
    pub fn push(&mut self, samples: &[f32]) -> String {
        let mut out = String::new();
        for &s in samples {
            if let Some(c) = self.process_sample(s) {
                out.push(c);
            }
        }
        out
    }

    fn process_sample(&mut self, s: f32) -> Option<char> {
        self.total_samples = self.total_samples.wrapping_add(1);
        // AGC first — normalizes long-term level. Then bandpass tightens
        // the spectrum around mark/space.
        let s = self.agc.process(s);
        let s = self.bp1.process(s);

        // Multiply by complex exponential at mark/space freqs.
        let (mc, ms) = (self.mark_phase.cos(), self.mark_phase.sin());
        let (sc, ss) = (self.space_phase.cos(), self.space_phase.sin());
        let mi = s * mc;
        let mq = s * ms;
        let si = s * sc;
        let sq = s * ss;

        self.mark_phase += self.mark_phase_inc;
        if self.mark_phase > TAU {
            self.mark_phase -= TAU;
        }
        self.space_phase += self.space_phase_inc;
        if self.space_phase > TAU {
            self.space_phase -= TAU;
        }

        // Sliding boxcar (matched filter over one bit period).
        let idx = self.buf_idx;
        self.mark_i_sum += mi - self.mark_i_buf[idx];
        self.mark_q_sum += mq - self.mark_q_buf[idx];
        self.space_i_sum += si - self.space_i_buf[idx];
        self.space_q_sum += sq - self.space_q_buf[idx];
        self.mark_i_buf[idx] = mi;
        self.mark_q_buf[idx] = mq;
        self.space_i_buf[idx] = si;
        self.space_q_buf[idx] = sq;
        self.buf_idx = (idx + 1) % self.int_len;

        let mark_mag2 =
            self.mark_i_sum * self.mark_i_sum + self.mark_q_sum * self.mark_q_sum;
        let space_mag2 =
            self.space_i_sum * self.space_i_sum + self.space_q_sum * self.space_q_sum;

        // Slicer with hysteresis: only flip the held bit when the normalized
        // discriminator clearly favors the other tone. Around the noise floor
        // both mag values are tiny and similar, so this prevents random flips.
        let total = mark_mag2 + space_mag2 + 1.0e-12;
        let disc = (mark_mag2 - space_mag2) / total;
        if self.last_bit && disc < -HYST_THRESHOLD {
            self.last_bit = false;
        } else if !self.last_bit && disc > HYST_THRESHOLD {
            self.last_bit = true;
        }
        let bit = self.last_bit;

        // Adaptive noise-floor tracking. The floor only catches up when the
        // current sample looks like noise (close to or below the existing
        // estimate) — this avoids being pulled up by a sustained signal.
        if self.noise_floor < 1.0e-9 || total < self.noise_floor * NOISE_FLOOR_UPDATE_RATIO {
            self.noise_floor =
                self.noise_floor * (1.0 - NOISE_FLOOR_ALPHA) + total * NOISE_FLOOR_ALPHA;
        }
        let signal_present = total > self.noise_floor * GATE_SNR;

        // Async-serial framing. Sample at end of each bit (matched filter peak).
        match self.state {
            SerialState::Idle => {
                if !bit && signal_present {
                    // Slicer flipped at ~mid-start-bit due to integrator
                    // delay; wait the rest of the start bit before sampling.
                    self.state = SerialState::WaitStartEnd;
                    self.sample_counter = self.samples_per_bit * 0.5;
                    // Frame starting fresh — reset transition history.
                    self.have_transition = false;
                    self.prev_data_bit = false; // start bit is space
                }
                None
            }
            SerialState::WaitStartEnd => {
                self.sample_counter -= 1.0;
                if self.sample_counter <= 0.0 {
                    if !bit {
                        self.state = SerialState::Data;
                        self.data_bits = 0;
                        self.bits_collected = 0;
                        self.sample_counter = self.samples_per_bit;
                    } else {
                        self.state = SerialState::Idle;
                    }
                }
                None
            }
            SerialState::Data => {
                // Bit-clock micro-tracking: every observed slicer transition
                // during the data bits gives us a new estimate of how many
                // samples the transmitter is actually spending per bit.
                if bit != self.prev_data_bit {
                    if self.have_transition {
                        let interval = self
                            .total_samples
                            .wrapping_sub(self.last_transition_sample)
                            as f32;
                        let n_bits = (interval / self.samples_per_bit).round();
                        if n_bits >= 1.0 && n_bits <= BIT_CLOCK_MAX_INTERVAL_BITS {
                            let observed = interval / n_bits;
                            let dev = (observed - self.samples_per_bit).abs()
                                / self.samples_per_bit;
                            if dev < BIT_CLOCK_MAX_DEVIATION {
                                self.samples_per_bit = self.samples_per_bit
                                    * (1.0 - BIT_CLOCK_ALPHA)
                                    + observed * BIT_CLOCK_ALPHA;
                            }
                        }
                    }
                    self.last_transition_sample = self.total_samples;
                    self.have_transition = true;
                    self.prev_data_bit = bit;
                }

                self.sample_counter -= 1.0;
                if self.sample_counter <= 0.0 {
                    if bit {
                        self.data_bits |= 1 << self.bits_collected;
                    }
                    self.bits_collected += 1;
                    self.sample_counter = self.samples_per_bit;
                    if self.bits_collected == 5 {
                        self.state = SerialState::Stop;
                    }
                }
                None
            }
            SerialState::Stop => {
                self.sample_counter -= 1.0;
                if self.sample_counter <= 0.0 {
                    self.state = SerialState::Idle;
                    if bit {
                        return self.decode_baudot(self.data_bits & 0b11111);
                    }
                    // Framing error (stop bit was space) — drop the character.
                }
                None
            }
        }
    }

    fn decode_baudot(&mut self, code: u8) -> Option<char> {
        if code == FIGS_SHIFT {
            self.figs = true;
            None
        } else if code == LTRS_SHIFT {
            self.figs = false;
            None
        } else {
            let c = if self.figs {
                BAUDOT_FIGS[code as usize]
            } else {
                BAUDOT_LTRS[code as usize]
            };
            // USOS: a received space resets shift state to LTRS. Standard
            // RTTY hygiene — noise that flips us into FIGS gets corrected on
            // the next inter-word space.
            if c == ' ' {
                self.figs = false;
            }
            if c == '\0' {
                None
            } else {
                Some(c)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Synthesize a noiseless AFSK signal carrying one Baudot character
    /// at the configured baud rate and feed it to the demod; check the
    /// decoded output.
    fn synth_char_signal(code: u8, cfg: &RttyConfig, sr: u32) -> Vec<f32> {
        let spb = sr as f32 / cfg.baud;
        let int_spb = spb.round() as usize;
        // 1 start (space) + 5 data + 1 stop (mark). Pad with a couple of
        // mark bits at start so the integrator settles.
        let bits = {
            let mut b = vec![true; int_spb * 4]; // mark idle pre-roll
            // start bit (space)
            b.extend(std::iter::repeat(false).take(int_spb));
            // data bits, bit 1 first (LSB first)
            for i in 0..5 {
                let v = ((code >> i) & 1) != 0;
                b.extend(std::iter::repeat(v).take(int_spb));
            }
            // stop bit (mark)
            b.extend(std::iter::repeat(true).take(int_spb * 2));
            b
        };
        let mut out = Vec::with_capacity(bits.len());
        let mut phase = 0.0f32;
        for &mark in &bits {
            let f = if mark { cfg.mark_hz } else { cfg.space_hz };
            let phase_inc = TAU * f / sr as f32;
            out.push(phase.sin());
            phase += phase_inc;
            if phase > TAU {
                phase -= TAU;
            }
        }
        out
    }

    #[test]
    fn decodes_letter_a() {
        // 'A' = code 3 in the LSB-first / fldigi convention.
        let cfg = RttyConfig::default();
        let sr = 48000u32;
        let mut d = RttyDemod::new(sr, cfg.clone());
        let signal = synth_char_signal(3, &cfg, sr);
        let text = d.push(&signal);
        assert!(text.contains('A'), "expected 'A' in decoded text, got: {:?}", text);
    }

    #[test]
    fn decodes_letter_e() {
        // 'E' = code 1.
        let cfg = RttyConfig::default();
        let sr = 48000u32;
        let mut d = RttyDemod::new(sr, cfg.clone());
        let signal = synth_char_signal(1, &cfg, sr);
        let text = d.push(&signal);
        assert!(text.contains('E'), "expected 'E' in decoded text, got: {:?}", text);
    }

    #[test]
    fn figs_shift_then_digit() {
        // FIGS shift, then code 22 = '0' (FIGS) / 'P' (LTRS).
        let cfg = RttyConfig::default();
        let sr = 48000u32;
        let mut d = RttyDemod::new(sr, cfg.clone());
        let mut signal = synth_char_signal(FIGS_SHIFT, &cfg, sr);
        signal.extend(synth_char_signal(22, &cfg, sr));
        let text = d.push(&signal);
        assert!(text.contains('0'), "expected '0' after FIGS, got: {:?}", text);
    }
}
