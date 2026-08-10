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

## [1.3.0] - 2026-08-06

Headline release: YouTube is built into Lumo. Search it, play it, download it,
read and translate its comments — without a browser.

### YouTube

* **Search and browse**
  Search with sort, duration, upload-date, type and HD filters. Open a channel or a playlist and browse it in place, with the same sort options the website offers. Recent and saved searches live in the search box itself and filter as you type.

* **Play in Lumo**
  Results play directly in the player at your chosen quality, with a per-video override. yt-dlp ships with the app, so there is nothing to install.

* **Up next, chapters and SponsorBlock**
  Related videos queue up and auto-advance, chapters appear as scene markers on the seek bar, and sponsor segments are skipped with an undo prompt — all in the **Extras** drawer beside the player.

* **Downloads**
  A queue with progress, speed and ETA; pause, resume, cancel and reorder. Choose quality, container, audio-only, subtitles, thumbnails and chapters.

* **Subtitles from YouTube**
  Pick any caption track the video offers, straight from the Subtitle menu, with a filter box for the long language list.

* **Comments**
  Read them in the drawer, sorted by Top or Newest, with replies expandable inline. Search every comment, including the ones not yet loaded. Export to PDF — all or selected, with original and translation side by side.

* **AI comment translation**
  Translate all comments or just the ones you tick, with your own choice of provider and model, and a refresh button that asks the provider what models it currently offers.

### Subtitles and AI

* **Subtitle sync** — quick offset nudging plus Smart AI re-timing for subtitles that drift.

* **Describe A–B Clip (AI)** — a vision-model summary of the marked range.

* AI subtitle jobs now ride out provider rate limits with a countdown and adaptive pacing instead of giving up.

### Video

* **Deblock filter** and a stronger **Old Film Restore** preset.

### Fixes

* Playback failures now report what actually went wrong instead of a generic message. The app writes its own diagnostic log to `logs/lumo.log`, beside mpv's.

* Fixed several stream-proxy faults found while making YouTube play: HLS playlists exhausting the backend registry, a URL being proxied twice, oversized rewritten playlists, and a malformed `206` response.

* YouTube streams that die with a `403` are now re-resolved automatically.
