#!/usr/bin/env bash
# Capture the Infiltrator Bevy frontend demo screenshot matrix in one fully
# backgrounded compositor stack (native bevy render evidence — the bevy
# sibling of capture-iced-matrix.sh).
#
# Pipeline: build once -> start one private virtual KWin host -> start one
# nested niri inside that host -> launch one demo scenario at a time -> wait
# for the app's CAPTURE_READY marker -> bind the screenshot to the exact
# PID/app-id/window-id via niri IPC -> publish the PNG plus a manifest into
# docs/screenshots/bevy/. The operator's Wayland session is never contacted:
# the nested niri's winit window lives inside the invisible
# `kwin_wayland --virtual` host, so nothing can pop up on or steal focus from
# the operator's desktop, and no compositor UI chrome (hotkey overlay etc.)
# is ever mapped.
#
# Differences from the iced pipeline (documented for reviewers):
#   * the bevy frontend is a STANDALONE workspace: `cargo build` runs inside
#     crates/infiltrator-bevy-ui/, producing
#     crates/infiltrator-bevy-ui/target/debug/infiltrator-bevy-ui;
#   * the app's env knobs are INFILTRATOR_BEVY_SKIN (dark|light),
#     INFILTRATOR_BEVY_WINDOW_SIZE (WxH) and INFILTRATOR_CAPTURE_MARKER
#     (frame-counted readiness file written by src/capture.rs);
#   * the window identity is the title "MusicFrog Infiltrator — Bevy"
#     (niri IPC + window-rule bind on title + spawned PID);
#   * BEVY_ASSET_ROOT points at the widget crate so the embedded icon
#     plates (assets/icons/*.png) resolve through the host AssetServer;
#     without it bevy_asset resolves assets under target/debug/assets/ and
#     every icon degrades to an invisible square;
#   * RENDERING FALLBACK: attempt 1 launches with bevy's default wgpu
#     backends; if the render device cannot initialize in this software
#     environment (no Vulkan ICD for the virtual host — observed as an app
#     crash before any frame), attempt 2 relaunches with WGPU_BACKEND=gl
#     (EGL/llvmpipe, which the nested niri itself also runs on).
#
# Usage:
#   bash scripts/capture-bevy-matrix.sh                  # full matrix
#   bash scripts/capture-bevy-matrix.sh overview-dark    # one scenario
#   INFILTRATOR_CAPTURE_SCENARIOS=a,b bash scripts/capture-bevy-matrix.sh
#
# Exit codes: 0 all scenarios pass; 1 some scenarios failed (evidence kept);
# 2 preflight/usage error; 3 BLOCKED (compositor never came up).
set -euo pipefail

REPO="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO"
export RUSTC_WRAPPER=

APP_ID="infiltrator-bevy-ui"
# Runtime check (niri msg -j windows): the app maps its toplevel with the
# title below; winit's app_id may be the binary name or empty depending on
# version, so the title is the stable observed identifier and the spawned
# PID stays the authoritative binding either way.
APP_TITLE="MusicFrog Infiltrator — Bevy"
CRATE_DIR="$REPO/crates/infiltrator-bevy-ui"
APP="$CRATE_DIR/target/debug/infiltrator-bevy-ui"
ASSET_ROOT="$REPO/crates/infiltrator-bevy-widgets"
MATRIX_SRC="${INFILTRATOR_CAPTURE_MATRIX:-$REPO/scripts/capture_bevy_scenarios.tsv}"
OUT_DIR="${INFILTRATOR_CAPTURE_OUT_DIR:-$REPO/docs/screenshots/bevy}"
MANIFEST_PUBLISHED="$OUT_DIR/manifest.tsv"
EVIDENCE_ROOT="$REPO/target/bevy-evidence"
RUN_STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
GIT_HEAD="$(git rev-parse --short=12 HEAD 2>/dev/null || printf 'no-git')"
if [ -n "$(git status --porcelain 2>/dev/null)" ]; then
  WORKTREE_STATE=dirty
else
  WORKTREE_STATE=clean
