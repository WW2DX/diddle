# Changelog

All notable changes to Diddle are documented here. Format loosely follows
[Keep a Changelog](https://keepachangelog.com/); versions follow [SemVer](https://semver.org/).

## [Unreleased]

## [0.1.4] — 2026-06-17

### Fixed
- Clickable callsigns now work everywhere in the decoder window — with the noise filter off and in transmitted (TX echo) text — not just in filtered RX lines. Callsigns are detected at render time across all displayed text.

## [0.1.3] — 2026-06-16

### Added
- Click-to-log callsigns: decoded callsigns in the RX window are highlighted and clickable — clicking one loads it into the entry form's Call field (and focuses Exch), ready to work.
- Search & Pounce ESM mode alongside Run. A RUN/S&P toggle in the entry header switches the Enter-key stepping: S&P sends your call (F4), then your exchange (F2), then TU + log; Run is unchanged (CQ → exchange → TU + log). The mode is persisted, and the ESM chip shows what Enter will do in the current mode.
- General QSO (ragchew) contest profile for non-contest operating — the received exchange is optional, so you can log a contact with just a callsign.
- Waterfall scroll-speed control (Fast/Med/Slow/Slowest). Slower speeds peak-hold the frames between rows so brief signals aren't lost.

### Changed
- Waterfall rendering quality: columns now peak-aggregate when zoomed out (so narrow RTTY carriers aren't skipped) and linearly interpolate when zoomed in (smooth image instead of blocky bars), with a smoothstep contrast curve for cleaner separation of signal from noise.

## [0.1.2] — 2026-06-16

### Added
- Scrollable decode history: the RX window keeps decoded text as it scrolls off, with a resizable pane (drag the bottom edge) and a scrollbar to read back. Scrolling up pauses auto-scroll and shows a "↓ latest" button; returning to the bottom resumes it.
- Configurable decode history length (in lines) — set how many decoded lines are retained before the oldest scroll off for good. Persisted across sessions; defaults to 1000 lines.

## [0.1.1] — 2026-06-15

### Added
- Live TX echo: transmitted characters now appear in the decoder window as their tones go on the air (paced to the TX audio, shown in a distinct color), instead of only after the transmission completes.
- Force the radio into DIGL on connect and before every transmit, so AFSK RTTY always lands on the expected sideband without the operator setting the mode by hand.

### Fixed
- Real-time RX decode: with the noise filter on, decoded text now prints character-by-character as it arrives (with a live caret) instead of only appearing once a full CR/LF line completed. Completed lines are still scored and junk is dropped.

## [0.1.0] — 2026-06-14

Initial public release.

### Added
- TCI WebSocket client with live rig state and frequency/mode/PTT control.
- Software RTTY demodulator (45.45 baud / 170 Hz shift, high tones) with ITA2/Baudot decode, AGC, and biquad pre-filtering.
- Multi-decoder (up to 12 slots) with automatic callsign extraction and clickable waterfall tags.
- Software RTTY transmit with editable F1–F8 macros and `<MYCALL>`/`<CALL>`/`<SERIAL>` substitution.
- N1MM-style ESM (Enter Sends Message) entry workflow with in-line SCP autocomplete.
- Persistent logbook with automatic serial numbering and band derivation.
- Contest profiles: Generic RTTY, CQ WW RTTY, CQ WPX RTTY, ARRL RTTY Roundup, NAQP RTTY.
- ADIF and Cabrillo 3.0 export.
- Super Check Partial database with one-click MASTER.SCP auto-download.
- DX cluster telnet client with bandmap, TTL-pruned spots, and click-to-QSY.
- Spectrum waterfall, crossed-bananas tuning scope, and WAV player for offline decoder testing.
- Cross-platform installers (macOS, Windows, Linux) built in CI.

[Unreleased]: https://github.com/WW2DX/diddle/compare/v0.1.4...HEAD
[0.1.4]: https://github.com/WW2DX/diddle/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/WW2DX/diddle/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/WW2DX/diddle/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/WW2DX/diddle/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/WW2DX/diddle/releases/tag/v0.1.0
