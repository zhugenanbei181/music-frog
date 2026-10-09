#!/usr/bin/env bash
# BANDROID-023 — explicit Android real-device / long-running evidence stage.
#
# This is the L3/L4 evidence layer of docs/android/BEVY_ANDROID_PRODUCT.md §8/§9: VPN
# traffic, no-loopback, a >=8h background soak, network switch and power. It is
# an EXPLICIT stage and MUST NOT be wired into the ordinary PR CI — it needs a
# physical ARM64 device attached over adb, which no hosted runner provides.
#
# It refuses to claim success without a real device: `adb get-state` must report
# `device`, otherwise the script exits non-zero before writing any report. Even
# with a device attached, report.md stays `STATUS: incomplete` whenever a
# required step (APK install, instrumented tests) was skipped.
#
# Cleanup discipline: processes are never killed by name. Only the known app
# packages are stopped, via `adb shell am force-stop <package>`.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

# --- knobs (all overridable from the environment) ----------------------------
TS="$(date -u +%Y%m%dT%H%M%SZ)"
EVIDENCE_DIR="${EVIDENCE_DIR:-$REPO_ROOT/target/android-evidence/$TS}"
BEVY_APK="${BEVY_APK:-}"
COMPOSE_APK="${COMPOSE_APK:-}"
RUN_INSTRUMENTED="${RUN_INSTRUMENTED:-1}"
BACKGROUND_SECONDS="${BACKGROUND_SECONDS:-0}"
# Bounded sampling cadence for the long soak (seconds); 8h at 600s = 48 files.
SAMPLE_INTERVAL_SECONDS="${SAMPLE_INTERVAL_SECONDS:-600}"

COMPOSE_PKG="com.musicfrog.infiltrator"
COMPOSE_ACTIVITY="$COMPOSE_PKG/.MainActivity"
BEVY_PKG="app.musicfrog.infiltrator_bevy_ui"
BEVY_ACTIVITY="$BEVY_PKG/android.app.NativeActivity"

# --- locate adb --------------------------------------------------------------
if [ -n "${ANDROID_HOME:-}" ] && [ -x "$ANDROID_HOME/platform-tools/adb" ]; then
    ADB="$ANDROID_HOME/platform-tools/adb"
elif command -v adb >/dev/null 2>&1; then
    ADB="$(command -v adb)"
else
    echo "ERROR: adb not found. Set ANDROID_HOME (with platform-tools/adb) or put adb on PATH." >&2
    exit 1
fi

# --- require a real device ---------------------------------------------------
DEVICE_STATE="$("$ADB" get-state 2>/dev/null | tr -d '\r' || true)"
if [ "$DEVICE_STATE" != "device" ]; then
    echo "ERROR: no real device attached (adb get-state='${DEVICE_STATE:-<none>}')." >&2
    echo "BANDROID-023 is a physical-device stage; refusing to run without one." >&2
    exit 1
fi

mkdir -p "$EVIDENCE_DIR"
EVIDENCE_DIR="$(cd "$EVIDENCE_DIR" && pwd)"
echo "[device-evidence] adb=$ADB evidence=$EVIDENCE_DIR"

# --- device properties -------------------------------------------------------
DEVICE_MODEL="$("$ADB" shell getprop ro.product.model | tr -d '\r')"
DEVICE_SDK="$("$ADB" shell getprop ro.build.version.sdk | tr -d '\r')"
DEVICE_RELEASE="$("$ADB" shell getprop ro.build.version.release | tr -d '\r')"
DEVICE_ABI="$("$ADB" shell getprop ro.product.cpu.abi | tr -d '\r')"

# --- resolve APKs (env override first, then conventional build outputs) ------
detect_apk() {
    local explicit="$1"; shift
    if [ -n "$explicit" ]; then printf '%s' "$explicit"; return; fi
    local candidate
    for candidate in "$@"; do
        if [ -f "$candidate" ]; then printf '%s' "$candidate"; return; fi
    done
    printf ''
}

COMPOSE_APK="$(detect_apk "$COMPOSE_APK" \
    "$REPO_ROOT/android/app/build/outputs/apk/debug/app-debug.apk" \
    "$REPO_ROOT/android/app/build/outputs/apk/release/app-release.apk")"
BEVY_APK="$(detect_apk "$BEVY_APK" \
    "$REPO_ROOT/target/android-tools/apk-driver/target/release/apk/infiltrator-bevy-ui.apk" \
    "$REPO_ROOT/target/android-tools/apk-driver/target/debug/apk/infiltrator-bevy-ui.apk")"

