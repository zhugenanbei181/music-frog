#!/usr/bin/env bash
# Constructs a Debian .deb package from the release binary and desktop metadata.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
BIN_PATH="${1:-target/release/infiltrator-iced}"
VERSION="${2:-0.20.0}"
ARCH="${3:-amd64}"
OUTPUT_DIR="${4:-dist}"
ICON_PATH="$REPO_ROOT/crates/infiltrator-iced/icons/icon.png"

if [ ! -f "$BIN_PATH" ]; then
    echo "Error: Binary not found at $BIN_PATH" >&2
    exit 1
fi

DEB_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/infiltrator-deb.XXXXXX")"
trap 'rm -rf "$DEB_ROOT"' EXIT

echo "[build-deb] Building directory tree for Infiltrator ${VERSION} (${ARCH})..."
mkdir -p "$DEB_ROOT/DEBIAN"
mkdir -p "$DEB_ROOT/usr/bin"
mkdir -p "$DEB_ROOT/usr/share/applications"
mkdir -p "$DEB_ROOT/usr/share/icons/hicolor/512x512/apps"

# Copy binary
cp "$BIN_PATH" "$DEB_ROOT/usr/bin/infiltrator-iced"
chmod 755 "$DEB_ROOT/usr/bin/infiltrator-iced"

# Copy desktop and icon metadata
cp packaging/linux/appimage/infiltrator.desktop "$DEB_ROOT/usr/share/applications/infiltrator.desktop"
test -s "$ICON_PATH"
cp "$ICON_PATH" "$DEB_ROOT/usr/share/icons/hicolor/512x512/apps/infiltrator.png"

# Prepare DEBIAN/control
sed -e "s/@VERSION@/${VERSION}/g" \
    -e "s/@ARCH@/${ARCH}/g" \
    packaging/linux/deb/debian/control > "$DEB_ROOT/DEBIAN/control"

# Copy control scripts
cp packaging/linux/deb/debian/postinst "$DEB_ROOT/DEBIAN/postinst"
cp packaging/linux/deb/debian/prerm "$DEB_ROOT/DEBIAN/prerm"
cp packaging/linux/deb/debian/postrm "$DEB_ROOT/DEBIAN/postrm"
chmod 755 "$DEB_ROOT/DEBIAN/"*

mkdir -p "$OUTPUT_DIR"
DEB_NAME="infiltrator_${VERSION}_${ARCH}.deb"
dpkg-deb --build "$DEB_ROOT" "$OUTPUT_DIR/$DEB_NAME"

echo "[build-deb] Created Debian package: $OUTPUT_DIR/$DEB_NAME"
