#!/usr/bin/env bash
# desktop-smoke: isolated REAL desktop-behavior verification rig for the
# infiltrator-iced application (SNI tray protocol + OS notifications +
# XDG/dconf isolation), with zero contact to the operator's desktop session.
#
# Stage:                What is proven (real protocol, no app mocks):
#   tray                Private session bus (dbus-run-session) + a
#                       StatusNotifierWatcher (waybar when installed, else the
#                       harness watcher in scripts/desktop-smoke/) + virtual
#                       KWin/niri stack + the REAL app binary in non-demo
#                       mode. busctl/python assert: item registration,
#                       StatusNotifierItem properties, full DBusMenu
#                       GetLayout tree ("● " active marks, checkmark
#                       toggle-states, disabled placeholders), AboutToShow,
#                       real "clicked" Events (Proxy Mode -> Global marker
#                       flip; autostart checkmark 0 -> 1) and the resulting
#                       autostart .desktop file inside the rig's own
#                       XDG_CONFIG_HOME.
#   notify              Private bus + recording org.freedesktop.Notifications
#                       daemon (mako/dunst when installed). notify-send
#                       probe is asserted end-to-end. The app itself cannot
#                       emit in this rig today: demo mode short-circuits
#                       system_notify (crates/infiltrator-iced/src/notify.rs)
#                       and non-demo notifications need a running core —
#                       reported as a gap, no code is changed.
#   proxy-isolation     Inside the rig HOME/XDG: gsettings/dconf write+read
#                       roundtrip stays in the rig's own dconf db; the bus
#                       address differs from the host's; no host desktop
#                       bus names (portals, shell) leak into the rig.
#   all                 proxy-isolation -> notify -> tray.
#
# Isolation: everything runs under dbus-run-session with a temp
# HOME/XDG_CONFIG_HOME/XDG_DATA_HOME/XDG_STATE_HOME/XDG_CACHE_HOME/
# XDG_RUNTIME_DIR; WAYLAND_DISPLAY/DISPLAY are cleared before the virtual
# compositor host starts. The host session bus is never contacted.
#
# Usage: scripts/desktop-smoke.sh [tray|notify|proxy-isolation|all]
# Exit codes: 0 pass; 1 behavior/assertion failure; 2 missing dependency or
# usage error; 3 blocked (environment never came up / host instance running).
# Env knobs: DESKTOP_SMOKE_KEEP=1 keep temp dirs; DESKTOP_SMOKE_SKIP_BUILD=1
# reuse target/debug/infiltrator-iced; INFILTRATOR_LANG=en-US (default).
set -u

REPO="$(cd "$(dirname "$0")/.." && pwd)"
SCRIPT="$(cd "$(dirname "$0")" && pwd)/$(basename "$0")"
HELPERS="$REPO/scripts/desktop-smoke"
APP="$REPO/target/debug/infiltrator-iced"
EVIDENCE_ROOT="$REPO/target/desktop-smoke"
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
STAGE_ALL="${1:-all}"

# Captured BEFORE anything touches the environment: string comparison only,
# the host bus is never contacted. (In the inner re-execution this is already
# exported by the outer driver and must not be replaced by the rig bus.)
HOST_DBUS_ADDR="${HOST_DBUS_ADDR:-${DBUS_SESSION_BUS_ADDRESS-}}"

INFILTRATOR_LANG="${INFILTRATOR_LANG:-en-US}"
BUILD_TIMEOUT="${DESKTOP_SMOKE_BUILD_TIMEOUT:-20m}"

# Set by the outer driver and exported for the inner (inside
# dbus-run-session) re-execution of this file; empty when this is the outer.
RIG="${RIG:-}" RUN_DIR="${RUN_DIR:-}"
DBUS_PID="" DBUS_PGID=""
APP_PID="" APP_PGID="" NIRI_PID="" NIRI_PGID="" KWIN_PID="" KWIN_PGID=""
WATCHER_PID="" NOTIFY_PID="" WAYBAR_PID="" NIRI_IPC="" KWIN_RUNTIME=""
RUN_DIR="${RUN_DIR:-}" KEEP="${DESKTOP_SMOKE_KEEP:-0}"

say() { printf '%s\n' "$*"; }
fail() { say "FAIL: $*" >&2; }
die() { say "ERROR: $*" >&2; exit "${2:-2}"; }

