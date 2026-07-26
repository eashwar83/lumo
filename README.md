<h1 align="center">
  <img src="./src-tauri/icons/icon.png" alt="Lumo" width="128" />
  <br>
  Lumo
  <br>
</h1>

<p align="center">
🎬 HDR & Dolby Vision · 🌐 WebDAV + DLNA + SMB Streaming · 🖥 Cross-platform
</p>

<p align="center">
<b><a href="https://github.com/eashwar83/lumo/releases">⬇️ Download Latest Release</a> · <a href="./docs/USER_GUIDE.md">📖 User Guide</a> · <a href="https://github.com/eashwar83/lumo/issues">🐞 Report a Bug</a></b>
</p>

> A modern mpv frontend focused on performance and clean design.

**Lumo** is a high-performance video player built on mpv, designed for smooth playback of everything from local Dolby Vision content to remote WebDAV, DLNA, and SMB streams — all in one fast, elegant, cross-platform experience.

> Lumo is a fork of **[Soia](https://github.com/FengZeng/soia)** by [@FengZeng](https://github.com/FengZeng), licensed under GPL-3.0. See [Credits](#credits) below.

## ✨ Key Features

### High-Performance Playback

- **mpv-powered playback** with hardware acceleration (4K, HDR, Dolby Vision*)
- Picture in Picture (PiP) on macOS and Windows
- Dual subtitles for bilingual viewing
- Fuzzy subtitle matching for both local and network media
- Online subtitle search via OpenSubtitles and SubSource
- Advanced subtitle appearance controls for font, color, size, and position
- Custom shaders for high-quality scaling and rendering
- Anime mode with auto-detection and shader auto-apply

<sub style="padding-left: 2em;">*Dolby Vision is not currently supported on Linux*</sub>

### Streaming & Remote Media

- WebDAV browsing and streaming
- DLNA and SMB/Samba discovery, browsing, and playback
- M3U (IPTV) parsing and playback
- Smart buffering with real-time speed indicators
- Resume playback with history tracking

### Native Experience

- Native media keys and Now Playing integration (macOS)
- Borderless window across macOS, Windows, and Linux (Wayland)
- Experimental Wallpaper Mode (Windows)
- Flexible playback preferences (speed, seek, auto-play, skip intro)

## 🚀 What's new in Lumo

Lumo rebuilds the upstream Soia player around a rich, **per-video** picture-
quality and personalization workflow. Everything below is new in Lumo.

### 🎨 Picture quality & grading

Open the **Video** popover (gear icon) in the player bar.

- **Auto Enhance** — one press samples the current frame and automatically fixes
  levels (contrast), exposure (brightness), colour cast (white balance), and
  dull colour (saturation), filling the sliders so you can fine-tune.
  Algorithmic — fast and reliable, no cloud, no wait.
- **Colour grade** — GPU sliders for **Exposure, Temperature, Tint, Highlights,
  Shadows**, alongside Brightness / Contrast / Saturation / Gamma / Hue.
- **Sharpness** — GPU unsharp mask with independent **Amount** and **Radius**
  (crisp edges → broad local-contrast / "HDR" glow), with no decode penalty.
- **Film Grain** — animated, luminance-aware GPU grain for a filmic texture.
- **Denoise** / **Deinterlace** toggles for noisy or interlaced sources.
- **Quality presets** (Fast / Balanced / High) for mpv's scaler and debanding.
- **AI Upscaling** — one-click bundled **Anime4K** (anime) and **ravu**
  (live-action) upscalers.

### 🤖 AI colour correction (cloud)

Bring your own key and let a **cloud vision model** design a tailored grade.

- **Multi-provider** — Gemini, Claude, OpenAI, Kimi (Moonshot), Qwen, DeepSeek,
  Grok, or any custom OpenAI-compatible endpoint. Keys are stored per-provider
  and stay on your device; frames upload only to the provider you pick.
- **Prompt-steered** — type an instruction ("warmer, 90s film look, lift the
  shadows") or run a blank best-effort pass. Switch provider/model right in the
  prompt window, and **reuse past prompts** from a Recent list.
- **Reference images** — attach up to 3 stills of a look you want and the AI
  grades the video *toward* them (palette, contrast, warmth, mood).
- **Full correction** — returns per-channel **curves** plus **saturation,
  temperature/tint, sharpen, and film grain**, applied as one undoable step.

### 🔍 Compare, history & non-destructive editing

- **Before / after split view** — split the frame down the middle to judge your
  grade against the untouched original (HDR-correct on both halves).
- **View Original** (`\`) — instantly bypass *all* picture **and** audio
  enhancements to see the source, without losing your settings.
- **Undo / Redo** (`Ctrl+Z` / `Ctrl+Shift+Z`) — a **per-file** history (up to 20
  steps) that survives restarts. AI-made steps are self-documenting: cycling to
  one shows the **model, prompt, and the model's result notes**.

### ✂️ Cutting & exporting

- **Merge files** (no re-encode) — combine same-format clips **losslessly and
  instantly**; a re-encode fallback combines mismatched files when needed.
- **Split file** (no re-encode) — cut one file into pieces by **timestamps,
  equal parts, every N minutes, or chapters**.
- **Clip & GIF export** — lossless stream-copied clips and in-process animated
  GIFs from an A-B range.
- **Contact sheet** — tile a grid of evenly-spaced frames into one image.
- **Scene detection & chapters** — auto-detect scene cuts (or read embedded
  chapters) and jump between them, with seek-bar marks.

### 🧭 Menu bar & command palette

- A familiar **VLC-style menu bar** (Media / Playback / Audio / Video / Subtitle
  / Tools / View / Help) — an additional route to every action.
- A searchable **command palette** to run any action by name.

### 🎞 Looks & presets

- **Presets** — 8 built-in looks (Punch, Warm, Cool, Vivid, Calm, B&W, B&W Warm,
  Vintage) plus **save your own** named custom presets (a reusable library).
- **Per-video memory** — the full look (brightness/contrast/saturation/gamma/hue
  + colour grade + sharpen/denoise/deinterlace/grain) is remembered **per file**
  and restored on reopen. A **Reset** button clears the current video's look; a
  **Global** toggle applies one look to every video instead.

### ⏱ Seek-bar thumbnails

- **Hover previews** on the seek bar, pre-rendered in the background per local
  file (no playback interruption) and cached to disk.
- **Adaptive & configurable** — a frame every N seconds (Settings → Advanced),
  with an on/off toggle and automatic cache cleanup.

### ❤️ Favourites

- Favourite videos while watching (top-bar heart, `B` shortcut), a **Favourites**
  thumbnail-grid view, and a Favourites list in the playlist drawer so prev/next
  can walk your favourites.

### 🖼 Window & framing

- **Aspect-ratio cycle** (`E`): Default / 16:9 / 4:3 / 21:9 / 2.35:1, remembered
  per file; the window auto-fits to remove letterboxing.
- **Fit window to video** (`G`), remembered per file.
- **Step window resize** (`Alt` +/−) keeping aspect, and a **window size lock**
  to keep one size across all videos (toggleable).

### ✂️ Crop, playlist & shortcuts

- **Auto-Crop Black Bars** — mpv `cropdetect` → `video-crop`, with an adjustable
  black threshold for noisy/analog sources; the detected crop is remembered per
  file. Manual crop shortcuts (`C` / `Shift+C`).
- **Auto-Load Folder to Playlist** — playing a local file loads its whole folder
  so prev/next walk the folder sequence.
- **Configurable keyboard shortcuts** — every action rebindable from Settings →
  Keyboard Shortcuts, with a live help overlay.
- **Screenshots** with a configurable folder.

### ⚡ Under the hood

- **Faster cold start** — non-blocking startup ping and deferred network
  discovery.
- **No phone-home** — the upstream telemetry ping and auto-updater have been
  removed.

## Install

Download from the [release page](https://github.com/eashwar83/lumo/releases).

Or you can build it yourself. Supports macOS 13+, Windows, and Linux (Ubuntu).
Current Linux builds target Ubuntu Wayland sessions only (`X11` is not supported at this time).

## FAQ

Q: macOS says the app "is damaged and can't be opened" or cannot verify it is free of malware.

A: This happens because the app is not signed with an Apple Developer ID certificate, so macOS may block it on first launch.

Easy fix (recommended):
1. Right-click Lumo.app
2. Click "Open"
3. Click "Open" again in the dialog

If that doesn't work, run:
```bash
sudo xattr -r -d com.apple.quarantine /Applications/Lumo.app
```

You can also go to System Settings -> Privacy & Security and click "Open Anyway" (it appears after a blocked launch attempt).

The app is open-source and its code is publicly available for anyone to inspect.

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

Common release build commands:

```bash
pnpm bundle:mac:release
pnpm bundle:linux:release
pnpm bundle:win:release
```

## Data Storage

App data is stored in Tauri's local app data directory (under `com.lumo.player`) and includes:

- `media.db`: default playlist entries, playback history, and local installation/device metadata
- `state.json`: UI state and settings (for example active panel, playlist metadata, and preferences)
- `network_connections.json`: saved network connections
- `thumbnails/`: captured artwork for Now Playing

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
  - `https://github.com/FengZeng/mpv/releases/tag/v0.41.0-r15`
  - or set `MPV_RELEASE_ASSET_URL` to a direct asset URL and retry.

- If Linux/Windows bundle scripts report missing runtime manifest, generate it on the target platform:

```bash
pnpm sync:runtime:linux
pnpm sync:runtime:win
```

---

## Credits

**Lumo** is a fork of **[Soia](https://github.com/FengZeng/soia)**, an independent
project created and maintained by **[@FengZeng](https://github.com/FengZeng)**.
All credit for the original player goes to the upstream author. Lumo continues
to build on that work under the same license.

The prebuilt `libmpv` runtime is sourced from the upstream
[FengZeng/mpv](https://github.com/FengZeng/mpv) release builds.

## License

This project is licensed under the GNU General Public License v3.0 only (`GPL-3.0-only`),
the same license as the upstream project. See [`LICENSE`](LICENSE) for the full text.

As required by the GPL, the original copyright notices are retained and the
complete corresponding source code remains available under GPL-3.0.
