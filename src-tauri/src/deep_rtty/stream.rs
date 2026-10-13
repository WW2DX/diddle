// From the model's per-frame character probabilities to text: greedy CTC
// tokens with a time and confidence, the merge of overlapping windows so each
// character comes out exactly once, and ITA2 (Baudot) with its LTRS/FIGS
// shift. Ports of the reference streaming receiver (e04/DeepRTTY,
// examples/python/rtty_stream.py).

pub const NUM_CLASSES: usize = 33;
const BLANK: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Token {
    /// Absolute stream position of the CTC spike, in model samples.
    pub sample: i64,
    /// Raw ITA2 code.
    pub code: u8,
    /// Peak posterior probability.
    pub score: f32,
}

/// Greedy CTC over one window's [time × class] log-probabilities: one token
/// per run of a non-blank class, placed at the run's centre.
pub fn ctc_tokens(log_probs: &[f32], window_start: i64, output_hop: usize) -> Vec<Token> {
    let frames = log_probs.len() / NUM_CLASSES;
    let mut out = Vec::new();
    let mut f = 0;
    while f < frames {
        let (code, mut peak) = argmax(&log_probs[f * NUM_CLASSES..(f + 1) * NUM_CLASSES]);
        let start = f;
        f += 1;
        while f < frames {
            let (c, p) = argmax(&log_probs[f * NUM_CLASSES..(f + 1) * NUM_CLASSES]);
            if c != code {
                break;
            }
            peak = peak.max(p);
            f += 1;
        }
        if code != BLANK {
            let centre = (start + f - 1) as f64 * 0.5;
            out.push(Token {
                sample: window_start + (centre * output_hop as f64).round() as i64,
                code: code as u8,
                score: peak.exp(),
            });
        }
    }
    out
}

fn argmax(row: &[f32]) -> (usize, f32) {
    let mut best = (0, f32::NEG_INFINITY);
    for (i, &v) in row.iter().enumerate() {
        if v > best.1 {
            best = (i, v);
        }
    }
    best
}

/// Time-constrained alignment of two short token sequences: pairs only
/// within `tolerance` samples; equal codes cost less than substitutions,
/// and both less than leaving two tokens unpaired.
fn align(a: &[Token], b: &[Token], tolerance: f64) -> Vec<(Option<Token>, Option<Token>)> {
    let (n, m) = (a.len(), b.len());
    let gap = 1.0;
    let mut cost = vec![vec![f64::INFINITY; m + 1]; n + 1];
    let mut mv = vec![vec![0u8; m + 1]; n + 1]; // 0 pair, 1 a-only, 2 b-only
    for i in 0..=n {
        cost[i][0] = i as f64 * gap;
        if i > 0 {
            mv[i][0] = 1;
        }
    }
    for j in 0..=m {
        cost[0][j] = j as f64 * gap;
        if j > 0 {
            mv[0][j] = 2;
        }
    }
    for i in 1..=n {
        for j in 1..=m {
            let (mut best, mut step) = (cost[i - 1][j] + gap, 1u8);
            if cost[i][j - 1] + gap < best {
                best = cost[i][j - 1] + gap;
                step = 2;
            }
            let dt = (a[i - 1].sample - b[j - 1].sample).abs() as f64;
            if dt <= tolerance {
                let pair = cost[i - 1][j - 1]
                    + 0.5 * dt / tolerance
                    + if a[i - 1].code == b[j - 1].code { 0.0 } else { 0.9 };
                if pair <= best {
                    best = pair;
                    step = 0;
                }
            }
            cost[i][j] = best;
            mv[i][j] = step;
        }
    }
    let mut pairs = Vec::new();
    let (mut i, mut j) = (n, m);
    while i > 0 || j > 0 {
        match mv[i][j] {
            0 => {
                pairs.push((Some(a[i - 1]), Some(b[j - 1])));
                i -= 1;
                j -= 1;
            }
            1 => {
                pairs.push((Some(a[i - 1]), None));
                i -= 1;
            }
            _ => {
                pairs.push((None, Some(b[j - 1])));
                j -= 1;
            }
        }
    }
    pairs.reverse();
    pairs
}

/// Merges the token streams of overlapping windows (pushed in order) into
/// one committed stream. Only the reliable part of each window — away from
/// its interior edges — competes; inside the overlap of two windows paired
/// tokens go to the more confident window, unpaired ones to the window that
/// owns that half of the overlap. Returned tokens are final.
pub struct OverlapMerger {
    edge_guard: i64,
    tolerance: f64,
    pending: Vec<Token>,
    pending_reliable_end: f64,
    cutoff: f64,
    windows: u64,
}

impl OverlapMerger {
    pub fn new(edge_guard: i64, tolerance: f64) -> Self {
        Self {
            edge_guard,
            tolerance,
            pending: Vec::new(),
            pending_reliable_end: f64::NEG_INFINITY,
            cutoff: f64::NEG_INFINITY,
            windows: 0,
        }
    }

