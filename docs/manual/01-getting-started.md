# 1. Getting started

## Install

Download the installer for your system from the [Releases page](https://github.com/WW2DX/diddle/releases):

| System | File |
|--------|------|
| macOS (Apple Silicon and Intel) | `.dmg` |
| Windows | `.msi` or `.exe` |
| Linux | `.AppImage`, `.deb` or `.rpm` |

Diddle isn't code-signed yet, so the first launch needs one extra step:

- **macOS:** right-click Diddle in Applications, choose **Open**, then confirm.
- **Windows:** on the SmartScreen warning, click **More info**, then **Run anyway**.

To build from source, see [BUILD.md](../../BUILD.md).

## Connect to the radio

Diddle talks to the radio over **TCI**, a WebSocket protocol built into ExpertSDR2/3, SunSDR and RemoteHamRadio. It carries frequency, mode, PTT, receive audio and transmit audio, so you don't need any other cable or interface.

1. In your radio software, turn on the **TCI server**. The usual address is `ws://localhost:40001`. For a radio on another computer, use that computer's address.
2. In Diddle's header, type the TCI address and click **Connect**.
3. When the dot turns green, the frequency, mode and band appear and the waterfall starts moving.

When it connects, Diddle puts the radio in **DIGL** (lower sideband data), the normal sideband for amateur RTTY. If something else changes the mode later (a band change, the RHR console), Diddle sets DIGL again. Leave **REV** off for normal signals.

The header shows two frequencies:

- **The big number** is the RF frequency of your *mark tone*: where your signal actually is. It's what Diddle logs and what the cluster reports.
- **dial** is the radio's suppressed-carrier dial reading.

## Enter your station and pick a contest

Open **Settings — operator + contest** at the bottom of the page.

| Field | Used for |
|-------|----------|
| **My call** | `<MYCALL>` in macros, Cabrillo, and points (your country) |
| **Name** | NAQP; the General QSO profile |
| **State** | CQ WW (US/VE), Roundup, NAQP |
| **CQ Zone** | CQ WW |
| **Grid** | Makrothen (4 characters are sent) |
| **Age** | JARL. Leave it blank to send `01`. |
| **Contest** | Sets the sent exchange, the hint for the received exchange, scoring, and the Cabrillo format |
| **ESM** | Enter Sends Message. On by default. See [Operating](03-operating.md#esm-enter-sends-message). |

Built-in contests:

| Contest | You send | You receive |
|---------|----------|-------------|
| General QSO (ragchew) | 599 + name | anything (optional) |
| Generic RTTY | 599 + serial | serial |
| CQ WW RTTY DX | 599 + zone (+ state/province in W/VE) | zone, state/province |
| CQ WPX RTTY | 599 + serial | serial |
| ARRL RTTY Roundup | 599 + state/province (DX: serial) | state/province or serial |
| NAQP RTTY | name + state/province | name + location |
| JARL World Wide RTTY | 599 + age | age |
| Makrothen RTTY | 4-character grid | grid |

### Reference files

These are also in Settings. Each one makes Diddle faster or more accurate:

- **SCP database (Super Check Partial):** a list of active contest calls. It powers call suggestions as you type, and it filters noise out of waterfall tags. Click **Update from web** to fetch the latest `MASTER.SCP`, or **Load file…** to use your own.
- **Country file:** AD1C's `cty.dat` comes with Diddle. It supplies country, zones and continent for every call, which drives scoring, new-mult highlighting and zone prediction. Click **Update from country-files.com** before a big contest.
- **Call history:** load an N1MM+ call history file and the received exchange fills itself in as soon as you enter a known call. It's shown in amber until you edit it. **Exch from** picks which column to use. *auto* does the right thing for the built-in contests and uses `Exch1` for any other contest.
- **Saved contest setups:** save the contest, your F-key messages and the history file under a name. Pick it next year and everything comes back.

## A tour of the screen

From top to bottom:

| Part | What it's for |
|------|---------------|
| **Header** | TCI connection, frequency and band, layout presets, and QSOs / score / rate / elapsed time |
| **Waterfall** | Every signal in the passband, with callsign tags. Click to tune. |
| **RX decoder** | Decoded text from the station you're tuned to, the DeepRTTY pane, and the tuning scope |
| **Entry** | Call, RST and Exch fields, the Run/S&P switch, the ESM chip, and the caller stack |
| **F-keys** | 16 macro buttons, TX audio level and RF power |
| **Score** | Score by band, rate, and the mults still needed on this band |
| **Bandmap** | Cluster spots and decoded stations, sorted by frequency |
| **Log** | The QSOs in the current log, plus export |
| **Settings / Test** | Setup, and practice tools. Both are folded until you open them. |

On a window about 1100 pixels wide or more, **Score and the bandmap sit in a column on the right** that stays on screen while the rest of the page scrolls. Click **docked** in the bandmap header to put them back in the main column. Drag the column's left edge to make it wider or narrower.

### Fit it to your screen

- **Zoom:** Cmd/Ctrl + `+` / `−` / `0`, or Cmd/Ctrl + mouse wheel. Diddle remembers the zoom.
- **Fold panels:** the small **▾** at the top left of a panel folds it to a one-line bar. Folded panels keep running: the decoders keep decoding and the log keeps logging.
- **Resize:** drag the bottom-right corner of the waterfall, RX window, bandmap or log.
- **Layout presets** (the **Layout…** menu in the header):
  - *Standard:* everything open.
  - *Compact:* short panels, so the most fits on one screen.
  - *Big RX window:* a tall RX window for reading copy.

  Start from a preset, then adjust.
- **Fonts:** Settings → *Display fonts* sets the font, size and slashed zero for the RX, Entry, Bandmap and Log windows separately.

Next: **[Receiving →](02-receiving.md)**
