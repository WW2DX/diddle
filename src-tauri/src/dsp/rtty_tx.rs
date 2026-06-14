// AFSK transmitter. Takes a text string, encodes it into ITA2 Baudot frames
// (with auto LTRS/FIGS shifting), and generates continuous-phase mark/space
// audio samples that can be sent to a TCI TX audio stream.

use std::collections::VecDeque;
use std::f32::consts::TAU;

const FIGS_SHIFT: u8 = 0b11011; // 27
const LTRS_SHIFT: u8 = 0b11111; // 31

// Reverse lookup of the tables in dsp::rtty (LSB-first, fldigi convention).
// Index = ITA2 5-bit symbol value. Entries marked '\0' are non-printable
// or never selected as TX output (we map '\n' → LF code 2).
#[rustfmt::skip]
const LTRS: [char; 32] = [
    '\0','E','\n','A',' ','S','I','U',
    '\r','D','R','J','N','F','C','K',
    'T','Z','L','W','H','Y','P','Q',
    'O','B','G','\0','M','X','V','\0',
];

#[rustfmt::skip]
const FIGS: [char; 32] = [
    '\0','3','\0','-',' ','\'','8','7',
    '\r','$','4','\0',',','!',':','(',
    '5','"',')','2','#','6','0','1',
    '9','?','&','\0','.','/',';','\0',
];

pub struct RttyTxGenerator {
    sample_rate: f32,
    samples_per_bit: f32,
    mark_hz: f32,
    space_hz: f32,
    phase: f32,

    bit_queue: VecDeque<bool>, // true = mark
    samples_in_current_bit: f32,
    current_bit: bool,

    figs: bool,

    // Running count of bits appended to the queue over the generator's life.
    // Used to time the per-character TX echo: a character's audio begins at
    // bit index `bits_appended` (just before its data frame is queued).
    bits_appended: usize,
}

impl RttyTxGenerator {
    pub fn new(sample_rate: u32, mark_hz: f32, space_hz: f32, baud: f32) -> Self {
        let sr = sample_rate as f32;
        Self {
            sample_rate: sr,
            samples_per_bit: sr / baud,
            mark_hz,
            space_hz,
            phase: 0.0,
            bit_queue: VecDeque::new(),
            samples_in_current_bit: 0.0,
            current_bit: true, // start in mark (idle)
            figs: false,
            bits_appended: 0,
        }
    }

    /// Append text to the TX queue. Auto-inserts LTRS/FIGS shifts as needed.
    /// Characters not in the Baudot tables are silently dropped.
    pub fn enqueue(&mut self, text: &str) {
        for c in text.chars() {
            self.encode_char(c);
        }
    }

    /// Like [`enqueue`], but also returns a timeline of `(bit_index, char)`
    /// marks — one per character actually queued — where `bit_index` is the
    /// bit offset (from the start of this generator's output) at which the
    /// character's own data frame begins. Any inserted LTRS/FIGS shift frame
    /// is charged to the *following* character, and dropped characters
    /// produce no mark, so the timeline stays aligned with the audio. Callers
    /// convert bit index → sample offset via [`samples_per_bit`] to pace a
    /// live TX echo in the UI.
    pub fn enqueue_with_marks(&mut self, text: &str) -> Vec<(usize, char)> {
        let mut marks = Vec::with_capacity(text.len());
        for c in text.chars() {
            let uc = c.to_ascii_uppercase();
            if uc == '\n' {
                marks.push((self.bits_appended, '\n'));
                self.append_frame(8); // CR
                self.append_frame(2); // LF
                continue;
            }
            if let Some(code) = lookup_ltrs(uc) {
                if self.figs {
                    self.append_frame(LTRS_SHIFT);
                    self.figs = false;
                }
                marks.push((self.bits_appended, uc));
                self.append_frame(code);
            } else if let Some(code) = lookup_figs(uc) {
                if !self.figs {
                    self.append_frame(FIGS_SHIFT);
                    self.figs = true;
                }
                marks.push((self.bits_appended, uc));
                self.append_frame(code);
            }
            // Else: char not in either table; drop it (and emit no mark).
        }
        marks
    }

