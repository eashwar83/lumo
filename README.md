<h1 align="center">
  <img src="./src-tauri/icons/icon.png" alt="Soia" width="128" />
  <br>
  Soia
  <br>
</h1>

<h3 align="center">
Minimalist Design. Maximum Performance.
</h3>

[**Download Latest Release**](https://github.com/FengZeng/soia/releases) | [**Report a Bug**](https://github.com/FengZeng/soia/issues)

---
Experience your media library like never before. **Soia** combines a sleek Vue 3 interface with a robust Rust backend to support everything from local Dolby Vision files to remote WebDAV streams—all in one elegant, cross-platform package.

## ✨ Key Features

### 🎬 Superior Playback

- Hardware Acceleration: Powered by libmpv for smooth playback, including 4K, HDR, and Dolby Vision on supported hardware (currently not supported on Linux).
- Picture in Picture (PiP): Multi-task effortlessly on macOS and Windows.
- Custom Shaders: Enhance your visual experience with high-quality scaling and processing.
- Dual Subtitles: Watch with bilingual subtitle workflows more comfortably.

### ☁️ Cloud & Local Integration

- Stream via WebDAV: Browse and play your remote media library with ease.
- Smart Buffering: Visual progress bars and real-time download speed for network streams.
- Library Management: History tracking with resume position, pin-to-top, and robust multiple playlist support.
- Multiple Playlists: Create and switch between playlists, each with sort, loop, and shuffle modes for flexible media sessions.

### 💻 Platform Native Experience

- Deep Integration: Native media keys, Now Playing metadata, and artwork capture (macOS).
- Modern UI: Borderless window support across macOS, Windows, and Linux (Wayland).
- Experimental Wallpaper Mode (Windows): Combine wallpaper playback with playlists and image media.
- Playback Preferences: Configure seek step, default speed, auto-play, and skip-intro behaviors.

## Install

Download from the [release page](https://github.com/FengZeng/soia/releases).

Or you can build it yourself. Support macOS 10.15+, Windows, and Linux(Ubuntu).
Current Linux builds target Ubuntu Wayland sessions only (`X11` is not supported at this time).

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

## Getting Started

1. Prerequisites

   Ensure you have the following installed:

   - Node.js 18+ & pnpm 10.x
   - Rust (stable toolchain)
   - Tauri build prerequisites for your specific platform

2. Setup

   ```bash
   # Automatically prepares runtime libs
   pnpm install
   ```

3. Run

   ```bash
   # Launches with auto-injected library paths
   pnpm tauri dev
   ```
## Build and Bundle

### Frontend

```bash
pnpm build
```

### macOS

```bash
pnpm bundle:mac:debug
pnpm bundle:mac:release
```

### Linux

```bash
pnpm bundle:linux:debug
pnpm bundle:linux:release
```

### Windows

```bash
pnpm bundle:win:debug
pnpm bundle:win:release
```

## Keyboard Shortcuts

- `Space`: play/pause
- `Left / Right`: seek backward/forward (step from settings)
- `I`: toggle playback info panel
- Double-click video area: toggle fullscreen

## Data Storage

App data is stored in Tauri's local app data directory and includes:

- `media.db`: default playlist entries, playback history, and local installation/device/sync metadata
- `state.json`: UI state and settings (for example active panel, multiple-playlist metadata, and preferences)
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

- Linux runtime note: current bundle targets Ubuntu Wayland sessions only; launching under pure `X11` is not supported.

- If build fails with `Cannot find libmpv`, run:

```bash
pnpm setup:libs
```

- If `pnpm setup:libs` fails, confirm release access to:
  - `https://github.com/FengZeng/mpv/releases/tag/v0.41.0-r0`
  - or set `MPV_RELEASE_ASSET_URL` to a direct asset URL and retry.

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

---

## 👤 Author & Maintainer

**Soia** is an independent, **solo project** developed and maintained by **[@FengZeng](https://github.com/FengZeng)**.

As a single developer balancing performance and features across macOS, Windows, and Linux, your support means the world to me. If Soia has improved your media experience, please consider giving it a **⭐ Star**—it's the best way to keep the project growing!

## License

This project is licensed under the GNU General Public License v3.0 only (`GPL-3.0-only`).
See [`LICENSE`](LICENSE) for the full text.
