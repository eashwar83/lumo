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

## [0.2.4] - 2026-05-30

### Features

- Added a Live playback experience for IPTV/M3U streams, including a Live indicator and a cleaner control bar without seek progress for live channels.
- Added a confirmation prompt before Soia automatically creates a playlist from an imported playlist file or URL, with the option to edit the playlist name first.

### Fixes

- Fixed Network browser breadcrumbs so deep folder paths collapse by available width instead of overflowing the header.
- Fixed a startup issue where the app window could briefly appear as a transparent shell before the first render.

### Improvements

- Refined the macOS compact top bar icon styling.
- Updated the Settings navigation icon.
