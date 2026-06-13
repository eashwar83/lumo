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

## [0.2.6] - 2026-06-13

### Highlights

- macOS now uses a vibrancy effect with a transparent glass background for a more native desktop look.
- Added a built-in Favorites Playlist that stays at the top of the playlist drawer, with an Add to Favorites action in the playback right-click menu.
- Added right-click actions to the URL field for copy, cut, paste, delete, and select all.

### Fixes

- Fixed excessive playback progress disk writes while media is playing.
