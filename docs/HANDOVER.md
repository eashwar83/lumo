# Lumo — Project Handover

Written 2026-10-19 for whoever (or whatever agent) continues this project. It
replaces the untracked `CLAUDE-HANDOFF.md` from July, which is stale. Everything
below was verified against the repository and the development machine on the
day of writing; where something is a judgement rather than a fact, it says so.

Read this top to bottom once. Afterwards the sections you will return to are
§3 (build), §6 (YouTube facts), §9 (open items) and §10 (working rules).

---

## 1. Snapshot

| | |
|---|---|
| Repository | `C:\Users\eashw\Projects\soia`, remote `https://github.com/eashwar83/lumo.git`, branch `main` |
| HEAD | `46b22fa` — *feat(favourites): sortable tab, and playing from it walks the favourites* |
| Working tree | clean, level with `origin/main` |
| Version in files | 1.4.0 (`package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, kept in step by `pnpm version:sync`) |
| Last published release | `v1.4.0`, 2026-08-12, built by CI for six targets |
| Unreleased on `main` | 26 commits since `v1.4.0` (listed in §9.1) |
| Latest local installer | `src-tauri\target\release\bundle\nsis\Lumo_1.4.0_arm64-setup.exe`, built 2026-10-04 from `46b22fa` and installed on the development machine |
| Lineage | Fork of [Soia](https://github.com/FengZeng/soia) by FengZeng, GPL-3.0; renamed Lumo in August 2026. Several identifiers still say `soia` (`SOIA_*` setting labels, `soia.log`, `libsoia_utils`, `SOIA_API`) — these are deliberate and must not be renamed, persisted data depends on them |

## 2. What the product is

Lumo is a desktop video player: a Vue 3 user interface over a libmpv engine
driven from a Rust backend, packaged with Tauri v2. It plays local files,
network shares (WebDAV, DLNA, SMB) and YouTube, and adds things mpv does not
have: GPU picture controls and AI colour grading, per-file memory of every
picture setting, a YouTube browser with search, comments, downloads and
SponsorBlock, AI subtitle generation and translation, seek-bar thumbnails,
clip/GIF/contact-sheet export, lossless merge and split, a command palette,
and fully rebindable shortcuts. `docs/USER_GUIDE.md` (445 lines) describes
every feature from the user's side and is accurate as of v1.4.0.

The owner uses it daily, mostly for Telugu and Hindi films (local files, a
USB drive with ~29,000 clips, and YouTube). Windows on ARM64 is the only
platform that is ever run and tested by hand. macOS and Linux are built by
CI and have not been exercised by the owner.

## 3. The development machine and how to build

### 3.1 Hardware and toolchain facts (all verified)

- Windows 11 Home on a Surface Pro 11, **ARM64**. Every build targets
  `aarch64-pc-windows-msvc`. Never produce x64 artefacts locally.
- Rust 1.94.0 (`rustc`, `cargo` on PATH). Node 24.14.0 on PATH.
- **pnpm 10.30.3 lives in `C:\usr\local` and is not on PATH.** Package
  scripts and Tauri's `beforeBuildCommand` call bare `pnpm`, so PATH must
  carry it for anything that builds.
- **clang (portable LLVM 20) is at `C:\Users\eashw\.llvm\bin`** and is not on
  PATH. The `ring` crate needs it on this target; without it cargo fails with
  `failed to find tool "clang"`.
- **MSVC ARM64 environment** comes from
  `"C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvarsall.bat" arm64`.
  Note the `(x86)` path. Its environment does not survive back into
  PowerShell, so the build must run in the same `cmd /c` line.
- Python 3.13.3, **native ARM64**, at
  `C:\Users\eashw\AppData\Local\Programs\Python\Python313-arm64\python.exe`,
  with `yt-dlp` installed from the nightly channel (2026.10.10 at the time of
  writing, and kept fresh by the app itself — §6.4).
- ffmpeg 8.1 (winget, Gyan build) on PATH; used by clip export, YouTube
  range export and yt-dlp merging.
- Memory is the scarce resource on this machine. A release build has been
  killed by low memory before. Close browsers before building if free memory
  is under ~2 GB (`(Get-CimInstance Win32_OperatingSystem).FreePhysicalMemory`).
- `src-tauri/libs/` holds the libmpv runtime DLLs (installed once by
  `pnpm setup:libs`). It is partly git-ignored and **must never be
  committed**. Windows Node's `fetch` to GitHub is unreliable on this
  machine, so if the libs ever need re-fetching, download the
  `libmpv-*-windows-clangarm64-arm64.tar.gz` from mpv-winbuild by other
  means and run `pnpm setup:libs <extracted-dir>`.

### 3.2 The build command (use verbatim)

```powershell
cd C:\Users\eashw\Projects\soia
$env:Path = "C:\usr\local;C:\Users\eashw\.llvm\bin;$env:Path"
cmd /c '"C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvarsall.bat" arm64 && pnpm tauri build --bundles nsis'
```

Output: `src-tauri\target\release\bundle\nsis\Lumo_<version>_arm64-setup.exe`
(~48 MB). A warm build takes 3–6 minutes. The installer is per-user
(`%LOCALAPPDATA%\Lumo\lumo.exe`), no admin needed; running it over an
existing install is the normal upgrade path.

### 3.3 Cheaper checks

```powershell
# Frontend type-check (does NOT check .vue files — see 3.4)
$env:Path = "C:\usr\local;$env:Path"; pnpm exec tsc --noEmit

# Rust compile + unit tests (24 tests; needs the same env as the build)
cmd /c '"...\vcvarsall.bat" arm64 && cargo test --manifest-path src-tauri/Cargo.toml --target aarch64-pc-windows-msvc -- --test-threads=1'

# Frontend bundle only
pnpm build        # = version:sync && tsc && vite build
```

### 3.4 Verification gaps to know about

- There is **no vue-tsc and no frontend test runner**. `tsc` ignores `.vue`
  files, so a type error inside a component's `<script setup>` or template
  surfaces only at runtime. Check declaration order by hand (a top-level
  `watch` in `App.vue` runs at setup; `player` and `playlistState` are
  created near line 100 and are safe to use below that).
- Composable logic can be tested in Node: bundle the module with the
  project's esbuild
  (`node_modules/.pnpm/esbuild@0.25.12/node_modules/esbuild`), stub
  `./useUiStateStore` (`loadUiState` → `{}`, `createDebouncedUiStateSaver`
  → no-op saver) and assert. This was done for the favourites sort/queue
  logic and worked well.
- Rust unit tests exist only for the stream proxy, the YouTube range cutter
  and a few parsers. Most behaviour is verified by installing and using the
  app.
- The cargo `debug` caches regrow to ~20 GB over time
  (`src-tauri\target\debug`, `src-tauri\target\aarch64-pc-windows-msvc\debug`).
  Both are safe to delete; `target\release` holds the installer cache and
  should be kept.

## 4. Repository map

```
src/                      Vue 3 + TypeScript frontend (Vite)
  App.vue                 ~4000 lines; the composition root. Nearly every
                          feature is a composable instantiated here.
  composables/            74 composables — the real frontend architecture
  components/             dialogs, player controls, drawer, menus, youtube/
  panels/                 Home, History, Favorites, YouTube, Network, Settings
  constants/              shortcuts registry, media extension table, …
  mock/settings.ts        the settings schema: every label, group and default
  types/, utils/, styles/
src-tauri/                Rust backend (Tauri v2)
  src/lib.rs              AppState, 129 registered commands, plugin setup
  src/app_bootstrap.rs    startup: mpv init, yt-dlp path resolution, updaters
  src/mpv/                libmpv FFI, event loop, stream proxy, yt-dlp resolver,
                          thumbnails, encode (ffmpeg), yt-dlp self-updater
  src/youtube/            Innertube client, search/browse, comments, captions,
                          downloads queue, PO-token server, SponsorBlock,
                          range export (ranges.rs)
  src/store/              SQLite media.db, JSON ui-state, play history
  src/commands/           Tauri command modules (playback, platform, window, …)
  src/ai_subtitles.rs     subtitle generation/translation (cloud + local Whisper)
  src/ai_enhance.rs       AI colour grading
  src/media_edit.rs       lossless merge/split
  binaries/               yt-dlp.exe + bgutil-pot-server.zip (git-ignored,
                          fetched/built by scripts/prepare_windows_binaries.mjs)
  resources/shaders/      bundled Anime4K / ravu upscalers
  tauri.conf.json         + tauri.windows.conf.json (resource map, file assoc.)
scripts/                  runtime-lib setup, version sync, Windows binary prep,
                          macOS/Linux bundling
.github/workflows/        ci.yml, release.yml (tag v* → 6 targets), per-OS builds
docs/                     USER_GUIDE.md, TODO.md (deferred work), SHOTLIST.md
CHANGELOG.md, RELEASE_NOTES.md, youtube-module-spec.md
```

Everything else at the root (`Capture-LumoShots.ps1`, `CLAUDE-HANDOFF.md`)
is untracked scaffolding; leave it uncommitted.

## 5. Architecture essentials

### 5.1 The mpv bridge

- One libmpv instance (`AppState.mpv_player: Arc<Mutex<MpvHandle>>`) renders
  into the window. Rust owns it; the frontend talks through Tauri commands
  (`mpv_run_command`, property setters) and receives events from
  `src-tauri/src/mpv/event_loop.rs`.
- **Stateful operations must go through the owning composable**, never raw
  mpv commands, or the UI desyncs: volume/mute (`usePlaybackVolume…`),
  speed (`usePlaybackSpeed`), tracks (`useMediaTracks`), sub/audio delay
  (`usePlaybackAdjustments`), seeking (`usePlaybackSeekActions`). Stateless
  one-shots (`frame-step`, `screenshot-to-file`) are fine via
  `mpv_run_command`.
- Progress events to the UI are **throttled to 4/s** in the event loop;
  pause/buffering transitions and seeks bypass the throttle. Do not add
  per-frame work to the event path — the UI was burning a core before this.
- `glsl-shaders` has a single owner (`GLSL_LAYERS` in
  `commands/persistence.rs`) composing five layers in a fixed order:
  bundled upscalers → colour grade → sharpen → film grain → the user's own
  shaders. Add new shader effects as a layer there, never by setting the
  property directly.
- A **second, headless libmpv** (`vo=image`, `mpv/thumbnails.rs`) generates
  seek-bar thumbnails, storyboards, posters and now-playing artwork.
  `generate_frames_inner` sets the mpv `frames` option as a hard cap —
  without it a broken keyframe index once produced 35 GB of JPEGs. Keep
  that cap on any new frame-extraction path.

### 5.2 Settings and persistence

- The settings schema is `src/mock/settings.ts`: groups (Playback, Video,
  Subtitles, Online Subtitles, Network, YouTube, Storage, Tools, Advanced,
  Experiments, Theme, Keyboard Shortcuts) of items `{label, displayLabel,
  value, type, options}`. The **label** is the stable key — many are
  `SOIA_*` and must stay so.
- Settings ride the ui-state channel: frontend `useUiStateStore`
  (`loadUiState`, debounced savers) ↔ Rust `store/ui_state_store.rs` ↔
  `%LOCALAPPDATA%\com.lumo.player\state.json`. Rust reads a setting with
  `load_setting_value(app, "LABEL")`.
- **Trap:** `UiState` in `ui_state_store.rs` is a typed struct with no
  catch-all. A new top-level key is silently dropped unless added to the
  struct. Two escape hatches already exist: put the data in a settings
  group (how keyboard shortcuts persist, no Rust change) or in an opaque
  `serde_json::Value` slice such as `favoritesMeta` (how favourite folders,
  sort mode and durations persist).
- Per-file memory (colour adjustments, enhancement look, aspect, crop,
  fit-to-window) is keyed by media URL in `perFile*` slices, capped at a
  few hundred entries.
- `media.db` (SQLite, `store/media_db.rs`) holds the default playlist,
  play history and sync bookkeeping. `youtube_downloads.json` is the
  download queue. Both live beside `state.json`.
- The frontend-wide signal for "settings changed" is the
  `SETTINGS_UPDATED_EVENT` window event; composables that cache a setting
  listen for it (`useAutoloadFolder`, `usePlaylistFileKinds`,
  `usePlaybackShortcuts` are the patterns to copy).

### 5.3 Playlists, favourites and transient queues

`src/composables/usePlaylistState.ts` (1,384 lines) owns all of it.

- Persistent playlists: `favorites` (id `"favorites"`) plus user playlists.
  Favourites additionally have folders, a sort mode (date/name/length, both
  directions) and known durations, all in the `favoritesMeta` slice.
- Three **transient** playlists are never persisted and are filtered out
  in `toPersistedState`: `pl_autoload_folder` (the folder around the
  playing file, from `useAutoloadFolder`), `pl_youtube_upnext` (YouTube's
  related videos) and `pl_favorites_queue` (what the Favourites tab showed
  when a video was started from it, in its sort order).
- "Next" resolution: `resolvePlaybackPlaylistId` prefers the playlist
  playback was started from (`playbackPlaylistId`) if it contains the
  current path, then the drawer's active playlist, then any playlist. The
  favourites queue keeps its own order (the drawer's name/added toggle does
  not reshuffle it) and ends when a file from outside it plays
  (`noteLoadedPath`, called from `App.vue`'s `onFileLoaded` before the
  folder scan).
- Performance rule learned on the 29k-file USB folder: never compare lists
  with `array.some` inside a loop; use Sets. The auto-load sync did, and
  cost 3–4 s per next-click.

### 5.4 YouTube pipeline

1. **Search/browse/comments** go straight to Innertube
   (`youtube/innertube.rs`, key-less `youtubei/v1/search|next|browse`),
   with a WEB client version string that needs bumping occasionally.
2. **Resolving a video for playback** runs yt-dlp as a subprocess
   (`mpv/ytdlp_resolver.rs`): `--dump-single-json` with a format selector
   capped at the configured height, through `ytdlp_base_command`, which
   always adds `--remote-components ejs:github --js-runtimes node` (the JS
   challenge solver and the runtime it needs — **they only work as a
   pair**; a call site with one and not the other gets storyboard images
   only). Cookies are a fallback used only on authentication errors.
   Resolutions are cached (`RESOLVE_CACHE_TTL`, 40 min) and the top search
   results are pre-resolved.
3. **yt-dlp binary selection** (`app_bootstrap::resolve_ytdl_path`): the
   `SOIA_YTDL_PATH` setting, else `SOIA_YTDL_PATH` env, else a native
   Python with the `yt_dlp` module under
   `%LOCALAPPDATA%\Programs\Python` (≈7× faster than the x64 exe under
   emulation — this is what the dev machine uses), else the bundled
   `binaries\yt-dlp.exe`. Command builders recognise a `python.exe` path
   and insert `-m yt_dlp`.
4. **PO tokens**: the bgutil provider plugin ships beside the bundled exe,
   and its Node server (`binaries/bgutil-pot-server.zip`, unpacked to
   `%LOCALAPPDATA%\com.lumo.player\bgutil-pot-server` on first use, port
   4416) is spawned with the user's `node` by `youtube/pot.rs` and killed on
   exit. Missing Node degrades to tokenless.
5. **Stream proxy** (`mpv/stream_proxy.rs`, 2,574 lines): mpv never
   touches googlevideo directly. The resolver registers each stream URL
   under a token; mpv plays `http://127.0.0.1:<port>/stream/<token>` (or an
   EDL wrapping video+audio tokens). The proxy rewrites HLS playlists,
   serves segments by index, keys headers by host, rotates mirror hosts
   from the `mn=` parameter, and — critically — **fetches googlevideo media
   in bounded 4 MB chunks via the system `curl`** (`serve_chunked_media`,
   `curl_fetch_range`), because YouTube refuses open-ended reads. reqwest
   uses `native-tls` (schannel). Refusals are logged with signatures masked
   and the last refused URL is written to `logs\refused_url.txt`.