    pub fn push(&mut self, window_start: i64, window_length: i64, tokens: Vec<Token>) -> Vec<Token> {
        let reliable_start = window_start + if self.windows > 0 { self.edge_guard } else { 0 };
        let tokens: Vec<Token> = tokens.into_iter().filter(|t| t.sample >= reliable_start).collect();
        self.windows += 1;
        let reliable_end = (window_start + window_length - self.edge_guard) as f64;
        if self.windows == 1 {
            self.pending = tokens;
            self.pending_reliable_end = reliable_end;
            return Vec::new();
        }
        let mut zone_start = (reliable_start as f64).max(self.cutoff);
        let mut zone_end = self.pending_reliable_end;
        if zone_end < zone_start {
            // No reliable overlap: trust each window up to the middle of the gap.
            let mid = 0.5 * (zone_start + zone_end);
            zone_start = mid;
            zone_end = mid;
        }
        let middle = 0.5 * (zone_start + zone_end);
        let inside = |t: &Token| zone_start <= t.sample as f64 && t.sample as f64 <= zone_end;

        let mut committed: Vec<Token> =
            self.pending.iter().filter(|t| (t.sample as f64) < zone_start).copied().collect();
        let previous: Vec<Token> = self.pending.iter().filter(|t| inside(t)).copied().collect();
        let current: Vec<Token> = tokens.iter().filter(|t| inside(t)).copied().collect();
        let mut merged = Vec::new();
        for (old, new) in align(&previous, &current, self.tolerance) {
            match (old, new) {
                (Some(o), Some(n)) => merged.push(if o.score >= n.score { o } else { n }),
                (Some(o), None) if (o.sample as f64) < middle => merged.push(o),
                (None, Some(n)) if (n.sample as f64) >= middle => merged.push(n),
                _ => {}
            }
        }
        merged.sort_by_key(|t| t.sample);
        committed.extend(merged);

        self.cutoff = zone_end;
        self.pending = tokens.into_iter().filter(|t| t.sample as f64 > zone_end).collect();
        self.pending_reliable_end = reliable_end;
        committed
    }
}

/// ITA2 with the LTRS/FIGS shift carried between calls, and unshift-on-
/// space (USOS) as most RTTY stations send it.
#[derive(Default)]
pub struct Ita2 {
    figures: bool,
}

const LETTERS: [char; 32] = [
    '\0', 'E', '\n', 'A', ' ', 'S', 'I', 'U', '\r', 'D', 'R', 'J', 'N', 'F', 'C', 'K', 'T', 'Z',
    'L', 'W', 'H', 'Y', 'P', 'Q', 'O', 'B', 'G', '\0', 'M', 'X', 'V', '\0',
];
// FIGS + S is BEL (not printed); FIGS + J is the apostrophe (US).
const FIGURES: [char; 32] = [
    '\0', '3', '\n', '-', ' ', '\0', '8', '7', '\r', '$', '4', '\'', ',', '!', ':', '(', '5', '"',
    ')', '2', '#', '6', '0', '1', '9', '?', '&', '\0', '.', '/', ';', '\0',
];

impl Ita2 {
    pub fn decode(&mut self, codes: impl IntoIterator<Item = u8>) -> String {
        let mut out = String::new();
        for code in codes {
            match code {
                31 => self.figures = false,
                27 => self.figures = true,
                0 => {}
                c if (c as usize) < 32 => {
                    let ch = if self.figures { FIGURES[c as usize] } else { LETTERS[c as usize] };
                    // CR and LF both end a line (the RX window flattens them).
                    out.push(if ch == '\r' { '\n' } else { ch });
                    if c == 4 {
                        self.figures = false;
                    }
                }
                _ => {}
            }
        }
        // Collapse the CR+LF pair a station sends into one line break.
        out.replace("\n\n", "\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tok(sample: i64, code: u8, score: f32) -> Token {
        Token { sample, code, score }
    }

    #[test]
    fn overlap_keeps_each_character_once() {
        let mut m = OverlapMerger::new(100, 50.0);
        // Window 1 covers 0..1000, window 2 300..1300; both see the
        // characters at 500 and 700, slightly displaced.
        assert!(m.push(0, 1000, vec![tok(200, 1, 0.9), tok(500, 3, 0.8), tok(700, 5, 0.6)]).is_empty());
        let out = m.push(300, 1000, vec![tok(505, 3, 0.7), tok(698, 5, 0.95), tok(1000, 7, 0.9)]);
        let codes: Vec<u8> = out.iter().map(|t| t.code).collect();
        assert_eq!(codes, vec![1, 3, 5]);
        assert_eq!(out[2].score, 0.95); // the more confident copy
    }

    #[test]
    fn ita2_shift_and_usos() {
        let mut d = Ita2::default();
        // C Q space FIGS 5 9 9 space(→ letters) T U
        assert_eq!(d.decode([14, 23, 4, 27, 16, 24, 24, 4, 16, 7]), "CQ 599 TU");
        // Shift survives across calls.
        assert_eq!(d.decode([27]), "");
        assert_eq!(d.decode([22]), "0");
    }

    #[test]
    fn ctc_runs_become_tokens() {
        // 4 frames: blank, A, A, blank → one A at frame 1.5.
        let mut lp = vec![-10f32; 4 * NUM_CLASSES];
        lp[BLANK] = 0.0;
        lp[NUM_CLASSES + 3] = -0.1;
        lp[2 * NUM_CLASSES + 3] = -0.2;
        lp[3 * NUM_CLASSES + BLANK] = 0.0;
        let t = ctc_tokens(&lp, 1000, 48);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].code, 3);
        assert_eq!(t[0].sample, 1000 + 72);
    }
}
