# 8. Troubleshooting and FAQ

### Diddle won't connect to the radio

- Make sure the TCI server is turned on in the radio software, and that the address and port match (default `ws://localhost:40001`).
- If the radio software is on another computer, use that computer's IP address, and allow the port through its firewall.
- If receive works but transmit doesn't, close any other TCI programs connected to the same radio and try again.

### I can see signals but the decoder prints garbage

- Tune more exactly: click right on the signal's mark line, or turn on **snap to peak**. A clean **+** on the tuning scope means you're on.
- Check **REV** is off and the radio is in **DIGL**. (Diddle sets DIGL on connect.)
- Check that shift is **170** and baud is **45.45**.
- **SQ** may be too high. Weak stations are the first to go quiet.
- Look at the **DeepRTTY** pane. It copies deeper into the noise, about two seconds late.

### A strong station nearby wrecks my copy

Use the **500** or **250** filter. If the station still pumps the receiver, set **AGC off** and lower the **gain**. Click **⇥ centre** if it appears, so your tones sit in the middle of the narrow filter.

### My transmitted signal is distorted, or the ALC is pinned

Turn **TX audio** down in the F-keys panel until the ALC barely moves.

### ESM sent the exchange again instead of TU

ESM sends TU only when **Exch** has his exchange in it. If Exch is empty, Enter repeats your exchange, which is the right thing when you haven't copied him yet. Click his exchange words in the RX window (or type them) before pressing Enter. If you get completely out of step, **Ctrl+Enter** logs the QSO without sending anything.

### The waterfall has no callsign tags

- Tags need the station's CQ to be decoded, and the call must be in the SCP database. Load or update SCP in Settings.
- Callers aren't tagged on purpose. Only the station running on a frequency is.
- Tags are cleared when the dial jumps by more than 500 Hz, and they come back as stations call CQ again.

### No cluster spots

Check **Settings → DX cluster** shows *connected*. Try `sh/dx` in the `dx›` line. If that shows spots but the bandmap doesn't, your login filters, or the bandmap's *band only* toggle (for example **20m only**), may be hiding them.

### Score or mults look wrong

- Update the **country file** (Settings) before the contest.
- Check that the received exchanges are complete. In CQ WW, Diddle fills in the zone it predicts; correct it if the station sends a different one.
- Diddle's scoring follows the rules in outline; the sponsor's log checker is final.

### Where are my logs and settings?

Logs are in the data folder (see [Logs](05-score-logs.md#logs)). Settings, macros and layout are saved on your computer and survive updates. Export ADIF regularly as a backup.

### Reporting a problem

Open an issue at [github.com/WW2DX/diddle/issues](https://github.com/WW2DX/diddle/issues). Include your OS, Diddle version (shown in the header) and radio software, and what you expected to happen.

← Back to **[Contents](README.md)**