fi
RUN_ID="${RUN_STAMP}_${GIT_HEAD}_${WORKTREE_STATE}_$$"
RUN_DIR="$EVIDENCE_ROOT/$RUN_ID"
# Keep niri's IPC and Wayland socket paths below Linux's sockaddr_un limit (108 bytes) even
# when the checkout itself lives under a long mounted workspace path.
TMP_BASE="${TMPDIR:-/tmp}"
RUNTIME_DIR="$(mktemp -d "${TMP_BASE}/nr.XXXXXX")"
CONF="$RUN_DIR/niri-capture.kdl"
METADATA="$RUN_DIR/capture-metadata.txt"
NIRI_OUTPUTS="$RUN_DIR/niri-outputs.json"
MANIFEST="$RUN_DIR/capture-manifest.tsv"
KWIN_RUNTIME=""
KWIN_DISPLAY=""

MARKER_TIMEOUT_ITERS=120 # 120 * 0.25s = 30s readiness budget
WINDOW_TIMEOUT_ITERS=80  # 80 * 0.25s = 20s window-discovery budget
KWIN_START_ITERS=40      # 40 * 0.2s = 8s virtual-host socket budget
NIRI_START_ITERS=60      # 60 * 0.2s = 12s compositor-start budget

NIRI_PID=""
NIRI_PGID=""
APP_PID=""
APP_PGID=""
KWIN_PID=""
KWIN_PGID=""
NIRI_SOCK=""
NIRI_IPC=""
KEEP_RUNTIME=0
START_EPOCH="$(date +%s)"

process_group() {
  local pid="$1"
  ps -o pgid= -p "$pid" 2>/dev/null | tr -d '[:space:]'
}

terminate_owned() {
  local pid="$1" pgid="$2"
  [ -n "$pid" ] || return 0
  if [[ "$pgid" =~ ^[0-9]+$ ]] && [ "$pgid" = "$pid" ]; then
    kill -TERM -- "-$pgid" 2>/dev/null || true
  else
    kill -TERM "$pid" 2>/dev/null || true
  fi
  for _ in $(seq 1 20); do
    if [[ "$pgid" =~ ^[0-9]+$ ]] && [ "$pgid" = "$pid" ]; then
      kill -0 -- "-$pgid" 2>/dev/null || break
    else
      kill -0 "$pid" 2>/dev/null || break
    fi
    sleep 0.1
  done
  if [[ "$pgid" =~ ^[0-9]+$ ]] && [ "$pgid" = "$pid" ]; then
    kill -0 -- "-$pgid" 2>/dev/null && kill -KILL -- "-$pgid" 2>/dev/null || true
  elif kill -0 "$pid" 2>/dev/null; then
    kill -KILL "$pid" 2>/dev/null || true
  fi
  wait "$pid" 2>/dev/null || true
}

niri_ipc() {
  # Run a niri msg command against the nested compositor's IPC socket.
  NIRI_SOCKET="$NIRI_IPC" timeout 3s niri msg "$@"
}

fail_hard() {
  printf '%s\n' "$1" >&2
  exit "${2:-1}"
}

