#!/usr/bin/env bash
# Build the final Linux artifacts of one registered desktop product.
#
# The builder produces *both* release artifacts of the product from the
# product identity registry:
#
#   * the Debian package  <linux_package_id>_<version>_<arch>.deb
#   * the AppImage         <release_artifact_prefix>-Linux-<arch>.AppImage
#
# The package name, the binary name, the icon and the desktop entry are read
# from `product-identity.properties`; this script owns no identity of its
# own. After the build both artifacts are verified against the registry and a
# build manifest (JSON) records the artifact identities.
#
# Usage:
#   bash packaging/linux/build-linux.sh [--product iced] [--arch x86_64]
#                                       [--binary PATH] [--profile release]
#                                       [--out dist] [--skip-verify]
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

PRODUCT="iced"
ARCH="x86_64"
BINARY=""
PROFILE="release"
OUT_DIR="dist"
VERIFY=1

usage() {
    sed -n '2,20p' "${BASH_SOURCE[0]}"
    exit "${1:-0}"
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --product)
            PRODUCT="${2:-}"
            shift 2
            ;;
        --arch)
            ARCH="${2:-}"
            shift 2
            ;;
        --binary)
            BINARY="${2:-}"
            shift 2
            ;;
        --profile)
            PROFILE="${2:-}"
            shift 2
            ;;
        --out)
            OUT_DIR="${2:-}"
            shift 2
            ;;
        --skip-verify)
            VERIFY=0
            shift
            ;;
        --help | -h)
            usage 0
            ;;
        *)
            printf 'unknown argument: %s\n' "$1" >&2
            usage 2
            ;;
    esac
done

identity_value() {
    python3 "$REPO_ROOT/scripts/product_identity.py" --get "desktop.$PRODUCT.$1"
}

case "$ARCH" in
    x86_64 | amd64)
        ARCH="x86_64"
        DEB_ARCH="amd64"
        ;;
    aarch64 | arm64)
        ARCH="aarch64"
        DEB_ARCH="arm64"
        ;;
    *)
        printf 'unsupported architecture: %s\n' "$ARCH" >&2
        exit 2
        ;;
esac

APP_BIN="$(identity_value binary_name)"
APP_PKG="$(identity_value linux_package_id)"
APP_PREFIX="$(identity_value release_artifact_prefix)"
VERSION="$(python3 "$REPO_ROOT/scripts/product_identity.py" --get version.name)"
if [ -z "$BINARY" ]; then
    BINARY="target/$PROFILE/$APP_BIN"
fi

if [ ! -f "$BINARY" ]; then
    printf 'the release binary is missing: %s\n' "$BINARY" >&2
    printf 'build it first: cargo build --locked --release -p infiltrator-iced --bin %s\n' "$APP_BIN" >&2
    exit 2
fi

mkdir -p "$OUT_DIR"
DEB_PATH="$OUT_DIR/${APP_PKG}_${VERSION}_${DEB_ARCH}.deb"
APPIMAGE_PATH="$OUT_DIR/${APP_PREFIX}-Linux-${ARCH}.AppImage"

printf '[build-linux] product=%s version=%s arch=%s\n' "$PRODUCT" "$VERSION" "$ARCH"
printf '[build-linux] deb      <- %s\n' "$DEB_PATH"
APP_PRODUCT="$PRODUCT" bash "$REPO_ROOT/packaging/linux/deb/build-deb.sh" \
    "$BINARY" "$VERSION" "$DEB_ARCH" "$OUT_DIR"

printf '[build-linux] appimage <- %s\n' "$APPIMAGE_PATH"
APP_PRODUCT="$PRODUCT" APPIMAGE_ARCH="$ARCH" \
    bash "$REPO_ROOT/packaging/linux/appimage/build-appimage.sh" \
    "$BINARY" "$APPIMAGE_PATH"

if [ "$VERIFY" -eq 1 ]; then
    printf '[build-linux] verifying the artifacts against the product identity registry\n'
    python3 "$REPO_ROOT/packaging/linux/verify-linux-artifacts.py" \
        --product "$PRODUCT" --arch "$ARCH" --version "$VERSION" \
        --deb "$DEB_PATH" --appimage "$APPIMAGE_PATH" \
        --json "$OUT_DIR/${APP_PREFIX}-Linux-${ARCH}-identity.json"
fi

MANIFEST_PATH="$OUT_DIR/${APP_PREFIX}-Linux-${ARCH}-build.json"
python3 - "$MANIFEST_PATH" "$PRODUCT" "$VERSION" "$ARCH" "$DEB_ARCH" "$BINARY" "$DEB_PATH" "$APPIMAGE_PATH" <<'PY'
import hashlib
import json
import sys
from pathlib import Path

manifest_path, product, version, arch, deb_arch, binary, deb, appimage = sys.argv[1:9]


def digest(path):
    hasher = hashlib.sha256()
    with open(path, "rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            hasher.update(block)
    return hasher.hexdigest()


manifest = {
    "schema_version": 1,
    "platform": "linux",
    "product": product,
    "version": version,
    "arch": arch,
    "deb_arch": deb_arch,
    "binary": {"path": binary, "sha256": digest(binary)},
    "artifacts": [
        {
            "kind": "deb",
            "path": deb,
            "sha256": digest(deb),
            "command": f"bash packaging/linux/deb/build-deb.sh {binary} {version} {deb_arch} dist",
        },
        {
            "kind": "appimage",
            "path": appimage,
            "sha256": digest(appimage),
            "command": (
                f"APPIMAGE_ARCH={arch} bash packaging/linux/appimage/build-appimage.sh "
                f"{binary} {appimage}"
            ),
        },
    ],
}
Path(manifest_path).write_text(json.dumps(manifest, ensure_ascii=False, indent=2), encoding="utf-8")
print(f"[build-linux] build manifest: {manifest_path}")
PY

printf '[build-linux] done\n'
