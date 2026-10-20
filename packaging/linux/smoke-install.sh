#!/usr/bin/env bash
# Install-and-start smoke for the final Linux artifacts.
#
# The smoke proves exactly three facts about a built Linux artifact:
#
#   1. install  the artifact installs on this machine and the installed tree
#               carries the registered product identity (package name, icon,
#               binary and desktop entry),
#   2. run      the installed binary starts and reaches its first window in a
#               private virtual display stack (nothing is mapped on the
#               operator's desktop session),
#   3. remove   the installation is removed again and nothing is left behind.
#
# The install strategy is a typed choice, never a silent fallback:
#
#   system   a real `dpkg -i` / `dpkg --purge` cycle; needs root and dpkg
#   staged   a rootless install: the package payload is unpacked into a
#            private staging root that is removed again afterwards
#
# The smoke always reports which strategy it used. A staged install is never
# reported as a system install.
#
# Usage:
#   bash packaging/linux/smoke-install.sh --product iced \
#       --deb dist/infiltrator_0.50.0_amd64.deb \
#       --appimage dist/Infiltrator-Linux-x86_64.AppImage
#
# Exit codes:
#   0  every leg passed
#   1  a real defect (the artifact does not install, start or clean up)
#   2  the environment for a leg is missing (no dpkg, no compositor)
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

PRODUCT="iced"
DEB_PATH=""
APPIMAGE_PATH=""
STRATEGY="auto"
EVIDENCE_ROOT=""
WINDOW_TIMEOUT=40

usage() {
    sed -n '2,30p' "${BASH_SOURCE[0]}"
    exit "${1:-0}"
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --product)
            PRODUCT="${2:-}"
            shift 2
            ;;
        --deb)
            DEB_PATH="${2:-}"
            shift 2
            ;;
        --appimage)
            APPIMAGE_PATH="${2:-}"
            shift 2
            ;;
        --strategy)
            STRATEGY="${2:-}"
            shift 2
            ;;
        --evidence-root)
            EVIDENCE_ROOT="${2:-}"
            shift 2
            ;;
        --window-timeout)
            WINDOW_TIMEOUT="${2:-}"
            shift 2
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

case "$STRATEGY" in
    auto | system | staged) ;;
    *)
        printf 'unknown install strategy: %s\n' "$STRATEGY" >&2
        exit 2
        ;;
esac

identity_value() {
    python3 "$REPO_ROOT/scripts/product_identity.py" --get "desktop.$PRODUCT.$1"
}

APP_BIN="$(identity_value binary_name)"
APP_PKG="$(identity_value linux_package_id)"
APP_DISPLAY_NAME="$(identity_value display_name)"
VERSION="$(python3 "$REPO_ROOT/scripts/product_identity.py" --get version.name)"

STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
GIT_HEAD="$(git rev-parse --short=12 HEAD 2>/dev/null || printf 'no-git')"
if [ -n "$(git status --porcelain 2>/dev/null | head -1)" ]; then
    WORKTREE_STATE="dirty"
else
    WORKTREE_STATE="clean"
fi
RUN_ID="${STAMP}_${GIT_HEAD}_${WORKTREE_STATE}_$$"
EVIDENCE_BASE="${EVIDENCE_ROOT:-$REPO_ROOT/target/platform-evidence/linux}"
EVIDENCE_DIR="$EVIDENCE_BASE/$RUN_ID"
mkdir -p "$EVIDENCE_DIR"

note() {
    printf '  %s\n' "$*"
}

RUNTIME_DIR=""
KWIN_RUNTIME=""
KWIN_DISPLAY=""
KWIN_PID=""
KWIN_PGID=""
NIRI_PID=""
NIRI_PGID=""
NIRI_SOCK=""
NIRI_IPC=""
APP_PID=""
APP_PGID=""
WINDOW_RECEIPT=""
INSTALL_ROOT=""
INSTALLED_TREE=""
INSTALLED_BINARY=""

process_group() {
    ps -o pgid= -p "$1" 2>/dev/null | tr -d '[:space:]'
}

