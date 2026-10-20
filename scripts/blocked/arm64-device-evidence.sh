#!/usr/bin/env bash
# ARM64 physical-device evidence procedure for the environment-blocked board.
#
# Board group: todos/CODE_TODO_BLOCKED.md §一 "ARM64 真机取证" (11 entries).
#
# What this script does
# ---------------------
# Locally (always, no device needed):
#   1. proves that every blocked board entry has a procedure (coverage preflight);
#   2. runs the Android toolchain check and the product-identity / manifest
#      contract guards;
#   3. locates the candidate APK, records its SHA-256 and its build age
#      against the current HEAD;
#   4. proves the ELF boundary of the packaged native driver: the
#      `lib/arm64-v8a/*.so` entries must be AArch64 objects;
#   5. writes the device evidence plan: for every one of the 11 ARM64 board
#      entries, the exact on-device observation, the adb command that produces
#      it and the evidence file it lands in.
#
# On a device (only when a physical ARM64 device is attached over adb):
#   installs the candidate, reads back device/OS identity, the process and
#   service state and a logcat excerpt, and writes the evidence report.
#
# A dry run never claims device evidence: `--dry-run` runs every local step and
# prints exactly what remains. Without `--dry-run` the script refuses to run
# when no physical ARM64 device is attached (exit code 2); it never fabricates
# a device result and never accepts an emulator as the ARM64 leg.
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/blocked/lib.sh
. "$SCRIPT_DIR/lib.sh"

BLOCKED_SCRIPT="arm64-device-evidence"
DRY_RUN=0
APK=""
FLAVOR="all"
LIFECYCLE_CYCLES=0

usage() {
    cat <<'EOF'
usage: bash scripts/blocked/arm64-device-evidence.sh [--dry-run] [--strict]
                                                        [--apk PATH] [--flavor compose|bevy|slint|all]
                                                        [--lifecycle-cycles N]

  --dry-run            run every step that does not need a physical ARM64 device
                       and print exactly what remains
  --apk                use this APK as the device candidate instead of the
                       discovered one
  --flavor             which product flavor to install on the device (default: all)
  --lifecycle-cycles   run the bounded lock/background/rotate cycle loop that
                       backs the AppLifecycle entry (default: 0, skipped)
  --strict             treat a repository gap as a failure

Exit codes: 0 = local steps passed, 1 = real defect, 2 = environment missing.
EOF
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --dry-run)
            DRY_RUN=1
            ;;
        --apk)
            APK="$2"
            shift
            ;;
        --flavor)
            FLAVOR="$2"
            shift
            ;;
        --lifecycle-cycles)
            LIFECYCLE_CYCLES="$2"
            shift
            ;;
        --strict)
            BLOCKED_STRICT=1
            ;;
        --help | -h)
            usage
            exit 0
            ;;
        *)
            printf 'unknown argument: %s\n' "$1" >&2
            usage >&2
            exit 2
            ;;
    esac
    shift
done

cd "$REPO_ROOT"
blocked_evidence_dir "$BLOCKED_SCRIPT" >/dev/null
EVIDENCE="$BLOCKED_EVIDENCE_DIR"

blocked_step "blocked-board coverage preflight"
blocked_coverage_preflight || true

blocked_step "Android toolchain baseline"
blocked_run "android-toolchain.py check" python3 scripts/android-toolchain.py check || true

blocked_step "product identity and manifest contract"
blocked_run "android-product-identity-guard.py" \
    python3 scripts/quality/android-product-identity-guard.py || true
blocked_run "android-manifest-contract-guard.py" \
    python3 scripts/quality/android-manifest-contract-guard.py || true

blocked_step "candidate APK discovery and identity"
APK_CANDIDATES=(
    "$REPO_ROOT/android/app/build/outputs/apk/compose/release/app-compose-universal-release.apk"
    "$REPO_ROOT/android/app/build/outputs/apk/bevy/release/app-bevy-universal-release.apk"
    "$REPO_ROOT/android/app/build/outputs/apk/slint/release/app-slint-universal-release.apk"
    "$REPO_ROOT/android/app/build/outputs/apk/compose/debug/app-compose-universal-debug.apk"
    "$REPO_ROOT/android/app/build/outputs/apk/bevy/debug/app-bevy-universal-debug.apk"
    "$REPO_ROOT/android/app/build/outputs/apk/slint/debug/app-slint-universal-debug.apk"
)
if [ -n "$APK" ]; then
    if [ ! -f "$APK" ]; then
        blocked_fail "--apk does not exist: $APK"
        blocked_finish
        exit $?
    fi
