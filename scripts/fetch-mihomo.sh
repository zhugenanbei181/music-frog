#!/usr/bin/env bash
#
# fetch-mihomo.sh — download the vendored mihomo core binaries into vendor/.
#
# Usage:
#   ./scripts/fetch-mihomo.sh
#
# Run this ONCE after a fresh clone. The vendor/ binaries are no longer git-
# tracked, but packaging (cargo-packager resources in
# crates/infiltrator-iced/Cargo.toml), the vendored-kernel lookup paths in
# infiltrator-desktop, and the Android build (android/app/build.gradle.kts)
# all expect the files in vendor/ to exist.
#
# Downloads the three binaries from the official MetaCubeX/mihomo releases:
#   vendor/mihomo.exe              <- mihomo-windows-amd64-v3-<version>.zip
#   vendor/mihomo-android-amd64    <- mihomo-android-amd64-<version>.gz
#   vendor/mihomo-android-arm64-v8 <- mihomo-android-arm64-v8-<version>.gz
#
# Environment overrides:
#   MIHOMO_VERSION   release tag to fetch (default: the pinned version below).
#                    Checksum verification is only performed for the pinned
#                    version; any other version is downloaded unverified.
#   MIHOMO_BASE_URL  alternate download base (default: official GitHub releases).
#
# Requirements: bash, curl (or wget), unzip (or python3), gunzip, sha256sum.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
VENDOR_DIR="$REPO_ROOT/vendor"

PINNED_VERSION="v1.19.18"
MIHOMO_VERSION="${MIHOMO_VERSION:-$PINNED_VERSION}"
BASE_URL="${MIHOMO_BASE_URL:-https://github.com/MetaCubeX/mihomo/releases/download}"

# MIHOMO_VERSION/MIHOMO_BASE_URL end up inside URLs and filenames; keep them
# to a safe charset so nothing can smuggle shell/URL syntax through the env.
[[ "$MIHOMO_VERSION" =~ ^[A-Za-z0-9._-]+$ ]] \
    || { echo "[fetch-mihomo] ERROR: unsafe MIHOMO_VERSION: $MIHOMO_VERSION" >&2; exit 1; }
url_re='^https?://[A-Za-z0-9._~:/?#@!$&*+,;=%-]+$'
[[ "$BASE_URL" =~ $url_re ]] \
    || { echo "[fetch-mihomo] ERROR: unsafe MIHOMO_BASE_URL: $BASE_URL" >&2; exit 1; }

# sha256 of the release archives as published on the GitHub release page, and
# of the extracted binaries (verified against the original vendor/ copies).
if [[ "$MIHOMO_VERSION" == "$PINNED_VERSION" ]]; then
    SHA_WINDOWS_ZIP="2892ea9ea7d0699d82e087bdf09baf4db36ccbd5f60635ec952d86fa154f3d87"
    SHA_ANDROID_AMD64_GZ="8a1d17fed63732df475a898227bc0cfde27645cbfdd9d84d1458dc280f35ae7c"
    SHA_ANDROID_ARM64_GZ="07add62d4ca60eebbe5898f39ecff3ee12c30befccaf8966b5fa71a9e01799f5"
    SHA_WINDOWS_EXE="3dbf9a49398ab5608c285b9175d55ba4fb06fb914c69f59222024baff7f354ed"
    SHA_ANDROID_AMD64="0e806abfa76c1022edec800b1cacc8459ecfa57f2187963da4b6cf463933e580"
    SHA_ANDROID_ARM64="a208b2cf0194a85003639217dbc01d3c51260dfe45bd6d6663d1822b7e89008b"
else
    SHA_WINDOWS_ZIP=""; SHA_ANDROID_AMD64_GZ=""; SHA_ANDROID_ARM64_GZ=""
    SHA_WINDOWS_EXE=""; SHA_ANDROID_AMD64=""; SHA_ANDROID_ARM64=""
    echo "NOTE: MIHOMO_VERSION=$MIHOMO_VERSION differs from pinned $PINNED_VERSION; checksum verification skipped."
fi

log() { printf '[fetch-mihomo] %s\n' "$*"; }
die() { printf '[fetch-mihomo] ERROR: %s\n' "$*" >&2; exit 1; }

fetch() { # fetch <url> <dest>
    local url="$1" dest="$2"
    if command -v curl >/dev/null 2>&1; then
        curl -fL --retry 3 -o "$dest" "$url"
    elif command -v wget >/dev/null 2>&1; then
        wget -O "$dest" "$url"
    else
        die "neither curl nor wget is available"
    fi
}

verify_archive() { # verify_archive <file> <expected-sha>
    local file="$1" expected="$2"
    [[ -s "$file" ]] || die "$file is empty or missing"
    if [[ -n "$expected" ]]; then
        local actual
        actual="$(sha256sum "$file" | awk '{print $1}')"
        [[ "$actual" == "$expected" ]] || die "sha256 mismatch for $file (got $actual, want $expected)"
        log "checksum OK: $(basename "$file")"
    fi
}

verify_and_install_gz() { # verify_and_install_gz <archive> <dest> <expected-binary-sha>
    local archive="$1" dest="$2" expected="$3"
    gunzip -c "$archive" > "$dest"
    [[ -s "$dest" ]] || die "extracted file $dest is empty"
    if [[ -n "$expected" ]]; then
        local actual
        actual="$(sha256sum "$dest" | awk '{print $1}')"
        [[ "$actual" == "$expected" ]] || die "sha256 mismatch for extracted $dest"
    fi
    chmod +x "$dest"
    log "installed $(basename "$dest") ($(wc -c < "$dest") bytes)"
}