6. **Downloads** (`youtube/downloads.rs`): a persisted queue running yt-dlp
   with `--embed-thumbnail --embed-metadata --print after_move:filepath`.
   Proof of completion is a **media** file on disk (fixed in `a63f8bc`: the
   thumbnail used to pass as the finished file). Full films download at
   YouTube's throttled ~400 KiB/s; that is normal.
7. **Range export** (`youtube/ranges.rs`): the K-K marked range of a
   YouTube video is cut with ffmpeg **from the proxied streams** (already
   resolved, full speed) into `yt_ranges\`, then handed to the ordinary
   clip/GIF/describe exporters. Do not switch this to
   `yt-dlp --download-sections`: measured 160 s for a 10 s range versus
   4 s this way.
8. **Captions** (`youtube/captions.rs`) list with `--skip-download
   --ignore-no-formats-error` — that flag belongs there only; it must never
   reach the download or resolve paths or failures become silent.
9. **yt-dlp self-update** (`mpv/ytdlp_updater.rs`): once a day, 20 s after
   launch, `pip install -U --pre yt-dlp` for the Python path or
   `yt-dlp.exe --update-to nightly` for the exe; setting
   `YTDLP_UPDATE_CHANNEL` = Nightly (default) / Stable / Off.

### 5.5 Other subsystems, one line each

- **Enhancements** (`useVideoEnhancements`, `useVideoPresets`,
  `usePlaybackAdjustments`): quality preset, sharpen (GPU unsharp shader),
  denoise (`hqdn3d`, forces `hwdec=auto-copy`), deinterlace, colour grade,
  film grain, AI upscale (bundled shaders, **works only from the installed
  app**, not the raw exe), presets, Auto Enhance (histogram, not neural).
- **AI colour correction** (`ai_enhance.rs`): user-supplied cloud API key.
- **AI subtitles** (`ai_subtitles.rs`): Gemini, Sarvam, OpenAI-style and
  local whisper.cpp (`C:\Users\eashw\whisper-arm64\whisper-cli.exe`, native
  ARM64 build, `ggml-large-v3` — slow on CPU). Owner's ranking: Gemini >
  Sarvam > local Whisper.
- **Shortcuts** (`constants/shortcuts.ts`, `usePlaybackShortcuts`,
  `ShortcutSettings.vue`): chord = `event.code` + modifiers; stored as the
  "Keyboard Shortcuts" settings group; `Esc`, `?`, `F1` reserved.
- **Window geometry** (`useVideoGeometry`, `useWindowSizeLock`): per-file
  aspect/crop/fit, window size lock, step resize.
- **Seek thumbnails / storyboards / posters**: headless mpv as above;
  caches under `com.lumo.player\{seek_thumbs,storyboards,posters,thumbnails}`,
  pruned to the newest N directories; all four extraction paths are gated by
  the `VIDEO_ARTWORK_THUMBNAIL` toggle (posters, storyboards, artwork) and
  `SEEK_THUMBNAILS_ENABLED`.
- **Remote control** (`remote_control.rs`): a TCP listener; a second app
  instance logs "Only one usage of each socket address" and carries on.
- **Playlist drawer edge-reveal** (`PlaylistPeekButton.vue`): the hover
  strip covers only the middle third of the right edge so it does not steal
  the top-right Open File button.

### 5.6 Where things live at runtime

```
%LOCALAPPDATA%\Lumo\                 the installed app (lumo.exe, resources\)
%LOCALAPPDATA%\com.lumo.player\
  state.json                         ui-state: settings, playlists, per-file memory
  media.db                           SQLite: default playlist, history
  youtube_downloads.json             download queue
  logs\lumo.log                      app log (Rust `log` crate; .prev.log rotates)
  logs\soia.log                      mpv's own log (verbose; VO/AO/hwdec lines)
  logs\refused_url.txt               last googlevideo URL that 403'd
  seek_thumbs\ storyboards\ posters\ thumbnails\ yt_thumbs\ yt_captions\
  yt_ranges\                         cut YouTube ranges (last 5 kept)
  bgutil-pot-server\                 unpacked PO-token server
  ytdlp_update_stamp                 epoch of the last yt-dlp update attempt
