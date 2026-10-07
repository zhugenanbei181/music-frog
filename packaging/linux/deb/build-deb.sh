#!/usr/bin/env bash
# Constructs a Debian .deb package from a release binary and desktop metadata.
#
# Product identity is env-overridable so the iced and bevy UIs each get their
# own equal package from one tested template. The defaults reproduce the
# original iced package byte-for-byte.
#
#   APP_BIN           installed binary filename        (default infiltrator-iced)
#   APP_PKG           deb package / icon basename      (default infiltrator)
#   APP_DISPLAY_NAME  human name used in build logs    (default Infiltrator)
#   APP_DESKTOP       tracked .desktop template path   (default appimage/infiltrator.desktop)
#   APP_ICON          tracked PNG icon path            (default iced icon)
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
BIN_PATH="${1:-target/release/infiltrator-iced}"
VERSION="${2:-0.20.0}"
ARCH="${3:-amd64}"
OUTPUT_DIR="${4:-dist}"

APP_BIN="${APP_BIN:-infiltrator-iced}"
APP_PKG="${APP_PKG:-infiltrator}"
APP_DISPLAY_NAME="${APP_DISPLAY_NAME:-Infiltrator}"
APP_DESKTOP="${APP_DESKTOP:-packaging/linux/appimage/infiltrator.desktop}"
APP_ICON="${APP_ICON:-crates/infiltrator-iced/icons/icon.png}"
ICON_PATH="$REPO_ROOT/$APP_ICON"

if [ ! -f "$BIN_PATH" ]; then
    echo "Error: Binary not found at $BIN_PATH" >&2
    exit 1
fi

DEB_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/infiltrator-deb.XXXXXX")"
trap 'rm -rf "$DEB_ROOT"' EXIT

echo "[build-deb] Building directory tree for ${APP_DISPLAY_NAME} ${VERSION} (${ARCH})..."
mkdir -p "$DEB_ROOT/DEBIAN"
mkdir -p "$DEB_ROOT/usr/bin"
mkdir -p "$DEB_ROOT/usr/share/applications"
mkdir -p "$DEB_ROOT/usr/share/icons/hicolor/512x512/apps"

# Copy binary
cp "$BIN_PATH" "$DEB_ROOT/usr/bin/$APP_BIN"
chmod 755 "$DEB_ROOT/usr/bin/$APP_BIN"
case "$ARCH" in
    amd64) kernel_target=x86_64-unknown-linux-gnu ;;
    arm64) kernel_target=aarch64-unknown-linux-gnu ;;
    *) echo "Error: unsupported kernel architecture: $ARCH" >&2; exit 1 ;;
esac
bash "$REPO_ROOT/scripts/fetch-mihomo.sh" --target "$kernel_target" \
    --stage "$DEB_ROOT/usr/libexec/musicfrog/$APP_BIN/mihomo"

# Copy desktop and icon metadata
cp "$REPO_ROOT/$APP_DESKTOP" "$DEB_ROOT/usr/share/applications/${APP_PKG}.desktop"
test -s "$ICON_PATH"
cp "$ICON_PATH" "$DEB_ROOT/usr/share/icons/hicolor/512x512/apps/${APP_PKG}.png"

# Prepare DEBIAN/control
sed -e "s/@VERSION@/${VERSION}/g" \
    -e "s/@ARCH@/${ARCH}/g" \
    -e "s/@PACKAGE@/${APP_PKG}/g" \
    -e "s/@DISPLAY_NAME@/${APP_DISPLAY_NAME}/g" \
    packaging/linux/deb/debian/control > "$DEB_ROOT/DEBIAN/control"

# Copy control scripts (parameterized with the installed binary name)
for script in postinst prerm postrm; do
    sed -e "s/@APP_BIN@/${APP_BIN}/g" \
        packaging/linux/deb/debian/$script > "$DEB_ROOT/DEBIAN/$script"
    chmod 755 "$DEB_ROOT/DEBIAN/$script"
done

mkdir -p "$OUTPUT_DIR"
DEB_NAME="${APP_PKG}_${VERSION}_${ARCH}.deb"
dpkg-deb --build --root-owner-group "$DEB_ROOT" "$OUTPUT_DIR/$DEB_NAME"

echo "[build-deb] Created Debian package: $OUTPUT_DIR/$DEB_NAME"
