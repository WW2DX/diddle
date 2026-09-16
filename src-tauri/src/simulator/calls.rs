// Station generator for the contest simulator: realistic callsigns (or
// real ones sampled from the Super Check Partial database, so the spot
// filter accepts them), plus the exchange each station sends for the
// active contest. Prefix tables ported from EC5W's RTTY Runner.

use std::sync::Arc;

use rand::seq::SliceRandom;
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::scp::ScpDb;

/// What the other station sends after the RST. Mirrors Diddle's contest
/// profiles (contests.ts) — the frontend maps the active profile onto one
/// of these when it starts the simulator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExchangeKind {
    /// 599 + serial (Generic, CQ WPX RTTY).
    Serial,
    /// 599 + CQ zone, US/VE add state (CQ WW RTTY).
    Zone,
    /// 599 + state/province; DX sends a serial (ARRL RTTY Roundup).
    State,
    /// Name + state/province; DX sends name only (NAQP RTTY).
    NameState,
    /// Casual: 599 + name + QTH (General QSO).
    Ragchew,
}

#[derive(Debug, Clone, Serialize)]
pub struct Station {
    pub call: String,
    /// The exchange text this station sends (without the leading RST for
    /// RST-style contests — e.g. "001", "05 MA", "MA", "JOHN MA").
    pub exchange: String,
    /// Whether the exchange starts with an RST on the air.
    pub sends_rst: bool,
    pub is_na: bool,
    pub state: Option<String>,
    pub zone: u8,
    pub name: String,
}

const LETTERS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";

const US_STATES: [&[&str]; 10] = [
    &["CO", "IA", "KS", "MN", "MO", "NE", "ND", "SD"], // 0
    &["CT", "MA", "ME", "NH", "RI", "VT"],             // 1
    &["NJ", "NY"],                                     // 2
    &["DE", "MD", "PA"],                               // 3
    &["AL", "FL", "GA", "KY", "NC", "SC", "TN", "VA"], // 4
    &["AR", "LA", "MS", "NM", "OK", "TX"],             // 5
    &["CA"],                                           // 6
    &["AZ", "ID", "MT", "NV", "OR", "UT", "WA", "WY"], // 7
    &["MI", "OH", "WV"],                               // 8
    &["IL", "IN", "WI"],                               // 9
];

const VE_PROVINCES: &[(&str, &[&str])] = &[
    ("VE1", &["NS", "NB", "PE"]),
    ("VE2", &["QC"]),
    ("VE3", &["ON"]),
    ("VE4", &["MB"]),
    ("VE5", &["SK"]),
    ("VE6", &["AB"]),
    ("VE7", &["BC"]),
    ("VE8", &["NT"]),
    ("VE9", &["NB"]),
    ("VY1", &["YT"]),
    ("VY2", &["PE"]),
    ("VO1", &["NL"]),
    ("VO2", &["NL"]),
];

const US_PREFIXES: &[(&str, u32)] = &[
    ("W", 25), ("K", 25), ("N", 20),
    ("WA", 8), ("WB", 8), ("KC", 8), ("KD", 8),
    ("WD", 5), ("WN", 3), ("KE", 5), ("KF", 5),
    ("KG", 5), ("KI", 3), ("KJ", 3), ("KK", 3),
    ("W1", 4), ("W2", 4), ("W3", 4), ("W4", 4), ("W5", 4),
    ("W6", 4), ("W7", 4), ("W8", 4), ("W9", 4), ("W0", 4),
    ("K1", 4), ("K2", 4), ("K3", 4), ("K4", 4), ("K5", 4),
    ("K6", 4), ("K7", 4), ("K8", 4), ("K9", 4), ("K0", 4),
    ("N1", 3), ("N2", 3), ("N3", 3), ("N4", 3), ("N5", 3),
    ("N6", 3), ("N7", 3), ("N8", 3), ("N9", 3), ("N0", 3),
    ("AA", 2), ("AB", 2), ("AC", 2), ("AD", 2), ("AE", 2),
    ("AF", 2), ("AG", 2), ("AI", 2), ("AJ", 2), ("AK", 2),
];

const VE_PREFIXES: &[(&str, u32)] = &[
    ("VE1", 3), ("VE2", 4), ("VE3", 8), ("VE4", 2),
    ("VE5", 2), ("VE6", 3), ("VE7", 4), ("VE9", 2),
    ("VA2", 2), ("VA3", 4), ("VA6", 2), ("VA7", 2),
];