%USERPROFILE%\Videos\YouTube\        default download folder (YOUTUBE_DOWNLOAD_DIR)
```

## 6. YouTube: the hard-won facts

These cost days to learn. Do not rediscover them.

1. **YouTube enforces a byte-position wall on stream URLs from older player
   clients** (seen 2026-08-18: ~8 MB, shrinking to ~4 MB, then 403 on every
   mirror). Small probes always succeed, so *every diagnostic that reads a
   small range lies*. Before believing a URL is healthy, read past 4 MB and
   read an open-ended range. The counter-move lives in yt-dlp (client
   hopping, VISIONOS at the time); it shipped in **nightly** a day later and
   in stable weeks later. That is why the self-updater defaults to nightly.
2. Open-ended reads (`Range: bytes=0-`, mpv's natural shape) are refused
   regardless of client. The proxy's 4 MB chunking is mandatory, not an
   optimisation.
3. It is not TLS, headers, IP, VPN, bursts or timing. All were eliminated
   (rustls, schannel and curl-as-child all refused equally). The native-tls
   switch (`864adec`) and the curl fetcher (`74382e0`) were made under
   wrong theories; they are harmless and still in place, but they are not
   the fix.
4. `youtubei/v1/player` is dead anonymously; yt-dlp is the only workable
   route to stream URLs. Reimplementing extraction would mean fighting the
   arms race alone — the decision was to keep yt-dlp and keep it fresh.
5. Anonymous URLs carry an `available_at` "ramp" (first byte withheld for
   seconds); the resolver waits it out. PO tokens remove it.
6. The bundled `yt-dlp.exe` is **stable 2026.07.04**, the walled version. On
   machines without Python it is what runs, and it self-updates to nightly
   on first launch only if the install dir is writable (per-user installs
   are). Before a wide release, either wait for a stable that contains the
   fix or bundle a nightly exe (one URL in
   `scripts/prepare_windows_binaries.mjs`).
7. The video id must be reasoned about carefully in file names: yt-dlp
   writes `Title [id].ext`, and the thumbnail gets the same stem — see the
   download fix above.

## 7. Release process

1. Bump the version in `package.json` (then `pnpm version:sync` propagates
   it to `tauri.conf.json` and `Cargo.toml`); write the entry at the top of
   `CHANGELOG.md`; update `RELEASE_NOTES.md` if the user-facing notes change.
2. Commit, then tag `vX.Y.Z` and push the tag. `release.yml` builds
   Windows (x64 + ARM64), macOS (Intel + Apple Silicon) and Linux
   (deb + AppImage), signs updater artefacts and publishes `latest.json`.
   `windows.yml`/`macos.yml`/`linux.yml` are the per-OS builds; `ci.yml`
   is the push check.
3. Required repository secrets, by name only: `TAURI_SIGNING_PRIVATE_KEY`,
   `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (the minisign key whose public half
   is in `tauri.conf.json`), and `SOIA_API` (optional telemetry/update
   base URL compiled in with `option_env!`; empty is correct and disables
   it). The private key exists only in GitHub and the owner's possession.
