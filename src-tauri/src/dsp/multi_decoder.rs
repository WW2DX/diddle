// Multi-decoder: scans the smoothed spectrum periodically for likely RTTY
// pairs and runs an independent RttyDemod for each. As decoded text streams
// from each slot, we scan for plausible callsigns and emit them as
// `signal:spot` events so the frontend can show floating tags on the
// waterfall (and let the operator click to QSY).

use std::collections::VecDeque;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tracing::debug;

use crate::dsp::{RttyConfig, RttyDemod};
use crate::scp::ScpDb;

/// Hard cap on simultaneous decoders. Each one is a few KB of state and a
/// few thousand ops/sec; comfortable on any modern machine.
const MAX_SLOTS: usize = 12;
/// Re-scan the spectrum for pair candidates every N spectrum frames.
/// At ~47 fps that's ~3 seconds — slow enough not to thrash, fast enough
/// to react to new signals coming up on the band.
const SCAN_INTERVAL_FRAMES: u32 = 140;
/// Per-slot rolling text buffer length (in characters).
const TEXT_BUFFER_LEN: usize = 100;
/// Target shift between mark/space when scanning for pairs (Hz).
const TARGET_SHIFT_HZ: f32 = 170.0;
/// How far an observed pair can be from TARGET_SHIFT_HZ and still count.
const SHIFT_TOLERANCE_HZ: f32 = 18.0;
/// Bins on each side considered for local-max peak detection.
const PEAK_WINDOW: usize = 4;
/// Peak must clear the spectrum mean by at least this much dB.
const PEAK_MIN_DB_ABOVE_MEAN: f32 = 8.0;
/// Don't consider tones outside this audio range (filters DC junk and
/// post-Nyquist garbage).
const MIN_TONE_HZ: f32 = 300.0;
const MAX_TONE_HZ: f32 = 5000.0;
/// Minimum spacing between adjacent slots — keeps two slots from locking
/// onto the same physical signal.
const SLOT_MIN_SEPARATION_HZ: f32 = 60.0;
/// Don't re-emit the same call from the same slot more often than this.
const RE_EMIT_INTERVAL: Duration = Duration::from_secs(20);
/// Existing slot is preserved across a re-scan if its mark is within this
/// of a new candidate pair (so we don't reset decoder state on tiny drift).
const SLOT_PRESERVE_TOLERANCE_HZ: f32 = 25.0;

struct Slot {
    mark_hz: f32,
    space_hz: f32,
    demod: RttyDemod,
    text: VecDeque<char>,
    last_emit_call: Option<String>,
    last_emit_at: Instant,
}

pub struct MultiDecoder {
    sample_rate: u32,
    fft_size: usize,
    slots: Vec<Slot>,
    smoothed: Option<Vec<f32>>,
    scan_counter: u32,
    app: AppHandle,
    /// Only callsigns present in this database are emitted as spots — keeps
    /// garbage partial decodes off the bandmap.
    scp: Arc<ScpDb>,
}

#[derive(Debug, Clone, Serialize)]
struct SignalSpot {
    audio_hz: f32,
    mark_hz: f32,
    space_hz: f32,
    call: String,
    timestamp_ms: i64,
}

impl MultiDecoder {
    pub fn new(sample_rate: u32, app: AppHandle, scp: Arc<ScpDb>) -> Self {
        Self {
            sample_rate,
            fft_size: 0,
            slots: Vec::new(),
            smoothed: None,
            scan_counter: 0,
            app,
            scp,
        }
    }

