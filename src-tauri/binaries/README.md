# Runtime binaries (not in git)

The Windows build bundles two large files from this directory that are
deliberately kept out of git. Recreate them before building:

1. **yt-dlp.exe** (~18 MB)
   curl -L -o yt-dlp.exe https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe

2. **bgutil-pot-server.zip** (~11 MB) — the bgutil PO-token provider server
   (github.com/Brainicism/bgutil-ytdlp-pot-provider), built with:
   git clone --depth 1 --branch 1.3.1 https://github.com/Brainicism/bgutil-ytdlp-pot-provider
   cd bgutil-ytdlp-pot-provider/server
   # remove "canvas" from package.json dependencies (no ARM64 prebuilt; unneeded)
   npm install --omit=optional && npm install -D typescript && npx tsc
   npm prune --omit=dev
   # zip build/, node_modules/, package.json into bgutil-pot-server.zip (zip root = those entries)

The `yt-dlp-plugins/` folder (checked in) is the matching yt-dlp plugin.
At runtime the app prefers a native ARM64 Python with `pip install yt-dlp
bgutil-ytdlp-pot-provider` (~7x faster) and falls back to yt-dlp.exe.