4. The Windows job fetches `yt-dlp.exe` and builds `bgutil-pot-server.zip`
   from source at build time (`scripts/prepare_windows_binaries.mjs`,
   pinned to bgutil 1.3.1, canvas dependency dropped, production-only
   install, then verifies every declared resource exists). Locally those
   two files are already in `src-tauri/binaries/` and git-ignored.
5. The in-app updater reads
   `https://github.com/eashwar83/lumo/releases/latest/download/latest.json`.
6. Installers are not code-signed; `RELEASE_NOTES.md` explains the
   SmartScreen / Gatekeeper prompts to users.

## 8. Debugging playbook

- **Playback won't start (YouTube)**: `logs\lumo.log` for `yt-dlp:` lines
  (resolve timing, selected formats, `bound=ip=… expire=… pot=…`) and
  `stream proxy:` lines; `logs\refused_url.txt` for the URL that 403'd;
  `logs\soia.log` for mpv's view (`VO: [gpu-next]`, `AO: [wasapi]` mean it
  is playing). Then probe the refused URL with curl past 4 MB (§6.1).
- **Something is slow**: measure before theorising. `soia.log` timestamps
  show how long mpv itself took (loadfile → first VO line); if that is
  small, the time is in the frontend. The 29k-folder stall and the
  "USB is slow" assumption were both wrong on first guess.
