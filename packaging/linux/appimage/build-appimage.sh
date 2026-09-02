#!/usr/bin/env bash
# Builds a self-contained Linux AppImage for Infiltrator.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
BIN_PATH="${1:-target/release/infiltrator-iced}"
OUTPUT_PATH="${2:-dist/Infiltrator-x86_64.AppImage}"
ICON_PATH="$REPO_ROOT/crates/infiltrator-iced/icons/icon.png"

if [ ! -f "$BIN_PATH" ]; then
    echo "Error: Binary not found at $BIN_PATH" >&2
    exit 1
fi

APP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/infiltrator-appimage.XXXXXX")"
trap 'rm -rf "$APP_DIR"' EXIT

echo "[build-appimage] Constructing AppDir structure..."
mkdir -p "$APP_DIR/usr/bin"
mkdir -p "$APP_DIR/usr/share/applications"
mkdir -p "$APP_DIR/usr/share/icons/hicolor/512x512/apps"
mkdir -p "$APP_DIR/usr/lib"

# Copy binary
cp "$BIN_PATH" "$APP_DIR/usr/bin/infiltrator-iced"
chmod +x "$APP_DIR/usr/bin/infiltrator-iced"

# Copy desktop and icon metadata
cp packaging/linux/appimage/infiltrator.desktop "$APP_DIR/infiltrator.desktop"
cp packaging/linux/appimage/infiltrator.desktop "$APP_DIR/usr/share/applications/infiltrator.desktop"
test -s "$ICON_PATH"
cp "$ICON_PATH" "$APP_DIR/infiltrator.png"
cp "$ICON_PATH" "$APP_DIR/usr/share/icons/hicolor/512x512/apps/infiltrator.png"

# Copy AppRun
cp packaging/linux/appimage/AppRun "$APP_DIR/AppRun"
chmod +x "$APP_DIR/AppRun"

# Check for appimagetool
if ! command -v appimagetool >/dev/null 2>&1; then
    echo "[build-appimage] appimagetool not found, downloading standalone tool..."
    TOOL_PATH="${TMPDIR:-/tmp}/appimagetool"
    curl --fail --silent --show-error --location --retry 3 \
        -o "$TOOL_PATH" \
        "https://github.com/AppImage/AppImageKit/releases/download/13/appimagetool-x86_64.AppImage"
    chmod +x "$TOOL_PATH"
    APPIMAGETOOL="$TOOL_PATH"
else
    APPIMAGETOOL="appimagetool"
fi

mkdir -p "$(dirname "$OUTPUT_PATH")"
echo "[build-appimage] Running appimagetool with zstd compression..."
APPIMAGE_EXTRACT_AND_RUN=1 ARCH=x86_64 "$APPIMAGETOOL" \
    --no-appstream -comp zstd "$APP_DIR" "$OUTPUT_PATH"
chmod +x "$OUTPUT_PATH"

echo "[build-appimage] Successfully generated: $OUTPUT_PATH"
