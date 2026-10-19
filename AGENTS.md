# Working on Lumo

Read `docs/HANDOVER.md` first. It is the complete state transfer: machine
quirks, the only build command that works, architecture, the YouTube facts
that took days to learn, open items, and the owner's working rules.

The short version:

- Windows 11 on ARM64. Build only `aarch64-pc-windows-msvc`. `pnpm` is in
  `C:\usr\local`, `clang` in `C:\Users\eashw\.llvm\bin`, neither on PATH; the
  build runs inside `vcvarsall.bat arm64`. The exact command is in
  `docs/HANDOVER.md` §3.2 — use it verbatim.
- Frontend: Vue 3 `<script setup>` + composables (`src/composables`), root in
  `src/App.vue`. Backend: Rust + Tauri v2 + libmpv (`src-tauri/src`). Settings
  schema in `src/mock/settings.ts`; a new persisted top-level key must also be
  added to the `UiState` struct or it is silently dropped.
- Stateful mpv operations go through their composable, never raw commands.
  Subprocesses never flash a console window. Nothing blocks the UI thread.
- YouTube streams are served to mpv through the local proxy in 4 MB chunks
  because YouTube refuses open-ended reads; small test reads always succeed
  and prove nothing. yt-dlp is kept on its nightly channel on purpose.
- Discuss before coding when asked, ask before adding dependencies, finish
  each change with a commit, a push and a fresh installer, and never print a
  secret. Do not commit `src-tauri/libs/`, `CLAUDE-HANDOFF.md`,
  `Capture-LumoShots.ps1` or `src-tauri/binaries/*.exe|*.zip`.
- `pnpm exec tsc --noEmit` does not check `.vue` files and there is no test
  runner for the frontend; verify component changes in the running app.
