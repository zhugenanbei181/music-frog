#!/usr/bin/env bash
# Builds a self-contained Linux AppImage for an Infiltrator UI.
#
# Product identity is env-overridable so the iced and bevy UIs each get their
# own equal AppImage from one tested template. Defaults reproduce the original
# iced package.
#
#   APP_BIN      binary filename inside the image   (default infiltrator-iced)
#   APP_PKG      desktop/icon basename              (default infiltrator)
#   APP_DESKTOP  tracked .desktop template path     (default appimage/infiltrator.desktop)
#   APP_ICON     tracked PNG icon path              (default iced icon)
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
BIN_PATH="${1:-target/release/infiltrator-iced}"
OUTPUT_PATH="${2:-dist/Infiltrator-x86_64.AppImage}"

APP_BIN="${APP_BIN:-infiltrator-iced}"
APP_PKG="${APP_PKG:-infiltrator}"
APP_DESKTOP="${APP_DESKTOP:-packaging/linux/appimage/infiltrator.desktop}"
APP_ICON="${APP_ICON:-crates/infiltrator-iced/icons/icon.png}"
ICON_PATH="$REPO_ROOT/$APP_ICON"

if [ ! -f "$BIN_PATH" ]; then
    echo "Error: Binary not found at $BIN_PATH" >&2
    exit 1
fi

APP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/infiltrator-appimage.XXXXXX")"
trap 'rm -rf "$APP_DIR"' EXIT

echo "[build-appimage] Constructing AppDir structure for ${APP_PKG}..."
mkdir -p "$APP_DIR/usr/bin"
mkdir -p "$APP_DIR/usr/share/applications"
mkdir -p "$APP_DIR/usr/share/icons/hicolor/512x512/apps"
mkdir -p "$APP_DIR/usr/lib"

# Copy binary
cp "$BIN_PATH" "$APP_DIR/usr/bin/$APP_BIN"
chmod +x "$APP_DIR/usr/bin/$APP_BIN"

# Copy desktop and icon metadata
cp "$REPO_ROOT/$APP_DESKTOP" "$APP_DIR/${APP_PKG}.desktop"
cp "$REPO_ROOT/$APP_DESKTOP" "$APP_DIR/usr/share/applications/${APP_PKG}.desktop"
test -s "$ICON_PATH"
cp "$ICON_PATH" "$APP_DIR/${APP_PKG}.png"
cp "$ICON_PATH" "$APP_DIR/usr/share/icons/hicolor/512x512/apps/${APP_PKG}.png"

# Copy AppRun, substituting the packaged binary name.
sed "s/@APP_BIN@/${APP_BIN}/g" packaging/linux/appimage/AppRun > "$APP_DIR/AppRun"
chmod +x "$APP_DIR/AppRun"

# Check for appimagetool
if ! command -v appimagetool >/dev/null 2>&1; then
    echo "[build-appimage] appimagetool not found, downloading standalone tool..."
    TOOL_PATH="${TMPDIR:-/tmp}/appimagetool"
    curl --fail --silent --show-error --location --retry 3 \
        -o "$TOOL_PATH" \
        "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage"
    chmod +x "$TOOL_PATH"
    APPIMAGETOOL="$TOOL_PATH"
else
    APPIMAGETOOL="appimagetool"
fi

mkdir -p "$(dirname "$OUTPUT_PATH")"
echo "[build-appimage] Running appimagetool with zstd compression..."
APPIMAGE_EXTRACT_AND_RUN=1 ARCH=x86_64 "$APPIMAGETOOL" \
    --no-appstream --comp zstd "$APP_DIR" "$OUTPUT_PATH"
chmod +x "$OUTPUT_PATH"

echo "[build-appimage] Successfully generated: $OUTPUT_PATH"