cleanup() {
  # Order matters: ask the nested niri to quit over IPC first (graceful), then
  # tear down app/niri/kwin process groups, then remove the private runtime
  # dirs. Run evidence under target/bevy-evidence/ is always preserved.
  if [ -n "$NIRI_IPC" ] && [ -S "$NIRI_IPC" ]; then
    niri_ipc action quit >/dev/null 2>&1 || true
    for _ in $(seq 1 10); do
      kill -0 "$NIRI_PID" 2>/dev/null || break
      sleep 0.1
    done
  fi
  terminate_owned "$APP_PID" "$APP_PGID"
  terminate_owned "$NIRI_PID" "$NIRI_PGID"
  terminate_owned "$KWIN_PID" "$KWIN_PGID"
  if [ -n "$KWIN_RUNTIME" ] && [ -d "$KWIN_RUNTIME" ]; then
    rm -rf -- "$KWIN_RUNTIME"
  fi
  if [ "$KEEP_RUNTIME" -eq 1 ] && [ -d "$RUNTIME_DIR" ]; then
    mv "$RUNTIME_DIR" "${RUNTIME_DIR}-debug" 2>/dev/null || true
  else
    rm -rf -- "$RUNTIME_DIR"
  fi
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

png_dims() {
  # Print "<width> <height>" for a PNG by reading the IHDR header directly
  # (no third-party image library required).
  python3 - "$1" <<'PY'
import struct
import sys

try:
    with open(sys.argv[1], "rb") as f:
        head = f.read(24)
    if head[:8] != b"\x89PNG\r\n\x1a\n" or head[12:16] != b"IHDR":
        raise ValueError("not a PNG IHDR")
    width, height = struct.unpack(">II", head[16:24])
    print(width, height)
except Exception:
    raise SystemExit(1)
PY
}

# ---------------------------------------------------------------- preflight --
for command in awk cargo find git jq niri python3 ps sha256sum setsid stat timeout; do
  command -v "$command" >/dev/null 2>&1 || {
    printf 'required capture command is unavailable: %s\n' "$command" >&2
    exit 2
  }
done
command -v kwin_wayland >/dev/null 2>&1 || {
  printf 'background capture requires kwin_wayland --virtual (invisible compositor host); it is not installed\n' >&2
  exit 2
}
mkdir -p "$RUN_DIR" "$OUT_DIR" "$ASSET_ROOT"
chmod 700 "$RUNTIME_DIR"

# --------------------------------------------------------- scenario matrix --
SCENARIO_FILTER="${INFILTRATOR_CAPTURE_SCENARIOS:-${1:-}}"
MATRIX="$MATRIX_SRC"
if [ -n "$SCENARIO_FILTER" ]; then
  MATRIX="$RUN_DIR/scenarios-selected.tsv"
  awk -F '\t' -v list="$SCENARIO_FILTER" '
    BEGIN {
      count = split(list, names, ",")
      for (i = 1; i <= count; i++) wanted[names[i]] = 1
    }
    NR == 1 || wanted[$1] { print }
  ' "$MATRIX_SRC" >"$MATRIX"
  [ "$(wc -l <"$MATRIX")" -gt 1 ] || {
    printf 'no scenarios matched INFILTRATOR_CAPTURE_SCENARIOS=%s (source: %s)\n' \
      "$SCENARIO_FILTER" "$MATRIX_SRC" >&2
    exit 2
  }
fi

while IFS=$'\t' read -r name page skin window_size; do
  [ "$name" = name ] && continue
  [ -n "$name" ] || continue
  [[ "$window_size" =~ ^[0-9]+x[0-9]+$ ]] || {
    printf 'invalid scenario row (bad window_size): %s\n' "$name" >&2
    exit 2
  }
  case "$page" in
  overview) ;;
  *)
    printf 'invalid scenario row (bad page): %s -> %s\n' "$name" "$page" >&2
    exit 2
    ;;
  esac
  case "$skin" in
  light | dark) ;;
  *)
    printf 'invalid scenario row (bad skin): %s -> %s\n' "$name" "$skin" >&2
    exit 2
    ;;
  esac
done <"$MATRIX"

SCENARIO_COUNT="$(awk -F '\t' 'NR > 1 && $1 != "" { count += 1 } END { print count + 0 }' "$MATRIX")"
[ "$SCENARIO_COUNT" -gt 0 ] || {
  printf 'capture matrix is empty: %s\n' "$MATRIX" >&2
  exit 2
}
# The published tree must reflect exactly this run: drop previous outputs for
# the scenarios about to be captured (full-matrix runs therefore start clean).
awk -F '\t' 'NR > 1 && $1 != "" { print $1 }' "$MATRIX" | while IFS= read -r name; do
  rm -f "$OUT_DIR/$name.png"
done

# ------------------------------------------------------------------- build --
printf 'build: cargo build --quiet (standalone workspace: crates/infiltrator-bevy-ui)\n'
timeout --kill-after=10s 20m cargo build --quiet --manifest-path "$CRATE_DIR/Cargo.toml"
[ -x "$APP" ] || fail_hard "bevy binary was not produced: $APP"
BINARY_SHA256="$(sha256sum "$APP" | cut -d' ' -f1)"

