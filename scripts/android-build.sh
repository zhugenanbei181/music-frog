#!/usr/bin/env bash
# Build the infiltrator_android Rust cdylib for the Android ABIs used by the
# Gradle project (arm64-v8a, x86_64), copy the .so files into jniLibs, and
# (re)generate the UniFFI Kotlin bindings into android/app/src/main/java.
#
# Invoked by the `cargoBuild` Gradle task in android/app/build.gradle.kts
# (ANDROids_SDK_ROOT/ANDROID_HOME/ANDROID_NDK_HOME are set by Gradle), but it
# can also be run standalone from anywhere:
#   ./scripts/android-build.sh
#
# Requires: Rust (rustup), Android NDK (r21+ layout), cargo target
# aarch64-linux-android (installed automatically when missing).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_DIR="$REPO_ROOT/android/app"
JNILIBS_DIR="$APP_DIR/src/main/jniLibs"
GENERATED_DIR="$APP_DIR/src/main/java"
CRATE="infiltrator-android"
LIBNAME="libinfiltrator_android.so"

cd "$REPO_ROOT"

# ---------------------------------------------------------------------------
# Locate the Android NDK (env vars first, then local.properties, then glob)
# ---------------------------------------------------------------------------
find_ndk() {
    local prop ndk sdk
    if [ -n "${ANDROID_NDK_HOME:-}" ] && [ -d "${ANDROID_NDK_HOME}" ]; then
        echo "$ANDROID_NDK_HOME"
        return 0
    fi
    if [ -n "${ANDROID_NDK_ROOT:-}" ] && [ -d "${ANDROID_NDK_ROOT}" ]; then
        echo "$ANDROID_NDK_ROOT"
        return 0
    fi
    local props="$REPO_ROOT/android/local.properties"
    if [ -f "$props" ]; then
        prop="$(grep -E '^ndk\.dir=' "$props" | cut -d= -f2- | tr -d '\r' || true)"
        if [ -n "$prop" ] && [ -d "$prop" ]; then echo "$prop"; return 0; fi
        sdk="$(grep -E '^sdk\.dir=' "$props" | cut -d= -f2- | tr -d '\r' || true)"
    fi
    for ndk_candidate in "${ANDROID_SDK_ROOT:-}" "${ANDROID_HOME:-}" "$sdk"; do
        if [ -n "$ndk_candidate" ] && ls -d "$ndk_candidate"/ndk/* >/dev/null 2>&1; then
            echo "$ndk_candidate"/ndk/* | sort -V | tail -1
            return 0
        fi
    done
    echo "error: Android NDK not found (set ANDROID_NDK_HOME or android/local.properties)" >&2
    return 1
}

NDK_DIR="$(find_ndk)"
API_LEVEL=21
TOOLCHAIN_BIN="$NDK_DIR/toolchains/llvm/prebuilt/linux-x86_64/bin"
if [ ! -d "$TOOLCHAIN_BIN" ]; then
    # Windows/other host layouts
    TOOLCHAIN_BIN="$(ls -d "$NDK_DIR"/toolchains/llvm/prebuilt/*/bin 2>/dev/null | head -1 || true)"
fi
if [ ! -d "$TOOLCHAIN_BIN" ]; then
    echo "error: NDK toolchain bin directory not found under $NDK_DIR" >&2
    exit 1
fi
export PATH="$TOOLCHAIN_BIN:$PATH"

# ---------------------------------------------------------------------------
# ABI -> Rust target mapping (keep in sync with abiFilters in build.gradle.kts)
# ---------------------------------------------------------------------------
build_abi() {
    local abi="$1" target="$2"
    rustup target add "$target" >/dev/null 2>&1
    # .cargo/config.toml pins aarch64-linux-android21-clang; set the linker
    # explicitly for every target so the script works without editing config.
    local linker_var
    linker_var="CARGO_TARGET_$(echo "$target" | tr 'a-z-' 'A-Z_')_LINKER"
    env "$linker_var=$TOOLCHAIN_BIN/${target}${API_LEVEL}-clang" \
        cargo build -p "$CRATE" --release --target "$target"
    mkdir -p "$JNILIBS_DIR/$abi"
    local dest="$JNILIBS_DIR/$abi/$LIBNAME"
    cp "target/$target/release/$LIBNAME" "$dest"
    if [ -x "$TOOLCHAIN_BIN/llvm-strip" ]; then
        "$TOOLCHAIN_BIN/llvm-strip" --strip-unneeded "$dest" 2>/dev/null || true
        echo "stripped symbols: $dest ($(stat -c%s "$dest" 2>/dev/null || stat -f%z "$dest" 2>/dev/null) bytes)"
    fi
    echo "installed $LIBNAME -> $JNILIBS_DIR/$abi/"
}

build_abi arm64-v8a aarch64-linux-android
build_abi x86_64 x86_64-linux-android

# ---------------------------------------------------------------------------
# Regenerate UniFFI Kotlin bindings (proc-macro mode: run against the host
# cdylib; uniffi.toml next to the crate provides package/cdylib names).
# ---------------------------------------------------------------------------
cargo build -p "$CRATE" --lib
cargo run -p "$CRATE" --bin uniffi-bindgen -- generate \
    "target/debug/$LIBNAME" \
    --language kotlin \
    --out-dir "$GENERATED_DIR" \
    --no-format
echo "UniFFI Kotlin bindings generated in $GENERATED_DIR/infiltrator_android/"
