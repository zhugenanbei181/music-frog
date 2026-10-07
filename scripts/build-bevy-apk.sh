#!/usr/bin/env bash
# BEVY-006 back half: build the bevy frontend as an aarch64-linux-android APK
# entirely in userspace (no sudo / system package manager). Idempotent:
# every stage is skipped when its artifact already exists.
#
# Layout under the toolchain root (default "$HOME/android-tools-bevy",
# override with ANDROID_TOOLS_ROOT; nothing here is tracked by the repo):
#   jdk/                                Temurin JDK 17
#   android/                            Android SDK root
#   android/cmdline-tools/latest/       sdkmanager / avdmanager
#   android/ndk/<version>/              NDK
#   logs/                               emulator + build logs, emulator PID
# The APK driver crate (cdylib exporting android_main) is generated under
# <repo>/target/android-tools/apk-driver — see its Cargo.toml header and the
# [package.metadata.android] rationale in
# crates/infiltrator-bevy-ui/Cargo.toml for the entry-contract citations.
# VALIDATED RUN (BEVY-006 back half, 2026-09-01, userspace toolchain at
# ~/android-tools-bevy, x86_64 API 36 emulator, KVM): L1 aapt badging +
# L2 crash-free launch + L3 screenshot all passed — but against *snapshot*
# drivers (git index state, generated under target/android-tools/apk-driver-
# snapshot{,-x86}) because the working tree was under parallel edit.
# CONTINUATION (same day): the three earlier live-tree attempts in
# logs/apk-build{,2,3}.log died on mid-edit compile errors in the widgets/ui
# crates, never on toolchain. The driver now packs BOTH ABIs as one fat APK:
# aarch64-linux-android is the task's canonical artifact, x86_64-linux-android
# exists solely because the API 36 x86_64 emulator image rejects arm64-only
# APKs (INSTALL_FAILED_NO_MATCHING_ABIS, observed live) and this emulator is
# the only KVM-capable smoke surface. The installer picks the matching ABI.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT="${ANDROID_TOOLS_ROOT:-$HOME/android-tools-bevy}"
# BANDROID-020: keep SDK platform / build-tools / NDK identical to the Compose
# host (android/app/build.gradle.kts) so "Android compiles in CI" means the same
# target for both products. Do not let these drift again.
API="android-36"
BUILD_TOOLS="36.0.0"
AVD_NAME="bevy-smoke"
DRIVER="$REPO_ROOT/target/android-tools/apk-driver"
PACKAGE="app.musicfrog.infiltrator_bevy_ui"
ACTIVITY="$PACKAGE/android.app.NativeActivity"
DRIVER_PKG="infiltrator-bevy-apk-driver"

# Toolchain source.
#   default (0): install a self-contained userspace JDK+SDK+NDK under $ROOT —
#                the validated local path, no sudo.
#   system  (1): reuse JAVA_HOME / ANDROID_HOME / ANDROID_NDK_ROOT provided by
#                the CI setup-java + setup-android + setup-ndk actions, so the
#                Bevy APK shares the exact SDK/NDK the iced Android build
#                already provisions instead of a second ~1.2 GB toolchain.
USE_SYSTEM_TOOLCHAIN="${BEVY_APK_USE_SYSTEM_TOOLCHAIN:-0}"
if [ "$USE_SYSTEM_TOOLCHAIN" = "1" ]; then
    : "${JAVA_HOME:?JAVA_HOME must be set when BEVY_APK_USE_SYSTEM_TOOLCHAIN=1}"
    : "${ANDROID_HOME:?ANDROID_HOME must be set when BEVY_APK_USE_SYSTEM_TOOLCHAIN=1}"
    # setup-ndk exports ANDROID_NDK_HOME; accept either spelling.
    ANDROID_NDK_ROOT="${ANDROID_NDK_ROOT:-${ANDROID_NDK_HOME:-}}"
    : "${ANDROID_NDK_ROOT:?ANDROID_NDK_ROOT (or ANDROID_NDK_HOME) must be set when BEVY_APK_USE_SYSTEM_TOOLCHAIN=1}"
    SDK="$ANDROID_HOME"
    JDK="$JAVA_HOME"
    NDK_VERSION="$(basename "$ANDROID_NDK_ROOT")"
