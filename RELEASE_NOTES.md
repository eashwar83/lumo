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

## [0.1.9] - 2026-05-04

### Highlights

- Improved macOS SDR Rendering: Optimized color accuracy for a more precise visual experience on macOS.
- DLNA Auto-Play: Added support for automatically playing the next item in the queue when streaming via DLNA.
- M3U (IPTV) Parsing: You can now import M3U files to automatically parse channels and generate organized playlists.

### Fixes

- Fixed incorrect play/pause button state.
- Fixed inability to manually play the next item.
