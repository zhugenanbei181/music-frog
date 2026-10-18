#!/usr/bin/env bash
# Provision the pinned Android SDK inputs and test every registered APK on a
# disposable Gradle Managed Device. This script never selects a physical adb device.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ANDROID_DIR="$REPO_ROOT/android"
SDK_ROOT=""
if [[ -f "$ANDROID_DIR/local.properties" ]]; then
    SDK_ROOT="$(sed -n 's/^sdk\.dir=//p' "$ANDROID_DIR/local.properties" | tail -1 | tr -d '\r')"
fi
if [[ -z "$SDK_ROOT" ]]; then SDK_ROOT="${ANDROID_SDK_ROOT:-${ANDROID_HOME:-$HOME/Android/Sdk}}"; fi
SDK_ROOT="${SDK_ROOT/#\~/$HOME}"
[[ -d "$SDK_ROOT" ]] || { printf 'Android SDK directory does not exist: %s\n' "$SDK_ROOT" >&2; exit 2; }

if [[ -z "${JAVA_HOME:-}" ]]; then
    export JAVA_HOME="$(python3 "$REPO_ROOT/scripts/android-toolchain.py" resolve-java)"
    export PATH="$JAVA_HOME/bin:$PATH"
fi

find_sdkmanager() {
    local candidate
    local candidates=(
        "$SDK_ROOT/cmdline-tools/latest/bin/sdkmanager"
        "${ANDROID_SDK_MANAGER:-}"
        "${ANDROID_HOME:-}/cmdline-tools/latest/bin/sdkmanager"
        "${ANDROID_SDK_ROOT:-}/cmdline-tools/latest/bin/sdkmanager"
    )
    shopt -s nullglob
    candidates+=("$HOME"/android-tools*/android/cmdline-tools/latest/bin/sdkmanager)
    shopt -u nullglob
    for candidate in "${candidates[@]}"; do
        if [[ -n "$candidate" && -x "$candidate" ]]; then printf '%s\n' "$candidate"; return 0; fi
    done
    return 1
}
SDKMANAGER="$(find_sdkmanager || true)"
if [[ -z "$SDKMANAGER" ]]; then
    printf 'sdkmanager missing: install Command-line Tools (latest) in Android Studio or set ANDROID_SDK_MANAGER.\n' >&2
    exit 2
fi

EVIDENCE_DIR="${ANDROID_EMULATOR_EVIDENCE_DIR:-$REPO_ROOT/target/android-emulator-evidence/$(date -u +%Y%m%dT%H%M%SZ)}"
mkdir -p "$EVIDENCE_DIR"

host_network_digest() {
    python3 - <<'PY'
import hashlib
import json
import subprocess

def read_ip(*arguments):
    result = subprocess.run(["ip", "-j", *arguments], check=True, capture_output=True, text=True)
    return json.loads(result.stdout)

address_fields = ("ifindex", "ifname", "flags", "mtu", "operstate", "link_type", "address", "broadcast")
address_info_fields = ("family", "local", "prefixlen", "scope", "label", "flags", "peer", "broadcast", "anycast")
route_fields = ("dst", "gateway", "dev", "protocol", "scope", "type", "table", "prefsrc", "metric", "mtu", "onlink", "flags", "nexthops")
addresses = read_ip("address", "show")
routes = (read_ip("route", "show", "table", "all"), read_ip("-6", "route", "show", "table", "all"))
stable_addresses = []
for device in addresses:
    normalized = {key: device[key] for key in address_fields if key in device}
    normalized["addr_info"] = [
        {key: row[key] for key in address_info_fields if key in row}
        for row in device.get("addr_info", [])
    ]
    stable_addresses.append(normalized)
stable_routes = [
    [{key: row[key] for key in route_fields if key in row} for row in family]
    for family in routes
]
payload = json.dumps([stable_addresses, stable_routes], sort_keys=True, separators=(",", ":"))
print(hashlib.sha256(payload.encode()).hexdigest())
PY
}

