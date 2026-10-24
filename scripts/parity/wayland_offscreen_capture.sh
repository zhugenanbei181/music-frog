#!/usr/bin/env bash
# EFFECTS-050 #197: virtual Wayland (Niri) offscreen capture pipeline.
#
# Spin a headless Niri compositor, run one capture-tools surface build inside
# it, take compositor-level screenshots of both viewports and file them as
# receipts. Everything the pipeline needs comes from the environment; the
# script never talks to the interactive session's compositor.
#
# Required on the host: niri (a wlroots compositor with IPC screenshots),
# WLR_BACKENDS=headless support (wlroots), and the surface's capture build.
#
# Usage:
#   scripts/parity/wayland_offscreen_capture.sh --surface bevy \
#       --run-id <capture-run-id> [--scenario-dir <dir>]
#
# Environment:
#   WAYLAND_OFFSCREEN_DIR   scratch dir for the compositor socket (default:
#                           a fresh dir under /tmp, removed on exit)
#   NIRI_CONFIG             optional niri config overlay; the embedded default
#                           pins one 1180x780 output (the standard viewport)
# Outputs (under $scenario-dir, default .evidence/wayland/<surface>-<run-id>):
#   standard.png            compositor screenshot of the 1180x780 output
#   compact.png             compositor screenshot after the compact resize
#   session.log             compositor + surface stderr/stdout
#   receipt.tsv             the run identity (surface, run id, pids, exit)
set -euo pipefail

surface="" run_id="" scenario_dir=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --surface) surface="$2"; shift 2 ;;
    --run-id) run_id="$2"; shift 2 ;;
    --scenario-dir) scenario_dir="$2"; shift 2 ;;
    *) printf 'unknown argument: %s\n' "$1" >&2; exit 2 ;;
  esac
done
[[ -n "$surface" && -n "$run_id" ]] || { grep '^#' "$0" | sed 's/^# \{0,1\}//' >&2; exit 2; }

command -v niri >/dev/null 2>&1 || { printf 'niri: not installed\n' >&2; exit 3; }
scenario_dir="${scenario_dir:-.evidence/wayland/${surface}-${run_id}}"
mkdir -p "$scenario_dir"

scratch="${WAYLAND_OFFSCREEN_DIR:-$(mktemp -d /tmp/wayland-offscreen.XXXXXX)}"
cleanup() {
  if [[ -n "${compositor_pid:-}" ]]; then
    kill "$compositor_pid" 2>/dev/null || true
  fi
  if [[ "$scratch" == /tmp/wayland-offscreen.* ]]; then
    rm -rf "$scratch"
  fi
}
trap cleanup EXIT

# One fixed 1180x780 output; the capture build's own viewport knobs drive the
# compact pass (INFILTRATOR_CAPTURE_WINDOW=720x480).
config="$scratch/niri-offscreen.kdl"
cat >"$config" <<'KDL'
output "HEADLESS-1" { mode 1180x780; }
input { keyboard { xkb { layout "us" } } }
layout { border { off } } 
prefer-no-csd
session { }
KDL

export WLR_BACKENDS=headless
export WLR_RENDERER=pixman
export XDG_RUNTIME_DIR="$scratch"
mkdir -p "$scratch"
export WAYLAND_DISPLAY="wayland-offscreen-$run_id"

session_log="$scenario_dir/session.log"
: >"$session_log"
niri -c "$config" >>"$session_log" 2>&1 &
compositor_pid=$!

# Wait for the compositor socket (bounded; a headless niri is up in ms).
for _ in $(seq 1 100); do
  [[ -S "$scratch/$WAYLAND_DISPLAY" ]] && break
  sleep 0.05
done
[[ -S "$scratch/$WAYLAND_DISPLAY" ]] || { printf 'niri socket never appeared\n' >&2; exit 4; }

# Surface build under the offscreen compositor, standard viewport first.
INFILTRATOR_CAPTURE_WINDOW=1180x780 \
INFILTRATOR_CAPTURE_OUT="$scenario_dir" \
  "scripts/capture_${surface}_matrix.sh" >>"$session_log" 2>&1 || true

# Compositor-level screenshots: the niri IPC screenshot action writes the
# screen to a file and prints its path.
standard_path="$(niri msg --socket "$scratch/$WAYLAND_DISPLAY" action screenshot-screen 2>>"$session_log" | tr -d '\n')"
[[ -n "$standard_path" && -f "$standard_path" ]] && cp -- "$standard_path" "$scenario_dir/standard.png"

# Compact pass: resize the output, rerun the capture build, screenshot again.
sed -i 's/1180x780/720x480/' "$config"
kill -SIGHUP "$compositor_pid" 2>/dev/null || true
sleep 0.5
INFILTRATOR_CAPTURE_WINDOW=720x480 \
INFILTRATOR_CAPTURE_OUT="$scenario_dir" \
  "scripts/capture_${surface}_matrix.sh" >>"$session_log" 2>&1 || true
compact_path="$(niri msg --socket "$scratch/$WAYLAND_DISPLAY" action screenshot-screen 2>>"$session_log" | tr -d '\n')"
[[ -n "$compact_path" && -f "$compact_path" ]] && cp -- "$compact_path" "$scenario_dir/compact.png"

printf 'surface\trun_id\tcompositor_pid\tstandard\tcompact\n%s\t%s\t%s\t%s\t%s\n' \
  "$surface" "$run_id" "$compositor_pid" \
  "$([[ -f "$scenario_dir/standard.png" ]] && echo ok || echo missing)" \
  "$([[ -f "$scenario_dir/compact.png" ]] && echo ok || echo missing)" \
  >"$scenario_dir/receipt.tsv"

cat "$scenario_dir/receipt.tsv"
