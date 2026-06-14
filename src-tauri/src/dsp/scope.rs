// Crossed-ellipses ("crossed bananas") RTTY tuning scope. Two narrow
// bandpass filters tuned to the mark and space frequencies; their
// time-domain outputs drive the X and Y axes of an oscilloscope-style
// display. A mark tone rings the X filter (horizontal ellipse), a space
// tone rings the Y filter (vertical ellipse). Properly tuned, the two
// ellipses are perpendicular and centered.

use serde::Serialize;

use crate::dsp::Biquad;

/// Bandwidth of each tuning-scope bandpass filter (Hz). Moderately narrow
/// so the ellipses are crisp but still show some width.
const SCOPE_BW_HZ: f32 = 100.0;
/// Keep every Nth sample — plenty of resolution to trace the ellipse while
/// reducing the data volume sent to the UI.
const DECIMATE: usize = 2;
/// Points per emitted frame.
const BATCH: usize = 512;

#[derive(Debug, Clone, Serialize)]
pub struct ScopeFrame {
    pub xs: Vec<f32>,
    pub ys: Vec<f32>,
}

pub struct TuningScope {
    mark_bp: Biquad,
    space_bp: Biquad,
    counter: usize,
    xs: Vec<f32>,
    ys: Vec<f32>,
}

impl TuningScope {
    pub fn new(sample_rate: u32, mark_hz: f32, space_hz: f32) -> Self {
        let sr = sample_rate as f32;
        let qm = (mark_hz / SCOPE_BW_HZ).max(1.0);
        let qs = (space_hz / SCOPE_BW_HZ).max(1.0);
        Self {
            mark_bp: Biquad::bandpass(sr, mark_hz, qm),
            space_bp: Biquad::bandpass(sr, space_hz, qs),
            counter: 0,
            xs: Vec::with_capacity(BATCH),
            ys: Vec::with_capacity(BATCH),
        }
    }

    /// Feed mono audio; returns any completed XY frames.
    pub fn push(&mut self, samples: &[f32]) -> Vec<ScopeFrame> {
        let mut frames = Vec::new();
        for &s in samples {
            let x = self.mark_bp.process(s);
            let y = self.space_bp.process(s);
            self.counter += 1;
            if self.counter >= DECIMATE {
                self.counter = 0;
                self.xs.push(x);
                self.ys.push(y);
                if self.xs.len() >= BATCH {
                    frames.push(ScopeFrame {
                        xs: std::mem::take(&mut self.xs),
                        ys: std::mem::take(&mut self.ys),
                    });
                    self.xs = Vec::with_capacity(BATCH);
                    self.ys = Vec::with_capacity(BATCH);
                }
            }
        }
        frames
    }
}