HOST_NETWORK_BEFORE="$(host_network_digest)"
printf 'host_address_and_route_sha256_before=%s\n' "$HOST_NETWORK_BEFORE" > "$EVIDENCE_DIR/host-network.txt"
verify_host_network() {
    local original_status=$?
    local after
    trap - EXIT
    if ! after="$(host_network_digest)"; then
        printf 'host_network_check=FAILED_TO_READ\n' >> "$EVIDENCE_DIR/host-network.txt"
        if [[ "$original_status" -eq 0 ]]; then exit 90; fi
        exit "$original_status"
    fi
    printf 'host_address_and_route_sha256_after=%s\n' "$after" >> "$EVIDENCE_DIR/host-network.txt"
    if [[ "$after" != "$HOST_NETWORK_BEFORE" ]]; then
        printf 'host_network_check=CHANGED; no repair attempted\n' >> "$EVIDENCE_DIR/host-network.txt"
        printf 'Host network state changed during testing; no host network repair was attempted.\n' >&2
        if [[ "$original_status" -eq 0 ]]; then exit 91; fi
        exit "$original_status"
    fi
    printf 'host_network_check=UNCHANGED\n' >> "$EVIDENCE_DIR/host-network.txt"
    exit "$original_status"
}
trap verify_host_network EXIT

if [[ ! -x "$SDK_ROOT/cmdline-tools/latest/bin/sdkmanager" ]]; then
    set +o pipefail
    yes | "$SDKMANAGER" --sdk_root="$SDK_ROOT" --licenses >/dev/null
    license_status=$?
    set -o pipefail
    [[ "$license_status" -eq 0 ]] || exit "$license_status"
    "$SDKMANAGER" --sdk_root="$SDK_ROOT" --install 'cmdline-tools;latest'
fi
SDKMANAGER="$SDK_ROOT/cmdline-tools/latest/bin/sdkmanager"

ANDROID_FLAVORS="${ANDROID_FLAVORS:-all}"
ANDROID_RUN_EMULATOR="${ANDROID_RUN_EMULATOR:-true}"

if [[ "$ANDROID_FLAVORS" == "all" ]]; then
    SELECTED_FLAVORS=(compose bevy slint)
else
    # shellcheck disable=SC2206
    SELECTED_FLAVORS=($ANDROID_FLAVORS)
fi

SDK_PLATFORM="$(python3 "$REPO_ROOT/scripts/android-toolchain.py" get android.sdk.platform)"
BUILD_TOOLS="$(python3 "$REPO_ROOT/scripts/android-toolchain.py" get android.build.tools)"
EMULATOR_API="$(python3 "$REPO_ROOT/scripts/android-toolchain.py" get android.emulator.api)"
EMULATOR_IMAGE="$(python3 "$REPO_ROOT/scripts/android-toolchain.py" get android.emulator.image)"
NDK_VERSION="$(python3 "$REPO_ROOT/scripts/android-toolchain.py" get android.ndk.version)"
set +o pipefail
yes | "$SDKMANAGER" --sdk_root="$SDK_ROOT" --licenses >/dev/null
license_status=$?
set -o pipefail
[[ "$license_status" -eq 0 ]] || exit "$license_status"

INSTALL_PACKAGES=(
    'platform-tools'
    "platforms;android-$SDK_PLATFORM"
    "build-tools;$BUILD_TOOLS"
    "ndk;$NDK_VERSION"
)
if [[ "$ANDROID_RUN_EMULATOR" == "true" ]]; then
    INSTALL_PACKAGES+=(
        'emulator'
        "system-images;android-$EMULATOR_API;$EMULATOR_IMAGE"
    )
fi
"$SDKMANAGER" --sdk_root="$SDK_ROOT" --install "${INSTALL_PACKAGES[@]}"

REQUIRED_PATHS=(
    "$SDK_ROOT/platform-tools/adb"
    "$SDK_ROOT/build-tools/$BUILD_TOOLS/aapt"
    "$SDK_ROOT/ndk/$NDK_VERSION/source.properties"
)
if [[ "$ANDROID_RUN_EMULATOR" == "true" ]]; then
    REQUIRED_PATHS+=(
        "$SDK_ROOT/emulator/emulator"
        "$SDK_ROOT/system-images/android-$EMULATOR_API/${EMULATOR_IMAGE/;/\/}/source.properties"
    )
fi

for required_path in "${REQUIRED_PATHS[@]}"; do
    [[ -e "$required_path" ]] || { printf 'Required Android SDK package file missing: %s\n' "$required_path" >&2; exit 2; }
done

