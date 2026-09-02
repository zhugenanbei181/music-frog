#!/usr/bin/env bash
# BEVY-006 back half: emulator smoke for the bevy APK built by
# scripts/build-bevy-apk.sh. Boot (KVM-accelerated, headless) -> install ->
# am start NativeActivity -> screenshot + logcat evidence.
#
# Cleanup discipline: the emulator is only ever stopped through the PID we
# recorded in "$ROOT/logs/emulator.pid" (or left running if this script
# booted it and KEEP_EMULATOR=1). No kill-by-name anywhere.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT="${ANDROID_TOOLS_ROOT:-$HOME/android-tools-bevy}"
SDK="$ROOT/android"
AVD_NAME="bevy-smoke"
PACKAGE="app.musicfrog.infiltrator_bevy_ui"
ACTIVITY="$PACKAGE/android.app.NativeActivity"
DRIVER="$REPO_ROOT/target/android-tools/apk-driver"
APK="$DRIVER/target/debug/apk/infiltrator-bevy-ui.apk"
EVIDENCE="${EVIDENCE_DIR:-$ROOT/logs/evidence}"

export JAVA_HOME="$ROOT/jdk"
export ANDROID_HOME="$SDK"
export ANDROID_SDK_ROOT="$SDK"
export ANDROID_NDK_ROOT="$SDK/ndk/"*   # only used transitively
export PATH="$JAVA_HOME/bin:$SDK/platform-tools:$SDK/emulator:$PATH"

ADB="$SDK/platform-tools/adb"
EMULATOR="$SDK/emulator/emulator"

[ -f "$APK" ] || { echo "ERROR: APK missing at $APK — run scripts/build-bevy-apk.sh first" >&2; exit 1; }
mkdir -p "$EVIDENCE"

# --- boot --------------------------------------------------------------------
booted() {
    "$ADB" get-state >/dev/null 2>&1 && \
        [ "$("$ADB" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" = "1" ]
}

if booted; then
    echo "[verify-bevy-apk] emulator already booted"
    WE_BOOTED=0
else
    if "$ADB" get-state >/dev/null 2>&1; then
        echo "[verify-bevy-apk] emulator device present, waiting for boot completion"
        WE_BOOTED=0
    else
        echo "[verify-bevy-apk] booting emulator -avd $AVD_NAME (headless, swiftshader)"
        "$EMULATOR" -avd "$AVD_NAME" -no-window -gpu swiftshader_indirect \
            -no-audio -no-boot-anim -no-snapshot -memory 3072 \
            > "$ROOT/logs/emulator.log" 2>&1 &
        echo $! > "$ROOT/logs/emulator.pid"
        WE_BOOTED=1
        echo "[verify-bevy-apk] emulator PID $(cat "$ROOT/logs/emulator.pid") recorded"
    fi
    "$ADB" wait-for-device
    for i in $(seq 1 120); do
        if booted; then break; fi
        sleep 5
    done
    booted || { echo "ERROR: emulator did not reach sys.boot_completed" >&2; exit 1; }
fi

# --- install + launch --------------------------------------------------------
# Uninstall first: the fat APK is ~2.1 GB and the AVD userdata partition is
# 6 GB; `install -r` stages the new base.apk alongside the old one and dies
# with INSTALL_FAILED_INSUFFICIENT_STORAGE. A fresh install also takes the
# incremental (on-demand block) path, so adb must stay connected while the
# app runs — true for this whole flow.
"$ADB" uninstall "$PACKAGE" >/dev/null 2>&1 || true
"$ADB" install -r "$APK" | tee "$EVIDENCE/install.txt"
"$ADB" logcat -c
"$ADB" shell am start -n "$ACTIVITY" | tee "$EVIDENCE/am-start.txt"

sleep 8

PID="$("$ADB" shell pidof "$PACKAGE" | tr -d '\r' || true)"
echo "[verify-bevy-apk] process pid: ${PID:-<none>}" | tee "$EVIDENCE/pid.txt"

# --- crash forensics ---------------------------------------------------------
"$ADB" logcat -d > "$EVIDENCE/logcat.txt" || true
grep -E "AndroidRuntime|DEBUG.*Fatal|bevy|wgpu|naga|RustStdoutStderr" "$EVIDENCE/logcat.txt" \
    | head -80 | tee "$EVIDENCE/logcat-key.txt" || true

# --- screenshots -------------------------------------------------------------
"$ADB" exec-out screencap -p > "$EVIDENCE/bevy-dark.png"
ls -la "$EVIDENCE/bevy-dark.png"
if [ -n "$PID" ]; then
    "$ADB" exec-out screencap -p > "$EVIDENCE/bevy-dark-8s.png"
fi
# Light-theme capture is intentionally skipped: INFILTRATOR_BEVY_SKIN is a
# process env knob (capture::skin_from_env) and android-activity 0.6.1
# exposes no env-injection channel for NativeActivity (no
# `android.app.env_vars` meta-data support), so the cold-start skin cannot be
# selected from outside the process.

echo "[verify-bevy-apk] evidence in $EVIDENCE"
echo "[verify-bevy-apk] inspect bevy-dark.png visually for the shell chrome"
[ -n "$PID" ] || echo "[verify-bevy-apk] WARNING: no live process — check logcat-key.txt"

# --- cleanup (PID-recorded only, never kill-by-name) --------------------------
"$ADB" shell am force-stop "$PACKAGE" >/dev/null 2>&1 || true
if [ "${WE_BOOTED:-0}" = "1" ] && [ "${KEEP_EMULATOR:-0}" != "1" ]; then
    EPID="$(cat "$ROOT/logs/emulator.pid")"
    if kill -0 "$EPID" 2>/dev/null; then
        echo "[verify-bevy-apk] stopping emulator PID $EPID"
        kill "$EPID"
        for i in $(seq 1 20); do kill -0 "$EPID" 2>/dev/null || break; sleep 1; done
        kill -9 "$EPID" 2>/dev/null || true
    fi
else
    echo "[verify-bevy-apk] emulator left running (not started here or KEEP_EMULATOR=1)"
fi