extract_windows_exe() { # extract_windows_exe <zip> <dest> <expected-binary-sha>
    local archive="$1" dest="$2" expected="$3" tmp
    tmp="$(mktemp -d)"
    if command -v unzip >/dev/null 2>&1; then
        unzip -o -q "$archive" -d "$tmp"
    elif command -v tar >/dev/null 2>&1 && tar -xf "$archive" -C "$tmp" 2>/dev/null; then
        # bsdtar (C:\Windows\System32\tar.exe) reads zip; GNU tar fails silently
        # and we fall through to python below
        if [[ -z "$(find "$tmp" -type f -name '*' -print -quit)" ]]; then
            die "tar could not extract $archive"
        fi
    elif command -v python3 >/dev/null 2>&1 || command -v python >/dev/null 2>&1; then
        local py; py="$(command -v python3 || command -v python)"
        "$py" -c "import zipfile,sys; zipfile.ZipFile(sys.argv[1]).extractall(sys.argv[2])" "$archive" "$tmp"
    else
        rm -rf "$tmp"
        die "need unzip, tar (bsdtar) or python to extract $archive"
    fi
    local inner
    inner="$(find "$tmp" -name 'mihomo-windows-amd64*.exe' -type f | head -n1)"
    [[ -n "$inner" ]] || { rm -rf "$tmp"; die "no mihomo-windows-amd64*.exe found in $archive"; }
    mv "$inner" "$dest"
    rm -rf "$tmp"
    [[ -s "$dest" ]] || die "extracted file $dest is empty"
    if [[ -n "$expected" ]]; then
        local actual
        actual="$(sha256sum "$dest" | awk '{print $1}')"
        [[ "$actual" == "$expected" ]] || die "sha256 mismatch for extracted $dest"
    fi
    log "installed $(basename "$dest") ($(wc -c < "$dest") bytes)"
}

already_ok() { # already_ok <dest> <expected-binary-sha> — true if present and matching
    local dest="$1" expected="$2"
    [[ -n "$expected" && -s "$dest" ]] || return 1
    local actual
    actual="$(sha256sum "$dest" | awk '{print $1}')"
    [[ "$actual" == "$expected" ]]
}

mkdir -p "$VENDOR_DIR"

WINDOWS_ZIP="mihomo-windows-amd64-v3-${MIHOMO_VERSION}.zip"
ANDROID_AMD64_GZ="mihomo-android-amd64-${MIHOMO_VERSION}.gz"
ANDROID_ARM64_GZ="mihomo-android-arm64-v8-${MIHOMO_VERSION}.gz"

# --- windows amd64 -> vendor/mihomo.exe -------------------------------------
if already_ok "$VENDOR_DIR/mihomo.exe" "$SHA_WINDOWS_EXE"; then
    log "vendor/mihomo.exe already present and up to date; skipping"
else
    log "downloading $WINDOWS_ZIP ..."
    fetch "$BASE_URL/$MIHOMO_VERSION/$WINDOWS_ZIP" "$VENDOR_DIR/$WINDOWS_ZIP"
    verify_archive "$VENDOR_DIR/$WINDOWS_ZIP" "$SHA_WINDOWS_ZIP"
    extract_windows_exe "$VENDOR_DIR/$WINDOWS_ZIP" "$VENDOR_DIR/mihomo.exe" "$SHA_WINDOWS_EXE"
    rm -f "$VENDOR_DIR/$WINDOWS_ZIP"
fi

# --- android amd64 -> vendor/mihomo-android-amd64 ---------------------------
if already_ok "$VENDOR_DIR/mihomo-android-amd64" "$SHA_ANDROID_AMD64"; then
    log "vendor/mihomo-android-amd64 already present and up to date; skipping"
else
    log "downloading $ANDROID_AMD64_GZ ..."
    fetch "$BASE_URL/$MIHOMO_VERSION/$ANDROID_AMD64_GZ" "$VENDOR_DIR/$ANDROID_AMD64_GZ"
    verify_archive "$VENDOR_DIR/$ANDROID_AMD64_GZ" "$SHA_ANDROID_AMD64_GZ"
    verify_and_install_gz "$VENDOR_DIR/$ANDROID_AMD64_GZ" "$VENDOR_DIR/mihomo-android-amd64" "$SHA_ANDROID_AMD64"
    rm -f "$VENDOR_DIR/$ANDROID_AMD64_GZ"
fi

# --- android arm64-v8 -> vendor/mihomo-android-arm64-v8 ---------------------
if already_ok "$VENDOR_DIR/mihomo-android-arm64-v8" "$SHA_ANDROID_ARM64"; then
    log "vendor/mihomo-android-arm64-v8 already present and up to date; skipping"
else
    log "downloading $ANDROID_ARM64_GZ ..."
    fetch "$BASE_URL/$MIHOMO_VERSION/$ANDROID_ARM64_GZ" "$VENDOR_DIR/$ANDROID_ARM64_GZ"
    verify_archive "$VENDOR_DIR/$ANDROID_ARM64_GZ" "$SHA_ANDROID_ARM64_GZ"
    verify_and_install_gz "$VENDOR_DIR/$ANDROID_ARM64_GZ" "$VENDOR_DIR/mihomo-android-arm64-v8" "$SHA_ANDROID_ARM64"
    rm -f "$VENDOR_DIR/$ANDROID_ARM64_GZ"
fi

log "done: mihomo $MIHOMO_VERSION binaries are in $VENDOR_DIR"