export ANDROID_HOME="$SDK_ROOT"
export ANDROID_SDK_ROOT="$SDK_ROOT"
export ANDROID_NDK_HOME="$SDK_ROOT/ndk/$NDK_VERSION"
export ANDROID_NDK_ROOT="$ANDROID_NDK_HOME"
export PATH="$SDK_ROOT/platform-tools:$SDK_ROOT/emulator:$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin:$PATH"
python3 "$REPO_ROOT/scripts/android-toolchain.py" check
{
    printf 'git_head=%s\n' "$(git -C "$REPO_ROOT" rev-parse HEAD)"
    printf 'git_dirty_sha256=%s\n' "$(git -C "$REPO_ROOT" status --porcelain | sha256sum | awk '{print $1}')"
    printf 'sdk_root=%s\n' "$SDK_ROOT"
    if [[ "$ANDROID_RUN_EMULATOR" == "true" ]]; then
        printf 'emulator_version=%s\n' "$("$SDK_ROOT/emulator/emulator" -version 2>&1 | head -1)"
        printf 'managed_device=pixel6Api36 (AOSP x86_64)\n'
        printf 'guest_network=airplane-mode during instrumentation; restored by test runner\n'
    else
        printf 'managed_device=none (emulator tests skipped)\n'
    fi
    printf 'selected_flavors=%s\n' "${SELECTED_FLAVORS[*]}"
} > "$EVIDENCE_DIR/provenance.txt"

GRADLE_TASKS=(
    --no-daemon --stacktrace --console=plain
    :core-host-tests:test
)
if [[ "$ANDROID_RUN_EMULATOR" == "true" ]]; then
    GRADLE_TASKS+=(-Pandroid.testoptions.manageddevices.emulator.gpu=swiftshader_indirect)
fi

for f in "${SELECTED_FLAVORS[@]}"; do
    case "$f" in
        compose|Compose)
            GRADLE_TASKS+=(:app:assembleComposeDebug :app:testComposeDebugUnitTest)
            if [[ "$ANDROID_RUN_EMULATOR" == "true" ]]; then
                GRADLE_TASKS+=(:app:pixel6Api36ComposeDebugAndroidTest)
            fi
            ;;
        bevy|Bevy)
            GRADLE_TASKS+=(:app:assembleBevyDebug :app:testBevyDebugUnitTest)
            if [[ "$ANDROID_RUN_EMULATOR" == "true" ]]; then
                GRADLE_TASKS+=(:app:pixel6Api36BevyDebugAndroidTest)
            fi
            ;;
        slint|Slint)
            GRADLE_TASKS+=(:app:assembleSlintDebug :app:testSlintDebugUnitTest)
            if [[ "$ANDROID_RUN_EMULATOR" == "true" ]]; then
                GRADLE_TASKS+=(:app:pixel6Api36SlintDebugAndroidTest)
            fi
            ;;
        *)
            printf 'Unknown Android flavor: %s\n' "$f" >&2
            exit 2
            ;;
    esac
done

(
    cd "$ANDROID_DIR"
    ./gradlew "${GRADLE_TASKS[@]}"
) 2>&1 | tee "$EVIDENCE_DIR/gradle.log"

GUARD_ARGS=(--aapt "$SDK_ROOT/build-tools/$BUILD_TOOLS/aapt")
for f in "${SELECTED_FLAVORS[@]}"; do
    case "$f" in
        compose|Compose)
            apk="$ANDROID_DIR/app/build/outputs/apk/compose/debug/app-compose-universal-debug.apk"
            GUARD_ARGS+=(--apk "compose=$apk")
            printf 'apk_compose_sha256=%s\n' "$(sha256sum "$apk" | awk '{print $1}')" >> "$EVIDENCE_DIR/provenance.txt"
            ;;
        bevy|Bevy)
            apk="$ANDROID_DIR/app/build/outputs/apk/bevy/debug/app-bevy-universal-debug.apk"
            GUARD_ARGS+=(--apk "bevy=$apk")
            printf 'apk_bevy_sha256=%s\n' "$(sha256sum "$apk" | awk '{print $1}')" >> "$EVIDENCE_DIR/provenance.txt"
            ;;
        slint|Slint)
            apk="$ANDROID_DIR/app/build/outputs/apk/slint/debug/app-slint-universal-debug.apk"
            GUARD_ARGS+=(--apk "slint=$apk")
            printf 'apk_slint_sha256=%s\n' "$(sha256sum "$apk" | awk '{print $1}')" >> "$EVIDENCE_DIR/provenance.txt"
            ;;
    esac
done
python3 "$REPO_ROOT/scripts/quality/android-product-identity-guard.py" "${GUARD_ARGS[@]}"

printf '[android-emulator-tests] all requested Android tests passed\n'
printf '[android-emulator-tests] evidence: %s\n' "$EVIDENCE_DIR"
