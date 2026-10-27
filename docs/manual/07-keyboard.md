# 7. Keyboard reference

On macOS, **Ctrl** really means Control, not Command. Diddle leaves Cmd shortcuts to the system, except Cmd +/−/0 for zoom.

## Sending

| Keys | Does |
|------|------|
| **Enter** | ESM: send the next message in the sequence, and log at the end ([details](03-operating.md#esm-enter-sends-message)). With ESM off: log. |
| **F1 – F8** | Macros, first row |
| **Shift+F1 – F8** | Macros, second row |
| **Esc** | Stop the transmission now (while sending) |
| **Ctrl+K** / **Alt+K** | Live keyboard send |
| **Ctrl+Enter** | Log it: log Call + Exch as they are, send nothing |

## Entry window

| Keys | Does |
|------|------|
| **Space** | Jump between Call and Exch (from Exch only while it's empty) |
| **Tab / Shift+Tab** | Cycle Call → RST → Exch |
| **↑ / ↓**, then **Tab** or **Enter** | Pick and take an SCP suggestion |
| **Esc** | Close suggestions, or clear the form |
| **Ctrl+W** / **Alt+W** | Wipe the form |
| **Alt+U** | Switch Run ↔ S&P |
| **Alt+X** | Drop the next caller off the stack |
| Frequency in Call + **Enter** | QSY there (kHz, for example `14080`) |

## Log

| Keys | Does |
|------|------|
| **Ctrl+Q** | Edit the last QSO's call (Tab: its exchange; ↑ / ↓: older QSOs) |
| **Ctrl+N** | Note on the last QSO |
| **Ctrl+D** | Delete the last QSO (press twice) |

## Tuning

| Keys | Does |
|------|------|
| **PgUp / PgDn** | Dial ±1 kHz, from anywhere |
| **Shift+PgUp / Shift+PgDn** | Nudge the mark ±5 Hz (tuning window), from anywhere |
| **Mouse wheel** over the waterfall | Tune 10 Hz (Shift: 100 Hz, Alt/Option: 1 kHz) |
| **← / →** with the waterfall focused | Same steps as the wheel |

## Mouse

| Where | Click | Right-click |
|-------|-------|-------------|
| Waterfall (Run) | Move receive only. Double-click moves transmit too. | |
| Waterfall (S&P) | Move receive and transmit | |
| Waterfall tag | Load the call, QSY, switch to S&P | Add to the stack |
| RX window callsign | Load the call | Add to the stack |
| RX window other word | Add it to Exch | |
| Bandmap row | Load the call, QSY, switch to S&P | |
| Stack call | Drop it | |
| Log cell | Edit it | |

## Window

| Keys | Does |
|------|------|
| **Cmd/Ctrl + `+` / `−` / `0`** | Zoom in / out / reset |
| **Cmd/Ctrl + mouse wheel** | Zoom |

Next: **[Troubleshooting →](08-troubleshooting.md)**
