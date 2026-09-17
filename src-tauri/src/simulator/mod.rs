// Built-in RTTY contest simulator. Synthesizes a band — stations answering
// your CQ, sending exchanges, repeating on request, plus optional
// background stations elsewhere in the passband and HF noise — and feeds
// it through the normal RX pipeline, so the waterfall, decoders, bandmap,
// entry window and logging all see it exactly as live TCI audio.
//
// Two modes:
//   * Pileup (interactive): stations react to what Diddle transmits. CQ
//     brings callers; naming one gets its exchange; TU/QRZ completes the
//     QSO and brings the next. Nothing is keyed on a radio — `transmit`
//     is routed here while the simulator runs.
//   * Playback (scripted): both sides of a run are played back to back,
//     the way EC5W's RTTY Runner does, for pure decoder calibration.
//
// The station/exchange tables and the noise model are ported from RTTY
// Runner (https://github.com/opalito/RTTYRunner, MIT).

mod calls;
mod noise;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rand::seq::SliceRandom;
use rand::Rng;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tracing::info;

use crate::dsp::{RttyTunable, RttyTxGenerator, RxPipeline};
use crate::scp::ScpDb;
use crate::tci::RigState;

pub use calls::ExchangeKind;
use calls::{Station, StationGen};
use noise::HfNoise;