- **Something burns CPU/power**: sample per-thread CPU of `lumo.exe` and
  per-process CPU of `msedgewebview2.exe` over a few seconds with
  PowerShell (`Get-Process … .Threads`). A pegged WebView browser process
  means an IPC storm (an `invoke` loop); five equal-weight Rust threads
  means a decoder pool. Shaders show as GPU load, not CPU.
- **Disk fills up**: the four cache directories under `com.lumo.player`
  are safe to empty; the app regenerates. If one is growing without bound,
  a frame-extraction path lost its `frames` cap.
- **Build fails mysteriously**: nine times out of ten it is PATH (pnpm or
  clang) or the missing `vcvarsall arm64` environment. See §3.1.

## 9. Open items

### 9.1 Unreleased work on `main` (26 commits since v1.4.0)

Grouped, newest first. `git log --oneline v1.4.0..HEAD` has the exact list.

- Favourites: sort control (date/name/length) and "play from favourites"
  queue (`46b22fa`). Logic verified by a Node harness; **the Sort dropdown
  and duration badges have not been looked at by eye in the running app**.
- Downloads: a job that dies early is filed as failed, not done (`a63f8bc`).
- Playlist: 29k-file folder no longer stalls next-click (`d58c7d7`);
  poster loader no longer loops on a declined poster (`f69a758`); edge
  strip owns only the middle of the edge (`b4d9322`); YouTube tab no
  longer autofocuses the search box (`d58c7d7`).
