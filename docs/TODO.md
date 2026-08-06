# Lumo — Deferred Work

Things worth doing that were consciously put off, with enough context to
pick them up cold. Not a bug tracker: these are known, accepted trade-offs
and unbuilt ideas, not defects to be fixed on sight.

When one is finished, delete its entry rather than marking it done — the
history lives in git.

---

## YouTube

### Pre-resolve on hover to hide the playback delay
**Deferred 2026-08-06.** A first play costs about 10 seconds:

| Stage | Cost |
| --- | --- |
| yt-dlp extraction | ~3–4s |
| Availability ramp (`wait_out_availability_ramp`) | ~5–6s |
| mpv open and buffer | ~1s |

YouTube withholds the first byte of a freshly minted URL until the
`available_at` it reports, so the ramp cannot be skipped — only moved
somewhere the user is not waiting on it. Doing both stages while a result
is merely hovered would make the click near-instant.

Most of this already works. `useYouTubeModule.ts` calls
`youtube_preresolve` for the **top 4 video results** when a search
completes, and a resolution is cached for 40 minutes
(`RESOLVE_CACHE_TTL`). Since the ramp wait now lives inside `resolve()`,
a pre-resolved entry has served its ramp too — so the first four results
should already be quick, and the 10 seconds is what you pay for anything
further down the list.

The gap is coverage, not plumbing: extend it to whatever the pointer is
resting on, beyond the initial four.

Worth knowing before starting:
- Every pre-resolve is a yt-dlp process and a YouTube request. Firing on
  every hover during a scroll would be abusive; it needs a dwell delay and
  a cap on in-flight resolves. `PRERESOLVE_GENERATION` in `data.rs`
  already exists to abandon a superseded batch.
- Resolutions are IP-bound and expire, so pre-resolving far ahead of a
  click wastes work and can hand back a dead URL.
- Worth measuring before building: if the top-4 case is now genuinely
  instant, hover may only matter for long result lists.

### Batch and range downloads for playlists and channels
**Deferred 2026-08-04.** Downloading a playlist is one item at a time.
Wanted: select a range ("episodes 3–12"), or queue a whole
playlist/channel. `youtube_download_add` already takes items individually
and the queue engine handles concurrency, so this is mostly selection UI
plus a bulk enqueue.

### Command-palette entries for the YouTube views
**Deferred 2026-08-04.** Trending, Downloads, channel and playlist views
are reachable only by clicking through the panel. They should be
addressable from the command palette like the rest of the app.

---

## Stream proxy

### `--remote-components ejs:github` is not user-controllable
**Raised 2026-08-05, no decision.** Lumo passes this to yt-dlp on every
call so it can fetch the JS challenge solver; without it, signed-in
requests return storyboard images only and nothing plays. It means yt-dlp
downloads a script from its own GitHub repo at runtime (cached
afterwards), and it needs network access on first use.

That was accepted to get playback working. If it should be a setting the
user can turn off, it belongs beside the other yt-dlp options in
Settings → YouTube.