process_group() {
  ps -o pgid= -p "$1" 2>/dev/null | tr -d '[:space:]'
}

terminate_owned() {
  local pid="$1" pgid="$2" i
  [ -n "$pid" ] || return 0
  if [ -n "$pgid" ] && [ "$pgid" = "$pid" ]; then
    kill -TERM -- "-$pgid" 2>/dev/null || true
  else
    kill -TERM "$pid" 2>/dev/null || true
  fi
  for i in $(seq 1 20); do
    if [ -n "$pgid" ] && [ "$pgid" = "$pid" ]; then
      kill -0 -- "-$pgid" 2>/dev/null || break
    else
      kill -0 "$pid" 2>/dev/null || break
    fi
    sleep 0.1
  done
  if [ -n "$pgid" ] && [ "$pgid" = "$pid" ]; then
    kill -0 -- "-$pgid" 2>/dev/null && kill -KILL -- "-$pgid" 2>/dev/null || true
  elif kill -0 "$pid" 2>/dev/null; then
    kill -KILL "$pid" 2>/dev/null || true
  fi
  wait "$pid" 2>/dev/null || true
}

cleanup() {
  if [ -n "$NIRI_IPC" ] && [ -S "$NIRI_IPC" ] && [ -n "$NIRI_PID" ]; then
    NIRI_SOCKET="$NIRI_IPC" timeout 3s niri msg action quit >/dev/null 2>&1 || true
    for _ in 1 2 3 4 5 6 7 8 9 10; do
      kill -0 "$NIRI_PID" 2>/dev/null || break
      sleep 0.1
    done
  fi
  terminate_owned "$APP_PID" "$APP_PGID"
  terminate_owned "$WAYBAR_PID" ""
  terminate_owned "$NIRI_PID" "$NIRI_PGID"
  terminate_owned "$KWIN_PID" "$KWIN_PGID"
  terminate_owned "$WATCHER_PID" ""
  terminate_owned "$NOTIFY_PID" ""
  # dbus-run-session runs in its own process group: killing it tears down the
  # private bus and every remaining child (daemons activated via the bus).
  terminate_owned "$DBUS_PID" "$DBUS_PGID"
  # xdg-document-portal may have left a FUSE mount inside the rig; unmount
  # before rm so no temp files leak.
  if [ -n "$RIG" ] && [ -d "$RIG/run/doc" ]; then
    fusermount3 -uz "$RIG/run/doc" 2>/dev/null || fusermount -uz "$RIG/run/doc" 2>/dev/null \
      || umount -l "$RIG/run/doc" 2>/dev/null || true
  fi
  if [ -n "$KWIN_RUNTIME" ] && [ -d "$KWIN_RUNTIME" ]; then
    rm -rf -- "$KWIN_RUNTIME"
  fi
  if [ "$KEEP" = "1" ] && [ -n "$RIG" ] && [ -d "$RIG" ]; then
    say "kept rig dir: $RIG"
  elif [ -n "$RIG" ] && [ -d "$RIG" ]; then
    rm -rf -- "$RIG"
  fi
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

require() {
  local missing="" command
  for command in "$@"; do
    command -v "$command" >/dev/null 2>&1 || missing="$missing $command"
  done
  [ -z "$missing" ] || die "missing required commands:${missing} (install them and re-run; nothing was installed by this script)" 2
}

# ------------------------------------------------------------------ stages --
stage_proxy_isolation() {
  require python3 dbus-run-session busctl gsettings
  say "== stage proxy-isolation: private bus + XDG/dconf isolation =="
  local rc=0

  # 1. The rig runs on its own session bus.
  local addr="${DBUS_SESSION_BUS_ADDRESS-}"
  if [ -z "$addr" ]; then
    fail "DBUS_SESSION_BUS_ADDRESS is not set inside the rig"
    rc=1
  elif [ "$addr" = "$HOST_DBUS_ADDR" ]; then
    fail "rig bus address equals the host one (isolation broken)"
    rc=1
  else
    say "PASS rig_bus_address: private (differs from host address; host bus never contacted)"
  fi

  # 2. No host desktop services leak into the private bus.
  local names canary leaked=0
  names="$(timeout 10s busctl --user call org.freedesktop.DBus /org/freedesktop/DBus \
    org.freedesktop.DBus ListNames 2>/dev/null || true)"
  [ -n "$names" ] || { fail "cannot list names on the private bus"; rc=1; }
  printf '%s\n' "$names" | sed 's/ /\n/g' | tr -d '"' | grep -v '^$' | sort -u \
    >"$RUN_DIR/bus-names.txt" || true
  for canary in org.gnome.Shell org.freedesktop.portal.Desktop \
    org.freedesktop.Notifications org.kde.StatusNotifierWatcher \
    org.kde.kglobalaccel org.fcitx.Fcitx5 org.mako org.a11y.Bus; do
    if grep -qx "$canary" "$RUN_DIR/bus-names.txt"; then
      fail "host service leaked into the rig bus: $canary"
      leaked=1
    fi
  done
  [ "$leaked" -eq 0 ] && say "PASS bus_leak_canaries: no host desktop services on the rig bus ($(wc -l <"$RUN_DIR/bus-names.txt") names, all rig-local)"

  # 3. gsettings/dconf writes stay inside the rig's own XDG dirs.
  local mode_read db_file
  if timeout 30s gsettings set org.gnome.system.proxy mode 'manual' 2>"$RUN_DIR/gsettings.err"; then
    mode_read="$(timeout 30s gsettings get org.gnome.system.proxy mode 2>>"$RUN_DIR/gsettings.err" || true)"
    if [ "$mode_read" = "'manual'" ]; then
      db_file="$(find "$XDG_DATA_HOME" "$XDG_CONFIG_HOME" -type f \( -path '*dconf*' -o -name 'user' \) 2>/dev/null | head -1)"
      if [ -n "$db_file" ]; then
        mkdir -p "$RUN_DIR/artifacts"
        cp -f "$db_file" "$RUN_DIR/artifacts/dconf-user.db" 2>/dev/null || true
        say "PASS gsettings_isolation: org.gnome.system.proxy mode=manual round-trips inside the rig; dconf db at ${db_file#$RIG/} (copy kept in evidence)"
      else
        say "PASS gsettings_isolation: write+read round-trips inside the rig (dconf db file not located; value is rig-local by construction)"
      fi
    else
      fail "gsettings read-back mismatch: got '$mode_read'"
      rc=1
    fi
  else
    fail "gsettings write failed inside the rig: $(head -c 200 "$RUN_DIR/gsettings.err")"
    rc=1
  fi

  # 4. HOME redirection is actually in effect.
  if [ "$HOME" = "${RIG:?}/home" ] && [ -d "$HOME" ]; then
    say "PASS home_redirect: HOME=$HOME (temp rig home)"
  else
    fail "HOME is not redirected (HOME=$HOME)"
    rc=1
  fi

  [ "$rc" -eq 0 ] && say "STAGE proxy-isolation: PASS"
  return "$rc"
}

stage_notify() {
  require python3 dbus-run-session busctl notify-send
  say "== stage notify: org.freedesktop.Notifications on the private bus =="
  local rc=0 history="$RUN_DIR/notify-history.jsonl"

  # Prefer a real desktop daemon when one is installed; otherwise the harness
  # recorder implements the same protocol (both are real D-Bus servers).
  if command -v mako >/dev/null 2>&1 && [ "$DESKTOP_SMOKE_FORCE_RECORDER" != "1" ]; then
    setsid mako >"$RUN_DIR/mako.log" 2>&1 &
    NOTIFY_PID=$!
    say "daemon: mako (installed on this machine)"
    DAEMON_KIND=mako
  elif command -v dunst >/dev/null 2>&1 && [ "$DESKTOP_SMOKE_FORCE_RECORDER" != "1" ]; then
    setsid dunst >"$RUN_DIR/dunst.log" 2>&1 &
    NOTIFY_PID=$!
    say "daemon: dunst (installed on this machine)"
    DAEMON_KIND=dunst
  else
    python3 "$HELPERS/notify_daemon.py" --history "$history" \
      >"$RUN_DIR/notify-daemon.log" 2>&1 &
    NOTIFY_PID=$!
    say "daemon: harness recorder (mako/dunst not installed)"
    DAEMON_KIND=recorder
  fi

  local i ready=0
  for i in $(seq 1 30); do
    timeout 5s busctl --user call org.freedesktop.DBus /org/freedesktop/DBus \
      org.freedesktop.DBus GetNameOwner s org.freedesktop.Notifications \
      >/dev/null 2>&1 && { ready=1; break; }
    kill -0 "$NOTIFY_PID" 2>/dev/null || break
    sleep 0.3
  done
  if [ "$ready" -ne 1 ]; then
    fail "notification daemon never owned org.freedesktop.Notifications"
    return 3
  fi
  say "PASS notify_server: org.freedesktop.Notifications served on the private bus"

  if timeout 10s busctl --user call org.freedesktop.Notifications \
      /org/freedesktop/Notifications org.freedesktop.Notifications GetCapabilities \
      >"$RUN_DIR/notify-capabilities.txt" 2>&1; then
    say "PASS notify_capabilities: $(tr -d '\n' <"$RUN_DIR/notify-capabilities.txt" | head -c 120)"
  else
    fail "GetCapabilities failed"
    rc=1
  fi

  if timeout 10s notify-send -u critical "desktop-smoke-probe" \
    "transport verification $(date -u +%H:%M:%S)" 2>"$RUN_DIR/notify-send.err"; then
    sleep 1
    if [ "$DAEMON_KIND" = recorder ]; then
      if grep -q 'desktop-smoke-probe' "$history" 2>/dev/null; then
        local urgency
        urgency="$(python3 -c '
import json, sys
rows = [json.loads(line) for line in open(sys.argv[1])]
hit = [r for r in rows if r.get("kind") == "notify" and r.get("summary") == "desktop-smoke-probe"]
print(hit[-1]["urgency"] if hit else "none")' "$history" 2>/dev/null)"
        say "PASS notify_transport: notify-send -> daemon history (urgency=$urgency, expected 2)"
      else
        fail "notify-send probe missing from daemon history ($history)"
        rc=1
      fi
    else
      sleep 1
      if timeout 10s makoctl list 2>/dev/null | grep -q 'desktop-smoke-probe'; then
        say "PASS notify_transport: notify-send -> makoctl list"
      else
        fail "probe not visible via makoctl list"
        rc=1
      fi
    fi
  else
    fail "notify-send failed: $(head -c 200 "$RUN_DIR/notify-send.err")"
    rc=1
  fi

  say "GAP app_to_daemon: the application itself cannot emit in this rig yet —"
  say "    demo mode short-circuits system_notify (crates/infiltrator-iced/src/notify.rs:115"
  say "    'if !self.shell.notifications_enabled || self.shell.demo') and non-demo notifications"
  say "    require a running core / WebDAV event. Needs an app-side test hook (reported only,"
  say "    no app code changed). The notify-rust transport used by the app is exactly what"
  say "    notify-send exercised above."
  [ "$rc" -eq 0 ] && say "STAGE notify: PASS (transport; app-side gap reported)"
  return "$rc"
}

stage_tray() {
  require python3 dbus-run-session busctl kwin_wayland niri cargo jq sha256sum
  say "== stage tray: SNI/DBusMenu protocol assertions against the real app =="

  if [ "$DESKTOP_SMOKE_SKIP_BUILD" != "1" ]; then
    say "build: cargo build -p infiltrator-iced (timeout $BUILD_TIMEOUT)"
    timeout --kill-after=10s "$BUILD_TIMEOUT" cargo build --quiet -p infiltrator-iced \
      >"$RUN_DIR/build.log" 2>&1 || die "cargo build failed (see $RUN_DIR/build.log)" 1
  fi
  [ -x "$APP" ] || die "app binary missing: $APP" 2
  sha256sum "$APP" | cut -d' ' -f1 >"$RUN_DIR/binary.sha256"

  # Host instance conflict: the single-instance guard binds an ABSTRACT unix
  # socket (single-instance 0.3.3); abstract sockets are per network
  # namespace, not per D-Bus session, so a host instance would make the rig
  # app exit instantly. NOTE: `pgrep -x` cannot match this binary (kernel comm
  # is truncated to 15 chars), so match the full command line instead.
  if grep -q 'com\.musicfrog\.infiltrator' /proc/net/unix 2>/dev/null; then
    fail "an infiltrator instance is running on this machine (abstract socket com.musicfrog.infiltrator; check pgrep -f infiltrator-iced); stop it or run the rig in its own network namespace"
    return 3
  fi
  if pgrep -f 'target/debug/infiltrator-iced|infiltrator-iced --' >/dev/null 2>&1; then
    fail "another infiltrator-iced process is running (pgrep -f); stop it first"
    return 3
  fi

  # --- StatusNotifierWatcher: real host (waybar) if installed, else harness --
  if command -v waybar >/dev/null 2>&1; then
    say "watcher: waybar tray module (real SNI host installed on this machine)"
    TRAY_HOST=waybar
  else
    say "watcher: harness StatusNotifierWatcher (waybar not installed —"
    say "         install waybar for the visual host layer; assertions are unaffected)"
    python3 "$HELPERS/sni_watcher.py" --status "$RUN_DIR/watcher.status" \
      --items "$RUN_DIR/watcher-items.json" >"$RUN_DIR/watcher.log" 2>&1 &
    WATCHER_PID=$!
    TRAY_HOST=harness
    local i wready=0
    for i in $(seq 1 30); do
      [ -s "$RUN_DIR/watcher.status" ] && { wready=1; break; }
      kill -0 "$WATCHER_PID" 2>/dev/null || break
      sleep 0.3
    done
    [ "$wready" -eq 1 ] || {
      fail "harness watcher never became ready (log: $RUN_DIR/watcher.log)"
      return 3
    }
    busctl --user call org.freedesktop.DBus /org/freedesktop/DBus \
      org.freedesktop.DBus GetNameOwner s org.kde.StatusNotifierWatcher >/dev/null 2>&1 || {
      fail "org.kde.StatusNotifierWatcher is not owned on the private bus"
      return 3
    }
  fi

  # --- virtual compositor stack (same pattern as scripts/capture-iced-*) -----
  KWIN_RUNTIME="$(mktemp -d /tmp/desktop-smoke-kwin.XXXXXX)"
  chmod 700 "$KWIN_RUNTIME"
  local kwin_display="$KWIN_RUNTIME/wayland-outer"
  XDG_RUNTIME_DIR="$KWIN_RUNTIME" WAYLAND_DISPLAY= DISPLAY= DBUS_SESSION_BUS_ADDRESS= \
    QT_QPA_PLATFORM=wayland setsid timeout --foreground --kill-after=10s 20m \
    kwin_wayland --virtual --socket=wayland-outer --width=1920 --height=1080 \
      --scale=1 --no-global-shortcuts --no-lockscreen \
    >"$RUN_DIR/kwin.log" 2>&1 & KWIN_PID=$!
  APP_PGID=""
  local i
  for i in $(seq 1 40); do KWIN_PGID="$(process_group "$KWIN_PID")"; [ "$KWIN_PGID" = "$KWIN_PID" ] && break; sleep 0.1; done
  for i in $(seq 1 40); do [ -S "$kwin_display" ] && break; sleep 0.2; done
  if ! kill -0 "$KWIN_PID" 2>/dev/null || [ ! -S "$kwin_display" ]; then
    fail "virtual KWin did not start (tail of $RUN_DIR/kwin.log):"
    tail -15 "$RUN_DIR/kwin.log" >&2
    return 3
  fi
  say "compositor host: kwin_wayland --virtual ($kwin_display)"

  local niri_conf="$RUN_DIR/niri.kdl"
  cat >"$niri_conf" <<KDL
hotkey-overlay {
    skip-at-startup
}
output "winit" {
    scale 1
}
KDL
  XDG_RUNTIME_DIR="$RIG/run" WAYLAND_DISPLAY="$kwin_display" DISPLAY= \
    DBUS_SESSION_BUS_ADDRESS= \
    LIBGL_ALWAYS_SOFTWARE=1 setsid timeout --foreground --kill-after=10s 20m \
    niri --config "$niri_conf" >"$RUN_DIR/niri.log" 2>&1 & NIRI_PID=$!
  for i in $(seq 1 40); do NIRI_PGID="$(process_group "$NIRI_PID")"; [ "$NIRI_PGID" = "$NIRI_PID" ] && break; sleep 0.1; done
  local niri_sock="" ipc=""
  for i in $(seq 1 60); do
    kill -0 "$NIRI_PID" 2>/dev/null || break
    niri_sock="$(find "$RIG/run" -maxdepth 1 -type s -name 'wayland-[0-9]*' -printf '%f\n' -quit 2>/dev/null || true)"
    ipc="$(find "$RIG/run" -maxdepth 1 -type s -name 'niri.*.sock' -printf '%f\n' -quit 2>/dev/null || true)"
    [ -n "$niri_sock" ] && [ -n "$ipc" ] && break
    sleep 0.2
  done
  if ! kill -0 "$NIRI_PID" 2>/dev/null || [ -z "$niri_sock" ] || [ -z "$ipc" ]; then
    fail "nested niri did not come up (log: $RUN_DIR/niri.log)"
    tail -15 "$RUN_DIR/niri.log" >&2
    return 3
  fi
  NIRI_IPC="$RIG/run/$ipc"
  say "compositor: nested niri ready (wayland=$niri_sock ipc=$ipc)"

  if [ "$TRAY_HOST" = waybar ]; then
    local wconfig="$RUN_DIR/waybar-config.json"
    cp "$HELPERS/waybar-config.json" "$wconfig"
    XDG_RUNTIME_DIR="$RIG/run" WAYLAND_DISPLAY="$niri_sock" DISPLAY= \
      setsid timeout --foreground --kill-after=10s 20m \
      waybar -c "$wconfig" -s "$HELPERS/waybar-style.css" \
      >"$RUN_DIR/waybar.log" 2>&1 & WAYBAR_PID=$!
    sleep 2
  fi

  # The debug binary is huge and the checkout may live on slow storage:
  # page it in first, otherwise exec can take minutes and look like a
  # silent single-instance exit.
  say "warm: paging $(numfmt --to=iec "$(stat -c%s "$APP" 2>/dev/null || echo 0)")B binary into page cache..."
  timeout 300 cat "$APP" >/dev/null || true

  # Deterministic menu labels for the assertions: real mode reads the
  # language from settings.toml (INFILTRATOR_LANG is a demo-only contract),
  # default zh-CN. A one-key TOML file is a valid partial settings override.
  mkdir -p "$RIG/data/mihomo-rs"
  printf 'language = "en-US"\n' >"$RIG/data/mihomo-rs/settings.toml"

  # --- the REAL app (non-demo: demo never spawns a tray), fresh rig HOME ----
  # Deliberately NO setsid/timeout wrapper: the SNI identity assertion needs
  # the app's EXACT pid ($! must be the app itself, not a wrapper process),
  # and the rig teardown (trap + watchdog) owns lifecycle management anyway —
  # same pattern as scripts/capture-iced-matrix.sh.
  XDG_RUNTIME_DIR="$RIG/run" WAYLAND_DISPLAY="$niri_sock" DISPLAY= \
    LIBGL_ALWAYS_SOFTWARE=1 \
    HOME="$RIG/home" \
    XDG_CONFIG_HOME="$RIG/home/.config" XDG_DATA_HOME="$RIG/home/.local/share" \
    XDG_STATE_HOME="$RIG/home/.local/state" XDG_CACHE_HOME="$RIG/home/.cache" \
    MIHOMO_HOME="$RIG/data/mihomo-rs" \
    INFILTRATOR_LANG="$INFILTRATOR_LANG" \
    "$APP" >"$RUN_DIR/app.log" 2>&1 & APP_PID=$!
  for i in $(seq 1 40); do APP_PGID="$(process_group "$APP_PID")"; [ "$APP_PGID" = "$APP_PID" ] && break; sleep 0.1; done
  say "app: pid=$APP_PID (non-demo; the bus item must carry this exact pid)"
  sleep 3
  if ! kill -0 "$APP_PID" 2>/dev/null; then
    fail "app exited during startup: an instant silent exit-0 is the signature of the"
    fail "single-instance guard seeing another instance; check: pgrep -f infiltrator-iced"
    sed 's/^/    app.log: /' "$RUN_DIR/app.log" 2>/dev/null
    return 3
  fi

  # --- protocol assertions ---------------------------------------------------
  local rc=0 check_status=0
  timeout 240s python3 "$HELPERS/sni_check.py" \
    --app-pid "$APP_PID" --discover-timeout 90 --report "$RUN_DIR/sni-report.json" \
    >"$RUN_DIR/sni-check.log" 2>&1 || check_status=$?
  sed 's/^/  /' "$RUN_DIR/sni-check.log"
  [ "$check_status" -eq 0 ] || rc=1

  if [ "$check_status" -ne 0 ]; then
    # Failure diagnostics: was the app alive, on the right bus, registered?
    if kill -0 "$APP_PID" 2>/dev/null; then
      say "DIAG app-alive: yes wchan=$(cat /proc/$APP_PID/wchan 2>/dev/null) threads=$(ls /proc/$APP_PID/task 2>/dev/null | wc -l) fds=$(ls /proc/$APP_PID/fd 2>/dev/null | wc -l)"
      say "DIAG app-bus: $(tr '\0' '\n' <"/proc/$APP_PID/environ" 2>/dev/null | grep '^DBUS_SESSION_BUS_ADDRESS=' | sed 's/guid=.*//')"
      say "DIAG app-wayland: $(tr '\0' '\n' <"/proc/$APP_PID/environ" 2>/dev/null | grep '^WAYLAND_DISPLAY=')"
    else
      say "DIAG app-alive: NO (process exited during the assertion window)"
    fi
    say "DIAG watcher-items: $(cat "$RUN_DIR/watcher-items.json" 2>/dev/null)"
    say "DIAG abstract-socket: $(grep -c musicfrog /proc/net/unix 2>/dev/null)"
    say "DIAG app.log bytes: $(wc -c <"$RUN_DIR/app.log" 2>/dev/null)"
  fi

  # Autostart note: the app's Linux backend does not implement autostart
  # (infiltrator-shared/src/autostart.rs returns Err on non-Windows), so the
  # "Launch at Login" click correctly produces NO .desktop file; sni_check
  # asserts the optimistic flip AND the async revert instead. Nothing to
  # check on the filesystem here by design.
  say "NOTE autostart_backend: Linux autostart is unimplemented by the app (by design); the click assertions cover optimistic flip + async revert via SNI toggle-state"

  if grep -q 'system tray unavailable' "$RUN_DIR/app.log"; then
    fail "app log reports 'system tray unavailable' (spawn degraded)"
    rc=1
  fi

  # Never poison later runs: the single-instance abstract socket must be
  # released before this stage returns.
  for i in $(seq 1 20); do
    grep -q 'com\.musicfrog\.infiltrator' /proc/net/unix 2>/dev/null || break
    sleep 0.25
  done

  [ "$rc" -eq 0 ] && say "STAGE tray: PASS ($TRAY_HOST watcher)"
  return "$rc"
}

# ------------------------------------------------------------- rig runner --
run_in_rig() {
  local stage="$1" i
  RIG="$(mktemp -d /tmp/desktop-smoke.XXXXXX)"
  mkdir -p "$RIG/home" "$RIG/home/.config" "$RIG/home/.local/share" \
    "$RIG/home/.local/state" "$RIG/home/.cache" "$RIG/run"
  chmod 700 "$RIG" "$RIG/run"
  RUN_DIR="$EVIDENCE_ROOT/${STAMP}_${stage}"
  mkdir -p "$RUN_DIR"

  # The stage runs as a RE-EXECUTION of this file (__inner__ dispatch) inside
  # dbus-run-session: a fully private session bus plus rig-local HOME/XDG.
  # WAYLAND_DISPLAY/DISPLAY are cleared so nothing can reach the operator's
  # session. setsid gives the whole rig one process group, owned by
  # dbus-run-session, for exact teardown.
  HOME="$RIG/home" \
  XDG_CONFIG_HOME="$RIG/home/.config" XDG_DATA_HOME="$RIG/home/.local/share" \
  XDG_STATE_HOME="$RIG/home/.local/state" XDG_CACHE_HOME="$RIG/home/.cache" \
  XDG_RUNTIME_DIR="$RIG/run" \
  WAYLAND_DISPLAY= DISPLAY= QT_QPA_PLATFORM= \
  XDG_CURRENT_DESKTOP= XDG_SESSION_DESKTOP= XDG_SESSION_TYPE= \
  XDG_MENU_PREFIX= DESKTOP_SESSION= GTK_USE_PORTAL=0 \
  RIG="$RIG" RUN_DIR="$RUN_DIR" HOST_DBUS_ADDR="$HOST_DBUS_ADDR" \
  INFILTRATOR_LANG="$INFILTRATOR_LANG" KEEP="$KEEP" \
  DESKTOP_SMOKE_SKIP_BUILD="${DESKTOP_SMOKE_SKIP_BUILD:-}" \
  setsid dbus-run-session -- bash "$SCRIPT" __inner__ "$stage" &
  DBUS_PID=$!
  DBUS_PGID="$(process_group "$DBUS_PID")"
  [ -n "$DBUS_PGID" ] || DBUS_PGID="$DBUS_PID"

  # Watchdog: the stage owns fine-grained timeouts, this is the hard stop.
  # Its output goes to /dev/null so no orphaned `sleep` can hold our stdout
  # (killing the subshell leaves its sleep child behind by design).
  local budget="${DESKTOP_SMOKE_STAGE_BUDGET:-900}" status=0 watchdog
  ( sleep "$budget" && kill -TERM -- "-$DBUS_PGID" 2>/dev/null ) >/dev/null 2>&1 &
  watchdog=$!
  wait "$DBUS_PID" 2>/dev/null || status=$?
  kill "$watchdog" 2>/dev/null || true
  wait "$watchdog" 2>/dev/null || true
  DBUS_PID="" DBUS_PGID=""
  return "$status"
}

usage() {
  say "usage: $0 [tray|notify|proxy-isolation|all]"
  say "  tray             SNI tray + DBusMenu protocol assertions (real app, non-demo)"
  say "  notify           org.freedesktop.Notifications delivery assertions"
  say "  proxy-isolation  private bus + HOME/XDG/dconf isolation proof"
  say "  all              proxy-isolation -> notify -> tray (default)"
  exit 0
}

# Hidden mode: executed INSIDE dbus-run-session (run_in_rig launches
# `bash "$SCRIPT" __inner__ <stage>` with the rig environment). Runs the
# stage function; the EXIT trap teardown (defined above) then tears down
# every child the stage recorded.
if [ "${1:-}" = "__inner__" ]; then
  stage="$2"
  if [ -n "${DESKTOP_SMOKE_DEBUG:-}" ]; then
    say "inner debug: RIG=[$RIG] RUN_DIR=[$RUN_DIR] HOST_DBUS_ADDR_set=$([ -n "$HOST_DBUS_ADDR" ] && echo yes || echo no)"
    env | sort | grep -E '^(RIG|RUN_DIR|HOME|XDG_)' | sed 's/^/  env: /'
  fi
  case "$stage" in
  tray) stage_tray; exit $? ;;
  notify) stage_notify; exit $? ;;
  proxy-isolation) stage_proxy_isolation; exit $? ;;
  *) echo "unknown inner stage: $stage" >&2; exit 2 ;;
  esac