- Player: progress events throttled to 4/s (`66c9ce0`).
- YouTube: range export of a K-K range — clip, GIF, AI description
  (`c3fe339`); yt-dlp self-updater (`79105bf`); the whole stream-proxy
  hardening series (chunking, curl, mirrors, refusal logging, native TLS).
- Settings: Video Artwork Thumbnail toggle covering posters, storyboards
  and now-playing artwork; runaway thumbnail/storyboard caches capped and
  self-healing; caption listing fixed.

### 9.2 Decisions waiting on the owner

- **Cut v1.4.1.** Blocked only by the bundled `yt-dlp.exe` question (§6.6).
  The `CHANGELOG.md` entry still needs writing from the list above.
- **Backfill `CHANGELOG.md` for 1.1.0 and 1.2.0** (offered, never done).
- **Resolve cache ignores network changes**: a cached YouTube resolution is
  IP-bound; after switching networks (VPN on/off) it 403s until the 40-min
  TTL expires. Fix is to key the cache on the egress IP or flush on
  network change. Noted, not started.
- **Simplify the proxy fetch path**: now that the real cause is known, the
  curl subprocess fetcher and the native-tls switch could be reverted to
  plain reqwest chunking. Only worth it if the proxy is being reworked
  anyway.
- `docs/TODO.md` holds the deliberately deferred ideas with context:
  pre-resolve on hover, batch/range downloads for playlists, command-palette
  entries for YouTube views, making `--remote-components` user-controllable.
  Delete an entry when it is done; do not tick it.

