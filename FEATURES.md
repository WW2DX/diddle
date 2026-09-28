# Features

A complete rundown of what Diddle does today, plus what's planned.

## Radio integration (TCI)

- **TCI WebSocket client** — connects to any TCI server (ExpertSDR2/3, SunSDR, [RemoteHamRadio](https://www.remotehamradio.com/), and compatibles). Default `ws://localhost:40001`.
- **Live rig state** — frequency, mode, and PTT reflected in the header in real time.
- **Rig control** — set frequency (click a spot or waterfall tag to QSY), set mode, key/unkey PTT.
- **Audio + spectrum streaming** — pulls the radio's RX stream over TCI to feed the software decoder and waterfall.

## RTTY decode (software)

- **Pure-software AFSK demodulator** written in Rust — two quadrature correlators (mark/space NCOs), matched-filter integration over one bit period, magnitude slicer, and an async-serial start/stop-bit state machine.
- **ITA2 / Baudot decode** with LTRS/FIGS shift handling (fldigi-canonical table).
- **Standard RTTY defaults** — 45.45 baud, 170 Hz shift, tones mark 2125 Hz / space 2295 Hz. Diddle forces the radio into **DIGL (LSB)** on connect, and puts it back whenever something else (the RHR Console, a band change) switches the rig to another mode, which is the usual sideband for amateur RTTY; keep **REV off**. Tones, shift, baud, and reverse are all tunable.
- **AGC + biquad pre-filtering** for clean copy on weak/crowded signals.
- **Decoder view** streams demodulated text as it arrives. Click a callsign to load it; click an exchange word (zone, state, serial, name) to add it to Exch — the first click replaces an empty or history-filled Exch, later clicks append.
- **Steep channel filter** — each decoder mixes its mark/space pair to baseband and applies an 8th-order Butterworth filter, so a station 30–40 dB stronger a few hundred Hz away no longer wipes out copy of a weak one.

## Multi-decoder + band spotting

- **Up to 12 simultaneous decoders** scanning the smoothed spectrum for likely RTTY pairs, each running its own independent demodulator.
- **Automatic callsign extraction** — decoded text from each slot is scanned for plausible callsigns.
- **Clickable waterfall tags** — surfaced callsigns appear as floating tags on the waterfall; click to QSY straight onto the signal.
- **Waterfall as a VFO knob** — mouse wheel over the waterfall (or `←`/`→` when it has focus) tunes the radio 10 Hz per step, 100 Hz with `Shift`, sliding signals under the mark/space markers.
- **Anchored AFC** — pulls the decoder onto a caller within ±15 Hz of your TX mark and snaps back to it each time you unkey, so it can't wander onto a neighbor while you're running. On in Run, off in S&P.
- **SCP-validated spots** — candidate calls can be checked against the Super Check Partial database to cut noise.

## RTTY transmit

- **Software AFSK modulator** (`rtty_tx`) — generates the RTTY tones for transmit over TCI; no FSK keying interface required.
- **Macro-driven sending** with `<MYCALL>`, `<CALL>`, and `<SERIAL>` substitution.
- **TX abort** — `Esc` (or the abort control) stops an in-flight transmission immediately.
- **Live keyboard send** (`Ctrl+K` / `Alt+K`) — keys up on the first keystroke and diddles (LTRS idles) between keystrokes, so the other station hears you at once. *Word* mode sends each word on `Space` (default), *Char* sends each key as typed, *Line* composes a whole line and sends it on `Enter`. `Enter` sends what's left and unkeys; `Esc` aborts; F-keys pressed meanwhile join the same transmission; it unkeys by itself after 30 s with nothing typed. Works against the contest simulator too.

## Macros & keyboard workflow

- **8 editable macros (F1–F8)** with sensible RTTY contest defaults (CQ, exchange, TU, repeat, AGN, BRK, 73…).
- **ESM — Enter Sends Message** (N1MM-style stepped Enter). Run: empty → CQ (F1), call entered → his call + your exchange (F2) and focus jumps to Exch, call+exch → TU (F3) + log. Search & Pounce: empty or call-only → your call (F4), call+exch → your exchange (F2) + log. The ESM chip shows what `Enter` will do next; with ESM off, `Enter` only logs and you move between fields yourself.
- **Frequency in the Call field** — type `14080` (kHz) and press `Enter` to QSY the radio so the signal lands on your mark tone.
- **Call History pre-fill** — load an N1MM+ Call History file and the received exchange is filled in the moment a known call is typed or grabbed (contest-aware: Name + State for NAQP, State for Roundup, Zone + State for CQ WW).
- **Saved contest setups** — contest + F-key messages + call-history file stored under a name; pick one to bring a previous contest's configuration back.
- **In-entry callsign autocomplete** — arrow keys to pick, `Tab`/`Enter` to accept SCP suggestions.
- Macros persist to local storage and merge cleanly when new default slots ship.

## Logbook

- **Persistent QSO log** stored by the Rust backend at `<app_data>/diddle/qsos.json`.
- **Automatic serial numbering** with running next-serial tracking.
- **Per-QSO fields** — call, timestamp (UTC), frequency, band, mode, RST sent/received, exchange sent/received, serial.
- **Band derivation** from frequency across 160 m – 2 m.
- Editable / deletable entries with auto-save.
- **Log It** — `Ctrl+Enter` (or the Log it button) logs Call + Exch as they stand without sending anything, the way out of a botched ESM sequence.
- **Quick edit** — `Ctrl+Q` edits the last QSO's call; `Tab` switches to its received exchange. `↑`/`↓` (or `Ctrl+Q` again) steps to older QSOs.

## Testing without a radio

- **Built-in contest simulator** (Test panel) — a synthetic RTTY band through the real decoder. *Pileup* mode is interactive: CQ brings callers, naming one gets its exchange, TU logs it and brings the next; stations repeat on `AGN?`, correct a mis-copied call, double in a busy pileup, and give up if ignored. *Playback* mode plays a scripted run (RTTY Runner style). Knobs for callers per CQ, HF noise, signal level, tone spread, and background stations elsewhere in the passband (they CQ, work people, and QSY, so spots and the bandmap get exercised). Calls are drawn from SCP; exchanges follow the active contest profile. A truth panel and log show what was really sent. The radio is never keyed while it runs.
- **Audio-device input** — decode any system input device, e.g. a virtual cable (BlackHole on macOS, VB-CABLE on Windows) carrying an external simulator's output, or a sound card fed by a radio.
- **WAV playback** — play a recording through the decoder for offline accuracy checks.

## Contest profiles

Built-in profiles that format your sent exchange and hint the received field:

| Profile | Exchange |
|---------|----------|
| Generic RTTY | RST + Serial |
| CQ WW RTTY DX | RST + CQ Zone (US/VE add State) |
| CQ WPX RTTY | RST + Serial |
| ARRL RTTY Roundup | RST + State/Prov (DX: Serial) |
| NAQP RTTY | Name + State/Prov/Country |

Each profile knows its Cabrillo `CONTEST:` name and builds the sent exchange from your operator settings.

## Exports

- **ADIF** — standard `.adi` with `PROGRAMID: Diddle`, per-QSO RTTY records, and an `APP_DIDDLE_SERIAL` field.
- **Cabrillo 3.0** — contest-ready log with header (callsign, contest, category, name, grid) and `QSO:` lines.

## Super Check Partial (SCP)

- **MASTER.SCP callsign database** for instant partial-call lookup as you type.
- **One-click auto-download** of the latest MASTER.SCP from supercheckpartial.com into the app data dir, with the path persisted for next launch.
- **Spot validation** — confirm a decoded candidate is a known contester before tagging it.

## DX cluster

- **Telnet DX cluster client** (default `dxc.k1ttt.net:7373`).
- **Live spot stream** with TTL pruning, feeding the **bandmap panel**.
- **Click-to-QSY** from cluster spots (DIGL-aware — the dial is set so the spot lands on your mark tone).
- **Cluster command line** under the bandmap for `set/filter`, `sh/dx`, etc., plus persisted **login commands** re-sent every time Diddle connects.
- **Bandmap current-band / all-bands toggle**, with per-band worked status.
- **Docked bandmap** — on windows about 1100 px wide or more, the bandmap sits in a full-height column beside the waterfall and stays on screen while the page scrolls (toggle *dock* in its header). A **you-are-here** marker sits at your operating frequency among the spots and the list keeps it centred as you tune, unless you've just scrolled it yourself.

## Display & tools

- **Spectrum waterfall** with callsign overlay.
- **Tuning scope** — classic crossed-bananas XY display for visually netting on a signal.
- **Collapsible settings panel** for operator + contest configuration.
- **WAV player** — load a recorded `.wav` to test/replay the decoder offline.

## Platform

- **Cross-platform desktop app** (macOS, Windows, Linux) via Tauri 2.
- **Small footprint** — native Rust core, no bundled browser-engine bloat beyond the system WebView.

---

## Roadmap

Rough and subject to change — ideas, not promises:

- [ ] Real-time dupe checking + mult tracking with on-screen needed/worked status
- [ ] Per-band/per-mode score window and rate meter
- [ ] N1MM-compatible / SO2R and second-radio support
- [ ] Configurable RTTY parameters in the UI (baud, shift, tones, USB/LSB)
- [ ] Click-to-decode: spawn a decoder on any waterfall click
- [ ] CW/PSK modes alongside RTTY
- [ ] Log import (ADIF) and merge
- [ ] Code signing / notarization for macOS & Windows installers
- [ ] Skimmer integration
- [ ] In-app screenshots & docs

Have a request? [Open an issue](https://github.com/WW2DX/diddle/issues).