const DX_PREFIXES: &[(&str, u32)] = &[
    ("DL", 10), ("DF", 5), ("DK", 5), ("DJ", 3),
    ("G", 8), ("M", 5), ("2E", 2),
    ("F", 8), ("F5", 3), ("F6", 3),
    ("I", 6), ("IK", 4), ("IZ", 3),
    ("EA", 8), ("EB", 3), ("EC", 2),
    ("CT", 4), ("PA", 5), ("PD", 3), ("ON", 4), ("HB9", 3), ("OE", 3),
    ("OK", 4), ("OL", 2), ("SP", 5), ("SQ", 3), ("OM", 3), ("HA", 4),
    ("YO", 3), ("LZ", 3), ("9A", 3), ("S5", 3), ("OZ", 3), ("SM", 4),
    ("LA", 3), ("OH", 4), ("ES", 2), ("YL", 2), ("LY", 2),
    ("UA", 5), ("RV", 3), ("RW", 3),
    ("PY", 5), ("PP", 3), ("LU", 4), ("CE", 3), ("CX", 2), ("HK", 3), ("YV", 2),
    ("JA", 6), ("JH", 4), ("JR", 3), ("HL", 3), ("BV", 2), ("VU", 3),
    ("VK", 4), ("ZL", 3), ("ZS", 3), ("CN", 2),
    ("PJ2", 2), ("PJ4", 1), ("VP5", 1), ("8P", 1), ("V4", 1),
];

const NAMES: &[&str] = &[
    "JOHN", "BOB", "JIM", "DAVE", "MIKE", "TOM", "BILL", "STEVE", "RICK", "DON",
    "JOE", "DAN", "ED", "RON", "KEN", "GARY", "PAUL", "MARK", "JEFF", "AL",
    "FRED", "LEE", "RAY", "CARL", "PETE", "CHUCK", "LARRY", "RANDY", "GREG", "SCOTT",
    "JACK", "WALT", "HANK", "ART", "ANN", "SUE", "LINDA", "MARY", "KAREN", "JAN",
];

pub struct StationGen {
    scp: Arc<ScpDb>,
    use_scp: bool,
    /// Percentage of stations that are US/VE (the rest are DX).
    na_pct: u32,
    us_weight: u32,
    ve_weight: u32,
    dx_weight: u32,
}

impl StationGen {
    pub fn new(scp: Arc<ScpDb>, use_scp: bool) -> Self {
        Self {
            scp,
            use_scp,
            na_pct: 70,
            us_weight: US_PREFIXES.iter().map(|p| p.1).sum(),
            ve_weight: VE_PREFIXES.iter().map(|p| p.1).sum(),
            dx_weight: DX_PREFIXES.iter().map(|p| p.1).sum(),
        }
    }

    /// Build one station for the given contest. `serial_hint` seeds a
    /// plausible serial number (stations deep in a contest have big ones).
    pub fn station(&self, kind: ExchangeKind) -> Station {
        let mut rng = rand::thread_rng();
        let call = if self.use_scp {
            self.scp.sample(1).pop().unwrap_or_else(|| self.generated_call(&mut rng))
        } else {
            self.generated_call(&mut rng)
        };
        self.station_for_call(&call, kind)
    }

    /// Derive location info + exchange for a given callsign.
    pub fn station_for_call(&self, call: &str, kind: ExchangeKind) -> Station {
        let mut rng = rand::thread_rng();
        let call = call.to_ascii_uppercase();
        let (is_na, state) = na_location(&call, &mut rng);
        let zone = cq_zone(&call, &mut rng);
        let name = NAMES.choose(&mut rng).unwrap().to_string();
        let serial = serial_number(&mut rng);
        let (exchange, sends_rst) = match kind {
            ExchangeKind::Serial => (fmt_serial(serial), true),
            ExchangeKind::Zone => {
                let z = format!("{zone:02}");
                match &state {
                    Some(s) if is_na => (format!("{z} {s}"), true),
                    _ => (z, true),
                }
            }
            ExchangeKind::State => match &state {
                Some(s) if is_na => (s.clone(), true),
                _ => (fmt_serial(serial), true),
            },
            ExchangeKind::NameState => match &state {
                Some(s) if is_na => (format!("{name} {s}"), false),
                _ => (name.clone(), false),
            },
            ExchangeKind::Ragchew => {
                let qth = state.clone().unwrap_or_else(|| "DX".into());
                (format!("{name} {qth}"), true)
            }
        };
        Station {
            call,
            exchange,
            sends_rst,
            is_na,
            state,
            zone,
            name,
        }
    }

    fn generated_call(&self, rng: &mut impl Rng) -> String {
        let na = rng.gen_range(0..100) < self.na_pct;
        let (prefix, _) = if na {
            if rng.gen_range(0..100) < 85 {
                weighted(US_PREFIXES, self.us_weight, rng)
            } else {
                weighted(VE_PREFIXES, self.ve_weight, rng)
            }
        } else {
            weighted(DX_PREFIXES, self.dx_weight, rng)
        };
        let suffix_len = match rng.gen_range(0..100) {
            0..=4 => 1,
            5..=44 => 2,
            _ => 3,
        };
        let suffix: String = (0..suffix_len)
            .map(|_| *LETTERS.choose(rng).unwrap() as char)
            .collect();
        if prefix.ends_with(|c: char| c.is_ascii_digit()) {
            format!("{prefix}{suffix}")
        } else {
            format!("{prefix}{}{suffix}", rng.gen_range(0..10))
        }
    }
}

