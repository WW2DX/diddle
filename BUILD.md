# Building Diddle from source

Diddle is a [Tauri 2](https://v2.tauri.app/) app: a SvelteKit frontend wrapped in a Rust shell. Because the shell compiles native code, **each OS's installer must be built on that OS** (or in CI on a matching runner — see [Releases](#releases)).

## Prerequisites

| Tool | Version used | Notes |
|------|--------------|-------|
| [Node.js](https://nodejs.org/) | 20+ (dev on 26) | ships `npm` |
| [Rust](https://rustup.rs/) | stable (dev on 1.95) | `rustup` recommended |
| Tauri system deps | per-OS | see below |

### Per-OS system dependencies

Follow the official [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your platform. In short:

- **macOS:** Xcode Command Line Tools (`xcode-select --install`).
- **Windows:** [Microsoft C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) and the [WebView2 runtime](https://developer.microsoft.com/microsoft-edge/webview2/) (preinstalled on Windows 11).
- **Linux:** `webkit2gtk`, `libappindicator`, `librsvg`, `patchelf`, etc. (see the Tauri guide for your distro's package names).

## Install dependencies

```bash
npm install
```

This pulls the JS deps. Rust crates are fetched on first build.

## Run in development

```bash
npm run tauri dev
```

Launches the Vite dev server and the Tauri window with hot reload. (`npm run dev` runs only the web frontend in a browser — useful for UI work, but TCI/DSP features need the Tauri shell.)

## Type-check

```bash
npm run check
```

Runs `svelte-check` against the TypeScript config.

## Build a production bundle

```bash
npm run tauri build
```

Output installers land in `src-tauri/target/release/bundle/`:

- **macOS:** `dmg/Diddle_<version>_<arch>.dmg` and `macos/Diddle.app`
- **Windows:** `msi/Diddle_<version>_<arch>.msi` and `nsis/Diddle_<version>_<arch>-setup.exe`
- **Linux:** `deb/`, `appimage/`, and (if configured) `rpm/`

## Releases

You don't have to own three machines. The [`.github/workflows/release.yml`](.github/workflows/release.yml) workflow builds **macOS, Windows, and Linux** installers on their native GitHub-hosted runners and attaches them to a GitHub Release.

To cut a release:

```bash
# bump the version in package.json, src-tauri/Cargo.toml, and src-tauri/tauri.conf.json first
git tag v0.1.0
git push origin v0.1.0
```

The tag push triggers the workflow; when it finishes, the installers are on the [Releases page](https://github.com/WW2DX/diddle/releases).

> **Note:** Installers are **not** committed to the git tree — they live on Releases. This keeps clones small and stays under GitHub's 100 MB file limit.
