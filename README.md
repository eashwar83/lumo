<h1 align="center">
  <img src="./src-tauri/icons/icon.png" alt="Soia" width="128" />
  <br>
  Soia
  <br>
</h1>

<h3 align="center">
A cross-platform media player based on <a href="https://github.com/tauri-apps/tauri">Tauri 2.0</a>.
</h3>

---
Soia is a cross-platform media player built with Tauri 2.0, Vue 3, TypeScript, Rust, and libmpv.

## Features

- Play local video files with mpv-backed playback.
- Support HDR and Dolby Vision playback on compatible media and hardware.
- Keep playback history with resume position and pin-to-top support.
- Manage a playlist with sort modes and loop/shuffle behaviors.
- Browse and stream media from WebDAV servers.
- Configure playback preferences (seek step, default speed, auto-play, skip intro).
- Platform integrations (currently macOS) for media keys, Now Playing metadata, and artwork capture.

## Install

Download from the [release page](https://github.com/FengZeng/soia/releases).

Or you can build it yourself. Support macOS 10.15+.
Current Linux builds are Wayland-only (`X11` is not supported at this time).

## FAQ

Q: macOS says "`Soia` is damaged and can't be opened" or cannot verify it is free of malware.

A: Current builds are not signed/notarized with an Apple Developer ID certificate, so Gatekeeper may block first launch. Open Terminal and run:

```bash
sudo xattr -r -d com.apple.quarantine /Applications/Soia.app
```

## Tech Stack

- Frontend: Vue 3 + TypeScript + Vite
- App runtime: Tauri v2
- Backend: Rust
- Playback engine: libmpv
- Persistence: SQLite (`media.db`) + JSON state files

## Requirements

- Node.js 18+
- pnpm 10.x
- Rust stable toolchain
- Tauri build prerequisites for your platform
- Network access to download prebuilt `libmpv` release assets

## Getting Started

1. Install dependencies:

```bash
pnpm install
```

`pnpm install` runs `setup:libs` automatically, so runtime libraries are prepared by default.
`setup:libs` goes through `scripts/setup_runtime_libs.mjs` as the unified entrypoint.
macOS runtime frameworks are generated into `src-tauri/tauri.runtime.macos.json` (runtime file, not hand-edited).
Current implementation status:
- `darwin`: implemented (`scripts/setup_runtime_libs_macos.sh`)
- `linux`: implemented (`scripts/setup_runtime_libs_linux.sh`)
- `win32`: implemented (`scripts/setup_runtime_libs_windows.mjs`)
- `android`: TODO placeholder (will be dispatched by `setup_runtime_libs.mjs`)
When running implicitly from `pnpm install`, Android is currently skipped (no failure).

2. Start the app in development mode:

```bash
pnpm tauri dev
```

`scripts/run_tauri.mjs` will auto-inject runtime library search paths for `dev`
(`DYLD_FALLBACK_LIBRARY_PATH` on macOS, `LD_LIBRARY_PATH` on Linux, `PATH` on Windows).
On macOS `dev/build`, it also auto-merges `src-tauri/tauri.runtime.macos.json` via `tauri --config`.

If you want to use a local bundled `mpv + dependencies` directory for dev testing:

```bash
pnpm setup:libs /absolute/path/to/mpv-bundle
```

Optional (frontend only):

```bash
pnpm dev
```

## Build and Bundle

- Build frontend assets:

```bash
pnpm build
```

- Build macOS app bundle (debug):

```bash
pnpm bundle:mac:debug
```

- Build macOS app bundle + package DMG (release):

```bash
pnpm bundle:mac:release
```

macOS bundle flow also runs:

- `scripts/bundle_runtime_libs_macos.sh` to copy/sign non-system dylibs into the app bundle
- `scripts/bundle_dmg_macos.sh` to create a distributable DMG

- Build Linux bundles:

```bash
pnpm bundle:linux:debug
pnpm bundle:linux:release
```

Linux bundle flow runs `scripts/bundle_runtime_libs_linux.mjs` after `tauri build`.
Linux release artifacts are named with `Wayland` (for example: `Soia-vX.Y.Z-Linux-x64-Wayland.deb`)
to indicate the current Linux target is Wayland-only.

- Build Windows bundles:

```bash
pnpm bundle:win:debug
pnpm bundle:win:release
```

Windows bundle flow uses `src-tauri/tauri.windows.conf.json` + `src-tauri/windows/hooks.nsh`
to include mpv DLLs and copy them next to `Soia.exe` during NSIS install.

Linux bundle post-processing expects runtime manifests:

- `src-tauri/libs/runtime-libs.linux.json`

## Keyboard Shortcuts

- `Space`: play/pause
- `Left / Right`: seek backward/forward (step from settings)
- `I`: toggle playback info panel
- Double-click video area: toggle fullscreen

## Data Storage

App data is stored in Tauri's local app data directory and includes:

- `media.db`: playlist and playback history
- `state.json`: UI state and settings
- `network_connections.json`: saved network connections
- `thumbnails/`: captured artwork for Now Playing

## Network Notes

- WebDAV browse/playback is implemented in the Rust backend.
- UI options for SMB/FTP/HTTP-DLNA exist, but backend browsing/streaming currently targets WebDAV.

## Security Note

Saved network credentials are currently persisted in `network_connections.json` as plain text. Avoid using sensitive production credentials on shared machines.

## Troubleshooting

- If Linux build fails with `glib-2.0` / `gdk-3.0` / `*.pc` not found, install the Ubuntu deps:

```bash
sudo apt update
sudo apt install -y \
    build-essential \
    curl \
    wget \
    file \
    libssl-dev \
    libgtk-3-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    pkg-config \
    libwebkit2gtk-4.1-dev
```

- Linux runtime note: current bundle targets Wayland sessions only; launching under pure `X11` is not supported.

- If build fails with `Cannot find libmpv`, run:

```bash
pnpm setup:libs
```

- If `pnpm setup:libs` fails, confirm release access to:
  - `https://github.com/FengZeng/mpv/releases/tag/v0.41.0-r0`
  - or set `MPV_RELEASE_ASSET_URL` to a direct asset URL and retry.

- If you need to explicitly test platform dispatch in the unified entrypoint:

```bash
node ./scripts/setup_runtime_libs.mjs --platform darwin
node ./scripts/setup_runtime_libs.mjs --platform linux
node ./scripts/setup_runtime_libs.mjs --platform win32
node ./scripts/setup_runtime_libs.mjs --platform android
```

- If Linux/Windows bundle scripts report missing runtime manifest, generate it on the target platform:

```bash
pnpm sync:runtime:linux
pnpm sync:runtime:win
```

- If you have a local bundled `mpv + dependencies` directory for dev testing, use:

```bash
pnpm setup:libs /absolute/path/to/mpv-bundle
```

Equivalent environment-variable form:

```bash
MPV_LOCAL_BUNDLE_DIR=/absolute/path/to/mpv-bundle pnpm setup:libs
```

## License

This project is licensed under the GNU General Public License v3.0 only (`GPL-3.0-only`).
See [`LICENSE`](LICENSE) for the full text.
