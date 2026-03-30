# Changelog

## [0.1.2] - 2026-03-30

### Fixes

- Fixed an issue where the video window could appear transparent or fail to render on certain macOS versions (including macOS Tahoe).
- Added MoltenVK Vulkan ICD manifest setup for both development and packaged app runtime to ensure stable video output.

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