terminate_owned() {
    local pid="$1" pgid="$2"
    [ -n "$pid" ] || return 0
    if [[ "$pgid" =~ ^[0-9]+$ ]] && [ "$pgid" = "$pid" ]; then
        kill -TERM -- "-$pgid" 2>/dev/null || true
    elif kill -0 "$pid" 2>/dev/null; then
        kill -TERM "$pid" 2>/dev/null || true
    fi
    for _ in $(seq 1 20); do
        kill -0 "$pid" 2>/dev/null || break
        sleep 0.1
    done
    if [[ "$pgid" =~ ^[0-9]+$ ]] && [ "$pgid" = "$pid" ]; then
        kill -KILL -- "-$pgid" 2>/dev/null || true
    elif kill -0 "$pid" 2>/dev/null; then
        kill -KILL "$pid" 2>/dev/null || true
    fi
    wait "$pid" 2>/dev/null || true
}

cleanup() {
    terminate_owned "$APP_PID" "$APP_PGID"
    if [ -n "$NIRI_IPC" ] && [ -S "$NIRI_IPC" ]; then
        NIRI_SOCKET="$NIRI_IPC" timeout 3s niri msg action quit >/dev/null 2>&1 || true
    fi
    terminate_owned "$NIRI_PID" "$NIRI_PGID"
    terminate_owned "$KWIN_PID" "$KWIN_PGID"
    [ -n "$RUNTIME_DIR" ] && rm -rf -- "$RUNTIME_DIR"
    [ -n "$KWIN_RUNTIME" ] && rm -rf -- "$KWIN_RUNTIME"
    return 0
}
trap cleanup EXIT

# --------------------------------------------------------------- the display --
# A private virtual display stack: an invisible kwin_wayland host with a
# nested niri inside it. The operator's own session is never contacted.
start_display() {
    command -v niri >/dev/null 2>&1 || {
        note "niri is not installed: the run leg needs a private virtual display"
        return 2
    }
    command -v kwin_wayland >/dev/null 2>&1 || {
        note "kwin_wayland is not installed: the run leg needs a private virtual display"
        return 2
    }
    RUNTIME_DIR="$(mktemp -d /tmp/infiltrator-install-smoke.XXXXXX)"
    chmod 700 "$RUNTIME_DIR"
    KWIN_RUNTIME="$(mktemp -d /tmp/infiltrator-install-smoke-kwin.XXXXXX)"
    chmod 700 "$KWIN_RUNTIME"
    KWIN_DISPLAY="$KWIN_RUNTIME/wayland-outer"

    XDG_RUNTIME_DIR="$KWIN_RUNTIME" WAYLAND_DISPLAY= DISPLAY= \
        QT_QPA_PLATFORM=wayland setsid timeout --foreground --kill-after=10s 10m \
        kwin_wayland --virtual --socket=wayland-outer --width=1920 --height=1080 \
        --scale=1 --no-global-shortcuts --no-lockscreen \
        &>"$EVIDENCE_DIR/kwin-wayland.log" &
    KWIN_PID=$!
    KWIN_PGID="$(process_group "$KWIN_PID")"
    local index
    for index in $(seq 1 40); do
        [ -S "$KWIN_DISPLAY" ] && break
        sleep 0.2
    done
    if [ ! -S "$KWIN_DISPLAY" ]; then
        note "the virtual KWin host did not come up"
        return 2
    fi

    local niri_config="$EVIDENCE_DIR/niri-smoke.kdl"
    cat >"$niri_config" <<'KDL'
// The install smoke only needs a compositor that reports windows; it never
// screenshots and it never touches the operator's session.
hotkey-overlay {
    skip-at-startup
}
output "winit" {
    scale 1
}
layout {
    focus-ring {
        off
    }
    border {
        off
    }
    shadow {
        off
    }
}
KDL
    if ! timeout 10s niri validate --config "$niri_config" >"$EVIDENCE_DIR/niri-validate.log" 2>&1; then
        note "the generated niri configuration is invalid"
        return 2
    fi

    XDG_RUNTIME_DIR="$RUNTIME_DIR" WAYLAND_DISPLAY="$KWIN_DISPLAY" DISPLAY= \
        LIBGL_ALWAYS_SOFTWARE=1 RUST_LOG=niri=info \
        setsid timeout --foreground --kill-after=10s 10m niri --config "$niri_config" \
        >"$EVIDENCE_DIR/niri.log" 2>&1 &
    NIRI_PID=$!
    NIRI_PGID="$(process_group "$NIRI_PID")"
    for index in $(seq 1 60); do
        kill -0 "$NIRI_PID" 2>/dev/null || break
        NIRI_SOCK="$(find "$RUNTIME_DIR" -maxdepth 1 -type s -name 'wayland-[0-9]*' \
            -printf '%f\n' -quit 2>/dev/null || true)"
        NIRI_IPC="$(find "$RUNTIME_DIR" -maxdepth 1 -type s -name 'niri.*.sock' \
            -print -quit 2>/dev/null || true)"
        if [ -n "$NIRI_SOCK" ] && [ -n "$NIRI_IPC" ]; then
            break
        fi
        sleep 0.2
    done
    if [ -z "$NIRI_SOCK" ] || [ -z "$NIRI_IPC" ]; then
        note "the nested niri compositor did not come up"
        return 2
    fi
    return 0
}

