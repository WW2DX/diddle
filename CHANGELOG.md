# Changelog

All notable changes to Diddle are documented here. Format loosely follows
[Keep a Changelog](https://keepachangelog.com/); versions follow [SemVer](https://semver.org/).

## [Unreleased]

## [0.1.28] — 2026-10-05

### Added
- **Stack macros: `<CRLF>`, `<LOGIT>`, `<POPSTACK>`.** Tokens run left to right as the macro goes out: `<CRLF>` starts a new line, `<LOGIT>` logs the QSO in the entry form, and `<POPSTACK>` loads the next stacked caller, so a later `<CALL>` is him and a later `<SERIAL>` is the next number. They work from an F-key pressed by hand as well as from ESM.
- **Stack box** under the entry fields in Run: the callers you've right-clicked, in order. Click one to drop it; *clear* empties it.
- **Font list from the computer.** Each window's font is now picked from every font installed on the computer, fixed-pitch fonts first, and each one is shown in its own typeface.
- **Test button for the new-multiplier chime** in Settings.
- **Squelch** (SQ slider, RX decoder): how clean a signal must look before the RX window prints it. 0 is wide open; turn it up until the noise between signals stops printing. The default is 30 and the setting is remembered. It works on tone contrast (how much more of the energy is in one tone than the other) rather than level, so it behaves the same at any audio level. The bandmap's spot decoder isn't squelched.
- **Radio RX filter: wide / 500 / 250** (RX decoder, over TCI). 500 and 250 are centred on the mark/space pair and follow it as you retune. *wide* puts back the filter the radio had before. A narrow filter also hides the rest of the band from the waterfall and bandmap, so it isn't remembered between launches.
- **Radio AGC: norm / fast / off** (RX decoder, over TCI), with the receiver gain in dB shown when AGC is off. Diddle reads the radio's filter and AGC on connect, so the buttons show what the radio is set to.
- **Call history: "Exch from" column** (Settings → Call history). *auto* keeps each contest's own fields and, for a contest with none (Generic, or one Diddle has no profile for, such as URC DX), uses `EXCH1`. A file headed `!!Order!!,Call,Exch1,UserText` then pre-fills Exch for any contest. Saved contest setups remember the choice.

### Changed
- **Stack (NEXT) flow redone** after feedback from the field. Right-click a caller into an empty Call and focus stays in Call, so Enter sends him the exchange. Right-click more callers and they go on the stack, and keyboard focus comes back to the entry window, so Enter still works the QSO you're on. At the TU step with callers stacked, ESM sends the stack-TU macro, **F8** by default (Settings → F-key macros): `TU <CALL><LOGIT> NOW<CRLF><POPSTACK><CALL> 599 <SERIAL> 599 <SERIAL>`. That's TU, log, then the next caller and your exchange, all in one transmission. The next Enter is the ordinary F3 TU. If the chosen macro has no `<POPSTACK>`, ESM sends TU, logs, then F2 to the next caller. **Existing setups keep their own F8**, so program it as above, or pick a different slot in Settings.
- **The new-multiplier chime is now a bell**: two rising strikes that ring out for over a second, much louder than the old blip.
- **"Slashed 0" is now "force slashed 0"**, and its tooltip explains what it does. It turns on a font's slashed-zero variant where the font has one. Unticking it can't take the slash out of a font whose zero is always slashed, such as Consolas (Windows' default monospace) or Menlo.

- **CQ WW zone prediction uses the history file's state for US stations.** The call area is only a guess: a W4 in Kentucky, Tennessee or Alabama is zone 4, a W8 in West Virginia zone 5, a W7 in Montana or Wyoming zone 4.
- **The Entry font size now applies to the Call field too.** It stays a size larger than the other fields.
- **Bandmap columns widen with the font size**, so large fonts no longer overlap.

### Fixed
- **The RX window's noise gate never closed on live radio audio.** The decoder only started a character when the signal stood out from its noise-floor estimate. But when audio began with band noise (the usual case), that estimate stuck near zero and the gate stayed open, so noise printed as random characters. The new squelch replaces it.
- **Right-clicking a caller took keyboard focus away from the entry window.** The next Enter then pressed the call that had just been right-clicked and replaced the station you were working.
- **The contest simulator counts a stack TU as a finished QSO.** Before, it reported that the station had "left without a TU".

## [0.1.27] — 2026-10-01

### Added
- **Multipliers and score.** Diddle now ships AD1C's country file (`cty.dat`) and knows every call's country, CQ/ITU zone and continent; update it from country-files.com or load your own `cty.dat` / `WL_CTY.DAT` in Settings.
  - **New-multiplier highlighting:** calls that would be a new mult on their band show purple in the RX window, on the waterfall, in the bandmap and in the Call field, with a `NEW: Zone 14 · Ukraine` tag as you enter them. Logging one flashes `NEW MULT` and plays a chime (Settings can turn the chime off).
  - **Score panel:** QSOs, points and multipliers by band, totals and claimed score for CQ WW RTTY, ARRL RTTY Roundup, NAQP RTTY and CQ WPX RTTY.
  - **Zone prediction:** in CQ WW, Exch is filled with the zone from the country file — call-history files don't carry zones — plus the history state when there is one. A DX station's exchange is there before he sends it.
- **Contest-format exports.** Cabrillo `QSO:` lines follow each contest's columns (CQ WW: RST, zone, state/province or `DX`) with the computed `CLAIMED-SCORE`; ADIF puts each exchange part in its own field (`CQZ`, `STATE`/`VE_PROV`, `SRX`, `NAME`, `PFX`, `CONTEST_ID`…) for N1MM+ and WriteLog import.
- **Chained F-keys.** F-keys pressed while one is on the air queue onto the same transmission instead of being refused.
- **NEXT queue.** Right-click callers in the RX window or on the waterfall to queue them; the first goes straight into an empty Call. In Run, ESM's TU step sends TU, logs, loads the next caller and sends him the exchange — one Enter per QSO, in one transmission.
- **Shift+F1–F8:** a second bank of eight macros (QRZ, AGN, CALL?, NR?, EXCH?, QSL, QRL?, TEST).
- **1 kHz tuning steps** on the waterfall: Alt/Option + wheel or arrows, or PgUp/PgDn.
- **Per-window fonts:** family, size and slashed zero for the RX decoder, Entry, Bandmap and Log windows (Settings → Display fonts).
- **Resizable docked bandmap:** drag its left edge; the width is remembered.

## [0.1.26] — 2026-09-29

### Added
- **The TCI server address is remembered.** Once Diddle connects, the address is saved and filled in on the next launch, so a radio on a non-default port (for example `ws://127.0.0.1:50005`) no longer has to be retyped every time. An address that fails to connect is never saved over a working one.

## [0.1.25] — 2026-09-29

### Fixed
- **No transmit audio on SunSDR / ExpertSDR** — the radio keyed up but sent nothing ([#3](https://github.com/WW2DX/diddle/issues/3)). Diddle keyed up with `trx:0,true,vac`, which RemoteHamRadio's server needs, but the TCI spec (and ExpertSDR) only takes a program's audio when keyed with `trx:0,true,tci` — otherwise it transmits the selected microphone. Diddle now reads the server's `protocol:` announcement on connect and, for ExpertSDR, follows the spec: keys with `tci`, asks for a 48 kHz float32 mono audio stream (in DIGL/DIGU a two-channel stream is I/Q, not left/right), and labels its TX audio correctly — float32 as format 3 and the channel count filled in ([#1](https://github.com/WW2DX/diddle/issues/1)). RemoteHamRadio behaves exactly as before.
- **Dead carrier when the radio never asks for audio.** If the radio keys up but hasn't requested any TX audio within 2.5 s, Diddle now unkeys and says so, instead of holding PTT for up to a minute.

## [0.1.24] — 2026-09-29

### Changed
- **Ad-hoc send keys up the moment the window opens** (Word and Char modes), diddling until you type — the other station hears you straight away instead of after your first character or word. Line mode still waits for Enter. `Esc` before you've sent anything aborts *and* closes the window, so an accidental `Alt+K` is one keystroke to undo.

### Fixed
- **Sending TU by hand (F3 key or button) didn't log the QSO** — only the ESM Enter step did. In Run with ESM on, a TU that goes out now logs the contact the same way. If Exch is empty (easy to miss with CQ WW DX stations, whose exchange is only the zone), the entry window says *TU sent — NOT logged: Exch is empty* and puts you in Exch, instead of silently not logging.
- **"DE" (Delaware) and "SK" (Saskatchewan) couldn't be clicked into Exch** from the RX window — they were ignored as the operating words "this is" and "end of contact". A click on either now adds it to the exchange.

## [0.1.23] — 2026-09-28

### Changed
- **Ad-hoc send (`Ctrl+K`) now goes on the air as you type.** The first keystroke keys up and the rig diddles (LTRS idles) until there's text, so the other station knows at once you're still there — no more composing a line in silence and waiting half a second for a lead-in after Enter. Three modes, picked in the popup and remembered:
  - **Word** (default) — each word goes out when you press Space; you can fix a word until then.
  - **Char** — each key goes out as you type it (Backspace can't recall it).
  - **Line** — the old behaviour: compose, Enter sends the whole line.

  `Enter` sends whatever is left and unkeys once it's out; `Esc` aborts on the spot. An F-key pressed during a live send joins the same transmission instead of being refused. If nothing is typed for 30 s the rig unkeys by itself. Works against the contest simulator as well as the radio.

## [0.1.22] — 2026-09-28

### Added
- **Click exchange words in the RX window.** Clicking a received word that isn't a callsign (zone, state, serial, name) adds it to Exch: the first click replaces an empty or call-history-filled Exch, later clicks append (`05`, then `NY` → `05 NY`), and a word already there isn't doubled. `599`, `TU`, `CQ` and the like are ignored, as is your own TX echo.
- **Docked bandmap.** On a window about 1100 px wide or more, the bandmap moves to a full-height column beside the waterfall that stays on screen while you scroll, so reading down a busy band no longer means scrolling away from the waterfall. The *dock* button in its header puts it back below the F-keys.
- **You-are-here marker in the bandmap** at your operating frequency, in frequency order with the spots. The list keeps it centred as you tune (unless you've scrolled the list in the last few seconds).

### Changed
- **Much better copy of a weak station with a strong one in the passband.** The decoders' front end was a single gentle bandpass, so a big signal a few hundred Hz away leaked in and lifted the squelch floor over the weak one. Each decoder (the main one and all 12 scanning slots) now mixes its mark/space pair down to baseband and applies an 8th-order Butterworth filter. On a test signal 30 dB under a neighbour 400 Hz away, copy went from about a third of the characters to all of them; 40 dB under a neighbour 600 Hz away, from 14 % to all. Weak-signal copy in plain noise is unchanged. A neighbour closer than ~300 Hz still overlaps the tones and can't be filtered out.

### Fixed
- Ctrl/Cmd + mouse wheel over the waterfall zoomed the UI *and* retuned the radio. It now only zooms.

## [0.1.21] — 2026-09-27

### Added
- **Log It: `Ctrl+Enter`** logs whatever is in Call and Exch right now, sending nothing — the same as Enter with ESM off, from any focus. The Log button (now labelled *Log it · Ctrl+↵*) does the same. It's the way out when an ESM sequence gets out of step and the QSO would otherwise be lost.
- **Correct the last exchange from `Ctrl+Q`.** `Tab` in the quick-edit popup switches between the logged call and the received exchange; `↑`/`↓` still steps to older QSOs.
- **Tune from the waterfall like the radio's dial.** Mouse wheel over the waterfall retunes the rig 10 Hz per step (100 Hz with `Shift`); wheel up is dial up. With the waterfall focused, `←`/`→` slide the signals left/right.

### Fixed
- **AFC walked away from your run frequency** before anyone called, following a neighbor or splatter up to 40 Hz per tick and never coming back. AFC now only looks within ±15 Hz of your TX mark and snaps the decoder back to it every time you unkey, so it still pulls in a caller who is a few Hz off without drifting onto the station next door.
- **The rig could sit in USB or DIGU after the RHR Console changed the mode** (spectrum clicks, band changes), swapping mark and space so the decoder printed garbage until your next transmission. Diddle now puts DIGL back as soon as the rig reports another mode (at most once every 2 s, so a radio that refuses can't ping-pong).

## [0.1.20] — 2026-09-26

### Fixed
- **ESM in Search & Pounce sent your call again instead of your exchange.** After a blind Enter on an empty Call field (F4), typing his call reset the sequence on every keystroke after the first, so Enter with his call and exchange filled in sent F4 a second time. The sequence now restarts only when the Call field is changed to a callsign other than the one you have already transmitted under; typing into an empty field, or grabbing a call from the decoder after a blind call, continues it. S&P is two Enters again: empty Call → your call; his call and exchange in → your exchange (F2) and log.

## [0.1.19] — 2026-09-25

### Fixed
- **ESM in Search & Pounce did nothing on Enter with an empty Call field.** It now sends your call (F4), N1MM-style, so you can answer a CQ before you've copied who it is; focus stays in Call for typing his when he comes back, and the next Enter with his call and exchange filled in sends your exchange and logs. Typing a call into an empty field no longer restarts the sequence, so that blind call isn't sent twice. The phase chip reads `S&P · ↵ Call` from the start instead of "enter a call".

## [0.1.18] — 2026-09-25

### Fixed
- **ESM in Run mode sent TU instead of your exchange** when the Exch field was pre-filled from the call-history file (CQ WW zone/state, NAQP name/state) — so the first Enter thanked a station you had said nothing to. The Run steps now follow what you have actually sent, not whether Exch happens to hold something. A bare Enter while you're still waiting for his exchange re-sends yours, the way an F-key repeat would.

### Changed
- **ESM in Search & Pounce is two steps now**, N1MM-style: Enter sends your call (and again on each Enter until he comes back), then — once you've copied his exchange — Enter sends your exchange and logs the QSO. The old third step sent TU, which in S&P is the running station's to send, and did it with F3, whose default ends in `CQ` — calling CQ on his run frequency. The phase chip reads `S&P · ↵ Call` then `S&P · ↵ Excg+Log`.

## [0.1.17] — 2026-09-21

### Fixed
- Simulator pileup: the station you just worked could start repeating its exchange in the middle of your TU, so it was still on the air when your macro ended. Patience timeouts now count *silence* on the QSO frequency — they no longer run down while the other station is sending its exchange or while your PTT is down — and no caller or worked station keys up while you are transmitting. Hesitating a second or two before TU is fine again.
- Macro tokens are case-insensitive: `<serial>` now expands the same as `<SERIAL>` instead of being sent literally. Unknown tokens are still passed through as typed.

## [0.1.16] — 2026-09-17

### Changed
- Simulator playback mode is listen-only: F-keys and ESM no longer key up, echo, or mute the decoder. The scripted run plays both sides, so there is nothing for you to send; just copy and log.

## [0.1.15] — 2026-09-16

### Fixed
- Simulator: a transmission never returned to RX (PTT stayed on until Esc). The TX length was miscounted and scheduled for hours; it is now computed exactly.

## [0.1.14] — 2026-09-16

### Fixed
- macOS asked for microphone access the moment the Test panel opened, and the prompt could refuse to dismiss. Diddle now asks explicitly (through AVFoundation) only when you scan or start an audio input device, never for the simulator or WAV playback, and reports clearly when access has been denied.

## [0.1.13] — 2026-09-16

### Added
- **Built-in contest simulator** (Test panel at the bottom of the window). Synthesizes a RTTY band and feeds it through the real decoder, so every feature — waterfall, multi-decoder spots, bandmap, entry window, ESM, logging — can be exercised with no radio and no on-air activity. Two modes:
  - **Pileup** (interactive): press F1 and stations answer your CQ; type one's call, send the exchange, and it sends its exchange back; TU completes the QSO and brings the next caller. Stations repeat on `AGN?`, correct you when you copy a call wrong, double over each other in a busy pileup, and walk away if ignored. Your F-keys never key the radio while the simulator runs (the header shows **SIM**).
  - **Playback** (scripted): both sides of a run play back-to-back, RTTY Runner style, for pure decoder calibration.
  - Knobs: callers per CQ, HF noise (atmospheric + QRM + splatter + static, ported from EC5W's RTTY Runner), signal level, caller tone spread, and up to eight background stations elsewhere in the passband that CQ, work people, and QSY over time — so the bandmap and spot tags have something to chew on. Callsigns come from the Super Check Partial database so spots pass the SCP filter; the contest exchange follows the active contest profile (serial, zone + state, state, name + state).
  - A truth panel shows what each simulated station really sent, and a running log of every simulated transmission, to compare against the decoder.
  - A pretend dial (band selectable) is shown in the header so QSOs log with a band.
- **Audio-device input** (Test panel): decode from any system input device — a sound card fed by a radio, or a virtual cable such as BlackHole (macOS) / VB-CABLE (Windows) carrying another program's output. This is how to run an external simulator like RTTY Runner into Diddle: point its output at the virtual cable and select the cable here. Shows a level meter. macOS will ask for microphone permission the first time.

### Changed
- The WAV player, audio input, and simulator share one RX pipeline (`RxPipeline`), so they behave identically to live TCI audio; starting one test source stops the others and pauses the TCI RX stream.

## [0.1.12] — 2026-09-15

### Added
- **Dupe coloring everywhere a call can be picked.** A station already logged on the current band turns red: the Call field as you type it, decoded callsign chips in the RX pane, waterfall labels (struck through), and bandmap rows (previously a grey that read like an expired spot).
- **Clear** button on the bandmap: drops every cluster and decoder spot and hides worked-station rows logged before the clear. The log itself is untouched and new spots keep arriving.
- **Nudge arrows** (◀ ▶) under the tuning scope step the mark 5 Hz at a time; hold to repeat. Moves RX and TX together like a waterfall click.
- The mark/space markers on the waterfall are now always clearly visible: 2 px lines with a dark halo, plus a shaded strip between the tones (red while keyed) so you can always see where the next transmission will land.

### Changed
- **AFC follows Run / S&P**: on when running (pulls slightly off-frequency callers onto the decoder), off when pouncing. The checkbox still overrides until the next mode change.
- **Snap to peak** now defaults off. Clicking a signal that had just stopped transmitting let the snap window pick noise or a neighbor and land the mark off frequency; clicks now go exactly where you put them.

### Fixed
- Clicking a cluster or decoder row in the bandmap now loads the callsign into the Call field as well as tuning the radio (it previously only tuned).
- Every plausible callsign in the RX pane is clickable, not only ones SCP knows: calls with a digit-first prefix (4X1ABC, 9A1A, 3DA0RU, 2E0ABC) were never turned into chips.

## [0.1.11] — 2026-09-01

### Added
- **2125** reset button in the RX decoder header (shown whenever the mark tone is off-standard): one click snaps mark back to the RTTY-standard 2125 Hz for both RX and TX, so "meet me on 21.090" by dial means the same thing it does for MMTTY-style stations.
- The header now shows the radio dial alongside the mark-tone RF (e.g. `21.088.102 · dial 21.090.000`) so the ~2 kHz DIGL offset between "where my signal is" and "what the radio reads" is self-explanatory.

## [0.1.10] — 2026-08-31

### Added
- The app version is shown next to the program name in the top-left header (e.g. `Diddle v0.1.10`), so it's always clear which build is running.
- **NET** checkbox next to AFC (visible while AFC is on): when checked, TX follows the AFC-tracked RX frequency (the old behavior); default off.
- When AFC has moved the decoder away from the TX frequency, a red **TX** marker on the waterfall shows where the signal will actually go out.

### Changed
- **AFC no longer moves your transmit frequency** (MMTTY-style AFC/NET split). AFC tracks only the decoder; the transmitted signal — and the frequency shown in the entry strip and written to the log — stays where you deliberately tuned (waterfall click, auto-tune, typed QSY, bandmap/spot click, each of which re-aligns RX with TX).
- AFC now holds still on dead air: it only follows a peak that stands well clear of the band noise, so clicking a spot whose station has stopped transmitting no longer lets AFC walk onto noise or a neighbor. It also ignores your own signal while transmitting.
- The header's big frequency readout now shows the mark-tone RF (what gets logged), matching the entry strip; hover for the raw dial. It previously showed the bare dial, which reads ~2 kHz high in DIGL.

### Fixed
- Transmitted text no longer comes out garbled in the RX pane: the RX decoder used to keep running during TX, so monitor/loopback audio was decoded and interleaved character-by-character with the TX echo. Decoding now pauses from key-down until shortly after unkey (the waterfall keeps scrolling), and with the noise filter on, TX echo is sequenced behind any RX line still being classified so the scrollback matches the on-air order.
- Clicking a waterfall callsign label no longer loads a neighboring call when two labels overlap — the click resolves to the spot nearest the pointer.
- Cluster spots dedupe per call *and* band, so a station spotted on two bands keeps both rows in the all-bands bandmap.

## [0.1.9] — 2026-08-27

### Added
- DX cluster command line under the bandmap (`dx›`): type any cluster command (e.g. `set/filter mode rtty`, `sh/dx 10`) and Enter sends it; ↑/↓ recalls history and the last few cluster replies are shown beneath. Settings has a "commands sent after login" box so filters are re-applied automatically every time Diddle connects.
- Bandmap **all bands / current band** toggle. All-bands mode shows spots from every band (with a band column) so mults elsewhere and band activity are visible before you QSY; worked-status stays per band. Current band only remains the default.
- Type a frequency into the Call field (kHz, e.g. `14080` or `14080.5`) and press Enter to QSY the radio there, N1MM/WriteLog style. The dial is set so the signal lands on your mark tone. The ESM chip shows `↵ QSY 14.080.000` while a frequency is typed.
- N1MM+ **Call History** file support (Settings → Call history → Load): `!!Order!!,Call,Name,State,…` files are parsed and, when a known call is typed or grabbed, the received exchange is pre-filled (tinted until you edit it). Which fields form the exchange depends on the contest — Name + State for NAQP, State for Roundup, Zone + State for CQ WW; serial contests are never guessed. The file path is remembered and reloaded on launch.
- **Saved contest setups** (Settings): save the current contest + F-key messages + call-history file under a name, and pick it from a dropdown later to bring everything back. Edits made while a setup is active are written back to it.

### Changed
- The ESM chip in S&P mode with an empty Call field now reads "S&P · enter a call" (was "grab a call", which read like a control).
- The waterfall's right-hand frequency label and mode hint now follow the actual sideband (DIGL reads high → low across the display).

### Fixed
- Frequency handling now consistently accounts for DIGL (LSB): the mark tone sits *below* the dial, so
  - clicking a bandmap row, waterfall tag, or cluster spot now tunes the dial to `spot + mark` (was `spot − mark`, landing ~4 kHz low);
  - decoder spots are placed at `dial − audio` on the bandmap and waterfall (were `dial + audio`);
  - the frequency shown in the Entry header and written to the log/ADIF/Cabrillo is the mark-tone RF (`dial − mark`) rather than the bare dial, matching what other loggers and the cluster report.

## [0.1.8] — 2026-08-14

### Fixed
- The Ctrl+Q quick-edit popup now opens with the cursor at the end of the call (nothing pre-selected), so a single wrong character can be fixed with backspace/arrows instead of retyping the whole call.

## [0.1.7] — 2026-08-14

### Added
- Entry keyboard navigation: Space toggles between the Call and Exch fields (from Exch it jumps when the field is empty, and always from RST, so exchanges with literal spaces like NAQP's "JOHN MA" still type normally); Tab / Shift+Tab now cycle strictly within Call → RST → Exch instead of wandering off to buttons.
- Log/entry shortcuts: Ctrl/Alt+W wipes the entry fields, Alt+U toggles Run / S&P, Ctrl+D deletes the most recent QSO (press twice within 3 s to confirm; its serial is freed for reuse), Ctrl+N attaches a note to a logged QSO (shown as ✎ in the log, exported as ADIF COMMENT), and Ctrl+Q quick-edits a logged callsign. In the note/edit popups ↑/↓ (or the shortcut again) steps back through earlier QSOs.

## [0.1.6] — 2026-08-13

### Added
- Ad-hoc keyboard send (WriteLog Alt-K / N1MM Ctrl-K style): Ctrl/Alt/Cmd-K opens a popup where you type free text and Enter transmits it. Macro tokens (`<MYCALL>`, `<CALL>`, `<SERIAL>`) expand, ↑/↓ recalls previous sends, ESC aborts an in-flight TX or closes the window. The window stays open for repeated sends.

## [0.1.5] — 2026-06-22

### Added
- UI zoom: Cmd/Ctrl with +/−/0 (and Cmd/Ctrl + scroll) scales the whole interface so the essential panels fit a laptop screen. The zoom level is persisted.
- Transmit indicator on the waterfall: while keyed, the mark/space markers turn red, the tone band is shaded, and a "● TX" flag shows — making it clear TX uses the same tones/orientation as RX.

### Changed
- The noise filter now defaults **off**, so the decoder window shows the raw real-time stream as it arrives instead of only committing whole lines on CR/LF (which read as delayed, several-seconds-late bursts). Turn it on to show only lines with a known call/marker.
- Clarified that Diddle uses **DIGL (LSB)** for RTTY (forced on connect) with REV off — corrected docs and the waterfall hint that previously implied USB.

### Fixed
- Clicking a waterfall callsign label or cluster spot now loads the call into the entry form (and QSYs), not just retunes.
- `<CALL>` now expands in F-key macros even with ESM off — the entry form's current Call is shared so manually-fired macros resolve it.

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

[Unreleased]: https://github.com/WW2DX/diddle/compare/v0.1.16...HEAD
[0.1.16]: https://github.com/WW2DX/diddle/compare/v0.1.15...v0.1.16
[0.1.15]: https://github.com/WW2DX/diddle/compare/v0.1.14...v0.1.15
[0.1.14]: https://github.com/WW2DX/diddle/compare/v0.1.13...v0.1.14
[0.1.13]: https://github.com/WW2DX/diddle/compare/v0.1.12...v0.1.13
[0.1.12]: https://github.com/WW2DX/diddle/compare/v0.1.11...v0.1.12
[0.1.11]: https://github.com/WW2DX/diddle/compare/v0.1.10...v0.1.11
[0.1.10]: https://github.com/WW2DX/diddle/compare/v0.1.9...v0.1.10
[0.1.9]: https://github.com/WW2DX/diddle/compare/v0.1.8...v0.1.9
[0.1.8]: https://github.com/WW2DX/diddle/compare/v0.1.7...v0.1.8
[0.1.7]: https://github.com/WW2DX/diddle/compare/v0.1.6...v0.1.7
[0.1.6]: https://github.com/WW2DX/diddle/compare/v0.1.5...v0.1.6
[0.1.5]: https://github.com/WW2DX/diddle/compare/v0.1.4...v0.1.5
[0.1.4]: https://github.com/WW2DX/diddle/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/WW2DX/diddle/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/WW2DX/diddle/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/WW2DX/diddle/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/WW2DX/diddle/releases/tag/v0.1.0
