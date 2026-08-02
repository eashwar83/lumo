# Lumo — YouTube Module: Implementation Spec

Companion to the mock in `YouTube Browser.dc.html`. Hand both to Claude Code.
Stack assumption: yt-dlp + ffmpeg CLI subprocesses, mpv playback (Lumo's existing engine), Windows first.

## 1. Placement & entry points
- New **YouTube icon in the left rail** (between Favourites and Network), Lucide `youtube` glyph, active state = lighter bg + 2px violet inset accent on the left edge (see mock).
- Menu route (Lumo rule: menu is an additional route, never a second implementation): `Media → Open YouTube…`; pasting any youtube.com / youtu.be URL into the existing home URL field routes into this module (video → play, playlist → playlist view, channel → channel view).
- Command palette entries: "YouTube: Search", "YouTube: Trending", "YouTube: Downloads", "YouTube: Paste link".

## 2. Internal navigation (state machine)
```
views: search | trending | channel(id) | playlist(id) | downloads | history | player(videoId)
modals: download(target) | settings
```
- Section tabs (pills): Search, Trending, History, Downloads (badge = active download count).
- Channel/playlist views are drill-ins from result rows; they keep the Search tab highlighted and show a "‹ Back to results" breadcrumb.
- Esc follows Lumo's layered rule: modal → drawer → player → browser → (rail) home.
- All views keep scroll position when navigating back.

## 3. Data layer (yt-dlp, JSON, no API key)
One long-lived worker that spawns yt-dlp with `--dump-json` / `-J --flat-playlist` and caches results (LRU, ~15 min TTL). Thumbnails: fetch `i.ytimg.com/vi/<id>/mqdefault.jpg`, disk-cache.

| Surface | Command sketch |
|---|---|
| Search | `yt-dlp -J --flat-playlist "ytsearch25:<query>"` (append `date` variant for date sort). Filters (duration, upload date, type, HD) need Innertube search params — implement a thin `youtubei/v1/search` POST (key-less, `WEB` client context) and fall back to post-filtering ytsearch results if it breaks. |
| Trending | `yt-dlp -J --flat-playlist https://www.youtube.com/feed/trending` (category tabs = trending sub-feeds via Innertube `browse` `FEtrending` params). |
| Channel | `-J --flat-playlist https://www.youtube.com/@handle/videos` (`/playlists`, `/about` per tab). Paginate with `--playlist-items N:M`. |
| Playlist | `-J --flat-playlist <playlist_url>` |
| Video detail (player) | `yt-dlp -J <url>` → formats, chapters, description, related is NOT included → use Innertube `next` endpoint for Up-next + comments continuation, or `--write-comments` for comments only (slower). |
| Comments | Innertube `next`/continuation (fast, paged) preferred; fallback `yt-dlp --write-comments --no-download`. |

Pagination: infinite scroll, fetch next page at 80% scroll ("Scroll to load more" row in mock is the sentinel).

## 4. Playback (mpv)
- Play = load URL into the existing mpv instance: `loadfile ytdl://<id>` with mpv's ytdl hook pointed at the bundled yt-dlp (`script-opts=ytdl_hook-ytdl_path=...`).
- Quality: global default in Settings + per-video override chip in the control bar (mock, bottom right). Map to `--ytdl-format`:
  - `2160p → bv*[height<=2160]+ba/b`, `1440p`, `1080p`, `720p` analogous; "Auto best" = `bv*+ba/b`.
  - Override mid-play = set `ytdl-format` property then `loadfile` same URL with `start=<current pos>`.
- Cookies: append `--cookies-from-browser <edge|chrome|firefox>` to every yt-dlp call (search worker + ytdl hook raw-options + downloader) when enabled.
- Up next / autoplay: when file ends and autoplay toggle is on, load first related item; related list also feeds the playlist drawer (Tab) so `<`/`>` walk it.
- Resume: reuse Lumo's per-file position store, keyed by canonical video ID (`yt:<11-char id>`), not the resolved stream URL.
- Title bar: show video title · channel (mock player header), not the googlevideo URL.

## 5. Integrations (all present in mock)
- **Chapters → Scenes**: `chapters` array from `yt-dlp -J` becomes Lumo scene marks (ticks on seek bar, Playback → Next/Previous Scene, "Chapters" tab in the player drawer with NOW highlight).
- **SponsorBlock → Skip Markers**: GET `https://sponsor.ajay.app/api/skipSegments?videoID=<id>&categories=[...]` (categories from Settings chips). Segments render green on the seek bar and register as Lumo skip markers; auto-skip fires the existing skip mechanism and shows the toast "Skipped sponsor (a – b) · Undo" (mock, bottom left). Undo seeks back and disables that segment for the session. Fail silent on API errors.
- **Favourites**: hearting a YouTube video stores `{videoId, title, channel, duration, thumbUrl}`; favourite entries with `yt:` keys play via ytdl. Channels and playlists can be hearted too (open their view on click).
- **History**: local only (SQLite/JSON), row = id, title, channel, duration, position, watchedAt. Never synced to YouTube. "Clear history…" confirms first.

## 6. Downloader
Queue engine, N concurrent workers (Settings, default 2), each a yt-dlp subprocess.

Per-item args (from Download dialog choices):
```
yt-dlp <url>
  -f "bv*[height<=1080]+ba/b"            # from Quality radio
  --remux-video mkv                       # container seg control (mp4|mkv|webm)
  -x --audio-format mp3 --audio-quality 0 # instead of -f, when "Audio only"
  --embed-subs --sub-langs "en.*,-live_chat"   # Embed subtitles + language chip
  --embed-thumbnail --embed-chapters --embed-metadata
  --cookies-from-browser edge
  --limit-rate 5M                         # Settings: speed limit (omit when off)
  -o "D:/Videos/YouTube/%(title)s [%(id)s].%(ext)s"
  --newline --progress-template "%(progress)j"   # parse JSON lines for progress
  --continue --retries 5 --fragment-retries 10
```
- Playlist/channel batch: same command on the playlist/channel URL + `--yes-playlist -o "...%(playlist_index)03d - %(title)s..."`; enqueue as one group row that expands per item (v2 ok: flat items).
- Estimated sizes in the Quality list come from the formats JSON (`filesize` / `filesize_approx`).
- States per item: `queued → downloading → done | failed(retrying n/5) | paused | cancelled`. Pause = kill process (resume re-runs with `--continue`). Retry backoff 20s shown in row status (mock, red row).
- Toolbar: aggregate counts, speed-limit + concurrency chips (read-only, edit in Settings), Open folder, Pause all/Resume all.
- **Library handoff**: on completion, register the file with Lumo's library so it appears in Recent (and is heartable) — mock's "In Library ✓" chip. Toast + Downloads tab badge decrement.

## 7. Download dialog (mock: modal)
- Opens from any download icon. Header = video summary. Sections: Quality (radios w/ codec + size, default pre-selected + "Default" tag), Container (MP4/MKV/WebM seg; MKV note "keeps any codec without re-encoding"), Extras (embed subs + lang picker, thumbnail cover, keep chapters "→ become Lumo Scenes"), Save-to row with Change….
- Footer: "N downloads ahead of this one" · Cancel · Add to queue (outline violet) · Download now (solid violet, jumps queue).
- For playlists/channels the same dialog gains an item-range field ("Videos 1–24") — apply settings to all.

## 8. Settings (mock: settings modal; lives under Settings → YouTube too)
```
youtube.cookies.enabled      bool      true
youtube.cookies.browser      enum      edge | chrome | firefox
youtube.playback.quality     enum      auto|2160|1440|1080|720   (1080 default)
youtube.playback.autoplay    bool      true
youtube.download.dir         path      D:\Videos\YouTube
youtube.download.template    string    %(title)s [%(id)s].%(ext)s
youtube.download.concurrent  int 1–4   2
youtube.download.rateLimit   MB/s|off  5
youtube.sponsorblock.enabled bool      true
youtube.sponsorblock.cats    set       sponsor,intro,selfpromo (outro off)
youtube.chaptersToScenes     bool      true
youtube.rememberPosition     bool      true
youtube.ytdlpPath            path      bundled; "Check for updates" runs yt-dlp -U
```
Cookie status line shows last import time + what it unlocks; cookies never leave the device.

## 9. Keyboard & misc
- `/` focuses YouTube search (when module open); Enter searches; Ctrl+D = download focused row; Tab = playlist drawer as everywhere in Lumo; all rebindable.
- Row affordances (always visible, muted → white on hover): Play, Add to queue, Download, Heart. Thumbnail + title also play (single click, per "play immediately" decision).
- Errors: geo/age-restricted without cookies → inline row banner "Sign-in required — import cookies in YouTube settings" (button opens settings); 403/throttle → suggest yt-dlp update; offline → cached results + banner.

## 10. Visual language (match the mock, which matches Lumo)
Window #141417, cards #1c1c21 with #26262c borders (hover #212127/#38383f), radius 10–12px, pill chips, violet #8b7cf7 primary + selection, blue #2f8ef5 progress/seek, pink #e8556d hearts, green #9fd8a4 success (cookies, In Library, SponsorBlock), red #e8556d failure, text #ececef / #9a9aa2 / #6f6f78, Segoe UI, 44px+ hit targets on rail.
