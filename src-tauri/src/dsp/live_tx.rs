// Live keyboard transmission — the ad-hoc send that goes on the air as you
// type, WriteLog/N1MM style. Unlike a macro (whose whole waveform is built
// before keying), this keys up at once and produces audio on demand: typed
// text is queued at the next character boundary, and whenever the queue
// runs dry it is topped up with diddles (LTRS frames) so the other station
// hears a solid RTTY signal instead of a dead carrier or nothing at all.
//
// Shared by the TCI transmitter (the radio pulls samples through TXChrono)
// and the contest simulator (which paces it in real time).

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::dsp::RttyTxGenerator;

/// Unkey on our own after this long with nothing typed — someone walked
/// away from the keyboard with the rig diddling.
pub const LIVE_IDLE_TIMEOUT: Duration = Duration::from_secs(30);
/// Plain mark before the first diddle, so the far decoder settles.
const LEAD_MS: u32 = 150;
/// Mark after the last character, for a clean end of transmission.
const TRAIL_MS: u32 = 200;

pub struct LiveTx {
    gen: RttyTxGenerator,
    lead_left: usize,
    trail_left: usize,
    finishing: bool,
    /// Queue drained after finish; sending the closing mark.
    trailing: bool,
    done: bool,
    /// Typed characters waiting to be echoed, keyed by the generator bit
    /// index where each one's frame starts.
    echo: VecDeque<(usize, char)>,
    sent: String,
    last_push: Instant,
}

impl LiveTx {
    pub fn new(sample_rate: u32, mark_hz: f32, space_hz: f32, baud: f32) -> Self {
        Self::with_lead(sample_rate, mark_hz, space_hz, baud, LEAD_MS)
    }

    /// Like `new`, with `lead_ms` of plain mark before anything else.
    pub fn with_lead(sample_rate: u32, mark_hz: f32, space_hz: f32, baud: f32, lead_ms: u32) -> Self {
        Self {
            gen: RttyTxGenerator::new(sample_rate, mark_hz, space_hz, baud),
            lead_left: (sample_rate as u64 * lead_ms as u64 / 1000) as usize,
            trail_left: (sample_rate * TRAIL_MS / 1000) as usize,
            finishing: false,
            trailing: false,
            done: false,
            echo: VecDeque::new(),
            sent: String::new(),
            last_push: Instant::now(),
        }
    }

    /// Text can still join this transmission: anything up to the moment
    /// the closing mark begins. (After Enter, more text — a chained F-key —
    /// still goes out before the unkey.)
    pub fn accepts_text(&self) -> bool {
        !self.trailing && !self.done
    }

    /// Queue text. Ignored once the closing mark has begun.
    pub fn push(&mut self, text: &str) {
        if !self.accepts_text() || text.is_empty() {
            return;
        }
        self.echo.extend(self.gen.enqueue_with_marks(text));
        self.sent.push_str(text);
        self.last_push = Instant::now();
    }

    /// Stop diddling: send what's queued, then the trail, then done.
    pub fn finish(&mut self) {
        self.finishing = true;
    }

    pub fn is_finishing(&self) -> bool {
        self.finishing
    }

    /// Everything, trail included, has been generated.
    pub fn is_done(&self) -> bool {
        self.done
    }

    pub fn idle_for(&self) -> Duration {
        self.last_push.elapsed()
    }

    /// All text pushed so far (for logs and the simulator's reaction).
    pub fn sent_text(&self) -> &str {
        &self.sent
    }

    /// Produce the next `n` samples (unit amplitude; silence once done).
    pub fn generate(&mut self, n: usize, out: &mut Vec<f32>) {
        out.reserve(n);
        for _ in 0..n {
            if self.done {
                out.push(0.0);
                continue;
            }
            if self.lead_left > 0 {
                self.lead_left -= 1;
            } else if !self.finishing && self.gen.queue_empty() {
                // Top up with a single diddle, so typed text never waits
                // behind more than one frame (~176 ms at 45.45 baud).
                self.gen.enqueue_diddle();
            }
            // The generator reports idle only at a bit boundary, so latch it.
            if self.finishing && !self.trailing && self.gen.is_idle() {
                self.trailing = true;
            }
            if self.trailing {
                if self.trail_left == 0 {
                    self.done = true;
                    out.push(0.0);
                    continue;
                }
                self.trail_left -= 1;
            }
            self.gen.next_samples(1, out);
        }
    }