    /// Drain the queue into the queue of bits (each char = start + 5 data +
    /// stop). Lowercase letters are uppercased; '\n' is sent as CR + LF.
    fn encode_char(&mut self, c: char) {
        let c = c.to_ascii_uppercase();
        if c == '\n' {
            // Most-robust newline: CR (code 8) + LF (code 2). Receivers
            // handle both, but writing pairs avoids surprises.
            self.append_frame(8); // CR
            self.append_frame(2); // LF
            return;
        }
        if let Some(code) = lookup_ltrs(c) {
            if self.figs {
                self.append_frame(LTRS_SHIFT);
                self.figs = false;
            }
            self.append_frame(code);
        } else if let Some(code) = lookup_figs(c) {
            if !self.figs {
                self.append_frame(FIGS_SHIFT);
                self.figs = true;
            }
            self.append_frame(code);
        }
        // Else: char not in either table; drop it.
    }

    fn append_frame(&mut self, code: u8) {
        // Start bit: space (false).
        self.bit_queue.push_back(false);
        // 5 data bits, LSB first.
        for i in 0..5 {
            self.bit_queue.push_back(((code >> i) & 1) != 0);
        }
        // 1.5 stop bits — represent as 1 stop bit slot at 1.5x duration. We
        // model this as two queued mark bits, but only the first gets the
        // full bit time; the next start-of-character bit can interrupt the
        // second. Simpler: push 2 mark bits and accept slightly long stops.
        self.bit_queue.push_back(true);
        self.bit_queue.push_back(true);
        // 1 start + 5 data + 2 stop = 8 bits per frame.
        self.bits_appended += 8;
    }

    /// Generate the next `n` audio samples. Phase is continuous between
    /// bits (no audible clicks). Mark tone when the bit queue is empty.
    pub fn next_samples(&mut self, n: usize, out: &mut Vec<f32>) {
        out.reserve(n);
        for _ in 0..n {
            let freq = if self.current_bit {
                self.mark_hz
            } else {
                self.space_hz
            };
            let phase_inc = TAU * freq / self.sample_rate;
            out.push(self.phase.sin());
            self.phase += phase_inc;
            if self.phase >= TAU {
                self.phase -= TAU;
            }

            self.samples_in_current_bit += 1.0;
            if self.samples_in_current_bit >= self.samples_per_bit {
                self.samples_in_current_bit -= self.samples_per_bit;
                self.current_bit = self.bit_queue.pop_front().unwrap_or(true);
            }
        }
    }

    /// True once the queue is empty AND the trailing mark bit has finished.
    pub fn is_idle(&self) -> bool {
        self.bit_queue.is_empty() && self.samples_in_current_bit < 1.0
    }

    pub fn samples_per_bit(&self) -> f32 {
        self.samples_per_bit
    }
}

fn lookup_ltrs(c: char) -> Option<u8> {
    LTRS.iter()
        .position(|&x| x == c)
        .filter(|&i| i != 0 && i != 27 && i != 31)
        .map(|i| i as u8)
}

fn lookup_figs(c: char) -> Option<u8> {
    FIGS.iter()
        .position(|&x| x == c)
        .filter(|&i| i != 0 && i != 27 && i != 31)
        .map(|i| i as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enqueue_basic_letters() {
        let mut g = RttyTxGenerator::new(6000, 2125.0, 2295.0, 45.45);
        g.enqueue("EA");
        // 'E' = code 1 (LTRS); 'A' = code 3 (LTRS). No FIGS shift between.
        // Each frame: 1 start + 5 data + 2 stop bits = 8 bits.
        assert!(g.bit_queue.len() >= 16);
    }

    #[test]
    fn inserts_figs_for_digit() {
        let mut g = RttyTxGenerator::new(6000, 2125.0, 2295.0, 45.45);
        g.enqueue("E5");
        // 'E' → 1 LTRS frame; then FIGS shift; then '5' → 1 FIGS frame.
        // 3 frames × 8 bits = 24 bits minimum.
        assert!(g.bit_queue.len() >= 24);
        assert!(g.figs);
    }

    #[test]
    fn idle_outputs_mark() {
        let mut g = RttyTxGenerator::new(6000, 2125.0, 2295.0, 45.45);
        let mut buf = Vec::new();
        g.next_samples(50, &mut buf);
        // No queue, so we stay in mark tone. The audio is a sine; just
        // check it's non-empty.
        assert_eq!(buf.len(), 50);
    }
}
