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

## [0.1.6] - 2026-04-18

This release focuses on network playback experience and overall stability.

### Highlights

- Added **DLNA support**: discover devices, browse media, and play DLNA streams.
- Added **Playback Title display modes**, including the option to hide the top title bar for a cleaner playback UI.

### Fixes

- Fixed an issue where the screen could sleep during active video playback.
- Improved robustness of the update check process and fixed update flow issues.