# ----------------------------------------------------------------- receipt --
CAPTURED_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
{
  printf 'run_id=%s\n' "$RUN_ID"
  printf 'captured_at=%s\n' "$CAPTURED_AT"
  printf 'git_head=%s\n' "$GIT_HEAD"
  printf 'worktree=%s\n' "$WORKTREE_STATE"
  printf 'rust=%s\n' "$(rustc -V 2>/dev/null || printf 'unknown')"
  printf 'niri=%s\n' "$(niri --version)"
  printf 'binary=crates/infiltrator-bevy-ui/target/debug/infiltrator-bevy-ui\n'
  printf 'binary_sha256=%s\n' "$BINARY_SHA256"
  printf 'app_id=%s\n' "$APP_ID"
  printf 'matrix=scripts/capture_bevy_scenarios.tsv\n'
  printf 'scenario_count=%s\n' "$SCENARIO_COUNT"
  printf 'niri_host=kwin-wayland-virtual\n'
  printf 'command=bash scripts/capture-bevy.sh\n'
} >"$METADATA"
printf 'scenario\tpage\tskin\trequested_window\tapp_pid\twindow_id\twidth\theight\tbytes\tsha256\tstatus\n' >"$MANIFEST"

# ------------------------------------------------------- niri config (KDL) --
cat >"$CONF" <<KDL
// Capture config: the app window floats so it keeps its requested
// INFILTRATOR_BEVY_WINDOW_SIZE instead of being tiled/resized by the
// compositor. Both matches are kept because winit has been observed to map
// windows with either the binary-name app_id or an empty one depending on
// version; the title is the stable identifier.
screenshot-path "$RUNTIME_DIR/shot.png"
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
    // niri's screenshot-window includes the window shadow in the view; turn it
    // off so the PNG is exactly the app's requested window size plus only the
    // compositor's screenshot margin.
    shadow {
        off
    }
}
window-rule {
    match app-id="$APP_ID"
    match title="^MusicFrog Infiltrator — Bevy\$"
    open-floating true
}
KDL
if ! validate_out="$(timeout 10s niri validate --config "$CONF" 2>&1)"; then
  printf '%s\n' "$validate_out" >&2
  fail_hard "generated niri config failed validation: $CONF"
fi

# ------------------------------------------------------------- capture host --
# The nested niri's winit backend needs *some* Wayland host. Using a private
# virtual KWin compositor keeps that host window invisible: it never maps on
# the operator's desktop, never takes focus, and cannot be seen at all. KWin
# runs in its own private XDG_RUNTIME_DIR and with WAYLAND_DISPLAY/DISPLAY
# cleared so it cannot reach any existing session.
start_capture_host() {
  local tmp_base="${TMPDIR:-/tmp}"
  KWIN_RUNTIME="$(mktemp -d "${tmp_base}/kw.XXXXXX")"
  KWIN_DISPLAY="$KWIN_RUNTIME/wayland-outer"
  chmod 700 "$KWIN_RUNTIME"
  local kwin_log="$RUN_DIR/kwin-wayland.log"
  XDG_RUNTIME_DIR="$KWIN_RUNTIME" WAYLAND_DISPLAY= DISPLAY= \
    QT_QPA_PLATFORM=wayland setsid timeout --foreground --kill-after=10s 20m \
    kwin_wayland --virtual --socket=wayland-outer --width=1920 --height=1080 \
      --scale=1 --no-global-shortcuts --no-lockscreen \
    &>"$kwin_log" & KWIN_PID=$!
  KWIN_PGID=""
  local i
  for i in $(seq 1 40); do
    KWIN_PGID="$(process_group "$KWIN_PID")"
    [ "$KWIN_PGID" = "$KWIN_PID" ] && break
    sleep 0.1
  done
  if [ "$KWIN_PGID" != "$KWIN_PID" ]; then
    printf 'BLOCKED (compositor): virtual KWin did not obtain a private process group; evidence retained at %s\n' \
      "$RUN_DIR" >&2
    return 1
  fi
  for i in $(seq 1 "$KWIN_START_ITERS"); do
    [ -S "$KWIN_DISPLAY" ] && break
    sleep 0.2
  done
  if ! kill -0 "$KWIN_PID" 2>/dev/null || [ ! -S "$KWIN_DISPLAY" ]; then
    printf 'BLOCKED (compositor): virtual KWin did not start; tail of log:\n' >&2
    tail -20 "$kwin_log" >&2
    return 1
  fi
  printf 'background capture host: kwin_wayland --virtual (%s)\n' "$KWIN_DISPLAY"
}

