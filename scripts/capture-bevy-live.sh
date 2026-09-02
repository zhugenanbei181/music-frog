#!/usr/bin/env bash
# Live-core capture (BEVY-005 acceptance): screenshot the Bevy Overview page
# fed by a REAL mihomo core, not the demo fixture.
#
# Pipeline:
#   1. ensure the pinned core binary exists at target/bevy-live/mihomo —
#     download `mihomo-linux-amd64-v1.19.18.gz` from the official
#     MetaCubeX/mihomo release (the same pinned version fetch-mihomo.sh
#     vendors), gunzip + chmod; target/ is git-ignored, nothing lands in
#     the tree;
#   2. generate a minimal config (external-controller 127.0.0.1:9099,
#     mixed-port 7899, mode rule, log-level silent) and start the core in
#     its own process group with a private XDG_RUNTIME_DIR (it can never
#     touch the operator's session or $HOME);
#   3. poll /version until the controller is ready, then re-run the
#     capture-bevy-matrix machinery for the `overview-live` scenario with
#     INFILTRATOR_BEVY_CONTROLLER exported — the app boots its live pump
#     (src/controller.rs) instead of the demo fixture, so the banner shows
#     the core's real version and the chips show the core's real numbers
#     (an idle core reports ~0 B/s rates: honest zeros, not a failure);
#   4. merge the overview-live row into docs/screenshots/bevy/manifest.tsv
#     (append: the demo rows of the last matrix run are preserved);
#   5. tear the core down via its process group (terminate_owned, the same
#     discipline as capture-iced / capture-bevy-matrix).
#
# Usage:
#   bash scripts/capture-bevy-live.sh
#   MIHOMO_LIVE_ASSET=mihomo-linux-arm64-v1.19.18.gz bash scripts/capture-bevy-live.sh
#
# Exit codes: 0 pass; 1 capture failed; 2 preflight error.
set -euo pipefail

REPO="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO"

LIVE_DIR="$REPO/target/bevy-live"
CORE_BIN="$LIVE_DIR/mihomo"
CONFIG="$LIVE_DIR/config.yaml"
CORE_HOME="$LIVE_DIR/run"
CORE_RUNTIME="$(mktemp -d /tmp/infiltrator-bevy-core.XXXXXX)"
CONTROLLER="127.0.0.1:9099"
MIXED_PORT="7899"
# Pinned in lockstep with scripts/fetch-mihomo.sh.
PINNED_VERSION="v1.19.18"
ASSET="${MIHOMO_LIVE_ASSET:-mihomo-linux-amd64-${PINNED_VERSION}.gz}"
BASE_URL="${MIHOMO_BASE_URL:-https://github.com/MetaCubeX/mihomo/releases/download}"
VERSION_URL="http://$CONTROLLER/version"
OUT_DIR="$REPO/docs/screenshots/bevy"
MANIFEST="$OUT_DIR/manifest.tsv"
READY_ITERS=60 # 60 * 0.25s = 15s controller budget

CORE_PID=""
CORE_PGID=""

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

