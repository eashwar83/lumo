#!/bin/bash
set -euo pipefail

usage() {
  cat <<EOF
Usage:
  $0 <path-to-app> [output-dmg] [volume-name]

Examples:
  $0 src-tauri/target/release/bundle/macos/Soia.app
  $0 src-tauri/target/release/bundle/macos/Soia.app src-tauri/target/release/bundle/dmg/Soia_0.1.0_aarch64.dmg Soia
EOF
}

if [[ $# -lt 1 ]]; then
  usage
  exit 1
fi

APP_PATH="$1"
OUT_DMG="${2:-}"
VOL_NAME="${3:-}"

if [[ ! -d "$APP_PATH" ]]; then
  echo "[ERROR] App bundle not found: $APP_PATH"
  exit 1
fi

APP_NAME="$(basename "$APP_PATH" .app)"
if [[ -z "$VOL_NAME" ]]; then
  VOL_NAME="$APP_NAME"
fi

if [[ -z "$OUT_DMG" ]]; then
  APP_PARENT="$(dirname "$APP_PATH")"
  BUNDLE_DIR="$(cd "$APP_PARENT/.." && pwd)"
  mkdir -p "$BUNDLE_DIR/dmg"
  OUT_DMG="$BUNDLE_DIR/dmg/${APP_NAME}.dmg"
else
  mkdir -p "$(dirname "$OUT_DMG")"
fi

TMP_DIR="$(mktemp -d /tmp/soia_dmg_src.XXXXXX)"
cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

cp -R "$APP_PATH" "$TMP_DIR/"
ln -s /Applications "$TMP_DIR/Applications"

echo "[INFO] Creating DMG: $OUT_DMG"
hdiutil create \
  -volname "$VOL_NAME" \
  -srcfolder "$TMP_DIR" \
  -ov \
  -format UDZO \
  "$OUT_DMG"

echo "[DONE] DMG created: $OUT_DMG"
