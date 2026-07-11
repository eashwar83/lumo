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

## [0.2.8] - 2026-07-11

### Highlights

* **Improved SMB Browsing & Playback**
  SMB stream proxy now uses async pipeline reads and respects server-negotiated max read size, resulting in smoother browsing and more reliable playback over SMB shares.

* **More yt-dlp Configuration Options**
  Added configurable max stream resolution, cookies-from-browser support, and improved format selection (preferring avc1 over vp9). YouTube `/show/` playlist URLs are now recognized as playlists.

### Fixes

* Fixed video aspect ratio distortion when resizing the window while paused on macOS.

* Fixed AppImage failing to launch on Fedora by excluding bundled libpulse.

* Fixed playback speed resetting when switching between videos.

* Fixed playback history not being saved before switching tracks.