### 9.3 Loose ends on the machine

- A half-downloaded film (`…[xudlwB0SqSw].f616.mp4.part`, ~1.5 GB) sits in
  a Claude scratch folder under `%LOCALAPPDATA%\Temp\claude\…\dltest3`.
  Re-downloading from the app is simpler than rescuing it; delete the
  folder.
- Two stale "done" download records point at `.webp` files in
  `Videos\YouTube` (the Kondaveeti Donga and Warning films). Re-download
  them; the fix prevents new ones.

## 10. How the owner works — rules to keep

These are the owner's standing instructions, learned over the project.

1. **Discuss before coding when told to**, and for anything that changes
   the feel of the player or adds a dependency. "If you have doubts, discuss
   before coding" is a frequent phrasing. A short design summary, then
   build.
2. **Ask before adding a dependency** (npm, cargo or a runtime tool).
3. **Windows first, ARM64 only** locally. Never spawn a subprocess that
   flashes a console window (`CREATE_NO_WINDOW` everywhere; see
   `quiet_command` helpers). Never block the UI thread or the Tauri
   command thread — `spawn_blocking` for anything that touches disk, mpv or
   a subprocess.
4. **Each change ends with a committed, pushed commit and a fresh local
   installer**, then a one-paragraph explanation of what changed and how to
   verify it. Commit messages are narrative: a subject in the form
   `type(scope): what changed`, then a body explaining the failure that was
   observed, the mechanism, and the fix — written for a reader, not a log.
