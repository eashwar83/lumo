# Release Notes

## Important (macOS)

macOS may show "Lumo is damaged and can't be opened" or say it cannot verify the app is free of malware.
This happens because the app is not yet signed with an Apple Developer ID certificate, so macOS may block it on first launch.

Workaround (Recommended):
1. Right-click Lumo.app
2. Click "Open"
3. Click "Open" again in the dialog

If that doesn't work, run:
```bash
sudo xattr -r -d com.apple.quarantine /Applications/Lumo.app
```

You can also go to System Settings > Privacy & Security and click "Open Anyway" (it appears after a blocked launch attempt).

## Important (Windows)

Windows SmartScreen may show "Windows protected your PC" and hide the Run button.
This happens because the installer is not yet signed with a code-signing certificate, not because anything is wrong with the download.

To continue: click "More info", then "Run anyway".

You can verify what you downloaded against the `.sha256` file published beside each installer.

The app is open-source and its code is publicly available for anyone to inspect.

## [1.4.0] - 2026-08-12

**YouTube playback works again.** Three separate faults had been compounding, and between them they took away the good video streams and then broke the ones that were left.

### YouTube playback

* **Videos play again.**
  Lumo's stream proxy decided what was a playlist by looking for `.m3u8` anywhere in the URL. A YouTube segment URL carries the playlist's name in the middle of its path and the real one at the end — `…/playlist/index.m3u8/…/file/seg.ts` — so every segment matched. A quarter megabyte of video was converted to text and rewritten line by line before being handed to the decoder, which understandably made nothing of it. Only the last part of the path answers that question now, and a body has to begin with `#EXTM3U` before a byte of it is rewritten.

* **Full quality is back.**
  Sending cookies restricts yt-dlp to the player clients that accept them, and for many videos those clients return no DASH streams at all. One film offered 27 formats anonymously and 11 while signed in, none of them DASH — so Lumo fell back to a low-quality stream and the fragile code path above. It now resolves anonymously and signs in only when YouTube actually asks for an account. Age-restricted videos still play; they take a moment longer.

* **Long videos no longer stall partway through.**
  The proxy keeps a limited number of playlists in memory and used to discard the oldest — which is always the one you are watching, since it was loaded first. It now discards the least recently used.

### Playlist

* **Images stay out of the playlist.**
  A folder of films usually has cover art sitting beside it, and those posters were becoming playlist entries. Playlists are video and audio only by default, everywhere a file can enter one — including Previous and Next walking a folder. Settings → General → **Include Images in Playlist** brings them back if you want them.

### Menu bar

* **The title of what is playing** now sits in the menu bar, where a title bar would put it.

* **Menus that no longer fit** collect into a » popup instead of being clipped off the edge, so every menu and the window buttons stay reachable however narrow the window gets.
