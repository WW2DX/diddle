# DeepRTTY model

`deeprtty.onnx` is the DeepRTTY RTTY-decoder model by e04, from
https://github.com/e04/DeepRTTY (commit `cf764176e9eefc0239b744131c7579d28c8844d4`,
2026-09-30), unmodified.

`src-tauri/src/deep_rtty/` ports that project's reference streaming receiver
(`examples/python/rtty_stream.py`: spectrogram and normalisation, greedy CTC,
overlapping-window merge, ITA2) to Rust.

DeepRTTY is licensed **AGPL-3.0-only**. The model and the ported code are
covered by that licence, and so is any build of Diddle that includes them.
