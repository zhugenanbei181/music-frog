#!/usr/bin/env bash
# Build the Slint-flavored Android Rust bridge/NativeActivity library for the
# ABIs declared by android/app/build.gradle.kts.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_DIR="$REPO_ROOT/android/app"
DEST_DIR="$APP_DIR/src/slint/jniLibs"
TARGETS=("arm64-v8a:aarch64-linux-android" "x86_64:x86_64-linux-android")

find_ndk() {
    local candidate="${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT:-}}"
    if [[ -n "$candidate" && -d "$candidate" ]]; then
        printf '%s\n' "$candidate"
        return 0
    fi
    local properties="$REPO_ROOT/android/local.properties"
    if [[ -f "$properties" ]]; then
        candidate="$(sed -n 's/^ndk\.dir=//p' "$properties" | tr -d '\r' | tail -1)"
        if [[ -n "$candidate" && -d "$candidate" ]]; then
            printf '%s\n' "$candidate"
            return 0
        fi
        candidate="$(sed -n 's/^sdk\.dir=//p' "$properties" | tr -d '\r' | tail -1)"
        if [[ -n "$candidate" && -d "$candidate/ndk" ]]; then
            find "$candidate/ndk" -mindepth 1 -maxdepth 1 -type d -print | sort -V | tail -1
            return 0
        fi
    fi
    for candidate in "${ANDROID_SDK_ROOT:-}" "${ANDROID_HOME:-}"; do
        if [[ -n "$candidate" && -d "$candidate/ndk" ]]; then
            find "$candidate/ndk" -mindepth 1 -maxdepth 1 -type d -print | sort -V | tail -1
            return 0
        fi
    done
    echo "error: Android NDK was not found" >&2
    return 1
}

NDK_DIR="$(find_ndk)"
TOOLCHAIN_BIN="$NDK_DIR/toolchains/llvm/prebuilt/linux-x86_64/bin"
if [[ ! -d "$TOOLCHAIN_BIN" ]]; then
    TOOLCHAIN_BIN="$(find "$NDK_DIR/toolchains/llvm/prebuilt" -mindepth 2 -maxdepth 2 -type d -name bin -print -quit)"
fi
if [[ ! -d "$TOOLCHAIN_BIN" ]]; then
    echo "error: Android NDK clang toolchain was not found under $NDK_DIR" >&2
    exit 1
fi

cd "$REPO_ROOT"
for mapping in "${TARGETS[@]}"; do
    abi="${mapping%%:*}"
    target="${mapping#*:}"
    rustup target add "$target" >/dev/null 2>&1
    linker_var="CARGO_TARGET_$(printf '%s' "$target" | tr '[:lower:]-' '[:upper:]_')_LINKER"
    env "$linker_var=$TOOLCHAIN_BIN/${target}21-clang" \
        "CC_${target//-/_}=$TOOLCHAIN_BIN/${target}21-clang" \
        "CC_${target}=$TOOLCHAIN_BIN/${target}21-clang" \
        "AR_${target//-/_}=$TOOLCHAIN_BIN/llvm-ar" \
        "AR_${target}=$TOOLCHAIN_BIN/llvm-ar" \
        cargo build --release --target "$target" -p infiltrator-android --features slint-ui
    destination="$DEST_DIR/$abi/libinfiltrator_android.so"
    mkdir -p "$(dirname "$destination")"
    cp "target/$target/release/libinfiltrator_android.so" "$destination"
    if [[ -x "$TOOLCHAIN_BIN/llvm-strip" ]]; then
        "$TOOLCHAIN_BIN/llvm-strip" --strip-unneeded "$destination" 2>/dev/null || true
    fi
    echo "built Slint Android library for $abi"
done
