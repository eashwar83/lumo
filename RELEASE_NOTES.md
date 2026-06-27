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

## [0.2.7] - 2026-06-27

### Highlights

* **YouTube Playlist Support**
  Paste a YouTube playlist URL and Soia will automatically load all videos into a playable playlist.

* **Built-in Online Subtitle Search**
  Search subtitles directly from the subtitle menu using OpenSubtitles and SubSource. Results are cached locally for faster access, and the cache can be cleared anytime from Settings.

### Fixes

* Fixed an issue on macOS where the window chrome did not update correctly when switching between light and dark appearance modes.

* Fixed playback control hover states that could occasionally block mouse interactions and prevent clicks from registering.

