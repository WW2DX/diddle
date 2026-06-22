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
- **Standard RTTY defaults** — 45.45 baud, 170 Hz shift, tones mark 2125 Hz / space 2295 Hz. Diddle forces the radio into **DIGL (LSB)** on connect, which is the usual sideband for amateur RTTY; keep **REV off**. Tones, shift, baud, and reverse are all tunable.
- **AGC + biquad pre-filtering** for clean copy on weak/crowded signals.
- **Decoder view** streams demodulated text as it arrives.

## Multi-decoder + band spotting

- **Up to 12 simultaneous decoders** scanning the smoothed spectrum for likely RTTY pairs, each running its own independent demodulator.
- **Automatic callsign extraction** — decoded text from each slot is scanned for plausible callsigns.
- **Clickable waterfall tags** — surfaced callsigns appear as floating tags on the waterfall; click to QSY straight onto the signal.
- **SCP-validated spots** — candidate calls can be checked against the Super Check Partial database to cut noise.

## RTTY transmit

- **Software AFSK modulator** (`rtty_tx`) — generates the RTTY tones for transmit over TCI; no FSK keying interface required.
- **Macro-driven sending** with `<MYCALL>`, `<CALL>`, and `<SERIAL>` substitution.
- **TX abort** — `Esc` (or the abort control) stops an in-flight transmission immediately.

## Macros & keyboard workflow

- **8 editable macros (F1–F8)** with sensible RTTY contest defaults (CQ, exchange, TU, repeat, AGN, BRK, 73…).
- **ESM — Enter Sends Message** (N1MM-style stepped Enter): empty → CQ, call entered → exchange, call+exch → TU + log. The ESM chip shows what `Enter` will do next.
- **In-entry callsign autocomplete** — arrow keys to pick, `Tab`/`Enter` to accept SCP suggestions.
- Macros persist to local storage and merge cleanly when new default slots ship.

## Logbook

- **Persistent QSO log** stored by the Rust backend at `<app_data>/diddle/qsos.json`.
- **Automatic serial numbering** with running next-serial tracking.
- **Per-QSO fields** — call, timestamp (UTC), frequency, band, mode, RST sent/received, exchange sent/received, serial.
- **Band derivation** from frequency across 160 m – 2 m.
- Editable / deletable entries with auto-save.

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
- **Click-to-QSY** from cluster spots.
- **Raw cluster console** for sending commands and watching the line log.

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
- [ ] Telnet cluster filters and skimmer integration
- [ ] In-app screenshots & docs

Have a request? [Open an issue](https://github.com/WW2DX/diddle/issues).