else
    APK=""
    for candidate in "${APK_CANDIDATES[@]}"; do
        if [ -f "$candidate" ]; then
            APK="$candidate"
            break
        fi
    done
fi

HEAD_SHA="$(git rev-parse HEAD)"
HEAD_DATE="$(git log -1 --format=%cI HEAD)"
if [ -z "$APK" ]; then
    blocked_gap "no candidate APK found; build one with 'cd android && ./gradlew :app:assembleComposeRelease' (see README)"
    APK_SHA=""
    APK_STAMP=""
else
    APK_SHA="$(sha256sum "$APK" | awk '{print $1}')"
    APK_STAMP="$(date -u -r "$APK" +%Y-%m-%dT%H:%M:%SZ)"
    blocked_ok "candidate APK ${APK#"$REPO_ROOT"/} sha256=$APK_SHA"
    {
        echo "apk=$APK"
        echo "apk_sha256=$APK_SHA"
        echo "apk_built_at=$APK_STAMP"
        echo "git_head=$HEAD_SHA"
        echo "git_head_date=$HEAD_DATE"
    } > "$EVIDENCE/candidate-apk.txt"
    if [ -n "$APK_STAMP" ] && [ -n "$HEAD_DATE" ]; then
        apk_epoch="$(date -d "$APK_STAMP" +%s 2>/dev/null || echo 0)"
        head_epoch="$(date -d "$HEAD_DATE" +%s 2>/dev/null || echo 0)"
    else
        apk_epoch=0
        head_epoch=0
    fi
    if [ "$apk_epoch" -gt 0 ] && [ "$head_epoch" -gt 0 ] && [ "$apk_epoch" -lt "$head_epoch" ]; then
        blocked_gap "the candidate APK predates HEAD ($APK_STAMP < $HEAD_DATE); rebuild before the device run"
    else
        blocked_ok "candidate APK is not older than HEAD"
    fi
fi

blocked_step "ELF boundary of the packaged native driver"
SO_LISTING="$EVIDENCE/apk-native-libraries.txt"

# The ELF machine is read from the header bytes (e_machine at offset 0x12,
# little endian) instead of from `readelf -h`, because the machine label of
# readelf is localized and would make the check locale dependent.
elf_machine() {
    local file="$1"
    local hex
    hex="$(od -An -tx1 -j18 -N2 "$file" 2>/dev/null | tr -d ' \n')"
    case "$hex" in
        b700) printf 'AArch64\n' ;;
        2800) printf 'ARM\n' ;;
        3e00) printf 'x86-64\n' ;;
        0300) printf 'x86\n' ;;
        *) printf 'unknown(%s)\n' "$hex" ;;
    esac
}

if [ -n "$APK" ] && [ -f "$APK" ]; then
    unzip -l "$APK" > "$SO_LISTING" 2>/dev/null || true
    ELF_OK=1
    ELF_REPORT="$EVIDENCE/elf-boundary.txt"
    : > "$ELF_REPORT"
    while IFS= read -r entry; do
        [ -n "$entry" ] || continue
        so_name="${entry%% *}"
        extracted="$EVIDENCE/$(basename "$so_name")"
        if unzip -p "$APK" "$so_name" > "$extracted" 2>/dev/null && [ -s "$extracted" ]; then
            machine="$(elf_machine "$extracted")"
            printf '%s -> %s\n' "$so_name" "$machine" >> "$ELF_REPORT"
            case "$machine" in
                AArch64) ;;
                *) ELF_OK=0 ;;
            esac
        else
            printf '%s -> not extracted\n' "$so_name" >> "$ELF_REPORT"
            ELF_OK=0
        fi
        rm -f "$extracted"
    done < <(grep -oE 'lib/arm64-v8a/[^ ]+\.so' "$SO_LISTING" | sort -u)
    if [ "$ELF_OK" -eq 1 ] && [ -s "$ELF_REPORT" ]; then
        blocked_ok "every packaged lib/arm64-v8a/*.so is an AArch64 object"
    else
        blocked_fail "the packaged arm64-v8a native libraries are not all AArch64 (see elf-boundary.txt)"
    fi
else
    blocked_skip "no candidate APK to inspect"
fi