/// Everything runs at this rate — same as the TCI TX path.
const SAMPLE_RATE: u32 = 48_000;
/// Mixer block size (~10.7 ms at 48 kHz).
const CHUNK: usize = 512;
/// Mark lead-in / trail around every simulated transmission, in bits.
const LEAD_BITS: f32 = 8.0;
const TRAIL_BITS: f32 = 4.0;
/// Our own TX: mark lead-in / trail (mirrors the TCI transmitter).
const OUR_LEAD_MS: u64 = 300;
const OUR_TRAIL_MS: u64 = 120;
/// How long callers wait for us before calling again / giving up.
const CALLERS_TIMEOUT_MS: u64 = 9_000;
/// How long a worked station waits for our TU before repeating / leaving.
const EXCH_TIMEOUT_MS: u64 = 10_000;
/// Minimum gap between status emits.
const STATUS_MIN_INTERVAL: Duration = Duration::from_millis(100);
/// Lines kept in the truth log shipped with status.
const LOG_KEEP: usize = 60;
/// Keep background stations out of this window around the RX pair.
const BG_GUARD_HZ: f32 = 160.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SimMode {
    Pileup,
    Playback,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimConfig {
    pub my_call: String,
    pub exchange: ExchangeKind,
    pub mode: SimMode,
    /// Average number of stations answering a CQ (1–9).
    pub activity: u8,
    /// HF noise level 0..1.
    pub noise: f32,
    /// Extra stations elsewhere in the passband (0–8).
    pub background: u8,
    /// Max ± tone offset of callers from the RX pair, Hz.
    pub spread_hz: f32,
    /// Overall level of simulated stations, 0..1.
    pub signal: f32,
    /// Pretend rig dial frequency, so QSOs log with a band.
    pub dial_hz: u64,
    /// Draw callsigns from the SCP database (spots then work).
    pub use_scp: bool,
}

impl SimConfig {
    fn sanitized(mut self) -> Self {
        self.my_call = self.my_call.trim().to_ascii_uppercase();
        if self.my_call.is_empty() {
            self.my_call = "N0CALL".into();
        }
        self.activity = self.activity.clamp(1, 9);
        self.noise = self.noise.clamp(0.0, 1.0);
        self.background = self.background.min(8);
        self.spread_hz = self.spread_hz.clamp(0.0, 200.0);
        self.signal = self.signal.clamp(0.05, 1.0);
        if self.dial_hz == 0 {
            self.dial_hz = 14_080_000;
        }
        self
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CallerInfo {
    pub call: String,
    pub exchange: String,
    pub offset_hz: f32,
    pub level: f32,
    /// "calling" | "worked" | "waiting"
    pub status: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct BgInfo {
    pub call: String,
    pub mark_hz: f32,
    pub exchange: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LogLine {
    pub t_ms: i64,
    /// "you" | "dx" | "bg" | "sim"
    pub who: &'static str,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SimStatus {
    pub running: bool,
    pub mode: SimMode,
    /// "idle" | "calling" | "exchanged" | "playback"
    pub phase: &'static str,
    pub ptt: bool,
    pub qso_count: u32,
    pub my_call: String,
    pub exchange: ExchangeKind,
    pub callers: Vec<CallerInfo>,
    pub worked: Option<CallerInfo>,
    pub background: Vec<BgInfo>,
    pub noise: f32,
    pub activity: u8,
    pub spread_hz: f32,
    pub signal: f32,
    pub dial_hz: u64,
    pub log: Vec<LogLine>,
}

impl SimStatus {
    fn idle() -> Self {
        Self {
            running: false,
            mode: SimMode::Pileup,
            phase: "idle",
            ptt: false,
            qso_count: 0,
            my_call: String::new(),
            exchange: ExchangeKind::Serial,
            callers: Vec::new(),
            worked: None,
            background: Vec::new(),
            noise: 0.0,
            activity: 0,
            spread_hz: 0.0,
            signal: 0.0,
            dial_hz: 0,
            log: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------
// World: everything the mixer task and the TX handler share.
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Who {
    Caller(u32),
    Worked(u32),
    Bg(u32),
    Playback,
}

struct Voice {
    samples: Vec<f32>,
    pos: usize,
    amp: f32,
    qsb_phase: f32,
    qsb_inc: f32,
    qsb_depth: f32,
    who: Who,
}

enum Ev {
    Say {
        who: Who,
        text: String,
        offset_hz: f32,
        amp: f32,
    },
    CallersTimeout {
        epoch: u64,
    },
    ExchTimeout {
        epoch: u64,
    },
    BgNext {
        id: u32,
    },
    BgChurn,
    PlaybackStep {
        step: u8,
    },
}

struct Event {
    due: u64,
    ev: Ev,
}

struct Caller {
    id: u32,
    st: Station,
    offset_hz: f32,
    amp: f32,
    /// How many more times this station will call before giving up.
    patience: u8,
}

struct BgStation {
    id: u32,
    st: Station,
    mark_hz: f32,
    amp: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Idle,
    Calling,
    Exchanged,
}

struct World {
    /// None in unit tests (no event sink).
    app: Option<AppHandle>,
    cfg: SimConfig,
    running: bool,
    now: u64,
    mark_hz: f32,
    space_hz: f32,
    baud: f32,
    voices: Vec<Voice>,
    events: Vec<Event>,
    noise: HfNoise,
    gen: StationGen,
    next_id: u32,
    phase: Phase,
    epoch: u64,
    callers: Vec<Caller>,
    worked: Option<Caller>,
    /// Re-call attempts left for the worked station before it walks away.
    worked_patience: u8,
    qso_count: u32,
    background: Vec<BgStation>,
    ptt: bool,
    log: Vec<LogLine>,
    dirty: bool,
    // Playback bookkeeping.
    pb_dx: Option<Station>,
    pb_me: Option<Station>,
    pb_serial: u32,
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

impl World {
    fn new(
        app: Option<AppHandle>,
        cfg: SimConfig,
        mark_hz: f32,
        space_hz: f32,
        baud: f32,
        scp: Arc<ScpDb>,
    ) -> Self {
        Self {
            app,
            running: true,
            now: 0,
            mark_hz,
            space_hz,
            baud,
            voices: Vec::new(),
            events: Vec::new(),
            noise: HfNoise::new(SAMPLE_RATE),
            gen: StationGen::new(scp, cfg.use_scp),
            next_id: 0,
            phase: Phase::Idle,
            epoch: 0,
            callers: Vec::new(),
            worked: None,
            worked_patience: 0,
            qso_count: 0,
            background: Vec::new(),
            ptt: false,
            log: Vec::new(),
            dirty: true,
            pb_dx: None,
            pb_me: None,
            pb_serial: 1,
            cfg,
        }
    }

    fn ms(&self, ms: u64) -> u64 {
        ms * SAMPLE_RATE as u64 / 1000
    }

    fn schedule(&mut self, delay_ms: u64, ev: Ev) {
        let due = self.now + self.ms(delay_ms);
        self.events.push(Event { due, ev });
    }

    fn log(&mut self, who: &'static str, text: impl Into<String>) {
        let line = LogLine {
            t_ms: now_ms(),
            who,
            text: text.into(),
        };
        if let Some(app) = &self.app {
            let _ = app.emit("sim:log", &line);
        }
        self.log.push(line);
        if self.log.len() > LOG_KEEP {
            let drop = self.log.len() - LOG_KEEP;
            self.log.drain(..drop);
        }
        self.dirty = true;
    }

    fn new_id(&mut self) -> u32 {
        self.next_id += 1;
        self.next_id
    }

    // ---- synthesis -------------------------------------------------

    fn waveform(&self, text: &str, offset_hz: f32) -> Vec<f32> {
        let mut gen = RttyTxGenerator::new(
            SAMPLE_RATE,
            self.mark_hz + offset_hz,
            self.space_hz + offset_hz,
            self.baud,
        );
        let spb = gen.samples_per_bit();
        let mut out = Vec::new();
        gen.next_samples((LEAD_BITS * spb) as usize, &mut out);
        gen.enqueue(text);
        let cap = SAMPLE_RATE as usize * 60;
        while !gen.is_idle() && out.len() < cap {
            gen.next_samples(1, &mut out);
        }
        gen.next_samples((TRAIL_BITS * spb) as usize, &mut out);
        out
    }

    fn say(&mut self, who: Who, text: String, offset_hz: f32, amp: f32) {
        let samples = self.waveform(&text, offset_hz);
        let mut rng = rand::thread_rng();
        let qsb_rate = rng.gen_range(0.04..0.3);
        let (depth, log_who) = match who {
            Who::Bg(_) => (0.5, "bg"),
            Who::Playback => (0.15, "dx"),
            _ => (0.35, "dx"),
        };
        self.voices.push(Voice {
            samples,
            pos: 0,
            amp: amp * self.cfg.signal,
            qsb_phase: rng.gen_range(0.0..std::f32::consts::TAU),
            qsb_inc: std::f32::consts::TAU * qsb_rate / SAMPLE_RATE as f32,
            qsb_depth: depth,
            who,
        });
        let label = match who {
            Who::Bg(id) => self
                .background
                .iter()
                .find(|b| b.id == id)
                .map(|b| format!("{} @ {:.0} Hz", b.st.call, b.mark_hz))
                .unwrap_or_default(),
            Who::Caller(id) => self
                .callers
                .iter()
                .find(|c| c.id == id)
                .map(|c| format!("{} ({:+.0} Hz)", c.st.call, c.offset_hz))
                .unwrap_or_default(),
            Who::Worked(_) => self
                .worked
                .as_ref()
                .map(|c| format!("{} ({:+.0} Hz)", c.st.call, c.offset_hz))
                .unwrap_or_default(),
            Who::Playback => "playback".into(),
        };
        self.log(log_who, format!("{label}: {}", text.trim()));
    }

    /// Advance the clock by `n` samples: fire due events, mix voices, add
    /// noise. Returns the block plus whether the RX decoders should be
    /// muted (our PTT is down).
    fn tick(&mut self, n: usize) -> (Vec<f32>, bool) {
        self.now += n as u64;

        // Due events (a fired event may schedule more; loop until quiet).
        loop {
            let idx = self.events.iter().position(|e| e.due <= self.now);
            let Some(i) = idx else { break };
            let ev = self.events.swap_remove(i).ev;
            self.handle_event(ev);
        }

        let mut buf = vec![0f32; n];
        let mut finished: Vec<Who> = Vec::new();
        for v in self.voices.iter_mut() {
            for s in buf.iter_mut() {
                if v.pos >= v.samples.len() {
                    break;
                }
                let fade = 1.0 - v.qsb_depth * 0.5 * (1.0 + v.qsb_phase.sin());
                *s += v.samples[v.pos] * v.amp * fade;
                v.pos += 1;
                v.qsb_phase += v.qsb_inc;
            }
            if v.pos >= v.samples.len() {
                finished.push(v.who);
            }
        }
        self.voices.retain(|v| v.pos < v.samples.len());
        for who in finished {
            self.voice_done(who);
        }

        let level = self.cfg.noise;
        self.noise.add(&mut buf, level);
        for s in buf.iter_mut() {
            *s = s.clamp(-1.0, 1.0);
        }
        (buf, self.ptt)
    }

    fn handle_event(&mut self, ev: Ev) {
        match ev {
            Ev::Say {
                who,
                text,
                offset_hz,
                amp,
            } => {
                // Skip if the station has meanwhile left the pileup.
                let alive = match who {
                    Who::Caller(id) => self.callers.iter().any(|c| c.id == id),
                    Who::Worked(id) => self.worked.as_ref().map(|w| w.id == id).unwrap_or(false),
                    Who::Bg(id) => self.background.iter().any(|b| b.id == id),
                    Who::Playback => true,
                };
                if alive {
                    self.say(who, text, offset_hz, amp);
                }
            }
            Ev::CallersTimeout { epoch } => {
                if self.phase == Phase::Calling && epoch == self.epoch {
                    self.callers_recall("no answer");
                }
            }
            Ev::ExchTimeout { epoch } => {
                if self.phase == Phase::Exchanged && epoch == self.epoch {
                    if self.worked_patience > 0 {
                        self.worked_patience -= 1;
                        self.worked_repeat();
                        self.arm_exch_timeout();
                    } else {
                        let call = self.worked.as_ref().map(|w| w.st.call.clone()).unwrap_or_default();
                        self.log("sim", format!("{call} gave up waiting for your TU"));
                        self.worked = None;
                        self.to_idle_or_calling();
                    }
                }
            }
            Ev::BgNext { id } => self.bg_next(id),
            Ev::BgChurn => self.bg_churn(),
            Ev::PlaybackStep { step } => self.playback_step(step),
        }
    }

    fn voice_done(&mut self, who: Who) {
        match who {
            Who::Bg(id) => {
                let mut rng = rand::thread_rng();
                let pause = rng.gen_range(2500..8000);
                self.schedule(pause, Ev::BgNext { id });
            }
            _ => {}
        }
    }

    // ---- pileup state machine -------------------------------------

    fn bump_epoch(&mut self) -> u64 {
        self.epoch += 1;
        self.epoch
    }

    fn arm_callers_timeout(&mut self) {
        let epoch = self.bump_epoch();
        self.schedule(CALLERS_TIMEOUT_MS, Ev::CallersTimeout { epoch });
    }

    fn arm_exch_timeout(&mut self) {
        let epoch = self.bump_epoch();
        self.schedule(EXCH_TIMEOUT_MS, Ev::ExchTimeout { epoch });
    }

    fn to_idle_or_calling(&mut self) {
        if self.callers.is_empty() {
            self.phase = Phase::Idle;
            self.bump_epoch();
        } else {
            self.phase = Phase::Calling;
            self.callers_recall("waiting");
        }
        self.dirty = true;
    }

    fn spawn_callers(&mut self) {
        let mut rng = rand::thread_rng();
        let a = self.cfg.activity as i32;
        let lo = (a - 2).max(0);
        let hi = a + 1;
        let mut n = rng.gen_range(lo..=hi) as usize;
        if n == 0 && rng.gen_bool(0.7) {
            n = 1;
        }
        if rng.gen_bool(0.12) {
            n = 0; // sometimes nobody comes back
        }
        let n = n.min(9);
        let existing: Vec<String> = self
            .callers
            .iter()
            .map(|c| c.st.call.clone())
            .chain(self.background.iter().map(|b| b.st.call.clone()))
            .collect();
        for _ in 0..n {
            let mut st = self.gen.station(self.cfg.exchange);
            let mut tries = 0;
            while (existing.contains(&st.call) || st.call == self.cfg.my_call) && tries < 10 {
                st = self.gen.station(self.cfg.exchange);
                tries += 1;
            }
            let spread = self.cfg.spread_hz;
            let offset = if spread > 0.0 {
                rng.gen_range(-spread..=spread)
            } else {
                0.0
            };
            let amp = rng.gen_range(0.3..1.0);
            let id = self.new_id();
            self.callers.push(Caller {
                id,
                st,
                offset_hz: offset,
                amp,
                patience: 2,
            });
        }
        if self.callers.is_empty() {
            self.log("sim", "nobody answered — call CQ again");
            self.phase = Phase::Idle;
            self.bump_epoch();
            self.dirty = true;
            return;
        }
        self.phase = Phase::Calling;
        self.callers_call_now();
        self.arm_callers_timeout();
    }

    /// Every caller in the pileup transmits its call. They don't all start
    /// at once: each next caller begins somewhere between halfway through
    /// and shortly after the previous one — tail-end collisions like a
    /// real pileup, not one unreadable blob.
    fn callers_call_now(&mut self) {
        let mut rng = rand::thread_rng();
        let my = self.cfg.my_call.clone();
        let baud = self.baud;
        let mut sched: Vec<(u64, Ev)> = Vec::new();
        let mut order: Vec<usize> = (0..self.callers.len()).collect();
        order.shuffle(&mut rng);
        let mut t: u64 = rng.gen_range(350..900);
        for i in order {
            let c = &self.callers[i];
            let text = answer_template(&mut rng, &my, &c.st.call);
            let delay = t;
            // ~8 bits per character (start + 5 data + 1.5 stop, rounded up).
            let dur_ms = ((text.len() as f32 + LEAD_BITS / 8.0 + TRAIL_BITS / 8.0) * 8.0 / baud * 1000.0) as u64;
            t += (dur_ms as f32 * rng.gen_range(0.5..1.2)) as u64;
            sched.push((
                delay,
                Ev::Say {
                    who: Who::Caller(c.id),
                    text,
                    offset_hz: c.offset_hz,
                    amp: c.amp,
                },
            ));
        }
        for (d, ev) in sched {
            self.schedule(d, ev);
        }
        self.dirty = true;
    }

    /// Callers try again (each burning one unit of patience); the ones
    /// out of patience leave.
    fn callers_recall(&mut self, why: &str) {
        let mut rng = rand::thread_rng();
        let before = self.callers.len();
        self.callers.retain_mut(|c| {
            if c.patience == 0 {
                return false;
            }
            c.patience -= 1;
            rng.gen_bool(0.8)
        });
        let left = before - self.callers.len();
        if left > 0 {
            self.log("sim", format!("{left} caller(s) gave up ({why})"));
        }
        if self.callers.is_empty() {
            self.phase = Phase::Idle;
            self.bump_epoch();
            self.dirty = true;
            return;
        }
        self.phase = Phase::Calling;
        self.callers_call_now();
        self.arm_callers_timeout();
    }

    fn worked_reply(&mut self, fuzzy: bool) {
        let Some(w) = self.worked.as_ref() else { return };
        let mut rng = rand::thread_rng();
        let text = exchange_template(&mut rng, &self.cfg.my_call, &w.st, fuzzy);
        let ev = Ev::Say {
            who: Who::Worked(w.id),
            text,
            offset_hz: w.offset_hz,
            amp: w.amp,
        };
        let delay = rng.gen_range(300..900);
        self.schedule(delay, ev);
    }

    fn worked_repeat(&mut self) {
        let Some(w) = self.worked.as_ref() else { return };
        let mut rng = rand::thread_rng();
        let text = repeat_template(&mut rng, &w.st);
        let ev = Ev::Say {
            who: Who::Worked(w.id),
            text,
            offset_hz: w.offset_hz,
            amp: w.amp,
        };
        let delay = rng.gen_range(300..900);
        self.schedule(delay, ev);
    }

    /// React to a transmission Diddle just finished sending.
    fn on_our_tx(&mut self, text: &str) {
        let tokens: Vec<String> = text
            .to_ascii_uppercase()
            .split_whitespace()
            .map(|t| t.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '/').to_string())
            .filter(|t| !t.is_empty())
            .collect();
        let has = |w: &str| tokens.iter().any(|t| t == w);
        let has_cq = has("CQ") || has("QRZ");
        let has_tu = has("TU") || has("QSL") || has("73") || has("QRZ");
        let has_agn = has("AGN") || has("AGN?") || text.contains('?') || has("PSE") || has("RPT");

        if self.cfg.mode == SimMode::Playback {
            return;
        }

        match self.phase {
            Phase::Exchanged => {
                // Did we name a *different* caller? Then we switched.
                if let Some(idx) = self.match_caller(&tokens).filter(|(_, exact)| *exact).map(|(i, _)| i) {
                    let prev = self.worked.take();
                    if let Some(p) = prev {
                        self.log("sim", format!("{} left without a TU", p.st.call));
                    }
                    let c = self.callers.remove(idx);
                    self.log("sim", format!("answering {} instead", c.st.call));
                    self.worked = Some(c);
                    self.worked_patience = 1;
                    self.worked_reply(false);
                    self.arm_exch_timeout();
                    return;
                }
                if has_tu || has_cq {
                    let w = self.worked.take();
                    if let Some(w) = w {
                        self.qso_count += 1;
                        self.log(
                            "sim",
                            format!(
                                "QSO #{} complete: {} sent {}",
                                self.qso_count, w.st.call, w.st.exchange
                            ),
                        );
                    }
                    if has_cq {
                        // Fresh CQ: the waiting callers come back plus new ones.
                        self.spawn_callers();
                    } else {
                        self.to_idle_or_calling();
                    }
                } else {
                    // Anything else (AGN?, NR?, re-sent exchange) → repeat.
                    let _ = has_agn;
                    self.worked_repeat();
                    self.arm_exch_timeout();
                }
            }
            Phase::Calling => {
                if let Some((idx, exact)) = self.match_caller(&tokens) {
                    let c = self.callers.remove(idx);
                    if !exact {
                        self.log(
                            "sim",
                            format!("you called {} — station is {} (it will correct you)",
                                tokens.iter().find(|t| fuzzy_score(t, &c.st.call).is_some()).cloned().unwrap_or_default(),
                                c.st.call),
                        );
                    }
                    self.worked = Some(c);
                    self.worked_patience = 1;
                    self.phase = Phase::Exchanged;
                    self.worked_reply(!exact);
                    self.arm_exch_timeout();
                    // Impatient callers double over the exchange now and then.
                    let mut rng = rand::thread_rng();
                    if self.cfg.activity >= 4 {
                        let my = self.cfg.my_call.clone();
                        let mut sched = Vec::new();
                        for c in &self.callers {
                            if rng.gen_bool(0.15) {
                                sched.push((
                                    rng.gen_range(400..1200),
                                    Ev::Say {
                                        who: Who::Caller(c.id),
                                        text: answer_template(&mut rng, &my, &c.st.call),
                                        offset_hz: c.offset_hz,
                                        amp: c.amp,
                                    },
                                ));
                            }
                        }
                        for (d, ev) in sched {
                            self.schedule(d, ev);
                        }
                    }
                } else if has_cq {
                    // Ignored the pileup and called CQ again: they persist.
                    self.callers_recall("CQ over them");
                    if self.callers.len() < self.cfg.activity as usize {
                        // and a newcomer or two may join
                        let mut rng = rand::thread_rng();
                        if rng.gen_bool(0.5) {
                            let st = self.gen.station(self.cfg.exchange);
                            let spread = self.cfg.spread_hz;
                            let offset = if spread > 0.0 { rng.gen_range(-spread..=spread) } else { 0.0 };
                            let amp = rng.gen_range(0.3..1.0);
                            let id = self.new_id();
                            let text = answer_template(&mut rng, &self.cfg.my_call.clone(), &st.call);
                            self.callers.push(Caller { id, st, offset_hz: offset, amp, patience: 2 });
                            self.schedule(rng.gen_range(500..1500), Ev::Say { who: Who::Caller(id), text, offset_hz: offset, amp });
                        }
                    }
                } else {
                    // AGN? / partial / garbage: everyone calls again.
                    self.log("sim", "callers repeating");
                    self.callers_call_now();
                    self.arm_callers_timeout();
                }
            }
            Phase::Idle => {
                if has_cq {
                    self.spawn_callers();
                } else {
                    self.log("sim", "nobody is calling — send CQ");
                }
            }
        }
        self.dirty = true;
    }

    /// Find the caller our TX addressed: (index, exact?). Exact matches
    /// win; otherwise the closest fuzzy match (one typo / partial call).
    fn match_caller(&self, tokens: &[String]) -> Option<(usize, bool)> {
        for (i, c) in self.callers.iter().enumerate() {
            if tokens.iter().any(|t| *t == c.st.call) {
                return Some((i, true));
            }
        }
        let mut best: Option<(usize, u32)> = None;
        for (i, c) in self.callers.iter().enumerate() {
            for t in tokens {
                if let Some(score) = fuzzy_score(t, &c.st.call) {
                    if best.map(|b| score < b.1).unwrap_or(true) {
                        best = Some((i, score));
                    }
                }
            }
        }
        best.map(|(i, _)| (i, false))
    }

    // ---- background stations --------------------------------------

    fn bg_pick_freq(&self, rng: &mut impl Rng) -> Option<f32> {
        for _ in 0..40 {
            let f = rng.gen_range(400.0..2700.0f32);
            let near_rx = (f - self.mark_hz).abs() < BG_GUARD_HZ
                || (f - self.space_hz).abs() < BG_GUARD_HZ
                || ((f + 170.0) - self.mark_hz).abs() < BG_GUARD_HZ;
            let near_bg = self.background.iter().any(|b| (b.mark_hz - f).abs() < 110.0);
            if !near_rx && !near_bg {
                return Some((f / 5.0).round() * 5.0);
            }
        }
        None
    }

    fn bg_add(&mut self) {
        let mut rng = rand::thread_rng();
        let Some(f) = self.bg_pick_freq(&mut rng) else { return };
        let st = self.gen.station(self.cfg.exchange);
        let id = self.new_id();
        let amp = rng.gen_range(0.15..0.55);
        self.background.push(BgStation { id, st, mark_hz: f, amp });
        let delay = rng.gen_range(200..3000);
        self.schedule(delay, Ev::BgNext { id });
        self.dirty = true;
    }

    fn bg_next(&mut self, id: u32) {
        let Some(b) = self.background.iter().find(|b| b.id == id) else { return };
        let mut rng = rand::thread_rng();
        let call = b.st.call.clone();
        let text = if rng.gen_bool(0.7) {
            let t = [
                format!(" CQ TEST {call} {call} CQ "),
                format!(" CQ CQ DE {call} {call} K "),
                format!(" CQ TEST DE {call} {call} {call} PSE K "),
            ];
            t.choose(&mut rng).unwrap().clone()
        } else {
            let other = self.gen.station(self.cfg.exchange);
            let rst = if b.st.sends_rst { "599 " } else { "" };
            let ex = &b.st.exchange;
            format!(" {} {rst}{ex} {ex} {call} K ", other.call)
        };
        // Background voices sit at an absolute audio frequency; express it
        // as an offset from the RX mark so the shared synth can place it.
        let offset = b.mark_hz - self.mark_hz;
        let amp = b.amp;
        self.say(Who::Bg(id), text, offset, amp);
    }

    /// Every so often one background station leaves and a new one shows
    /// up somewhere else — keeps the bandmap honest.
    fn bg_churn(&mut self) {
        let mut rng = rand::thread_rng();
        if !self.background.is_empty() {
            let i = rng.gen_range(0..self.background.len());
            let gone = self.background.remove(i);
            self.voices.retain(|v| v.who != Who::Bg(gone.id));
            self.log("sim", format!("{} QSYed away from {:.0} Hz", gone.st.call, gone.mark_hz));
            self.bg_add();
        }
        let next = rng.gen_range(60_000..150_000);
        self.schedule(next, Ev::BgChurn);
    }

    fn set_background(&mut self, n: u8) {
        while self.background.len() > n as usize {
            let gone = self.background.pop().unwrap();
            self.voices.retain(|v| v.who != Who::Bg(gone.id));
        }
        while self.background.len() < n as usize {
            let before = self.background.len();
            self.bg_add();
            if self.background.len() == before {
                break;
            }
        }
        self.dirty = true;
    }

    // ---- playback (scripted run) ----------------------------------

    fn playback_step(&mut self, step: u8) {
        let mut rng = rand::thread_rng();
        let my = self.cfg.my_call.clone();
        let me = self
            .pb_me
            .get_or_insert_with(|| self.gen.station_for_call(&my, self.cfg.exchange))
            .clone();
        let dx_amp = 0.75;
        match step {
            0 => {
                let dx = self.gen.station(self.cfg.exchange);
                self.pb_dx = Some(dx);
                self.say(Who::Playback, format!(" CQ CQ TEST DE {my} {my} K "), 0.0, 1.0);
                let d = self.playback_gap(&mut rng);
                self.schedule_after_voices(d, Ev::PlaybackStep { step: 1 });
            }
            1 => {
                let dx = self.pb_dx.clone().unwrap();
                self.say(Who::Playback, format!(" {my} DE {} {} ", dx.call, dx.call), self.pb_offset(&mut rng), dx_amp);
                let d = self.playback_gap(&mut rng);
                self.schedule_after_voices(d, Ev::PlaybackStep { step: 2 });
            }
            2 => {
                let dx = self.pb_dx.clone().unwrap();
                let ex = match self.cfg.exchange {
                    ExchangeKind::Serial => format!("{:03}", self.pb_serial),
                    _ => me.exchange.clone(),
                };
                let rst = if me.sends_rst { "599 " } else { "" };
                self.say(Who::Playback, format!(" {} {rst}{ex} {ex} K ", dx.call), 0.0, 1.0);
                let d = self.playback_gap(&mut rng);
                self.schedule_after_voices(d, Ev::PlaybackStep { step: 3 });
            }
            3 => {
                let dx = self.pb_dx.clone().unwrap();
                let rst = if dx.sends_rst { "599 " } else { "" };
                let ex = &dx.exchange;
                self.say(Who::Playback, format!(" {rst}{ex} {ex} TU "), self.pb_offset(&mut rng), dx_amp);
                let d = self.playback_gap(&mut rng);
                self.schedule_after_voices(d, Ev::PlaybackStep { step: 4 });
            }
            _ => {
                let dx = self.pb_dx.clone().unwrap();
                self.say(Who::Playback, format!(" TU {my} CQ "), 0.0, 1.0);
                self.qso_count += 1;
                self.pb_serial += 1;
                self.log("sim", format!("playback QSO #{} with {} ({})", self.qso_count, dx.call, dx.exchange));
                let d = rng.gen_range(500..2000);
                self.schedule_after_voices(d, Ev::PlaybackStep { step: 0 });
            }
        }
    }

    fn pb_offset(&self, rng: &mut impl Rng) -> f32 {
        let s = self.cfg.spread_hz;
        if s > 0.0 { rng.gen_range(-s..=s) } else { 0.0 }
    }

    fn playback_gap(&self, rng: &mut impl Rng) -> u64 {
        rng.gen_range(200..800)
    }

    /// Schedule `ev` `delay_ms` after the currently playing voices finish.
    fn schedule_after_voices(&mut self, delay_ms: u64, ev: Ev) {
        let remaining = self
            .voices
            .iter()
            .map(|v| (v.samples.len() - v.pos) as u64)
            .max()
            .unwrap_or(0);
        let due = self.now + remaining + self.ms(delay_ms);
        self.events.push(Event { due, ev });
    }

    // ---- status ----------------------------------------------------

    fn status(&self) -> SimStatus {
        let ci = |c: &Caller, status: &'static str| CallerInfo {
            call: c.st.call.clone(),
            exchange: c.st.exchange.clone(),
            offset_hz: c.offset_hz,
            level: c.amp,
            status,
        };
        SimStatus {
            running: self.running,
            mode: self.cfg.mode,
            phase: if self.cfg.mode == SimMode::Playback {
                "playback"
            } else {
                match self.phase {
                    Phase::Idle => "idle",
                    Phase::Calling => "calling",
                    Phase::Exchanged => "exchanged",
                }
            },
            ptt: self.ptt,
            qso_count: self.qso_count,
            my_call: self.cfg.my_call.clone(),
            exchange: self.cfg.exchange,
            callers: self
                .callers
                .iter()
                .map(|c| ci(c, if self.phase == Phase::Calling { "calling" } else { "waiting" }))
                .collect(),
            worked: self.worked.as_ref().map(|w| ci(w, "worked")),
            background: self
                .background
                .iter()
                .map(|b| BgInfo {
                    call: b.st.call.clone(),
                    mark_hz: b.mark_hz,
                    exchange: b.st.exchange.clone(),
                })
                .collect(),
            noise: self.cfg.noise,
            activity: self.cfg.activity,
            spread_hz: self.cfg.spread_hz,
            signal: self.cfg.signal,
            dial_hz: self.cfg.dial_hz,
            log: self.log.clone(),
        }
    }
}

/// Timeline of our own transmission: per-character echo points (sample
/// offset, char) and the total length in samples, lead-in and trail
/// included. Drains the generator one sample at a time — its idle check
/// only fires on an exact sample boundary, so coarser steps can miss it
/// and run to the safety cap (which once left the PTT "on" for hours).
fn our_tx_timeline(text: &str, mark: f32, space: f32, baud: f32) -> (Vec<(u64, char)>, u64) {
    let mut gen = RttyTxGenerator::new(SAMPLE_RATE, mark, space, baud);
    let spb = gen.samples_per_bit();
    let lead = SAMPLE_RATE as u64 * OUR_LEAD_MS / 1000;
    let marks: Vec<(u64, char)> = gen
        .enqueue_with_marks(text)
        .into_iter()
        .map(|(bit, c)| (lead + (bit as f32 * spb) as u64, c))
        .collect();
    let mut scratch = Vec::with_capacity(1);
    let mut message = 0u64;
    let cap = SAMPLE_RATE as u64 * 120;
    while !gen.is_idle() && message < cap {
        gen.next_samples(1, &mut scratch);
        scratch.clear();
        message += 1;
    }
    let total = lead + message + SAMPLE_RATE as u64 * OUR_TRAIL_MS / 1000;
    (marks, total)
}

// ---- message templates ------------------------------------------------

fn answer_template(rng: &mut impl Rng, my: &str, call: &str) -> String {
    let t = [
        format!(" {my} DE {call} {call} K "),
        format!(" {call} {call} "),
        format!(" DE {call} {call} {call} "),
        format!(" {my} {call} {call} "),
        format!(" {call} {call} K "),
    ];
    let weights = [4, 3, 2, 2, 2];
    let total: u32 = weights.iter().sum();
    let mut roll = rng.gen_range(0..total);
    for (i, w) in weights.iter().enumerate() {
        if roll < *w {
            return t[i].clone();
        }
        roll -= w;
    }
    t[0].clone()
}

fn exchange_template(rng: &mut impl Rng, my: &str, st: &Station, correcting: bool) -> String {
    let rst = if st.sends_rst { "599 " } else { "" };
    let ex = &st.exchange;
    let call = &st.call;
    if correcting {
        return format!(" DE {call} {call} {rst}{ex} {ex} {call} K ");
    }
    let t = [
        format!(" {my} TU {rst}{ex} {ex} {call} "),
        format!(" TU {rst}{ex} {ex} K "),
        format!(" {rst}{ex} {ex} {ex} K "),
        format!(" {my} {rst}{ex} {ex} {call} K "),
        format!(" R {rst}{ex} {ex} {call} "),
    ];
    t.choose(rng).unwrap().clone()
}

fn repeat_template(rng: &mut impl Rng, st: &Station) -> String {
    let rst = if st.sends_rst { "599 " } else { "" };
    let ex = &st.exchange;
    let t = [
        format!(" {ex} {ex} {ex} "),
        format!(" {rst}{ex} {ex} {ex} K "),
        format!(" AGN {ex} {ex} {ex} {} ", st.call),
    ];
    t.choose(rng).unwrap().clone()
}

/// Lower is better; None when `token` is not a plausible attempt at `call`.
fn fuzzy_score(token: &str, call: &str) -> Option<u32> {
    if token.len() < 3 || token == call {
        return None;
    }
    // Skip obvious non-call tokens (RST, serials, common words).
    if token.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if call.contains(token) {
        return Some(1 + (call.len() - token.len()) as u32);
    }
    if token.contains(call) {
        return Some(1 + (token.len() - call.len()) as u32);
    }
    if call.len() >= 4 {
        let d = levenshtein(token, call);
        if d <= 1 || (d == 2 && call.len() >= 6) {
            return Some(d + 2);
        }
    }
    None
}

fn levenshtein(a: &str, b: &str) -> u32 {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<u32> = (0..=b.len() as u32).collect();
    let mut cur = vec![0u32; b.len() + 1];
    for i in 1..=a.len() {
        cur[0] = i as u32;
        for j in 1..=b.len() {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

// ---------------------------------------------------------------------
// Simulator: the handle held in AppState.
// ---------------------------------------------------------------------

pub struct Simulator {
    app: AppHandle,
    rtty: Arc<RttyTunable>,
    scp: Arc<ScpDb>,
    world: Arc<Mutex<Option<World>>>,
    task: RwLock<Option<JoinHandle<()>>>,
    tx_busy: AtomicBool,
    tx_cancel: AtomicBool,
}

impl Simulator {
    pub fn new(app: AppHandle, rtty: Arc<RttyTunable>, scp: Arc<ScpDb>) -> Self {
        Self {
            app,
            rtty,
            scp,
            world: Arc::new(Mutex::new(None)),
            task: RwLock::new(None),
            tx_busy: AtomicBool::new(false),
            tx_cancel: AtomicBool::new(false),
        }
    }

    pub fn is_running(&self) -> bool {
        self.world.lock().unwrap().as_ref().map(|w| w.running).unwrap_or(false)
    }

    pub fn status(&self) -> SimStatus {
        self.world
            .lock()
            .unwrap()
            .as_ref()
            .map(|w| w.status())
            .unwrap_or_else(SimStatus::idle)
    }

    fn emit_status(&self) {
        let _ = self.app.emit("sim:status", &self.status());
    }

    fn emit_rig(&self, ptt: bool) {
        let dial = self.world.lock().unwrap().as_ref().map(|w| w.cfg.dial_hz).unwrap_or(0);
        let r = RigState {
            freq: dial,
            mode: "digl".into(),
            ptt,
        };
        let _ = self.app.emit("tci:rig", &r);
    }

    /// Re-publish a rig state (used after stop to put the real radio's
    /// frequency back on the header in place of the pretend dial).
    pub fn emit_rig_state(&self, rig: &RigState) {
        let _ = self.app.emit("tci:rig", rig);
    }

    pub async fn start(self: Arc<Self>, cfg: SimConfig) -> anyhow::Result<()> {
        self.stop().await;
        let cfg = cfg.sanitized();
        let rcfg = self.rtty.get().await;
        info!(
            mode = ?cfg.mode,
            my_call = %cfg.my_call,
            exchange = ?cfg.exchange,
            activity = cfg.activity,
            noise = cfg.noise,
            background = cfg.background,
            "sim: starting"
        );

        let mut world = World::new(
            Some(self.app.clone()),
            cfg.clone(),
            rcfg.mark_hz,
            rcfg.space_hz,
            rcfg.baud,
            self.scp.clone(),
        );
        world.log(
            "sim",
            match cfg.mode {
                SimMode::Pileup => format!(
                    "pileup mode — send CQ as {} and work what comes back ({:?} exchange)",
                    cfg.my_call, cfg.exchange
                ),
                SimMode::Playback => format!(
                    "playback mode — a scripted run by {} plays on the RX tones",
                    cfg.my_call
                ),
            },
        );
        world.set_background(cfg.background);
        if cfg.background > 0 {
            world.schedule(90_000, Ev::BgChurn);
        }
        if cfg.mode == SimMode::Playback {
            world.schedule(800, Ev::PlaybackStep { step: 0 });
        }
        *self.world.lock().unwrap() = Some(world);
        self.emit_rig(false);

        let me = self.clone();
        let handle = tokio::spawn(async move {
            let mut pipeline = RxPipeline::new(
                SAMPLE_RATE,
                me.app.clone(),
                me.rtty.clone(),
                me.scp.clone(),
            )
            .await;
            let period = Duration::from_micros(CHUNK as u64 * 1_000_000 / SAMPLE_RATE as u64);
            let mut interval = tokio::time::interval(period);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Burst);
            let mut last_status = Instant::now() - STATUS_MIN_INTERVAL;
            loop {
                interval.tick().await;
                let (buf, muted, dirty) = {
                    let mut guard = me.world.lock().unwrap();
                    let Some(w) = guard.as_mut() else { break };
                    if !w.running {
                        break;
                    }
                    let (buf, muted) = w.tick(CHUNK);
                    let dirty = w.dirty;
                    if dirty && last_status.elapsed() >= STATUS_MIN_INTERVAL {
                        w.dirty = false;
                    }
                    (buf, muted, dirty)
                };
                pipeline.push(&buf, muted).await;
                if dirty && last_status.elapsed() >= STATUS_MIN_INTERVAL {
                    me.emit_status();
                    last_status = Instant::now();
                }
            }
        });
        *self.task.write().await = Some(handle);
        self.emit_status();
        Ok(())
    }

    pub async fn stop(&self) {
        self.tx_cancel.store(true, Ordering::SeqCst);
        if let Some(h) = self.task.write().await.take() {
            h.abort();
        }
        let was = self.world.lock().unwrap().take().is_some();
        if was {
            info!("sim: stopped");
        }
        self.emit_status();
    }

    /// Live-tune the knobs that don't need a restart.
    pub fn update(&self, noise: Option<f32>, activity: Option<u8>, background: Option<u8>, spread_hz: Option<f32>, signal: Option<f32>) {
        let mut guard = self.world.lock().unwrap();
        let Some(w) = guard.as_mut() else { return };
        if let Some(n) = noise {
            w.cfg.noise = n.clamp(0.0, 1.0);
        }
        if let Some(a) = activity {
            w.cfg.activity = a.clamp(1, 9);
        }
        if let Some(s) = spread_hz {
            w.cfg.spread_hz = s.clamp(0.0, 200.0);
        }
        if let Some(s) = signal {
            w.cfg.signal = s.clamp(0.05, 1.0);
        }
        if let Some(b) = background {
            w.set_background(b.min(8));
        }
        w.dirty = true;
    }

    /// Our transmission while simulating: echo the characters into the RX
    /// window at real-time pace, mute the decoders meanwhile (as during a
    /// real TX), then let the world react to the text. Never touches TCI.
    pub async fn transmit(&self, text: String) -> anyhow::Result<()> {
        if !self.is_running() {
            anyhow::bail!("simulator not running");
        }
        if text.trim().is_empty() {
            anyhow::bail!("refusing to transmit empty text");
        }
        // Playback is listen-only: the scripted run plays both sides, so
        // an F-key has nothing to say to it. Don't key up, don't echo, and
        // don't mute the RX — just note it in the log.
        let playback = {
            let guard = self.world.lock().unwrap();
            guard.as_ref().map(|w| w.cfg.mode == SimMode::Playback).unwrap_or(false)
        };
        if playback {
            let mut guard = self.world.lock().unwrap();
            if let Some(w) = guard.as_mut() {
                w.log("sim", format!("playback mode — nothing sent: {}", text.trim()));
            }
            return Ok(());
        }
        if self.tx_busy.swap(true, Ordering::SeqCst) {
            anyhow::bail!("TX already in progress");
        }
        self.tx_cancel.store(false, Ordering::SeqCst);
        let result = self.transmit_inner(&text).await;
        self.tx_busy.store(false, Ordering::SeqCst);
        result
    }

    async fn transmit_inner(&self, text: &str) -> anyhow::Result<()> {
        let rcfg = self.rtty.get().await;
        let (mark, space) = rcfg.tx_tones();
        let (marks, total) = our_tx_timeline(text, mark, space, rcfg.baud);

        {
            let mut guard = self.world.lock().unwrap();
            if let Some(w) = guard.as_mut() {
                w.ptt = true;
                w.dirty = true;
                w.log("you", text.trim().to_string());
            }
        }
        self.emit_rig(true);

        let start = Instant::now();
        let mut idx = 0usize;
        let mut cancelled = false;
        loop {
            if self.tx_cancel.load(Ordering::SeqCst) {
                cancelled = true;
                break;
            }
            let pos = (start.elapsed().as_secs_f64() * SAMPLE_RATE as f64) as u64;
            while idx < marks.len() && marks[idx].0 <= pos {
                let _ = self.app.emit("tx:echo", marks[idx].1.to_string());
                idx += 1;
            }
            if pos >= total {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }

        {
            let mut guard = self.world.lock().unwrap();
            if let Some(w) = guard.as_mut() {
                w.ptt = false;
                w.dirty = true;
                if cancelled {
                    w.log("sim", "TX aborted");
                } else {
                    w.on_our_tx(text);
                }
            }
        }
        self.emit_rig(false);
        if cancelled {
            info!("sim: tx cancelled");
        }
        Ok(())
    }

    pub fn abort_tx(&self) {
        if self.tx_busy.load(Ordering::SeqCst) {
            self.tx_cancel.store(true, Ordering::SeqCst);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::{RttyConfig, RttyDemod};

    fn cfg(mode: SimMode, activity: u8) -> SimConfig {
        SimConfig {
            my_call: "W1AW".into(),
            exchange: ExchangeKind::Serial,
            mode,
            activity,
            noise: 0.0,
            background: 0,
            spread_hz: 0.0,
            signal: 0.8,
            dial_hz: 14_080_000,
            use_scp: false,
        }
        .sanitized()
    }

    fn world(mode: SimMode, activity: u8) -> World {
        World::new(None, cfg(mode, activity), 2125.0, 2295.0, 45.45, Arc::new(ScpDb::new()))
    }

    /// Advance the world by `secs`, decoding its audio with the real demod.
    fn run(w: &mut World, secs: f32, demod: &mut RttyDemod) -> String {
        let mut out = String::new();
        let blocks = (secs * SAMPLE_RATE as f32 / CHUNK as f32) as usize;
        for _ in 0..blocks {
            let (buf, _muted) = w.tick(CHUNK);
            out.push_str(&demod.push(&buf));
        }
        out
    }

    fn demod() -> RttyDemod {
        RttyDemod::new(SAMPLE_RATE, RttyConfig::default())
    }

    #[test]
    fn cq_brings_callers_and_the_audio_decodes() {
        let mut w = world(SimMode::Pileup, 1);
        let mut d = demod();
        // A CQ may (by design) go unanswered now and then; retry a few times.
        let mut tries = 0;
        while w.callers.is_empty() && tries < 12 {
            w.on_our_tx("CQ TEST DE W1AW W1AW K");
            tries += 1;
        }
        assert_eq!(w.phase, Phase::Calling);
        assert!(!w.callers.is_empty(), "no callers after {tries} CQs");
        // Keep exactly one caller so nothing collides on the tones (the
        // Say events of the dropped ones are skipped — station gone).
        w.callers.truncate(1);
        let call = w.callers[0].st.call.clone();
        // Callers are staggered; pull the kept one's transmission forward
        // so the 9 s "nobody answered" timeout can't interfere.
        for e in w.events.iter_mut() {
            if matches!(e.ev, Ev::Say { .. }) {
                e.due = w.now;
            }
        }
        let text = run(&mut w, 6.0, &mut d);
        assert!(
            text.contains(&call),
            "decoded text should contain the caller {call}; got {text:?}"
        );

        // Answer the caller with our exchange → it sends its exchange back.
        w.on_our_tx(&format!("{call} 599 001 001 K"));
        assert_eq!(w.phase, Phase::Exchanged);
        let worked = w.worked.as_ref().expect("worked station");
        assert_eq!(worked.st.call, call);
        let ex = worked.st.exchange.clone();
        let text = run(&mut w, 8.0, &mut d);
        assert!(
            text.contains(&ex),
            "decoded text should contain the exchange {ex}; got {text:?}"
        );

        // TU closes the QSO.
        w.on_our_tx("TU W1AW CQ");
        assert_eq!(w.qso_count, 1);
        assert!(w.worked.is_none());
    }

    #[test]
    fn wrong_call_gets_a_correction() {
        let mut w = world(SimMode::Pileup, 3);
        let mut d = demod();
        // Need a call of at least four characters: a three-character
        // partial is the shortest thing the fuzzy matcher will consider.
        let mut tries = 0;
        while !w.callers.iter().any(|c| c.st.call.len() >= 4) && tries < 20 {
            w.callers.clear();
            w.on_our_tx("CQ TEST DE W1AW W1AW K");
            tries += 1;
        }
        let call = w
            .callers
            .iter()
            .find(|c| c.st.call.len() >= 4)
            .map(|c| c.st.call.clone())
            .expect("a caller with a 4+ character call");
        // Drop the last character of the call: a typical partial copy.
        let partial: String = call.chars().take(call.len() - 1).collect();
        w.on_our_tx(&format!("{partial} 599 001 001 K"));
        assert_eq!(w.phase, Phase::Exchanged, "fuzzy match should be answered");
        assert_eq!(w.worked.as_ref().unwrap().st.call, call);
        let _ = run(&mut w, 2.0, &mut d);
        // The correction transmission names the station twice up front,
        // followed by the exchange.
        let ex = w.worked.as_ref().unwrap().st.exchange.clone();
        let want = format!("DE {call} {call} 599 {ex}");
        assert!(
            w.log.iter().any(|l| l.who == "dx" && l.text.contains(&want)),
            "expected a correction containing {want:?}; log: {:?}",
            w.log.iter().map(|l| l.text.clone()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn ignored_callers_eventually_leave() {
        let mut w = world(SimMode::Pileup, 2);
        let mut d = demod();
        let mut tries = 0;
        while w.callers.is_empty() && tries < 12 {
            w.on_our_tx("CQ TEST DE W1AW W1AW K");
            tries += 1;
        }
        // Say nothing for a long time: two recalls, then everyone gives up.
        let _ = run(&mut w, 40.0, &mut d);
        assert_eq!(w.phase, Phase::Idle);
        assert!(w.callers.is_empty());
    }

    #[test]
    fn playback_runs_a_scripted_qso() {
        let mut w = world(SimMode::Playback, 1);
        let mut d = demod();
        w.schedule(100, Ev::PlaybackStep { step: 0 });
        let text = run(&mut w, 40.0, &mut d);
        assert!(w.qso_count >= 1, "playback should complete a QSO in 40 s; log: {:?}", w.log);
        assert!(text.contains("CQ"), "{text:?}");
        assert!(text.contains("W1AW"), "{text:?}");
    }

    #[test]
    fn background_stations_are_placed_off_the_rx_pair() {
        let mut w = world(SimMode::Pileup, 1);
        w.cfg.background = 4;
        w.set_background(4);
        assert_eq!(w.background.len(), 4);
        for b in &w.background {
            assert!((b.mark_hz - 2125.0).abs() >= BG_GUARD_HZ, "{}", b.mark_hz);
        }
        // Each background station eventually transmits.
        let mut d = demod();
        let _ = run(&mut w, 4.0, &mut d);
        assert!(w.log.iter().any(|l| l.who == "bg"));
    }

    #[test]
    fn playback_ignores_our_tx() {
        let mut w = world(SimMode::Playback, 1);
        w.on_our_tx("CQ TEST DE W1AW W1AW K");
        assert_eq!(w.phase, Phase::Idle);
        assert!(w.callers.is_empty());
    }

    #[test]
    fn our_tx_length_matches_the_message() {
        let text = "W1AW DE K6AC K6AC K";
        let (marks, total) = our_tx_timeline(text, 2125.0, 2295.0, 45.45);
        assert_eq!(marks.len(), text.len());
        // 19 chars + 4 FIGS/LTRS shifts = 23 frames × 8 bits at 45.45 baud
        // ≈ 4.0 s, plus lead-in and trail. Anything far outside that means
        // the length calculation ran away again.
        let secs = total as f32 / SAMPLE_RATE as f32;
        assert!((4.0..5.5).contains(&secs), "TX would take {secs} s");
        assert!(marks.last().unwrap().0 < total);
    }

    #[test]
    fn fuzzy_matching_is_sane() {
        assert!(fuzzy_score("K1AB", "K1ABC").is_some());
        assert!(fuzzy_score("K1ABC", "K1ABD").is_some());
        assert!(fuzzy_score("599", "K1ABC").is_none());
        assert!(fuzzy_score("TU", "K1ABC").is_none());
        assert!(fuzzy_score("W1AW", "K1ABC").is_none());
        assert_eq!(fuzzy_score("K1ABC", "K1ABC"), None); // exact handled elsewhere
    }
}