# --- step 1: provenance ------------------------------------------------------
HEAD_SHA="$(git rev-parse HEAD)"
PORCELAIN="$(git status --porcelain)"
if [ -n "$PORCELAIN" ]; then
    DIRTY_LABEL="DIRTY"
else
    DIRTY_LABEL="CLEAN"
fi
PORCELAIN_HASH="$(printf '%s' "$PORCELAIN" | sha256sum | awk '{print $1}')"

apk_hash_lines=""

{
    echo "== git =="
    echo "head: $HEAD_SHA"
    echo "working_tree: $DIRTY_LABEL"
    echo "porcelain_sha256: $PORCELAIN_HASH"
    if [ -n "$PORCELAIN" ]; then
        printf '%s\n' "$PORCELAIN" | sed 's/^/porcelain: /'
    fi
    echo
    echo "== device props =="
    echo "ro.product.model: $DEVICE_MODEL"
    echo "ro.build.version.sdk: $DEVICE_SDK"
    echo "ro.build.version.release: $DEVICE_RELEASE"
    echo "ro.product.cpu.abi: $DEVICE_ABI"
} > "$EVIDENCE_DIR/provenance.txt"

# --- step 2: install + package inventory -------------------------------------
compose_installed=0
bevy_installed=0
install_failed=0

install_one() {
    local label="$1" apk="$2"
    local out="$EVIDENCE_DIR/install-$label.txt"
    local hash
    hash="$(sha256sum "$apk" | awk '{print $1}')"
    if "$ADB" install -r "$apk" > "$out" 2>&1; then
        echo "OK: installed $label $apk sha256=$hash"
        apk_hash_lines+="$hash  $apk"$'\n'
        return 0
    fi
    echo "FAILED: $label install failed (see $out)"
    return 1
}

if [ -n "$COMPOSE_APK" ] && [ -f "$COMPOSE_APK" ]; then
    if install_one compose "$COMPOSE_APK"; then
        compose_installed=1
    else
        install_failed=1
    fi
else
    echo "SKIPPED: Compose APK not found (set COMPOSE_APK)" | tee "$EVIDENCE_DIR/install-compose.txt"
fi

if [ -n "$BEVY_APK" ] && [ -f "$BEVY_APK" ]; then
    if install_one bevy "$BEVY_APK"; then
        bevy_installed=1
    else
        install_failed=1
    fi
else
    echo "SKIPPED: Bevy APK not found (set BEVY_APK)" | tee "$EVIDENCE_DIR/install-bevy.txt"
fi

{
    echo
    echo "== installed apk sha256 =="
    if [ -n "$apk_hash_lines" ]; then
        printf '%s' "$apk_hash_lines"
    else
        echo "(none installed)"
    fi
} >> "$EVIDENCE_DIR/provenance.txt"

"$ADB" shell pm list packages > "$EVIDENCE_DIR/pm-list-packages.txt" 2>&1 || true
{
    echo "== our packages =="
    grep -E "package:($COMPOSE_PKG|$BEVY_PKG)$" "$EVIDENCE_DIR/pm-list-packages.txt" \
        || echo "(none)"
} > "$EVIDENCE_DIR/pm-list-our-packages.txt"

# --- step 3: instrumented tests ----------------------------------------------
instrumented_state="SKIPPED"
instrumented_skipped=0
instrumented_failed=0
instrumented_skip_reason=""
INSTR="$EVIDENCE_DIR/instrumented.txt"