blocked_step "device evidence plan"
PLAN="$EVIDENCE/device-plan.md"
cat > "$PLAN" <<'EOF'
# ARM64 device evidence plan

Every row is one blocked board entry of `todos/CODE_TODO_BLOCKED.md` §一. The
`adb` column is the exact command that produces the observation; the evidence
column is the raw file the device stage writes, from which the observation is
read. A leg is only closed by a real run on a physical ARM64 device — a dry
run of this script closes none of them.

| Board entry | On-device observation | adb command | Raw evidence file |
| --- | --- | --- | --- |
| BANDROID-041.3.2R.15 | the protocol editor form encodes, rejects, cancels and saves through the Application use case | `adb shell input text` plus `adb logcat -d` | `logcat.txt` |
| CORE-007.2 | after a Binder death the Core and VPN state are read back before a new command is admitted | `adb shell am force-stop` plus relaunch | `session-readback.txt` |
| BANDROID-041.3.3 | the packaged driver `.so` loads and the Bevy surface renders | `adb logcat -d` | `logcat.txt` and `elf-boundary.txt` |
| BANDROID-002.4 | the WakeLock is held only inside the Start/Cleanup transaction and released at the terminal state | `adb shell dumpsys power` | `power.txt` |
| BANDROID-005.1 | 100 or more lock/background/rotate cycles: no surface panic, no duplicated VPN command, flat memory curve | `bash scripts/blocked/arm64-device-evidence.sh --lifecycle-cycles 100` | `lifecycle-cycles.txt` and `meminfo-<package>.txt` |
| BANDROID-040.5 | the Core and VPN state/generation are read back after a Binder death; a stale session does not rewrite the owner snapshot | `adb shell am force-stop` plus relaunch | `session-readback.txt` |
| BANDROID-040.7 | Compose, Bevy and Slint observe the same owner snapshot revision | launch all three flavors | `session-readback.txt` |
| BANDROID-040.8 | the Age credential read/set/clear round-trips over Binder with no secret in log, snapshot or profile state | UI interaction plus `adb logcat -d` | `logcat.txt` |
| BANDROID-042.5c | the same-ABI Mihomo `-t` validates a candidate config while the Core is Stopped and writes nothing | UI interaction plus `adb logcat -d` | `logcat.txt` |
| BANDROID-040.4 | the Bevy product path JNI to Activity to UniFFI to host to Messenger to owner answers a request | launch the Bevy flavor | `session-readback.txt` and `logcat.txt` |
| BANDROID-043.4 | the JNI host waits asynchronously by request id; a late reply is dropped instead of forging success | UI interaction plus `adb logcat -d` | `logcat.txt` |

## Shared evidence identity

Every leg binds to the same identity, collected by the device stage:

- the SHA-256 of the installed candidate APK (`candidate-apk.txt`);
- the device model, Android API level, release and build fingerprint
  (`report.md`);
- the primary ABI, proven to be `arm64-v8a` (an emulator is refused);
- the process id read back with `adb shell pidof` (`session-readback.txt`);
- the VPN service state read back with `adb shell dumpsys activity services`
  (`session-readback.txt`);
- the logcat excerpt filtered by the package under test (`logcat.txt`).
EOF
blocked_ok "device evidence plan written to ${PLAN#"$REPO_ROOT"/}"

blocked_step "device stage"
ADB=""
if command -v adb >/dev/null 2>&1; then
    ADB="$(command -v adb)"
elif SDK="$(blocked_android_sdk_root)" && [ -x "$SDK/platform-tools/adb" ]; then
    ADB="$SDK/platform-tools/adb"
fi

device_state() {
    [ -n "$ADB" ] || return 1
    "$ADB" get-state 2>/dev/null | tr -d '\r' || true
}

DEVICE_STATE="$(device_state || true)"
if [ "$DRY_RUN" -eq 1 ]; then
    if [ -z "$ADB" ]; then
        blocked_pending "adb is not installed: install the Android platform-tools to run the device stage"
    elif [ "$DEVICE_STATE" != "device" ]; then
        blocked_pending "no physical device attached (adb get-state='${DEVICE_STATE:-<none>}'): attach an ARM64 device and re-run without --dry-run"
    else
        blocked_pending "a device is attached but --dry-run was requested: re-run without --dry-run to collect the device evidence"
    fi
    blocked_finish
    exit $?
fi

if [ -z "$ADB" ]; then
    printf 'environment required: adb (Android platform-tools) is not installed.\n' >&2
    exit 2
