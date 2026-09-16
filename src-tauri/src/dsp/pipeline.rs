// RX pipeline: the bundle of DSP consumers that every audio source feeds —
// spectrum (waterfall), multi-decoder (spots), primary RTTY demod (RX
// window) and the tuning scope. The TCI client has its own inline copy of
// this wiring (it predates the bundle and interleaves with TX handling);
// the WAV player, audio-device input and the contest simulator share this
// one so they all behave identically.

use std::sync::Arc;

use tauri::{AppHandle, Emitter};

use crate::dsp::{MultiDecoder, RttyDemod, RttyTunable, Spectrum, TuningScope};
use crate::scp::ScpDb;

// FFT size of 4096 at 48 kHz gives 11.72 Hz/bin; 75% overlap keeps the
// waterfall scrolling smoothly. Same numbers the TCI path uses.
const FFT_SIZE: usize = 4096;
const FFT_STRIDE: usize = 1024;

pub struct RxPipeline {
    sample_rate: u32,
    app: AppHandle,
    rtty: Arc<RttyTunable>,
    rtty_gen: u64,
    spectrum: Spectrum,
    multi: MultiDecoder,
    demod: RttyDemod,
    scope: TuningScope,
}

impl RxPipeline {
    pub async fn new(
        sample_rate: u32,
        app: AppHandle,
        rtty: Arc<RttyTunable>,
        scp: Arc<ScpDb>,
    ) -> Self {
        let cfg = rtty.get().await;
        let rtty_gen = rtty.current_gen();
        Self {
            sample_rate,
            spectrum: Spectrum::new(FFT_SIZE, FFT_STRIDE, sample_rate),
            multi: MultiDecoder::new(sample_rate, app.clone(), scp),
            demod: RttyDemod::new(sample_rate, cfg.clone()),
            scope: TuningScope::new(sample_rate, cfg.mark_hz, cfg.space_hz),
            app,
            rtty,
            rtty_gen,
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Feed a block of mono samples in [-1, 1]. The waterfall always runs;
    /// with `muted` set the decoders and scope are skipped (used while our
    /// own transmission would otherwise be decoded as garbage).
    pub async fn push(&mut self, mono: &[f32], muted: bool) {
        if mono.is_empty() {
            return;
        }
        // Hot-retune when the operator clicks a new mark/space pair.
        let cur_gen = self.rtty.current_gen();
        if cur_gen != self.rtty_gen {
            let cfg = self.rtty.get().await;
            self.demod = RttyDemod::new(self.sample_rate, cfg.clone());
            self.scope = TuningScope::new(self.sample_rate, cfg.mark_hz, cfg.space_hz);
            self.rtty_gen = cur_gen;
        }

        if !muted {
            self.multi.push_audio(mono);
        }
        for frame in self.spectrum.push(mono) {
            if !muted {
                self.multi.push_spectrum(&frame.mags_db, frame.fft_size);
            }
            let _ = self.app.emit("spectrum", &frame);
        }
        if muted {
            return;
        }
        let chars = self.demod.push(mono);
        if !chars.is_empty() {
            let _ = self.app.emit("rtty", &chars);
        }
        for f in self.scope.push(mono) {
            let _ = self.app.emit("scope", &f);
        }
    }
}