cleanup() {
  terminate_owned "$CORE_PID" "$CORE_PGID"
  if [ -d "$CORE_RUNTIME" ]; then
    rm -rf -- "$CORE_RUNTIME"
  fi
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

for command in curl gunzip sha256sum; do
  command -v "$command" >/dev/null 2>&1 || {
    printf 'required live-capture command is unavailable: %s\n' "$command" >&2
    exit 2
  }
done
# The asset name ends up in a URL; keep it to a safe charset.
[[ "$ASSET" =~ ^[A-Za-z0-9._-]+$ ]] || {
  printf 'unsafe MIHOMO_LIVE_ASSET: %s\n' "$ASSET" >&2
  exit 2
}
[[ "$ASSET" == *"$PINNED_VERSION"* ]] || {
  printf 'refusing to capture a live receipt against a non-pinned core (%s != %s)\n' \
    "$ASSET" "$PINNED_VERSION" >&2
  exit 2
}
if [ -n "$(ss -ltn "( sport = :${CONTROLLER##*:} or sport = :$MIXED_PORT )" 2>/dev/null | tail -n +2)" ]; then
  printf 'refusing to start: controller port %s or proxy port %s is already in use\n' \
    "$CONTROLLER" "$MIXED_PORT" >&2
  exit 2
fi

mkdir -p "$LIVE_DIR" "$CORE_HOME"

# ------------------------------------------------------- 1. the core binary --
if [ ! -x "$CORE_BIN" ]; then
  printf 'live-core: downloading %s (pinned %s)\n' "$ASSET" "$PINNED_VERSION"
  curl -fL --retry 3 -o "$LIVE_DIR/$ASSET" "$BASE_URL/$PINNED_VERSION/$ASSET"
  gunzip -c "$LIVE_DIR/$ASSET" >"$CORE_BIN"
  rm -f "$LIVE_DIR/$ASSET"
  chmod +x "$CORE_BIN"
  [ -s "$CORE_BIN" ] || {
    printf 'live-core: extracted core is empty\n' >&2
    exit 2
  }
fi
CORE_SHA256="$(sha256sum "$CORE_BIN" | cut -d' ' -f1)"
printf 'live-core: %s (sha256 %s)\n' "$CORE_BIN" "$CORE_SHA256"

# -------------------------------------------------------- 2. config + start --
cat >"$CONFIG" <<YAML
# Minimal live-capture core (scripts/capture-bevy-live.sh): a private
# controller for the Bevy frontend's real data pump. No proxies, no DNS:
# the Overview page reads /version, /connections, /memory, /configs only.
mixed-port: $MIXED_PORT
external-controller: $CONTROLLER
mode: rule
log-level: silent
YAML

CORE_LOG="$LIVE_DIR/mihomo.log"
XDG_RUNTIME_DIR="$CORE_RUNTIME" HOME="$CORE_HOME" \
  setsid "$CORE_BIN" -d "$CORE_HOME" -f "$CONFIG" >"$CORE_LOG" 2>&1 &
CORE_PID=$!
CORE_PGID="$(process_group "$CORE_PID")"
[ "$CORE_PGID" = "$CORE_PID" ] || {
  printf 'BLOCKED (live-core): mihomo did not obtain a private process group\n' >&2
  exit 1
}

READY=0
VERSION_JSON=""
for _ in $(seq 1 "$READY_ITERS"); do
  if ! kill -0 "$CORE_PID" 2>/dev/null; then
    printf 'BLOCKED (live-core): mihomo exited during startup; tail of %s:\n' "$CORE_LOG" >&2
    tail -20 "$CORE_LOG" >&2
    exit 1
  fi
  if VERSION_JSON="$(curl -fsS --max-time 2 "$VERSION_URL" 2>/dev/null)"; then
    READY=1
    break
  fi
  sleep 0.25
done
[ "$READY" -eq 1 ] || {
  printf 'BLOCKED (live-core): controller never answered %s; tail of %s:\n' "$VERSION_URL" "$CORE_LOG" >&2
  tail -20 "$CORE_LOG" >&2
  exit 1
}
printf 'live-core: controller ready %s\n' "$VERSION_JSON"
printf 'live_core_asset=%s\nlive_core_sha256=%s\nlive_core_version=%s\ncontroller=%s\n' \
  "$ASSET" "$CORE_SHA256" "$VERSION_JSON" "$CONTROLLER" >"$LIVE_DIR/receipt.txt"

# --------------------------------------------------- 3. the matrix scenario --
# The scenario row comes from scripts/capture_bevy_scenarios.tsv; the env
# knob switches the frontend from the demo fixture to the live pump
# (src/controller.rs reads it in the windowed launcher). Everything else —
# build, virtual compositor stack, PID/title-bound screenshot, manifest —
# is the standard capture-bevy-matrix machinery.
# The matrix republishes manifest.tsv for exactly the scenarios it ran, so
# the previous manifest is preserved here for the append-merge afterwards.
MANIFEST_PREVIOUS="$LIVE_DIR/manifest.previous.tsv"
if [ -f "$MANIFEST" ]; then
  cp "$MANIFEST" "$MANIFEST_PREVIOUS"
fi
export INFILTRATOR_BEVY_CONTROLLER="http://$CONTROLLER"
export INFILTRATOR_CAPTURE_SCENARIOS="overview-live"
if ! bash "$REPO/scripts/capture-bevy-matrix.sh"; then
  printf 'live capture FAILED: see the matrix output above\n' >&2
  exit 1
fi

# ------------------------------------------- 4. append the row to the manifest --
# Merge instead of clobber: the previous manifest's rows (the demo
# scenarios) survive, the fresh overview-live row replaces any older one.
LIVE_ROW="$(awk -F '\t' '$1 == "overview-live" { print }' "$MANIFEST")"
[ -n "$LIVE_ROW" ] || {
  printf 'manifest merge failed: no overview-live row found\n' >&2
  exit 1
}
if [ -f "$MANIFEST_PREVIOUS" ]; then
  HEADER="$(head -n1 "$MANIFEST_PREVIOUS")"
  { printf '%s\n' "$HEADER"
    awk -F '\t' '$1 != "overview-live" && $1 != "scenario" && $1 != "" { print }' "$MANIFEST_PREVIOUS"
    printf '%s\n' "$LIVE_ROW"
  } >"$MANIFEST.merged"
else
  awk -F '\t' -v live="$LIVE_ROW" '
    NR == 1 { print; next }
    $1 == "overview-live" { print live; next }
    $1 != "" { print }
  ' live="$LIVE_ROW" "$MANIFEST" >"$MANIFEST.merged"
fi
install -m 644 "$MANIFEST.merged" "$MANIFEST"
rm -f "$MANIFEST.merged" "$MANIFEST_PREVIOUS"

printf 'LIVE CAPTURE PASS: %s/overview-live.png\n' "$OUT_DIR"
printf 'manifest rows:\n'
cat "$MANIFEST"
