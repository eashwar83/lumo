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

## [0.2.0] - 2026-05-10

### Highlights

- SMB Browsing and Playback: Added SMB/Samba network share discovery, browsing, and direct playback support.
- Advanced Subtitle Appearance: Added persistent controls for subtitle font, color, size, and position, including separate primary and secondary subtitle positioning for dual subtitles.

### Fixes

- Improved network connection dialog interactions and path entry.
- Fixed the network browser so the currently playing item is revealed in the file list.