else
    SDK="$ROOT/android"
    JDK="$ROOT/jdk"
    NDK_VERSION="29.0.14206865"
fi

mkdir -p "$ROOT/dl" "$ROOT/logs"

log() { printf '[build-bevy-apk] %s\n' "$*"; }

# --- 1. Temurin JDK 17 (userspace mode only) ---------------------------------
if [ "$USE_SYSTEM_TOOLCHAIN" != "1" ]; then
    if [ ! -x "$JDK/bin/java" ]; then
        log "installing Temurin JDK 17"
        curl -sL --retry 3 -o "$ROOT/dl/jdk17.tar.gz" \
            "https://github.com/adoptium/temurin17-binaries/releases/download/jdk-17.0.20.1%2B1/OpenJDK17U-jdk_x64_linux_hotspot_17.0.20.1_1.tar.gz"
        tar xzf "$ROOT/dl/jdk17.tar.gz" -C "$ROOT"
        mv "$ROOT/jdk-17.0.20.1+1" "$JDK"
    else
        log "JDK present"
    fi

    # --- 2. cmdline-tools (userspace mode only) ------------------------------
    if [ ! -x "$SDK/cmdline-tools/latest/bin/sdkmanager" ]; then
        log "installing Android cmdline-tools"
        curl -sL --retry 3 -o "$ROOT/dl/cmdline-tools.zip" \
            "https://dl.google.com/android/repository/commandlinetools-linux-11076708_latest.zip"
        unzip -q "$ROOT/dl/cmdline-tools.zip" -d "$ROOT/cmdline-tools-tmp"
        mkdir -p "$SDK/cmdline-tools"
        mv "$ROOT/cmdline-tools-tmp/cmdline-tools" "$SDK/cmdline-tools/latest"
        rm -rf "$ROOT/cmdline-tools-tmp"
    else
        log "cmdline-tools present"
    fi
else
    log "system toolchain: SDK=$SDK NDK=$NDK_VERSION JDK=$JDK"
fi

export JAVA_HOME="$JDK"
export ANDROID_HOME="$SDK"
export ANDROID_SDK_ROOT="$SDK"
export PATH="$JDK/bin:$SDK/platform-tools:$SDK/emulator:$PATH"

SDKMANAGER="$SDK/cmdline-tools/latest/bin/sdkmanager"

# --- 3. SDK components -------------------------------------------------------
# sdkmanager package names use ';' separators; the on-disk layout uses '/',
# so probe the mapped path (a literal "$SDK/platforms;android-36" never
# exists and would re-invoke sdkmanager on every run). `yes |` dies with
# SIGPIPE once sdkmanager stops reading; neutralize that for the pipeline
# only (pipefail would otherwise abort the script after a *successful*
# install).
# CI packaging only needs to *build* the APK, not run it. Setting
# BEVY_APK_SKIP_EMULATOR=1 drops the emulator + system image (~1 GB) and the
# AVD step, which is the dominant install cost on a release runner.
SKIP_EMULATOR="${BEVY_APK_SKIP_EMULATOR:-0}"
sdk_packages=("platform-tools" "platforms;$API" "build-tools;$BUILD_TOOLS" "ndk;$NDK_VERSION")
if [ "$SKIP_EMULATOR" != "1" ]; then
    sdk_packages+=("emulator" "system-images;$API;default;x86_64")
fi
missing=""
for pkg in "${sdk_packages[@]}"; do
    [ -e "$SDK/${pkg//;//}" ] || missing="$missing $pkg"
done
if [ -n "$missing" ]; then
    log "installing SDK components:$missing"
    set +o pipefail
    yes | "$SDKMANAGER" --sdk_root="$SDK" $missing > "$ROOT/logs/sdkmanager.log" 2>&1
    rc=$?
    set -o pipefail
    [ "$rc" -eq 0 ] || { log "ERROR: sdkmanager failed (rc=$rc)"; exit "$rc"; }
