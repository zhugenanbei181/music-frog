#!/usr/bin/env bash
# Constructs a Debian .deb package from a release binary and desktop metadata.
#
# The product identity (package id, binary name, display name, icon and
# desktop entry) comes from the product identity registry and from nowhere
# else: APP_PRODUCT only selects which registered desktop product is
# packaged, every field is read back from the registry.
#
#   APP_PRODUCT   registered desktop product to package (default iced)
#   BIN_PATH      the release binary to package
#   VERSION       the package version (default: the workspace version)
#   ARCH          the Debian architecture: amd64 | arm64 (default amd64)
#   OUTPUT_DIR    where the .deb is written (default dist)
#
# The package itself is written by packaging/linux/deb/deb_archive.py, so the
# packaging leg does not depend on the `dpkg` tools being installed on the
# build machine.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
BIN_PATH="${1:-target/release/infiltrator-iced}"
APP_PRODUCT="${APP_PRODUCT:-iced}"
identity_value() {
    python3 "$REPO_ROOT/scripts/product_identity.py" --get "desktop.$APP_PRODUCT.$1"
}
VERSION="${2:-$(python3 "$REPO_ROOT/scripts/product_identity.py" --get version.name)}"
ARCH="${3:-amd64}"
OUTPUT_DIR="${4:-dist}"

APP_BIN="$(identity_value binary_name)"
APP_PKG="$(identity_value linux_package_id)"
APP_DISPLAY_NAME="$(identity_value display_name)"
APP_DESKTOP="$(identity_value linux_desktop_file)"
APP_ICON="$(identity_value icon_png)"
ICON_PATH="$REPO_ROOT/$APP_ICON"

if [ ! -f "$BIN_PATH" ]; then
    echo "Error: Binary not found at $BIN_PATH" >&2
    exit 1
fi

DEB_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/infiltrator-deb.XXXXXX")"
trap 'rm -rf "$DEB_ROOT"' EXIT
# The installed file tree and the package metadata are separate: only the
# tree becomes data.tar.gz, the control files become control.tar.gz.
DATA_ROOT="$DEB_ROOT/data"

echo "[build-deb] Building directory tree for ${APP_DISPLAY_NAME} ${VERSION} (${ARCH})..."
mkdir -p "$DEB_ROOT/DEBIAN"
mkdir -p "$DATA_ROOT/usr/bin"
mkdir -p "$DATA_ROOT/usr/share/applications"
mkdir -p "$DATA_ROOT/usr/share/icons/hicolor/512x512/apps"

# Copy binary
cp "$BIN_PATH" "$DATA_ROOT/usr/bin/$APP_BIN"
chmod 755 "$DATA_ROOT/usr/bin/$APP_BIN"
case "$ARCH" in
    amd64) kernel_target=x86_64-unknown-linux-gnu ;;
    arm64) kernel_target=aarch64-unknown-linux-gnu ;;
    *) echo "Error: unsupported kernel architecture: $ARCH" >&2; exit 1 ;;
esac
bash "$REPO_ROOT/scripts/fetch-mihomo.sh" --target "$kernel_target" \
    --stage "$DATA_ROOT/usr/libexec/musicfrog/$APP_BIN/mihomo"

# Copy desktop and icon metadata
python3 "$REPO_ROOT/scripts/render-product-template.py" \
    --input "$REPO_ROOT/$APP_DESKTOP" \
    --output "$DATA_ROOT/usr/share/applications/${APP_PKG}.desktop"
test -s "$ICON_PATH"
cp "$ICON_PATH" "$DATA_ROOT/usr/share/icons/hicolor/512x512/apps/${APP_PKG}.png"

# Prepare DEBIAN/control
sed -e "s/@VERSION@/${VERSION}/g" \
    -e "s/@ARCH@/${ARCH}/g" \
    -e "s/@PACKAGE@/${APP_PKG}/g" \
    -e "s/@DISPLAY_NAME@/${APP_DISPLAY_NAME}/g" \
    "$REPO_ROOT/packaging/linux/deb/debian/control" > "$DEB_ROOT/DEBIAN/control"

# Copy control scripts (parameterized with the installed binary name)
SCRIPT_ARGS=()
for script in postinst prerm postrm; do
    sed -e "s/@APP_BIN@/${APP_BIN}/g" \
        "$REPO_ROOT/packaging/linux/deb/debian/$script" > "$DEB_ROOT/DEBIAN/$script"
    chmod 755 "$DEB_ROOT/DEBIAN/$script"
    SCRIPT_ARGS+=(--script "$script=$DEB_ROOT/DEBIAN/$script")
done

mkdir -p "$OUTPUT_DIR"
DEB_NAME="${APP_PKG}_${VERSION}_${ARCH}.deb"
python3 "$REPO_ROOT/packaging/linux/deb/deb_archive.py" build \
    --output "$OUTPUT_DIR/$DEB_NAME" \
    --control "$DEB_ROOT/DEBIAN/control" \
    --tree "$DATA_ROOT" \
    "${SCRIPT_ARGS[@]}"

echo "[build-deb] Created Debian package: $OUTPUT_DIR/$DEB_NAME"
