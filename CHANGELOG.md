# Changelog

## Important (macOS)
- If macOS blocks the downloaded app, run:
  ```bash
  sudo xattr -r -d com.apple.quarantine /Applications/Soia.app
  ```
- Why: apps downloaded from the internet may get a quarantine attribute (`com.apple.quarantine`) from Gatekeeper. This command removes that attribute recursively so Soia can launch normally.

## [0.1.1] - 2026-03-28

First official release.

### Features

- MPV-backed local media playback.
- Picture in Picture (PiP) support on macOS and Windows.
- Borderless window mode on macOS, Windows, and Linux (Ubuntu Wayland).
- HDR and Dolby Vision playback on compatible media/hardware (not supported on Linux).
- Playback history with resume position and pin-to-top support.
- Multiple playlist support with sort modes and loop/shuffle behaviors.
- WebDAV media browsing and streaming.
- Buffering progress bar for network video playback.
- Download speed display when network video buffering is paused.
- Playback preferences (seek step, default speed, auto-play, skip intro).
- macOS platform integrations for media keys, Now Playing metadata, and artwork capture.

### Platform Notes

- Linux builds currently target Ubuntu Wayland sessions only (`X11` is not supported).