    /// Feed the same audio samples that go to the primary decoder. Each
    /// slot's demod pushes them through and the rolling text buffer is
    /// scanned for callsigns.
    pub fn push_audio(&mut self, samples: &[f32]) {
        let now = Instant::now();
        for slot in &mut self.slots {
            let chars = slot.demod.push(samples);
            for c in chars.chars() {
                slot.text.push_back(c);
                if slot.text.len() > TEXT_BUFFER_LEN {
                    slot.text.pop_front();
                }
            }
            if let Some(call) = find_callsign(&slot.text) {
                // Only surface callsigns that exist in the SCP database —
                // this filters out the steady stream of garbage partial
                // decodes (JD2, ISFXG9, ...) from noise.
                if !self.scp.contains(&call) {
                    continue;
                }
                let should_emit = match &slot.last_emit_call {
                    Some(prev) if *prev == call => {
                        now.duration_since(slot.last_emit_at) > RE_EMIT_INTERVAL
                    }
                    _ => true,
                };
                if should_emit {
                    slot.last_emit_call = Some(call.clone());
                    slot.last_emit_at = now;
                    let timestamp_ms = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.as_millis() as i64)
                        .unwrap_or(0);
                    let spot = SignalSpot {
                        audio_hz: slot.mark_hz,
                        mark_hz: slot.mark_hz,
                        space_hz: slot.space_hz,
                        call,
                        timestamp_ms,
                    };
                    let _ = self.app.emit("signal:spot", &spot);
                }
            }
        }
    }

    /// Feed each new spectrum frame so we can keep a smoothed average for
    /// peak detection. Triggers a periodic re-scan that updates the active
    /// slot set.
    pub fn push_spectrum(&mut self, mags_db: &[f32], fft_size: usize) {
        match &mut self.smoothed {
            Some(s) if s.len() == mags_db.len() => {
                let a = 0.12;
                for (i, &m) in mags_db.iter().enumerate() {
                    s[i] = (1.0 - a) * s[i] + a * m;
                }
            }
            _ => {
                self.smoothed = Some(mags_db.to_vec());
                self.fft_size = fft_size;
                return;
            }
        }
        self.scan_counter += 1;
        if self.scan_counter >= SCAN_INTERVAL_FRAMES {
            self.scan_counter = 0;
            self.scan_and_assign();
        }
    }

    fn scan_and_assign(&mut self) {
        let smoothed = match &self.smoothed {
            Some(s) => s,
            None => return,
        };
        let bin_hz = self.sample_rate as f32 / self.fft_size as f32;
        if bin_hz <= 0.0 {
            return;
        }

        // 1. Pick spectral peaks.
        let mean: f32 = smoothed.iter().copied().sum::<f32>() / smoothed.len() as f32;
        let threshold = mean + PEAK_MIN_DB_ABOVE_MEAN;
        let min_bin = (MIN_TONE_HZ / bin_hz).ceil() as usize;
        let max_bin = ((MAX_TONE_HZ / bin_hz).min(smoothed.len() as f32 - 1.0)) as usize;
        let mut peaks: Vec<(usize, f32)> = Vec::new();
        for i in min_bin.max(PEAK_WINDOW)..max_bin.min(smoothed.len() - PEAK_WINDOW) {
            let v = smoothed[i];
            if v < threshold {
                continue;
            }
            let mut is_peak = true;
            for j in (i - PEAK_WINDOW)..=(i + PEAK_WINDOW) {
                if j != i && smoothed[j] >= v {
                    is_peak = false;
                    break;
                }
            }
            if is_peak {
                peaks.push((i, v));
            }
        }
        peaks.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        peaks.truncate(MAX_SLOTS * 4);

        // 2. Build candidate pairs (mark_hz, space_hz, score).
        let target_shift_bins = TARGET_SHIFT_HZ / bin_hz;
        let tol_bins = SHIFT_TOLERANCE_HZ / bin_hz;
        let mut pairs: Vec<(f32, f32, f32)> = Vec::new();
        for i in 0..peaks.len() {
            for j in 0..peaks.len() {
                if i == j {
                    continue;
                }
                let lo = peaks[i].0;
                let hi = peaks[j].0;
                if hi <= lo {
                    continue;
                }
                let shift_bins = (hi - lo) as f32;
                if (shift_bins - target_shift_bins).abs() > tol_bins {
                    continue;
                }
                let mag_lo = peaks[i].1;
                let mag_hi = peaks[j].1;
                let mag_diff = (mag_lo - mag_hi).abs();
                let mag_bonus = (10.0 - mag_diff).max(0.0);
                let score = mag_lo + mag_hi + mag_bonus
                    - (shift_bins - target_shift_bins).abs();
                pairs.push((lo as f32 * bin_hz, hi as f32 * bin_hz, score));
            }
        }
        pairs.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());

        // 3. Deduplicate by mark proximity.
        let mut chosen: Vec<(f32, f32)> = Vec::new();
        for (m, s, _) in &pairs {
            if chosen.len() >= MAX_SLOTS {
                break;
            }
            let too_close = chosen
                .iter()
                .any(|(cm, _)| (m - cm).abs() < SLOT_MIN_SEPARATION_HZ);
            if !too_close {
                chosen.push((*m, *s));
            }
        }

        if chosen.is_empty() && self.slots.is_empty() {
            return;
        }

        // 4. Preserve existing slots that still match a chosen pair.
        let mut new_slots: Vec<Slot> = Vec::new();
        let mut used = vec![false; chosen.len()];
        let existing: Vec<Slot> = self.slots.drain(..).collect();
        for slot in existing {
            let mut found = None;
            for (idx, (m, _)) in chosen.iter().enumerate() {
                if used[idx] {
                    continue;
                }
                if (slot.mark_hz - m).abs() < SLOT_PRESERVE_TOLERANCE_HZ {
                    found = Some(idx);
                    break;
                }
            }
            if let Some(idx) = found {
                used[idx] = true;
                new_slots.push(slot);
            }
        }

        // 5. Fill remaining slots with fresh decoders for unmatched pairs.
        for (idx, (m, s)) in chosen.iter().enumerate() {
            if used[idx] {
                continue;
            }
            if new_slots.len() >= MAX_SLOTS {
                break;
            }
            let cfg = RttyConfig {
                mark_hz: *m,
                space_hz: *s,
                baud: 45.45,
            };
            let demod = RttyDemod::new(self.sample_rate, cfg);
            new_slots.push(Slot {
                mark_hz: *m,
                space_hz: *s,
                demod,
                text: VecDeque::new(),
                last_emit_call: None,
                last_emit_at: Instant::now() - RE_EMIT_INTERVAL,
            });
        }

        if !new_slots.is_empty() {
            debug!(
                slots = new_slots.len(),
                "multi-decoder slots: {:?}",
                new_slots
                    .iter()
                    .map(|s| (s.mark_hz as u32, s.space_hz as u32))
                    .collect::<Vec<_>>()
            );
        }
        self.slots = new_slots;
        // Tell the frontend how many slots are active (info badge).
        let _ = self.app.emit("multi:slots", self.slots.len());
    }
}