    /// Typed characters whose audio has started since the last call, in
    /// order — for the TX echo in the decoder window.
    pub fn take_echoes(&mut self) -> Vec<char> {
        let started = self.gen.bits_started();
        let mut out = Vec::new();
        while let Some(&(bit, c)) = self.echo.front() {
            if bit >= started {
                break;
            }
            out.push(c);
            self.echo.pop_front();
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::{RttyConfig, RttyDemod};

    const SR: u32 = 48_000;

    fn decode(wave: &[f32]) -> String {
        let mut d = RttyDemod::new(SR, RttyConfig::default());
        let mut out = String::new();
        for c in wave.chunks(512) {
            out.push_str(&d.push(c));
        }
        out
    }

    /// Diddles between typed words decode to nothing; the words decode
    /// intact, figures included, and the transmission ends on its own.
    #[test]
    fn typed_words_with_diddle_fill_decode_cleanly() {
        let mut tx = LiveTx::new(SR, 2125.0, 2295.0, 45.45);
        let mut wave = Vec::new();
        tx.generate(SR as usize, &mut wave); // 1 s of lead + diddles
        tx.push("CQ ");
        tx.generate(SR as usize * 2, &mut wave); // typed, then more diddles
        tx.push("TEST 5NN 05 NY ");
        tx.finish();
        let mut guard = 0;
        while !tx.is_done() && guard < 400 {
            tx.generate(4800, &mut wave);
            guard += 1;
        }
        assert!(tx.is_done(), "never finished");
        let got = decode(&wave);
        assert!(got.contains("CQ TEST 5NN 05 NY"), "decoded {got:?}");
        assert_eq!(tx.sent_text(), "CQ TEST 5NN 05 NY ");
    }

    /// The carrier never drops out while waiting for keystrokes: every
    /// 10 ms window before finish carries signal.
    #[test]
    fn no_gaps_while_waiting_for_keystrokes() {
        let mut tx = LiveTx::new(SR, 2125.0, 2295.0, 45.45);
        let mut wave = Vec::new();
        tx.generate(SR as usize * 3, &mut wave);
        for w in wave.chunks(480) {
            let e: f32 = w.iter().map(|x| x * x).sum::<f32>() / w.len() as f32;
            assert!(e > 0.3, "gap in live TX");
        }
    }

    /// Echo hands characters back in order, each once its frame starts.
    #[test]
    fn echo_follows_the_audio() {
        let mut tx = LiveTx::new(SR, 2125.0, 2295.0, 45.45);
        let mut wave = Vec::new();
        tx.generate(SR as usize / 2, &mut wave);
        tx.push("K6AC");
        assert!(tx.take_echoes().is_empty(), "echoed before it was sent");
        let mut echoed = String::new();
        for _ in 0..40 {
            tx.generate(2400, &mut wave);
            echoed.extend(tx.take_echoes());
        }
        assert_eq!(echoed, "K6AC");
    }

    /// A second message pushed after finish (a chained F-key) still goes
    /// out in the same transmission, back to back.
    #[test]
    fn text_pushed_after_finish_is_chained() {
        let mut tx = LiveTx::with_lead(SR, 2125.0, 2295.0, 45.45, 500);
        let mut wave = Vec::new();
        tx.push("TU ");
        tx.finish();
        tx.generate(SR as usize / 2, &mut wave);
        assert!(tx.accepts_text());
        tx.push("K6AC 599 05 ");
        let mut guard = 0;
        while !tx.is_done() && guard < 400 {
            tx.generate(4800, &mut wave);
            guard += 1;
        }
        let got = decode(&wave);
        assert!(got.contains("TU K6AC 599 05"), "decoded {got:?}");
    }

    #[test]
    fn finish_with_nothing_typed_ends() {
        let mut tx = LiveTx::new(SR, 2125.0, 2295.0, 45.45);
        let mut wave = Vec::new();
        tx.generate(SR as usize / 2, &mut wave);
        tx.finish();
        tx.generate(SR as usize, &mut wave);
        assert!(tx.is_done());
        tx.push("IGNORED");
        assert_eq!(tx.sent_text(), "");
    }
}