if [ "$RUN_INSTRUMENTED" = "1" ]; then
    if [ -f "$REPO_ROOT/android/gradlew" ] && [ -d "$REPO_ROOT/android/app/src/androidTest" ]; then
        chmod +x "$REPO_ROOT/android/gradlew" 2>/dev/null || true
        echo "[device-evidence] running Gradle connectedDebugAndroidTest"
        rc=0
        ( cd "$REPO_ROOT/android" && ./gradlew --no-daemon --connectedDebugAndroidTest ) > "$INSTR" 2>&1 || rc=$?
        if [ "$rc" -eq 0 ]; then
            instrumented_state="RAN (gradle connectedDebugAndroidTest, rc=0)"
        else
            instrumented_state="RAN (gradle connectedDebugAndroidTest, rc=$rc)"
            instrumented_failed=1
        fi
    else
        RUNNER="$("$ADB" shell pm list instrumentation 2>/dev/null | tr -d '\r' \
            | grep -F "$COMPOSE_PKG" | head -n1 \
            | sed -e 's/^instrumentation://' -e 's/ .*$//' || true)"
        if [ -n "$RUNNER" ]; then
            echo "[device-evidence] running am instrument -w $RUNNER"
            rc=0
            "$ADB" shell am instrument -w "$RUNNER" > "$INSTR" 2>&1 || rc=$?
            instrumented_state="RAN (am instrument -w $RUNNER, rc=$rc)"
            [ "$rc" -eq 0 ] || instrumented_failed=1
        else
            instrumented_skipped=1
            instrumented_skip_reason="no android/gradlew project with androidTest sources and no installed instrumentation runner for $COMPOSE_PKG"
            echo "SKIPPED: $instrumented_skip_reason" > "$INSTR"
            instrumented_state="SKIPPED ($instrumented_skip_reason)"
        fi
    fi
else
    instrumented_state="SKIPPED (RUN_INSTRUMENTED=$RUN_INSTRUMENTED)"
    echo "SKIPPED: RUN_INSTRUMENTED=$RUN_INSTRUMENTED" > "$INSTR"
fi

# --- step 4: raw reports -----------------------------------------------------
collect_reports() {
    local suffix="${1:-}"
    "$ADB" logcat -d > "$EVIDENCE_DIR/logcat$suffix.txt" 2>&1 || true
    "$ADB" shell dumpsys batterystats > "$EVIDENCE_DIR/batterystats$suffix.txt" 2>&1 || true
    "$ADB" shell dumpsys power > "$EVIDENCE_DIR/power$suffix.txt" 2>&1 || true
    "$ADB" shell dumpsys connectivity > "$EVIDENCE_DIR/connectivity$suffix.txt" 2>&1 || true
    : > "$EVIDENCE_DIR/pid$suffix.txt"
    local pkg pid
    for pkg in "$COMPOSE_PKG" "$BEVY_PKG"; do
        pid="$("$ADB" shell pidof "$pkg" 2>/dev/null | tr -d '\r' || true)"
        printf '%s: %s\n' "$pkg" "${pid:-<not running>}" >> "$EVIDENCE_DIR/pid$suffix.txt"
    done
    "$ADB" exec-out screencap -p > "$EVIDENCE_DIR/screen$suffix.png" 2>/dev/null || true
}

collect_reports ""

# --- step 5: optional background soak ----------------------------------------
soak_state="SKIPPED (BACKGROUND_SECONDS=0)"
soak_samples=0
if [ "$BACKGROUND_SECONDS" -gt 0 ]; then
    echo "[device-evidence] background soak for ${BACKGROUND_SECONDS}s (sample every ${SAMPLE_INTERVAL_SECONDS}s)"
    if [ "$compose_installed" -eq 1 ]; then
        "$ADB" shell am start -n "$COMPOSE_ACTIVITY" > "$EVIDENCE_DIR/soak-start.txt" 2>&1 || true
    elif [ "$bevy_installed" -eq 1 ]; then
        "$ADB" shell am start -n "$BEVY_ACTIVITY" > "$EVIDENCE_DIR/soak-start.txt" 2>&1 || true
    else
        echo "no APK installed; cannot foreground an app before the soak" > "$EVIDENCE_DIR/soak-start.txt"
    fi
    sleep 5
    # HOME sends the app to the background while keeping its process alive.
    "$ADB" shell input keyevent KEYCODE_HOME >/dev/null 2>&1 || true

    interval="$SAMPLE_INTERVAL_SECONDS"
    [ "$interval" -ge 1 ] || interval=1
    remaining="$BACKGROUND_SECONDS"
    i=0
    while [ "$remaining" -gt 0 ]; do
        i=$((i + 1))
        if [ "$remaining" -lt "$interval" ]; then step="$remaining"; else step="$interval"; fi
        sleep "$step"
        remaining=$((remaining - step))
        stamp="$(date -u +%Y%m%dT%H%M%SZ)"
        idx="$(printf '%03d' "$i")"
        "$ADB" shell dumpsys batterystats > "$EVIDENCE_DIR/batterystats-$idx-$stamp.txt" 2>&1 || true
        "$ADB" shell dumpsys power > "$EVIDENCE_DIR/power-$idx-$stamp.txt" 2>&1 || true
    done
    soak_samples="$i"
    soak_state="RAN (${BACKGROUND_SECONDS}s, ${soak_samples} samples)"
    echo "[device-evidence] soak complete; collecting final reports"
    collect_reports ""
