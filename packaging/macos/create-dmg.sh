#!/usr/bin/env bash
# Generates a polished macOS DMG with /Applications symlink, icon positioning, and volume styling.
set -euo pipefail

DMG_PATH="${1:-dist/Infiltrator-macOS.dmg}"
APP_PATH="${2:-dist/Infiltrator.app}"
VOL_NAME="Infiltrator"

if [ ! -d "$APP_PATH" ]; then
    echo "Error: App bundle not found at $APP_PATH" >&2
    exit 1
fi

TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/infiltrator-dmg.XXXXXX")"
trap 'rm -rf "$TMP_DIR"' EXIT

echo "[create-dmg] Staging DMG contents in $TMP_DIR..."
cp -R "$APP_PATH" "$TMP_DIR/"
ln -s /Applications "$TMP_DIR/Applications"

# If create-dmg tool is available, use it for rich visual layout
if command -v create-dmg >/dev/null 2>&1; then
    echo "[create-dmg] Using create-dmg CLI..."
    rm -f "$DMG_PATH"
    create-dmg \
        --volname "$VOL_NAME" \
        --window-pos 200 120 \
        --window-size 600 400 \
        --icon-size 128 \
        --icon "Infiltrator.app" 175 120 \
        --hide-extension "Infiltrator.app" \
        --app-drop-link 425 120 \
        "$DMG_PATH" \
        "$TMP_DIR"
else
    # Standard hdiutil fallback
    echo "[create-dmg] Using hdiutil fallback..."
    rm -f "$DMG_PATH"
    hdiutil create -volname "$VOL_NAME" -srcfolder "$TMP_DIR" -ov -format UDZO "$DMG_PATH"
fi

echo "[create-dmg] Generated DMG at $DMG_PATH"