fn weighted<'a>(table: &'a [(&'a str, u32)], total: u32, rng: &mut impl Rng) -> (&'a str, u32) {
    let roll = rng.gen_range(0..total.max(1));
    let mut acc = 0;
    for &(p, w) in table {
        acc += w;
        if roll < acc {
            return (p, w);
        }
    }
    table[0]
}

fn fmt_serial(n: u32) -> String {
    format!("{n:03}")
}

fn serial_number(rng: &mut impl Rng) -> u32 {
    match rng.gen_range(0..100) {
        0..=19 => rng.gen_range(1..100),
        20..=49 => rng.gen_range(100..500),
        50..=79 => rng.gen_range(500..2000),
        _ => rng.gen_range(2000..5000),
    }
}

/// Is this a US or Canadian call, and if so which state/province? Uses
/// the call district digit for the US and the VE/VA prefix for Canada.
fn na_location(call: &str, rng: &mut impl Rng) -> (bool, Option<String>) {
    let b = call.as_bytes();
    if b.len() < 3 {
        return (false, None);
    }
    let digit_pos = b.iter().position(|c| c.is_ascii_digit());
    let Some(dp) = digit_pos else {
        return (false, None);
    };
    let prefix = &call[..dp];
    // Canada: VE/VA/VY/VO + digit.
    if matches!(prefix, "VE" | "VA" | "VY" | "VO") {
        let key = format!("VE{}", b[dp] as char);
        let key2 = format!("{}{}", prefix, b[dp] as char);
        let list = VE_PROVINCES
            .iter()
            .find(|(k, _)| *k == key2 || *k == key)
            .map(|(_, v)| *v)
            .unwrap_or(&["ON"]);
        return (true, Some(list.choose(rng).unwrap().to_string()));
    }
    // USA: K, N, W (any 1–2 letter prefix) or AA–AL.
    let us = match prefix.len() {
        1 => matches!(prefix, "K" | "N" | "W"),
        2 => {
            let first = prefix.as_bytes()[0];
            let second = prefix.as_bytes()[1];
            matches!(first, b'K' | b'N' | b'W') || (first == b'A' && (b'A'..=b'L').contains(&second))
        }
        _ => false,
    };
    if !us {
        return (false, None);
    }
    let district = (b[dp] - b'0') as usize;
    let list = US_STATES[district.min(9)];
    (true, Some(list.choose(rng).unwrap().to_string()))
}

/// Approximate CQ zone from the prefix — good enough for a plausible
/// exchange; the operator only needs to log what was sent.
fn cq_zone(call: &str, rng: &mut impl Rng) -> u8 {
    let c = call;
    let starts = |p: &str| c.starts_with(p);
    let digit = c.bytes().find(|b| b.is_ascii_digit()).map(|b| b - b'0');
    if starts("VE") || starts("VA") || starts("VY") || starts("VO") {
        return match digit {
            Some(1) | Some(9) | Some(0) => 5,
            Some(2) | Some(3) => rng.gen_range(2..=5),
            Some(4) | Some(5) => 4,
            Some(6) | Some(7) => 3,
            _ => 2,
        };
    }
    if starts("K") || starts("N") || starts("W") || starts("A") {
        return match digit {
            Some(1) | Some(2) | Some(3) | Some(4) | Some(8) => rng.gen_range(4..=5),
            Some(9) | Some(0) => 4,
            Some(5) => 4,
            Some(6) | Some(7) => 3,
            _ => 4,
        };
    }
    if starts("JA") || starts("JH") || starts("JR") || starts("JE") || starts("JF") || starts("JG") || starts("JI") || starts("JK") || starts("JL") || starts("JM") || starts("JN") || starts("JO") || starts("JP") || starts("JQ") || starts("JS") || starts("7K") || starts("7L") || starts("7M") || starts("7N") {
        return 25;
    }
    if starts("HL") || starts("DS") { return 25; }
    if starts("BV") || starts("BX") { return 24; }
    if starts("VU") { return 22; }
    if starts("VK") { return rng.gen_range(29..=30); }
    if starts("ZL") { return 32; }
    if starts("PY") || starts("PP") || starts("PU") { return 11; }
    if starts("LU") || starts("CX") { return 13; }
    if starts("CE") { return 12; }
    if starts("HK") || starts("YV") || starts("HC") { return 9; }
    if starts("ZS") { return 38; }
    if starts("CN") || starts("EA8") { return 33; }
    if starts("UA9") || starts("R9") || starts("RA9") { return rng.gen_range(17..=18); }
    if starts("UA") || starts("R") || starts("UR") || starts("UT") || starts("US") { return 16; }
    if starts("YO") || starts("LZ") || starts("SV") || starts("ER") || starts("YL") || starts("ES") || starts("LY") || starts("OH") || starts("SM") || starts("LA") || starts("OZ") || starts("SP") || starts("SQ") || starts("OK") || starts("OL") || starts("OM") || starts("HA") || starts("9A") || starts("S5") || starts("YU") || starts("OE") { return 15; }
    if starts("PJ") || starts("VP5") || starts("8P") || starts("V4") || starts("KP4") || starts("HI") { return 8; }
    14
}