start_capture_host || exit 3

# -------------------------------------------------------- nested niri start --
# Niri connects to the virtual KWin host (so its own window is invisible) and
# serves the demo app plus the screenshot IPC from the private RUNTIME_DIR.
# DISPLAY is cleared so winit cannot fall back to any X11 server either.
XDG_RUNTIME_DIR="$RUNTIME_DIR" WAYLAND_DISPLAY="$KWIN_DISPLAY" DISPLAY= \
  LIBGL_ALWAYS_SOFTWARE=1 RUST_LOG=niri=info \
  setsid timeout --foreground --kill-after=10s 20m niri --config "$CONF" \
  >"$RUN_DIR/niri.log" 2>&1 &
NIRI_PID=$!
NIRI_PGID="$(process_group "$NIRI_PID")"
[ "$NIRI_PGID" = "$NIRI_PID" ] || {
  printf 'BLOCKED (compositor): nested niri did not obtain a private process group\n' >&2
  exit 3
}

# Socket + IPC discovery: poll the private runtime dir; the niri log is the
# fallback source for both names.
NIRI_SOCK=""
NIRI_IPC=""
for _ in $(seq 1 "$NIRI_START_ITERS"); do
  if ! kill -0 "$NIRI_PID" 2>/dev/null; then
    break
  fi
  NIRI_SOCK="$(find "$RUNTIME_DIR" -maxdepth 1 -type s -name 'wayland-[0-9]*' \
    -printf '%f\n' -quit 2>/dev/null || true)"
  if [ -z "$NIRI_SOCK" ]; then
    NIRI_SOCK="$(grep -oE 'listening on Wayland socket: wayland-[0-9]+' \
      "$RUN_DIR/niri.log" 2>/dev/null | tail -1 | awk '{ print $NF }' || true)"
  fi
  NIRI_IPC="$(find "$RUNTIME_DIR" -maxdepth 1 -type s -name 'niri.*.sock' \
    -print -quit 2>/dev/null || true)"
  if [ -z "$NIRI_IPC" ]; then
    NIRI_IPC="$(grep -oE "$RUNTIME_DIR/niri\.[^ ]*\.sock" "$RUN_DIR/niri.log" | head -1 || true)"
  fi
  if [ -n "$NIRI_SOCK" ] && [ -S "$RUNTIME_DIR/$NIRI_SOCK" ] && [ -n "$NIRI_IPC" ]; then
    break
  fi
  sleep 0.2
done
if ! kill -0 "$NIRI_PID" 2>/dev/null || [ -z "$NIRI_SOCK" ] || [ -z "$NIRI_IPC" ]; then
  {
    printf 'BLOCKED (compositor): nested niri never came up'
    printf ' (wayland socket: %s, ipc socket: %s)\n' "${NIRI_SOCK:-none}" "${NIRI_IPC:-none}"
    printf 'tail of %s:\n' "$RUN_DIR/niri.log"
  } >&2
  tail -30 "$RUN_DIR/niri.log" >&2 || true
  exit 3
fi

# Require a responsive IPC and the scale-1 winit output before any scenario.
NIRI_OUTPUTS_TMP="$RUN_DIR/niri-outputs.json.tmp"
OUTPUT_READY=0
for _ in $(seq 1 30); do
  if niri_ipc -j outputs >"$NIRI_OUTPUTS_TMP" 2>"$RUN_DIR/niri-outputs-error.log" \
    && jq -e '((type == "object" and (.winit | type == "object") and .winit.name == "winit") or (type == "array" and length == 1 and .[0].name == "winit"))' \
      "$NIRI_OUTPUTS_TMP" >/dev/null 2>&1; then
    mv "$NIRI_OUTPUTS_TMP" "$NIRI_OUTPUTS"
    OUTPUT_READY=1
    break
  fi
  sleep 0.5