else
    log "SDK components present"
fi
# Userspace mode installs the NDK under $SDK/ndk; system mode already points
# ANDROID_NDK_ROOT at the setup-ndk checkout and must not rewrite it.
if [ "$USE_SYSTEM_TOOLCHAIN" != "1" ]; then
    export ANDROID_NDK_ROOT="$SDK/ndk/$NDK_VERSION"
fi
log "ANDROID_NDK_ROOT=$ANDROID_NDK_ROOT"

# --- 4. AVD ------------------------------------------------------------------
if [ "$SKIP_EMULATOR" = "1" ]; then
    log "skipping AVD (BEVY_APK_SKIP_EMULATOR=1)"
elif [ ! -e "$HOME/.android/avd/$AVD_NAME.avd" ] && [ ! -e "$HOME/.android/avd/$AVD_NAME.ini" ]; then
    log "creating AVD $AVD_NAME"
    "$SDK/cmdline-tools/latest/bin/avdmanager" create avd \
        -n "$AVD_NAME" -k "system-images;$API;default;x86_64" -d pixel_5 --force
else
    log "AVD $AVD_NAME present"
fi

# --- 5. cargo-apk ------------------------------------------------------------
command -v cargo-apk >/dev/null 2>&1 || cargo install cargo-apk --locked

# --- 6. APK driver crate (cdylib + android_main shim) ------------------------
if [ ! -f "$DRIVER/Cargo.toml" ]; then
    log "generating APK driver crate under target/android-tools/apk-driver"
    mkdir -p "$DRIVER/src"
    cat > "$DRIVER/Cargo.toml" <<'EOF'
# Generated by scripts/build-bevy-apk.sh (BEVY-006 back half). DO NOT EDIT by
# hand, DO NOT track: lives under the untracked target/android-tools/ tree.
# Purpose: the crate's own src/ is not allowed to carry the android entry
# glue in this task split, but android-activity 0.6.1 requires the packaged
# cdylib to export `android_main` (extern "Rust",
# src/native_activity/glue.rs:663) and bevy_winit 0.20.0-rc.2 requires
# bevy_android::ANDROID_APP to be set before DefaultPlugins (src/lib.rs:123).
# This cdylib driver provides exactly that and calls the real shell's run().
[package]
name = "infiltrator-bevy-apk-driver"
version = "0.1.0"
edition = "2024"
publish = false

[lib]
name = "infiltrator_bevy_apk_driver"
crate-type = ["cdylib"]

[dependencies]
infiltrator-bevy-ui = { path = "../../../crates/infiltrator-bevy-ui" }
android-activity = { version = "0.6.1", features = ["native-activity"] }
bevy_android = "=0.20.0-rc.2"

[package.metadata.android]
package = "app.musicfrog.infiltrator_bevy_ui"
apk_name = "infiltrator-bevy-ui"
# Fat APK: see the header — arm64-v8a is the canonical target, x86_64 keeps
# the API 36 x86_64 emulator image installable for the smoke run.
build_targets = ["aarch64-linux-android", "x86_64-linux-android"]

# BANDROID-019: the mobile product host needs these permissions. cargo-apk can
# only express uses_permission; the VPN <service>/foregroundServiceType and the
# BootReceiver live in the Gradle product manifest
# (android/app/src/main/AndroidManifest.xml) — see docs/BEVY_ANDROID_PRODUCT.md.
[[package.metadata.android.uses_permission]]
name = "android.permission.INTERNET"

[[package.metadata.android.uses_permission]]
name = "android.permission.ACCESS_NETWORK_STATE"

[[package.metadata.android.uses_permission]]
name = "android.permission.FOREGROUND_SERVICE"

[[package.metadata.android.uses_permission]]
name = "android.permission.POST_NOTIFICATIONS"

[[package.metadata.android.uses_permission]]
name = "android.permission.RECEIVE_BOOT_COMPLETED"

