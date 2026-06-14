# Diddle

**A RTTY contest logger — TCI-first, rate-focused.**

Diddle is a desktop RTTY contest logger built around [TCI](https://github.com/maksimus1210/TCI) software-defined radios (ExpertSDR / SunSDR and compatibles). It decodes and transmits RTTY entirely in software, runs a multi-channel decoder across the whole waterfall, and is tuned for one thing: keeping your QSO rate high during a contest.

> "Diddle" is the idle stream of LTRS a RTTY transmitter sends between characters to hold sync. Fitting name for a logger that lives in the RTTY sub-bands.

[![Release](https://img.shields.io/github/v/release/WW2DX/diddle?include_prereleases&sort=semver)](https://github.com/WW2DX/diddle/releases)
[![Build](https://github.com/WW2DX/diddle/actions/workflows/release.yml/badge.svg)](https://github.com/WW2DX/diddle/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

---

## Why Diddle?

Most RTTY contesting setups glue together MMTTY/2Tone + N1MM + a FSK keying interface. Diddle collapses that whole chain into a single app that talks to your TCI radio over one WebSocket:

- **No external decoder, no soundcard routing, no FSK interface.** RTTY demod and modulation happen in Rust, driven by the radio's IQ/audio stream over TCI.
- **The whole band, decoded at once.** A multi-decoder watches the waterfall and surfaces callsigns as clickable tags — click to QSY and work them.
- **Built for rate.** N1MM-style ESM (Enter Sends Message), Super Check Partial, DX cluster spots, and contest-aware exchange macros keep your hands on the keyboard.

See **[FEATURES.md](FEATURES.md)** for the full feature list.

---

## Screenshots

> _Coming soon._ (Header / waterfall / multi-decoder / entry window / logbook.)

---

## Install

Pre-built installers for **macOS, Windows, and Linux** are attached to each [GitHub Release](https://github.com/WW2DX/diddle/releases).

| OS | Download |
|----|----------|
| macOS | `.dmg` (Apple Silicon + Intel) |
| Windows | `.msi` or NSIS `.exe` |
| Linux | `.AppImage`, `.deb`, or `.rpm` |

These artifacts are produced by CI on each platform's native runner — see [`.github/workflows/release.yml`](.github/workflows/release.yml). Diddle is not code-signed yet, so:

- **macOS:** right-click the app → **Open** the first time (Gatekeeper).
- **Windows:** click **More info → Run anyway** on the SmartScreen prompt.

To build from source instead, see **[BUILD.md](BUILD.md)**.

---

## Quick start

1. **Start your TCI radio software** (ExpertSDR2/3, SunSDR, etc.) and enable the **TCI server** (default `ws://localhost:40001`).
2. **Launch Diddle**, type your TCI URL in the header, and click **Connect**. The frequency/mode readout should come alive.
3. **Open Settings** (bottom panel) and fill in your operator info — callsign, name, state/province, CQ zone, grid — and pick your **contest**.
4. **Tune to a RTTY signal.** The decoder view shows demodulated text; the waterfall shows callsign tags from the multi-decoder.
5. **Work stations.** With **ESM** on, `Enter` steps through CQ → exchange → TU + log automatically.

---

## Operating workflow (ESM)

Diddle uses N1MM-style **Enter Sends Message** stepping in the entry window:

| Entry state | `Enter` does |
|-------------|--------------|
| Empty form | Fire **F1** (CQ) |
| Callsign entered | Fire **F2** (exchange) and jump to the Exch field |
| Call + exchange filled | Fire **F3** (TU) and **log the QSO** |

The ESM chip in the entry window always shows what the next `Enter` will do. `Esc` aborts an in-flight transmission. The `F1`–`F8` macros are fully editable in Settings and support `<MYCALL>`, `<CALL>`, and `<SERIAL>` substitution.

---

## Tech stack

- **Frontend:** [SvelteKit](https://kit.svelte.dev/) + Svelte 5 (runes), TypeScript, Vite, `adapter-static`.
- **Shell + backend:** [Tauri 2](https://v2.tauri.app/) with a Rust core (`tokio` async).
- **DSP (Rust):** software RTTY demodulator/modulator, AGC, biquad filtering, spectrum/scope generation, multi-decoder.
- **Radio I/O:** TCI WebSocket protocol client.

---

## Project layout

```
src/                     SvelteKit frontend
  lib/
    Header.svelte        TCI connect bar + rig readout
    Waterfall.svelte     spectrum waterfall + callsign tags
    DecoderView.svelte   demodulated RTTY text
    EntryWindow.svelte   call/exchange entry, ESM logic
    FKeys.svelte         F1–F8 macro buttons
    Logbook.svelte       QSO table + ADIF/Cabrillo export
    BandmapPanel.svelte  DX cluster spots / bandmap
    SettingsPanel.svelte operator + contest config
    TuningScope.svelte   crossed-bananas XY scope
    contests.ts          contest profiles + exchange builders
    exports.ts           ADIF + Cabrillo formatters
    macros.svelte.ts     macro store + expansion
    *.svelte.ts          rune-based state stores (qsoLog, settings, spots, cluster)
src-tauri/               Rust backend (Tauri)
  src/
    tci/                 TCI protocol + client
    dsp/                 rtty, rtty_tx, agc, biquad, scope, spectrum, multi_decoder
    cluster.rs           DX cluster telnet client
    scp.rs               Super Check Partial database
    log_storage.rs       QSO log persistence
    wav_player.rs        WAV playback for decoder testing
    ipc.rs               Tauri command handlers
.github/workflows/       CI: cross-platform release builds
```

---

## Contributing

Issues and PRs welcome — see **[CONTRIBUTING.md](CONTRIBUTING.md)**. Diddle is early (v0.1) and the feature surface is moving quickly; check the [roadmap](FEATURES.md#roadmap) before starting big work.

## License

[MIT](LICENSE) © 2026 WW2DX

---

*73 and good contest. — WW2DX*