done
[ "$OUTPUT_READY" -eq 1 ] || {
  printf 'BLOCKED (compositor): nested niri output receipt failed; evidence retained at %s\n' \
    "$RUN_DIR" >&2
  [ -s "$RUN_DIR/niri-outputs-error.log" ] && {
    printf 'last niri msg error:\n' >&2
    cat "$RUN_DIR/niri-outputs-error.log" >&2
  }
  # Preserve the private runtime dir so the socket state stays inspectable.
  KEEP_RUNTIME=1
  exit 3
}
printf 'compositor: ready (wayland=%s ipc=%s)\n' "$NIRI_SOCK" "$NIRI_IPC"

# -------------------------------------------------------------- per scenario --
capture_one() {
  local name="$1" page="$2" skin="$3" window_size="$4"
  # The app writes the marker naming its only mounted route (overview).
  local marker_page="$page"
  local scenario_dir="$RUN_DIR/$name"
  local log="$scenario_dir/app.log"
  local marker="$scenario_dir/marker.log"
  local windows_json="$scenario_dir/windows.json"
  local windows_tmp="$scenario_dir/windows.json.tmp"
  local windows_error="$scenario_dir/windows.err"
  local action="$scenario_dir/action.log"
  local image="$scenario_dir/image.png"
  local published="docs/screenshots/bevy/$name.png"
  local exp_w="${window_size%x*}" exp_h="${window_size#*x}"
  local window_id="" window_ready=0 marker_ready=0 action_status=failed
  local width=0 height=0 bytes=0 hash="-"
  local attempt=1
  # Attempt 1 = bevy's default wgpu backends; attempt 2 = WGPU_BACKEND=gl
  # (software-environment fallback, see the header comment).
  local wgpu_backend="" attempt_note=""

  mkdir -p "$scenario_dir"

  # Two attempts per scenario: attempt 1 proves the default render device;
  # if it cannot initialize in the virtual host (or the session shows the
  # nested-Wayland launch flakiness), attempt 2 relaunches on the GL
  # backend. Each attempt tears the previous app down and starts clean; the
  # manifest row is written exactly once from the final attempt.
  while [ "$attempt" -le 2 ]; do
    if [ "$attempt" -gt 1 ]; then
      wgpu_backend="gl"
      attempt_note="(retry, WGPU_BACKEND=gl)"
      printf '  retry %-22s (attempt %d/2, WGPU_BACKEND=gl)\n' "$name" "$attempt"
    fi
    rm -f "$marker" "$windows_json" "$windows_tmp" "$windows_error" "$action" "$image"
    terminate_owned "$APP_PID" "$APP_PGID"
    APP_PID=""
    APP_PGID=""
    window_id=""
    window_ready=0
    marker_ready=0
    action_status=failed
    width=0
    height=0
    bytes=0
    hash="-"

    printf '  start %-22s page=%-8s skin=%-5s window=%s %s\n' \
      "$name" "$page" "$skin" "$window_size" "$attempt_note"
    if [ -n "$wgpu_backend" ]; then
      XDG_RUNTIME_DIR="$RUNTIME_DIR" WAYLAND_DISPLAY="$NIRI_SOCK" DISPLAY= \
        LIBGL_ALWAYS_SOFTWARE=1 WGPU_BACKEND="$wgpu_backend" \
        BEVY_ASSET_ROOT="$ASSET_ROOT" \
        INFILTRATOR_BEVY_SKIN="$skin" \
        INFILTRATOR_BEVY_WINDOW_SIZE="$window_size" \
        INFILTRATOR_CAPTURE_MARKER="$marker" \
        setsid "$APP" >"$log" 2>&1 &
    else
      XDG_RUNTIME_DIR="$RUNTIME_DIR" WAYLAND_DISPLAY="$NIRI_SOCK" DISPLAY= \
        LIBGL_ALWAYS_SOFTWARE=1 \
        BEVY_ASSET_ROOT="$ASSET_ROOT" \
        INFILTRATOR_BEVY_SKIN="$skin" \
        INFILTRATOR_BEVY_WINDOW_SIZE="$window_size" \
        INFILTRATOR_CAPTURE_MARKER="$marker" \
        setsid "$APP" >"$log" 2>&1 &
    fi
    APP_PID=$!
    APP_PGID="$(process_group "$APP_PID")"

    if [ "$APP_PGID" = "$APP_PID" ]; then
      # Window discovery first: bind to the exact spawned PID plus the
      # observed identity (title; app_id kept as an alternative match), so
      # the screenshot can never attach to a leftover or foreign window.
      # windows.json is kept per scenario as receipt.
      for _ in $(seq 1 "$WINDOW_TIMEOUT_ITERS"); do
        if niri_ipc -j windows >"$windows_tmp" 2>"$windows_error" \
          && jq -e --arg app "$APP_ID" --arg title "$APP_TITLE" --arg pid "$APP_PID" \
            'any(.[]; ((.pid | tostring) == $pid) and (.title == $title or .app_id == $app))' \
            "$windows_tmp" >/dev/null 2>&1; then
          mv "$windows_tmp" "$windows_json"
          window_id="$(jq -r --arg app "$APP_ID" --arg title "$APP_TITLE" --arg pid "$APP_PID" \
            '.[] | select(((.pid | tostring) == $pid) and (.title == $title or .app_id == $app)) | .id' \
            "$windows_json" | head -1)"
          window_ready=1
          break
        fi
        kill -0 "$APP_PID" 2>/dev/null || break
        sleep 0.25
      done

      # Readiness: the app writes "CAPTURE_READY page=... skin=..." to the
      # marker file after 60 rendered frames (src/capture.rs). No fixed
      # sleeps.
      for _ in $(seq 1 "$MARKER_TIMEOUT_ITERS"); do
        if grep -q "CAPTURE_READY page=$marker_page skin=$skin" "$marker" 2>/dev/null; then
          marker_ready=1
          break
        fi
        kill -0 "$APP_PID" 2>/dev/null || break
        sleep 0.25
      done
      sleep 0.3 # tiny settle: let the first frame reach the compositor
    fi

    {
      printf 'app_pid=%s\n' "$APP_PID"
      printf 'window_id=%s\n' "$window_id"
      printf 'requested_window=%s\n' "$window_size"
      printf 'wgpu_backend=%s\n' "${wgpu_backend:-default}"
      printf 'command=niri msg action screenshot-window --id %s --write-to-disk true --path %s\n' \
        "$window_id" "$image"
    } >"$action"

    # Re-verify the same window is still alive right before the screenshot,
    # then capture with a small retry budget.
    if [ "$window_ready" -eq 1 ] && [ "$marker_ready" -eq 1 ] \
      && kill -0 "$APP_PID" 2>/dev/null \
      && niri_ipc -j windows >"$windows_tmp" 2>/dev/null \
      && jq -e --arg app "$APP_ID" --arg title "$APP_TITLE" --arg pid "$APP_PID" --arg id "$window_id" \
        'any(.[]; ((.pid | tostring) == $pid) and (.title == $title or .app_id == $app) and ((.id | tostring) == $id))' \
        "$windows_tmp" >/dev/null 2>&1; then
      mv "$windows_tmp" "$windows_json"
      local shot
      for shot in 1 2 3 4 5; do
        rm -f "$image"
        if NIRI_SOCKET="$NIRI_IPC" timeout 8s niri msg action screenshot-window \
          --id "$window_id" --write-to-disk true --path "$image" >>"$action" 2>&1; then
          action_status=ok
          break
        fi
        sleep 0.25
      done
    fi

    if [ "$action_status" = ok ]; then
      # Wait briefly for niri to finish flushing the file, then verify the
      # PNG receipt: readable IHDR, exactly the requested window size or
      # larger (niri's screenshot-window can carry a shadow margin; nested
      # output scale=1 makes logical == physical pixels).
      local wait
      for wait in $(seq 1 20); do
        if [ -s "$image" ]; then
          bytes="$(stat -c%s "$image")"
          [ "$bytes" -gt 5000 ] && break
        fi
        sleep 0.1
      done
      if [ "$bytes" -gt 5000 ] && dims="$(png_dims "$image")"; then
        read -r width height <<<"$dims"
        hash="$(sha256sum "$image" | cut -d' ' -f1)"
        if [ "$width" -lt "$exp_w" ] || [ "$height" -lt "$exp_h" ]; then
          action_status="failed-dims ($width x $height, smaller than requested $window_size)"
        fi
      else
        action_status=failed
      fi
    fi

    case "$action_status" in
    ok) break ;;
    failed-dims*) break ;; # a wrong-size retry will not fix itself
    esac
    # Unlike the iced matrix, a dead app DOES get the second attempt: the
    # first crash is exactly the signature of a failed wgpu device init in
    # this software environment, and attempt 2 switches to WGPU_BACKEND=gl.
    if [ "$attempt" -ge 2 ]; then
      break # the GL fallback already had its chance
    fi
    attempt=$((attempt + 1))
  done

  # Give the failure a precise reason for the manifest and the operator.
  if [ "$action_status" != ok ] && [ "$action_status" != "failed-dims"* ]; then
    if [ "$window_ready" -eq 0 ]; then
      action_status="failed-window (no pid+identity match via niri IPC)"
    elif [ "$marker_ready" -eq 0 ]; then
      action_status="failed-marker (no CAPTURE_READY within ${MARKER_TIMEOUT_ITERS}x0.25s; see app.log — likely wgpu device init)"
    fi
  fi

  if [ "$action_status" = ok ]; then
    install -m 644 "$image" "$OUT_DIR/$name.png"
    printf '  pass  %-22s %sx%s %s B\n' "$name" "$width" "$height" "$bytes"
  else
    printf '  FAIL  %-22s %s (app log: %s)\n' "$name" "$action_status" "$log" >&2
    width=0
    height=0
    bytes=0
    hash="-"
  fi
  grep 'CAPTURE_READY' "$marker" 2>/dev/null | sed "s/^/$name\t/" \
    >>"$RUN_DIR/capture-markers.log" || true
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$name" "$page" "$skin" "$window_size" \
    "$APP_PID" "$window_id" "$width" "$height" "$bytes" "$hash" "$action_status" \
    >>"$MANIFEST"

  terminate_owned "$APP_PID" "$APP_PGID"
  APP_PID=""
  APP_PGID=""
  [ "$action_status" = ok ]
}