[[package.metadata.android.uses_permission]]
name = "android.permission.FOREGROUND_SERVICE_SYSTEM_EXEMPTED"

# BANDROID-020: min/target SDK must match android/app/build.gradle.kts (29/36).
[package.metadata.android.sdk]
min_sdk_version = 29
target_sdk_version = 36

[package.metadata.android.application]
label = "MusicFrog Infiltrator Bevy"
theme = "@android:style/Theme.DeviceDefault.NoActionBar.Fullscreen"

[workspace]
EOF
    cat > "$DRIVER/src/lib.rs" <<'EOF'
//! Generated by scripts/build-bevy-apk.sh (BEVY-006 back half). See the
//! Cargo.toml header for why this shim exists.
//!
//! bevy's own mobile example shape: export `android_main` (android-activity
//! native-activity contract), hand the `AndroidApp` to
//! `bevy_android::ANDROID_APP`, then run the real shell entrypoint.
#[unsafe(no_mangle)]
fn android_main(app: android_activity::AndroidApp) {
    bevy_android::ANDROID_APP
        .set(app)
        .expect("ANDROID_APP must only be set once, by android_main");
    infiltrator_bevy_ui::run();
}
EOF
fi

# --- 7. Build ----------------------------------------------------------------
# No --target here: cargo-apk then honors build_targets (both ABIs) and packs
# one fat APK. With `--target aarch64-linux-android` it packages arm64 only,
# which the x86_64 emulator image refuses to install.
BUILD_FLAGS=""
if [ "${1:-}" = "--release" ] || [ "${RELEASE:-0}" = "1" ]; then
    BUILD_FLAGS="--release"
    APK_SUBDIR="release"
else
    APK_SUBDIR="debug"
fi
log "building APK (cargo apk build -p $DRIVER_PKG $BUILD_FLAGS, build_targets from metadata)"
(cd "$DRIVER" && cargo apk build -p "$DRIVER_PKG" $BUILD_FLAGS)

APK="$DRIVER/target/$APK_SUBDIR/apk/infiltrator-bevy-ui.apk"
[ -f "$APK" ] || { log "ERROR: APK not found at $APK"; exit 1; }

# --- 8. L1 verification: aapt badging + packaged .so -------------------------
AAPT="$SDK/build-tools/$BUILD_TOOLS/aapt"
log "aapt badging: $APK"
"$AAPT" dump badging "$APK" | grep -E "^package|native-code|launchable-activity|sdkVersion|application-label" | tee "$ROOT/logs/aapt-badging.txt"
"$AAPT" dump xmltree "$APK" AndroidManifest.xml | grep -E "lib_name|NativeActivity|theme" | tee "$ROOT/logs/aapt-manifest.txt"
unzip -l "$APK" | grep -E "\.so" | tee "$ROOT/logs/apk-libs.txt"

# --- 9. L1.5 verification: packaged permissions (BANDROID-019) ---------------
# The generated metadata is the source; this asserts the declaration actually
# landed in the signed APK. Missing any required permission fails the build so a
# silently dropped manifest entry cannot ship.
"$AAPT" dump permissions "$APK" > "$ROOT/logs/aapt-permissions.txt"
required_permissions=(
    android.permission.INTERNET
    android.permission.ACCESS_NETWORK_STATE
    android.permission.FOREGROUND_SERVICE
    android.permission.POST_NOTIFICATIONS
    android.permission.RECEIVE_BOOT_COMPLETED
    android.permission.FOREGROUND_SERVICE_SYSTEM_EXEMPTED
)
missing_permissions=()
for permission in "${required_permissions[@]}"; do
    grep -q "name='$permission'" "$ROOT/logs/aapt-permissions.txt" \
        || missing_permissions+=("$permission")
done
if [ "${#missing_permissions[@]}" -ne 0 ]; then
    log "ERROR: APK missing required permissions: ${missing_permissions[*]}"
    exit 1
fi
log "aapt permissions verified (${#required_permissions[@]} present)"

log "OK: $APK"
log "next: scripts/verify-bevy-apk.sh"
