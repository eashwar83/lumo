# Changelog

## Important (macOS)
- If macOS blocks the downloaded app, run:
  ```bash
  sudo xattr -r -d com.apple.quarantine /Applications/Soia.app
  ```
- Why: apps downloaded from the internet may get a quarantine attribute (`com.apple.quarantine`) from Gatekeeper. This command removes that attribute recursively so Soia can launch normally.