: >"$RUN_DIR/capture-markers.log"
FAILURES=0
FAILED_NAMES=""
while IFS=$'\t' read -r name page skin window_size; do
  [ "$name" = name ] && continue
  [ -n "$name" ] || continue
  if ! capture_one "$name" "$page" "$skin" "$window_size"; then
    FAILURES=$((FAILURES + 1))
    FAILED_NAMES="$FAILED_NAMES $name"
  fi
done <"$MATRIX"

# ----------------------------------------------------------------- summary --
install -m 644 "$MANIFEST" "$MANIFEST_PUBLISHED"
ELAPSED=$(( $(date +%s) - START_EPOCH ))
if [ "$FAILURES" -ne 0 ]; then
  printf 'CAPTURE MATRIX FAILED: %d/%d scenario(s) failed:%s\n' \
    "$FAILURES" "$SCENARIO_COUNT" "$FAILED_NAMES" >&2
  printf 'successful shots and the manifest were still published to %s\n' "$OUT_DIR" >&2
  printf 'debug evidence retained at %s\n' "$RUN_DIR" >&2
  exit 1
fi

printf 'CAPTURE MATRIX PASS: %d scenario(s) in %ds -> %s\n' \
  "$SCENARIO_COUNT" "$ELAPSED" "$OUT_DIR"
printf 'manifest: %s (run evidence: %s, binary sha256 %s)\n' \
  "$MANIFEST_PUBLISHED" "$RUN_DIR" "$BINARY_SHA256"
