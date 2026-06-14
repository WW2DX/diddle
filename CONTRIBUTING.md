# Contributing to Diddle

Thanks for your interest! Diddle is an early-stage (v0.1) RTTY contest logger and contributions are welcome — bug reports, feature ideas, and PRs alike.

## Before you start

- Check the [roadmap](FEATURES.md#roadmap) and [open issues](https://github.com/WW2DX/diddle/issues) so we don't duplicate effort.
- For anything non-trivial, **open an issue first** to discuss the approach. The feature surface is still moving.

## Development setup

See **[BUILD.md](BUILD.md)** for prerequisites and how to run `npm run tauri dev`.

```bash
npm install
npm run tauri dev      # run the app with hot reload
npm run check          # type-check before pushing
```

## Project conventions

- **Frontend** is Svelte 5 with **runes** (`$state`, `$derived`, `$effect`). State stores live in `*.svelte.ts` files; match the existing pattern rather than introducing a store library.
- **All radio/DSP work happens in Rust** (`src-tauri/src`). The TypeScript `tci.ts` layer is a thin `invoke()`/`listen()` binding — keep logic in Rust.
- **Comments explain _why_,** matching the existing density. The DSP modules document their signal-processing decisions; preserve that.
- Keep diffs focused. One logical change per PR.

## Commit & PR guidelines

- Write clear commit messages in the imperative mood ("Add mult tracking", not "Added").
- Reference the issue you're closing (`Fixes #12`).
- Make sure `npm run check` passes and the app builds (`npm run tauri build`) before opening the PR.
- Describe what you changed and how you tested it (which radio / contest / sample WAV).

## Reporting bugs

Open an issue with:

- Your OS and Diddle version.
- Your TCI radio/software and version.
- Steps to reproduce, and what you expected vs. what happened.
- For decode issues, a short `.wav` sample is gold — the built-in WAV player makes these easy to reproduce against.

## License

By contributing, you agree that your contributions are licensed under the [MIT License](LICENSE).

*73 — WW2DX*