fi
if [ "$DEVICE_STATE" != "device" ]; then
    printf 'environment required: no physical device attached (adb get-state=%s).\n' "${DEVICE_STATE:-<none>}" >&2
    printf 'This procedure never substitutes an emulator for the ARM64 leg.\n' >&2
    exit 2
fi

QEMU="$( "$ADB" shell getprop ro.kernel.qemu 2>/dev/null | tr -d '\r' || true )"
if [ "$QEMU" = "1" ]; then
    printf 'environment required: the attached device reports ro.kernel.qemu=1 (emulator).\n' >&2
    printf 'The ARM64 board entries require a physical ARM64 device.\n' >&2
    exit 2
fi

DEVICE_ABI="$( "$ADB" shell getprop ro.product.cpu.abi 2>/dev/null | tr -d '\r' || true )"
case "$DEVICE_ABI" in
    *arm64*) ;;
    *)
        printf 'environment required: the attached device reports ABI %s, not arm64.\n' "${DEVICE_ABI:-<unknown>}" >&2
        exit 2
        ;;
esac

blocked_ok "physical ARM64 device attached (abi=$DEVICE_ABI)"

DEVICE_MODEL="$( "$ADB" shell getprop ro.product.model 2>/dev/null | tr -d '\r' || true )"
DEVICE_API="$( "$ADB" shell getprop ro.build.version.sdk 2>/dev/null | tr -d '\r' || true )"
DEVICE_RELEASE="$( "$ADB" shell getprop ro.build.version.release 2>/dev/null | tr -d '\r' || true )"
DEVICE_FINGERPRINT="$( "$ADB" shell getprop ro.build.fingerprint 2>/dev/null | tr -d '\r' || true )"

INSTALL_LOG="$EVIDENCE/install.txt"
INSTALL_OK=0
if [ -n "$APK" ] && [ -f "$APK" ]; then
    if "$ADB" install -r "$APK" > "$INSTALL_LOG" 2>&1; then
        blocked_ok "installed ${APK#"$REPO_ROOT"/} sha256=$APK_SHA"
        INSTALL_OK=1
    else
        blocked_fail "adb install failed (see install.txt)"
    fi
else
    blocked_fail "no candidate APK to install; build one first"
fi

PACKAGES=()
ACTIVITIES=()
ANDROID_NAMESPACE="$(blocked_identity android.namespace)"
launch_activity() {
    local activity
    activity="$(blocked_identity "$1.launcher_activity")"
    case "$activity" in
        .*) printf '%s%s' "$ANDROID_NAMESPACE" "$activity" ;;
        *) printf '%s' "$activity" ;;
    esac
}
register_flavor() {
    local flavor="$1"
    PACKAGES+=("$(blocked_identity "$flavor.application_id")")
    ACTIVITIES+=("$(launch_activity "$flavor")")
}
case "$FLAVOR" in
    all)
        register_flavor compose
        register_flavor bevy
        register_flavor slint
        ;;
    compose | bevy | slint)
        register_flavor "$FLAVOR"
        ;;
    *)
        printf 'unknown flavor: %s\n' "$FLAVOR" >&2
        exit 2
        ;;
esac

PRIMARY_PACKAGE="${PACKAGES[0]}"
PRIMARY_ACTIVITY="${ACTIVITIES[0]}"
LAUNCH_ACTIVITY="$PRIMARY_PACKAGE/$PRIMARY_ACTIVITY"

RUNNING_PACKAGES=0
PID_FILE="$EVIDENCE/session-readback.txt"
: > "$PID_FILE"
for index in "${!PACKAGES[@]}"; do
    pkg="${PACKAGES[$index]}"
    activity="${ACTIVITIES[$index]}"
    "$ADB" shell am force-stop "$pkg" >/dev/null 2>&1 || true
    "$ADB" shell am start -n "$pkg/$activity" >> "$PID_FILE" 2>&1 || true
    sleep 2
    pid="$( "$ADB" shell pidof "$pkg" 2>/dev/null | tr -d '\r' || true )"
    printf '%s pid=%s\n' "$pkg" "${pid:-<not running>}" >> "$PID_FILE"
    "$ADB" shell dumpsys activity services "$pkg" >> "$PID_FILE" 2>&1 || true
    "$ADB" shell dumpsys package "$pkg" | grep -E "versionCode|versionName" >> "$PID_FILE" 2>&1 || true
    if [ -n "$pid" ]; then
        RUNNING_PACKAGES=$((RUNNING_PACKAGES + 1))
    fi
