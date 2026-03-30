# Release Notes

## Important (macOS)
- If macOS blocks the downloaded app, run:
  ```bash
  sudo xattr -r -d com.apple.quarantine /Applications/Soia.app
  ```
- Why: apps downloaded from the internet may get a quarantine attribute (`com.apple.quarantine`) from Gatekeeper. This command removes that attribute recursively so Soia can launch normally.

## [0.1.2] - 2026-03-30

### Fixes

- Fixed an issue where the video window could appear transparent or fail to render on certain macOS versions (including macOS Tahoe).
- Added MoltenVK Vulkan ICD manifest setup for both development and packaged app runtime to ensure stable video output.