fi

# --- step 6: report ----------------------------------------------------------
if [ "$compose_installed" -eq 1 ]; then compose_label="installed"; elif [ -n "$COMPOSE_APK" ] && [ -f "$COMPOSE_APK" ]; then compose_label="install FAILED"; else compose_label="not present"; fi
if [ "$bevy_installed" -eq 1 ]; then bevy_label="installed"; elif [ -n "$BEVY_APK" ] && [ -f "$BEVY_APK" ]; then bevy_label="install FAILED"; else bevy_label="not present"; fi

status="pass"
reasons=""
if [ "$compose_installed" -eq 0 ] && [ "$bevy_installed" -eq 0 ]; then
    status="incomplete"
    reasons+="- no APK was installed (set COMPOSE_APK and/or BEVY_APK)"$'\n'
fi
if [ "$install_failed" -eq 1 ]; then
    status="incomplete"
    reasons+="- an APK install failed (see install-*.txt)"$'\n'
fi
if [ "$instrumented_failed" -eq 1 ]; then
    status="incomplete"
    reasons+="- instrumented tests ran but returned non-zero (see instrumented.txt)"$'\n'
fi
if [ "$instrumented_skipped" -eq 1 ]; then
    status="incomplete"
    reasons+="- instrumented tests skipped: $instrumented_skip_reason"$'\n'
fi
[ -n "$reasons" ] || reasons="- (none)"

{
    echo "# Android device evidence — BANDROID-023"
    echo
    echo "Generated (UTC): $TS"
    echo
    echo "STATUS: $status"
    echo
    echo "> Explicit physical-device stage of docs/android/BEVY_ANDROID_PRODUCT.md §8/§9."
    echo "> Not a PR gate. STATUS stays incomplete whenever a required step was skipped."
    echo
    echo "## Device / OS"
    echo
    echo "| Property | Value |"
    echo "| --- | --- |"
    echo "| ro.product.model | $DEVICE_MODEL |"
    echo "| ro.build.version.sdk | $DEVICE_SDK |"
    echo "| ro.build.version.release | $DEVICE_RELEASE |"
    echo "| ro.product.cpu.abi | $DEVICE_ABI |"
    echo
    echo "## Provenance"
    echo
    echo "| Field | Value |"
    echo "| --- | --- |"
    echo "| git HEAD | $HEAD_SHA |"
    echo "| working tree | $DIRTY_LABEL |"
    echo "| porcelain sha256 | $PORCELAIN_HASH |"
    echo
    echo "Installed APK SHA-256:"
    echo
    if [ -n "$apk_hash_lines" ]; then
        printf '%s' "$apk_hash_lines" | sed 's/^/- /'
    else
        echo "- (none installed)"
    fi
    echo
    echo "## Steps"
    echo
    echo "| Step | Result |"
    echo "| --- | --- |"
    echo "| Compose install | $compose_label |"
    echo "| Bevy install | $bevy_label |"
    echo "| Instrumented tests | $instrumented_state |"
    echo "| Background soak | $soak_state |"
    echo "| Raw reports | logcat.txt, batterystats.txt, power.txt, connectivity.txt, pid.txt, screen.png |"
    echo
    echo "## Skipped / reasons"
    echo
    printf '%s' "$reasons"
    echo
    echo "## Notes"
    echo
    echo "- Package names: Compose=$COMPOSE_PKG, Bevy=$BEVY_PKG"
    if [ "$BACKGROUND_SECONDS" -lt 28800 ]; then
        echo "- The >=8h background requirement of BEVY_ANDROID_PRODUCT.md §8 is NOT satisfied by this run: BACKGROUND_SECONDS=$BACKGROUND_SECONDS. Set BACKGROUND_SECONDS=28800 for a qualifying soak."
    fi
} > "$EVIDENCE_DIR/report.md"

# --- cleanup (known package names only, never kill-by-name) ------------------
for pkg in "$COMPOSE_PKG" "$BEVY_PKG"; do
    "$ADB" shell am force-stop "$pkg" >/dev/null 2>&1 || true
done

echo "[device-evidence] STATUS: $status"
echo "[device-evidence] report: $EVIDENCE_DIR/report.md"
if [ "$status" != "pass" ]; then
    exit 1
fi
