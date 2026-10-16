# README screenshots

`docs/screenshots/*.png` are rendered from Diddle's real interface in headless
Chrome, with a stand-in for the Tauri backend:

- `mock.js` — injected before the app loads; answers the backend calls (rig on
  20m, TCI and cluster connected, a CQ WW RTTY log of placeholder-style calls)
  and lets the scene emit backend events.
- `scene.js` — plays the scene: waterfall frames with seven RTTY signals,
  decoder tags and cluster spots, the RX and DeepRTTY panes, then the
  operator's right-clicks (a caller into Call, two onto the stack, a dupe the
  stack refuses).
- `rx.json` — the RX text. It's real decoder output: the callers' lines were
  synthesised as RTTY audio with mild fading and noise, then decoded by
  Diddle's classic decoder (`classic`) and by DeepRTTY (`deep`), mistakes and
  all.

To regenerate after UI changes:

```bash
npm run dev -- --port 1499          # in one shell
node tools/screenshots/capture.mjs docs/screenshots
```

`CHROME=/path/to/chrome` and `URL=http://localhost:1499/` override the
defaults (macOS Chrome, port 1499). Each run uses a fresh headless profile in
the system temp folder.
