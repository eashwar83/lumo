# Release Notes

## Important (macOS)
- If macOS blocks the downloaded app, run:
  ```bash
  sudo xattr -r -d com.apple.quarantine /Applications/Soia.app
  ```
- Why: apps downloaded from the internet may get a quarantine attribute (`com.apple.quarantine`) from Gatekeeper. This command removes that attribute recursively so Soia can launch normally.

## [0.1.4] - 2026-04-04

### Highlights

- Dual subtitles support.
- Custom shader support.
- Experimental Wallpaper mode support on Windows.
- Common image formats can now be played and browsed, and can be combined with playlist + Wallpaper mode.

### Fixes

- Fixed a potential crash when quitting the app during playback.