5. **Never commit** `src-tauri/libs/`, `CLAUDE-HANDOFF.md`,
   `Capture-LumoShots.ps1`, or the two git-ignored binaries.
6. **Secrets**: never print, echo or log an API key, cookie value, signing
   key or token. When identifying a stored key, length and last four
   characters only. Cookie files: names only. A GitHub token and an API key
   have leaked into transcripts before; the owner was told to rotate them.
7. **Report honestly.** When a theory is wrong, say so plainly and show the
   measurement that disproved it. The owner caught several wrong diagnoses
   by asking "but wasn't it working on the 12th?" — timeline questions
   deserve a straight answer before any new theory.
8. **Do not restart heavy background jobs on your own** after the machine
   runs low on memory; ask.
9. Keep `docs/TODO.md` for deliberately deferred work and
   `docs/USER_GUIDE.md` in step with user-visible features.
10. Style in code: comments explain constraints the code cannot show
    (why a flag pair must stay together, why a cap exists), not what the
    next line does. Match the surrounding idiom; the frontend is Vue 3
    `<script setup>` with composables, the backend is plain Rust with
    `log` macros (`info!/warn!/debug!`).

## 11. Cheat sheet

```powershell
# one-time per shell
$env:Path = "C:\usr\local;C:\Users\eashw\.llvm\bin;$env:Path"
$VC = '"C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvarsall.bat" arm64'

pnpm exec tsc --noEmit                                   # frontend types
cmd /c "$VC && cargo test --manifest-path src-tauri/Cargo.toml --target aarch64-pc-windows-msvc -- --test-threads=1"
cmd /c "$VC && pnpm tauri build --bundles nsis"          # installer
Get-Content "$env:LOCALAPPDATA\com.lumo.player\logs\lumo.log" -Tail 50
Get-Content "$env:LOCALAPPDATA\com.lumo.player\logs\soia.log" | Select-String "VO:|AO:|hwdec"
py -m yt_dlp --version                                   # the yt-dlp the app uses
git log --oneline v1.4.0..HEAD                           # unreleased work
```