niri_window_for_pid() {
    local pid="$1"
    NIRI_SOCKET="$NIRI_IPC" timeout 5s niri msg -j windows 2>/dev/null \
        | python3 -c '
import json
import sys

pid = sys.argv[1]
try:
    windows = json.load(sys.stdin)
except ValueError:
    raise SystemExit(1)
for window in windows:
    if str(window.get("pid")) == pid:
        print("{}|{}|{}".format(window.get("id"), window.get("title", ""), window.get("app_id", "")))
        raise SystemExit(0)
raise SystemExit(1)
' "$pid"
}

# ------------------------------------------------------------------- the run --
start_product() {
    local binary="$1"
    local home="$EVIDENCE_DIR/home"
    mkdir -p "$home/.config" "$home/.local/share" "$home/.local/state" "$home/.cache"
    XDG_RUNTIME_DIR="$RUNTIME_DIR" WAYLAND_DISPLAY="$NIRI_SOCK" DISPLAY= \
        LIBGL_ALWAYS_SOFTWARE=1 HOME="$home" \
        XDG_CONFIG_HOME="$home/.config" XDG_DATA_HOME="$home/.local/share" \
        XDG_STATE_HOME="$home/.local/state" XDG_CACHE_HOME="$home/.cache" \
        INFILTRATOR_LANG="${INFILTRATOR_LANG:-en-US}" \
        setsid "$binary" >"$EVIDENCE_DIR/app.log" 2>&1 &
    APP_PID=$!
    APP_PGID="$(process_group "$APP_PID")"
    local index
    for index in $(seq 1 $((WINDOW_TIMEOUT * 4))); do
        if WINDOW_RECEIPT="$(niri_window_for_pid "$APP_PID")"; then
            return 0
        fi
        kill -0 "$APP_PID" 2>/dev/null || break
        sleep 0.25
    done
    return 1
}

# ------------------------------------------------------------------ the legs --
ARTIFACTS_JSON=""
INSTALL_STATUS="unsupported"
INSTALL_STRATEGY=""
INSTALL_DETAIL=""
RUN_STATUS="unsupported"
RUN_DETAIL=""
REMOVE_STATUS="unsupported"
REMOVE_DETAIL=""

artifact_digest() {
    sha256sum "$1" | cut -d' ' -f1
}