/// Walks the text buffer from newest backward, returns the first
/// callsign-shaped token found. "Callsign-shaped" is intentionally loose:
/// 3-8 alphanumeric uppercase chars containing at least one letter and
/// at least one digit. Real callsigns like R120RB and SX150ITU fit; common
/// noise tokens like "59" or "ZDYB" do not.
fn find_callsign(text: &VecDeque<char>) -> Option<String> {
    let s: String = text.iter().collect();
    // Tokenize on whitespace.
    for token in s.split(|c: char| !c.is_ascii_alphanumeric()).rev() {
        if looks_like_callsign(token) {
            return Some(token.to_string());
        }
    }
    None
}

fn looks_like_callsign(s: &str) -> bool {
    let bytes = s.as_bytes();
    let len = bytes.len();
    if !(3..=8).contains(&len) {
        return false;
    }
    let mut has_letter = false;
    let mut has_digit = false;
    for &b in bytes {
        if b.is_ascii_uppercase() {
            has_letter = true;
        } else if b.is_ascii_digit() {
            has_digit = true;
        } else {
            return false;
        }
    }
    has_letter && has_digit
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn callsign_shapes() {
        assert!(looks_like_callsign("W1AW"));
        assert!(looks_like_callsign("K5ZD"));
        assert!(looks_like_callsign("R120RB"));
        assert!(looks_like_callsign("SX150ITU"));
        assert!(looks_like_callsign("AA1AAA"));

        assert!(!looks_like_callsign("CQ")); // too short
        assert!(!looks_like_callsign("HELLO")); // no digit
        assert!(!looks_like_callsign("12345")); // no letter
        assert!(!looks_like_callsign("ABCDEFGHI")); // too long
        assert!(!looks_like_callsign("W1@W")); // invalid char
    }

    #[test]
    fn finds_callsign_in_buffer() {
        let buf: VecDeque<char> = "CQ TEST DE W1AW W1AW K".chars().collect();
        assert_eq!(find_callsign(&buf).as_deref(), Some("W1AW"));
    }

    #[test]
    fn finds_special_event_call() {
        let buf: VecDeque<char> = "CQ DE R120RB R120RB".chars().collect();
        assert_eq!(find_callsign(&buf).as_deref(), Some("R120RB"));
    }
}
