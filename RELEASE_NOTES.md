# Release Notes

## Important (macOS)

macOS may show "Soia is damaged and can't be opened" or say it cannot verify the app is free of malware.
This happens because the app is not yet signed with an Apple Developer ID certificate, so macOS may block it on first launch.

Workaround (Recommended):
1. Right-click Soia.app
2. Click "Open"
3. Click "Open" again in the dialog

If that doesn't work, run:
```bash
sudo xattr -r -d com.apple.quarantine /Applications/Soia.app
```

You can also go to System Settings > Privacy & Security and click "Open Anyway" (it appears after a blocked launch attempt).

The app is open-source and its code is publicly available for anyone to inspect.

## [0.1.5] - 2026-04-11

### Highlights

- Added experimental support for Windows Portable and Linux AppImage builds.
- Improved Custom Shader settings with mode-aware behavior and cleaner controls.
- Added Anime-specialized mode with automatic detection and automatic shader application.
- Added Factory Reset to quickly clear local data and restore default state.

### Improvements

- Added per-playback shader toggle and active shader display.
- On Windows, Update now uses different behavior for Portable vs Setup installs.