artifact_path() {
    case "$1" in
        "$REPO_ROOT"/*) printf '%s\n' "${1#"$REPO_ROOT"/}" ;;
        *) printf '%s\n' "$1" ;;
    esac
}

# One record per artifact: the artifact identity and the verdict of each of
# its three legs. The records are written as JSON by python so that a detail
# text can never break the file.
write_leg_record() {
    local kind="$1" artifact="$2"
    python3 - "$EVIDENCE_DIR/legs-$kind.json" "$kind" "$artifact" "$(artifact_path "$artifact")" \
        "$INSTALL_STATUS" "$INSTALL_STRATEGY" "$INSTALL_DETAIL" \
        "$RUN_STATUS" "$RUN_DETAIL" "$REMOVE_STATUS" "$REMOVE_DETAIL" <<'PY'
import hashlib
import json
import sys
from pathlib import Path

(
    output,
    kind,
    absolute,
    display,
    install_status,
    install_strategy,
    install_detail,
    run_status,
    run_detail,
    remove_status,
    remove_detail,
) = sys.argv[1:12]

record = {
    "kind": kind,
    "path": display,
    "sha256": hashlib.sha256(Path(absolute).read_bytes()).hexdigest(),
    "legs": {
        "install": {
            "status": install_status,
            "strategy": install_strategy,
            "detail": install_detail,
        },
        "run": {"status": run_status, "detail": run_detail},
        "remove": {"status": remove_status, "detail": remove_detail},
    },
}
Path(output).write_text(json.dumps(record, ensure_ascii=False, indent=2), encoding="utf-8")
PY
}

install_deb() {
    local root=""
    case "$STRATEGY" in
        system | auto)
            if command -v dpkg >/dev/null 2>&1 && [ "$(id -u)" -eq 0 ]; then
                if dpkg -i "$DEB_PATH" >>"$EVIDENCE_DIR/install.log" 2>&1; then
                    INSTALL_STRATEGY="system"
                    INSTALL_ROOT="/"
                    INSTALLED_TREE="/"
                    INSTALLED_BINARY="/usr/bin/$APP_BIN"
                    return 0
                fi
                if [ "$STRATEGY" = "system" ]; then
                    INSTALL_DETAIL="dpkg -i failed; see install.log"
                    return 1
                fi
                note "dpkg -i failed; falling back to the staged install"
            elif [ "$STRATEGY" = "system" ]; then
                INSTALL_DETAIL="the system install needs root and the dpkg tools"
                return 2
            fi
            ;;
    esac
    root="$EVIDENCE_DIR/staged-root"
    rm -rf "$root"
    if ! python3 "$REPO_ROOT/packaging/linux/deb/deb_archive.py" extract \
        --package "$DEB_PATH" --destination "$root" >>"$EVIDENCE_DIR/install.log" 2>&1; then
        INSTALL_DETAIL="the package payload cannot be unpacked; see install.log"
        return 1
    fi
    INSTALL_STRATEGY="staged"
    INSTALL_ROOT="$root"
    INSTALLED_TREE="$root"
    INSTALLED_BINARY="$root/usr/bin/$APP_BIN"
    return 0
}

install_appimage() {
    local root="$EVIDENCE_DIR/appimage-root"
    rm -rf "$root"
    mkdir -p "$root"
    if ! (cd "$root" && APPIMAGE_EXTRACT_AND_RUN=1 "$APPIMAGE_PATH" --appimage-extract) \
        >>"$EVIDENCE_DIR/install.log" 2>&1; then
        INSTALL_DETAIL="the AppImage payload cannot be extracted; see install.log"
        return 1
    fi
    INSTALL_STRATEGY="extracted"
    INSTALL_ROOT="$root"
    INSTALLED_TREE="$root/squashfs-root"
    INSTALLED_BINARY="$INSTALLED_TREE/usr/bin/$APP_BIN"
    return 0
}

verify_installed() {
    local layout="$1"
    python3 "$REPO_ROOT/packaging/linux/verify-linux-artifacts.py" \
        --product "$PRODUCT" --layout "$layout" --installed-root "$INSTALLED_TREE" \
        >>"$EVIDENCE_DIR/identity.log" 2>&1
}

remove_install() {
    case "$INSTALL_STRATEGY" in
        system)
            dpkg -r "$APP_PKG" >>"$EVIDENCE_DIR/remove.log" 2>&1 || true
            dpkg --purge "$APP_PKG" >>"$EVIDENCE_DIR/remove.log" 2>&1 || true
            if [ -e "/usr/bin/$APP_BIN" ]; then
                REMOVE_DETAIL="the installed binary is still present after the purge"
                return 1
            fi
            ;;
        staged | extracted)
            rm -rf "$INSTALL_ROOT"
            if [ -e "$INSTALL_ROOT" ]; then
                REMOVE_DETAIL="the installation root is still present after the removal"
                return 1
            fi
            ;;
    esac
    REMOVE_DETAIL="the ${INSTALL_STRATEGY} installation was removed"
    return 0
}

run_artifact() {
    local kind="$1" artifact="$2" layout="$3"
    printf '\n== %s ==\n' "$kind"
    note "artifact: $artifact"

    INSTALL_STATUS="unsupported"
    INSTALL_STRATEGY=""
    INSTALL_DETAIL=""
    RUN_STATUS="unsupported"
    RUN_DETAIL=""
    REMOVE_STATUS="unsupported"
    REMOVE_DETAIL=""

    if [ "$kind" = "deb" ]; then
        DEB_PATH="$artifact"
        install_deb || return $?
    else
        APPIMAGE_PATH="$artifact"
        install_appimage || return $?
    fi
    note "install: ${INSTALL_STRATEGY} -> ${INSTALLED_TREE}"
    INSTALL_STATUS="proven"

    if ! verify_installed "$layout"; then
        INSTALL_STATUS="failed"
        INSTALL_DETAIL="the installed tree does not carry the registered identity; see identity.log"
        remove_install || true
        return 1
    fi
    INSTALL_DETAIL="the installed tree carries the registered identity"

    if ! start_display; then
        RUN_STATUS="unsupported"
        RUN_DETAIL="no private virtual display is available on this machine"
        remove_install || true
        return 2
    fi

    if start_product "$INSTALLED_BINARY"; then
        RUN_STATUS="proven"
        RUN_DETAIL="the installed binary mapped its window: ${WINDOW_RECEIPT}"
        note "run: window ${WINDOW_RECEIPT}"
    else
        RUN_STATUS="failed"
        RUN_DETAIL="the installed binary did not map a window; see app.log"
        note "run: no window"
    fi
    terminate_owned "$APP_PID" "$APP_PGID"
    APP_PID=""
    APP_PGID=""

    if remove_install; then
        REMOVE_STATUS="proven"
    else
        REMOVE_STATUS="failed"
    fi
    note "remove: ${REMOVE_DETAIL}"

    write_leg_record "$kind" "$artifact"
    [ "$RUN_STATUS" = "proven" ] || return 1
    return 0
}
# ------------------------------------------------------------------- the run --
printf 'install smoke: product=%s version=%s run=%s\n' "$PRODUCT" "$VERSION" "$RUN_ID"
note "evidence: ${EVIDENCE_DIR#$REPO_ROOT/}"

STATUS="proven"
DETAIL="every leg passed"

if [ -n "$DEB_PATH" ]; then
    [ -f "$DEB_PATH" ] || { printf 'the Debian package is missing: %s\n' "$DEB_PATH" >&2; exit 2; }
    if run_artifact "deb" "$DEB_PATH" "deb"; then
        :
    else
        code=$?
        if [ "$code" -eq 2 ]; then
            STATUS="unsupported"
            DETAIL="the environment for the Debian package leg is missing"
        else
            STATUS="failed"
            DETAIL="the Debian package leg did not pass"
        fi
    fi
fi

if [ -n "$APPIMAGE_PATH" ]; then
    [ -f "$APPIMAGE_PATH" ] || { printf 'the AppImage is missing: %s\n' "$APPIMAGE_PATH" >&2; exit 2; }
    if run_artifact "appimage" "$APPIMAGE_PATH" "appimage"; then
        :
    else
        code=$?
        if [ "$code" -eq 2 ]; then
            STATUS="unsupported"
            DETAIL="the environment for the AppImage leg is missing"
        else
            STATUS="failed"
            DETAIL="the AppImage leg did not pass"
        fi
    fi
fi

if [ -z "$DEB_PATH" ] && [ -z "$APPIMAGE_PATH" ]; then
    printf 'nothing to smoke: pass --deb and/or --appimage\n' >&2
    exit 2
fi

python3 - "$EVIDENCE_DIR/evidence.json" "$PRODUCT" "$VERSION" "$RUN_ID" "$GIT_HEAD" \
    "$WORKTREE_STATE" "$STATUS" "$DETAIL" "$EVIDENCE_DIR" <<'PY'
import json
import sys
from pathlib import Path

(
    output,
    product,
    version,
    run_id,
    git_head,
    worktree,
    status,
    detail,
    evidence_dir,
) = sys.argv[1:10]

records = []
for record_path in sorted(Path(evidence_dir).glob("legs-*.json")):
    records.append(json.loads(record_path.read_text(encoding="utf-8")))

evidence = {
    "schema_version": 1,
    "platform": "linux",
    "product": product,
    "version": version,
    "run_id": run_id,
    "git_head": git_head,
    "worktree": worktree,
    "status": status,
    "detail": detail,
    "artifacts": records,
}
Path(output).write_text(json.dumps(evidence, ensure_ascii=False, indent=2), encoding="utf-8")
print(f"install smoke: evidence written to {output}")
PY

printf 'install smoke: %s (%s)\n' "$STATUS" "$DETAIL"
case "$STATUS" in
    proven) exit 0 ;;
    unsupported) exit 2 ;;
    *) exit 1 ;;
esac