done
blocked_ok "process/session readback collected for ${#PACKAGES[@]} package(s), $RUNNING_PACKAGES running"

"$ADB" logcat -d > "$EVIDENCE/logcat.txt" 2>&1 || true
for pkg in "${PACKAGES[@]}"; do
    "$ADB" shell dumpsys power > "$EVIDENCE/power.txt" 2>&1 || true
    "$ADB" shell dumpsys meminfo "$pkg" > "$EVIDENCE/meminfo-$pkg.txt" 2>&1 || true
done
blocked_ok "logcat excerpt and power/memory dumps collected"

if [ "$LIFECYCLE_CYCLES" -gt 0 ]; then
    CYCLES_FILE="$EVIDENCE/lifecycle-cycles.txt"
    {
        echo "cycles=$LIFECYCLE_CYCLES"
        echo "started=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
        for cycle in $(seq 1 "$LIFECYCLE_CYCLES"); do
            "$ADB" shell input keyevent KEYCODE_SLEEP >/dev/null 2>&1 || true
            "$ADB" shell input keyevent KEYCODE_WAKEUP >/dev/null 2>&1 || true
            "$ADB" shell input keyevent KEYCODE_HOME >/dev/null 2>&1 || true
            "$ADB" shell am start -n "$LAUNCH_ACTIVITY" >/dev/null 2>&1 || true
            printf 'cycle=%s stamp=%s ' "$cycle" "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
            "$ADB" shell pidof "$PRIMARY_PACKAGE" 2>/dev/null | tr -d '\r' || true
            printf '\n'
            "$ADB" shell dumpsys meminfo "$PRIMARY_PACKAGE" 2>/dev/null \
                | grep -E "TOTAL PSS|TOTAL RSS" | tr -d '\r' || true
        done
        echo "finished=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    } > "$CYCLES_FILE" 2>&1
    blocked_ok "the bounded lifecycle cycle loop wrote ${CYCLES_FILE#"$REPO_ROOT"/}"
else
    blocked_skip "the bounded lifecycle cycle loop (--lifecycle-cycles N)"
fi

STATUS="pass"
REASONS=""
if [ "$INSTALL_OK" -ne 1 ]; then
    STATUS="incomplete"
    REASONS+="- the candidate APK was not installed"$'\n'
fi
if [ "$RUNNING_PACKAGES" -eq 0 ]; then
    STATUS="incomplete"
    REASONS+="- no package under test was running after the launch"$'\n'
fi
if [ "$LIFECYCLE_CYCLES" -le 0 ]; then
    REASONS+="- the bounded lifecycle cycle loop was not run (--lifecycle-cycles N)"$'\n'
fi
[ -n "$REASONS" ] || REASONS="- (none)"$'\n'

REPORT="$EVIDENCE/report.md"
{
    echo "# ARM64 device evidence"
    echo
    echo "STATUS: $STATUS"
    echo
    echo "| Field | Value |"
    echo "| --- | --- |"
    echo "| git HEAD | $HEAD_SHA |"
    echo "| APK | ${APK#"$REPO_ROOT"/} |"
    echo "| APK SHA-256 | ${APK_SHA:-<none>} |"
    echo "| device model | $DEVICE_MODEL |"
    echo "| Android API level | $DEVICE_API |"
    echo "| Android release | $DEVICE_RELEASE |"
    echo "| build fingerprint | $DEVICE_FINGERPRINT |"
    echo "| primary ABI | $DEVICE_ABI |"
    echo "| packages running | $RUNNING_PACKAGES of ${#PACKAGES[@]} |"
    echo "| lifecycle cycles | $LIFECYCLE_CYCLES |"
    echo
    echo "Raw evidence: candidate-apk.txt, apk-native-libraries.txt, elf-boundary.txt,"
    echo "install.txt, session-readback.txt, logcat.txt, power.txt, meminfo-*.txt"
    echo "and, when the cycle loop ran, lifecycle-cycles.txt."
    echo
    echo "## Open points"
    echo
    printf '%s' "$REASONS"
} > "$REPORT"
if [ "$STATUS" = "pass" ]; then
    blocked_ok "device evidence report written to ${REPORT#"$REPO_ROOT"/}"
else
    blocked_gap "the device evidence report is incomplete (see report.md)"
fi

blocked_finish
exit $?