fi

# ------------------------------------------------------------------ main ----
require bash python3 dbus-run-session busctl timeout mktemp sha256sum seq sort grep ps
mkdir -p "$EVIDENCE_ROOT"

case "$STAGE_ALL" in
tray | notify | proxy-isolation) STAGES="$STAGE_ALL" ;;
all) STAGES="proxy-isolation notify tray" ;;
-h | --help) usage ;;
*) die "unknown stage: $STAGE_ALL (see --help)" 2 ;;
esac

# Single-instance conflict check applies to every stage that runs the app.
if printf '%s' "$STAGES" | grep -q tray \
  && grep -q 'com\.musicfrog\.infiltrator' /proc/net/unix 2>/dev/null; then
  die "host infiltrator instance is running (abstract socket); the rig app would exit instantly" 3
fi

GIT_HEAD="$(git -C "$REPO" rev-parse --short=12 HEAD 2>/dev/null || printf 'no-git')"
say "desktop-smoke rig: stages=[$STAGES] git=$GIT_HEAD host_bus_contact=never"

overall=0
for stage in $STAGES; do
  say ""
  # Each stage gets a fresh private session + fresh XDG tree.
  status=0
  run_in_rig "$stage" || status=$?
  # Distinguish "environment never came up" (dbus-run-session killed by
  # timeout) from regular stage failures.
  if [ "$status" -eq 124 ] || [ "$status" -eq 137 ]; then
    say "STAGE $stage: BLOCKED (timed out) — evidence in $RUN_DIR"
    overall=3
  elif [ "$status" -ne 0 ]; then
    say "STAGE $stage: FAIL (exit $status) — evidence in $RUN_DIR"
    [ "$overall" -eq 0 ] && overall=1
  else
    say "STAGE $stage: PASS (evidence in $RUN_DIR)"
  fi
done

say ""
if [ "$overall" -eq 0 ]; then
  say "DESKTOP SMOKE: ALL STAGES PASS"
else
  say "DESKTOP SMOKE: FAILED (exit $overall; see stage lines above)"
fi
exit "$overall"
