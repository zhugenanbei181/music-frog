# DESKTOP_SMOKE — isolated real desktop-behavior verification rig

`scripts/desktop-smoke.sh` verifies the **real** desktop-facing behavior of the
iced application — the SNI tray (StatusNotifierItem + com.canonical.dbusmenu),
OS notifications (org.freedesktop.Notifications) and XDG/dconf side effects —
**without ever touching the operator's desktop session**, without mocking any
application code, and without installing anything.

Run evidence lands in `target/desktop-smoke/<UTC-stamp>_<stage>/` (report JSON,
logs, bus name lists, dconf db copies). Temp rig trees (`/tmp/desktop-smoke.*`)
are removed on exit unless `DESKTOP_SMOKE_KEEP=1`.

```
Usage: scripts/desktop-smoke.sh [tray|notify|proxy-isolation|all]   # default all
Exit codes: 0 pass · 1 behavior/assertion failure · 2 missing dependency/usage
            · 3 blocked (environment never came up / instance conflict)
Env knobs:  DESKTOP_SMOKE_KEEP=1 keep rig dirs · DESKTOP_SMOKE_SKIP_BUILD=1
            reuse target/debug/infiltrator-iced · DESKTOP_SMOKE_STAGE_BUDGET=900
```

## How it works

Every stage runs as a re-execution of the script **inside `dbus-run-session`**:
the session bus is fully private, and the rig gets its own
`HOME`, `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `XDG_STATE_HOME`, `XDG_CACHE_HOME`,
`XDG_RUNTIME_DIR`; `WAYLAND_DISPLAY`/`DISPLAY` are cleared, host desktop
identity env (`XDG_CURRENT_DESKTOP`, `DESKTOP_SESSION`, …) is neutralised and
`GTK_USE_PORTAL=0`. The host session bus is never contacted (the host address
is only read as a string for an inequality check).

```
setsid dbus-run-session ─┬─ sni_watcher.py        (org.kde.StatusNotifierWatcher, tray stage)
                         ├─ kwin_wayland --virtual (own XDG_RUNTIME_DIR, no D-Bus)
                         │   └─ niri               (nested; D-Bus cleared)
                         │       └─ infiltrator-iced  ← REAL binary, non-demo, fresh HOME
                         ├─ sni_check.py           (host role: read + click the real menu)
                         └─ notify_daemon.py       (org.freedesktop.Notifications, notify stage)
```

* **tray** — the app runs in **non-demo mode** on purpose: `demo::run` never
  spawns a tray (see `crates/infiltrator-iced/src/lib.rs`). Fresh rig HOME, and
  `settings.toml` seeded with `language = "en-US"` (real mode reads the language
  from settings; `INFILTRATOR_LANG` is a demo-only contract). With no
  StatusNotifierWatcher on the bus, ksni refuses to spawn
  (`tray/ksni_backend.rs` degrades to window-only), so the harness provides the
  Watcher: `scripts/desktop-smoke/sni_watcher.py`. If **waybar** is installed
  the script uses it instead (real visual host, `scripts/desktop-smoke/
  waybar-config.json`); assertions are identical either way because
  `sni_check.py` talks to the Watcher/item protocols, not to waybar.
* Protocol assertions (`sni_check.py`): item name
  `org.kde.StatusNotifierItem-<exact app pid>-<n>`; item properties
  (`Id=MusicFrogInfiltrator`, `Title`, `Menu=/MenuBar`); full `GetLayout` tree —
  18 top-level entries in exact order, 5 separators, submenu children, disabled
  placeholders (`No profiles` proves the fresh HOME is real), absence of the
  `● ` active-mode mark; `AboutToShow`; a real `Event(id,"clicked")` on
  **Proxy Mode → Global** must produce the `● Global` label (optimistic flip +
  `refresh_tray`, `tray.rs` `handle_tray_event`); a real click on
  **Launch at Login** must flip `toggle-state 0 → 1` and — because the Linux
  autostart backend is unimplemented (`infiltrator-shared/src/autostart.rs`
  returns `Err` off-Windows) — revert to 0 when the async handler completes,
  proving the whole SNI → iced event loop → state → `refresh_tray` → SNI
  round trip.
* **notify** — `mako`/`dunst` when installed, else
  `scripts/desktop-smoke/notify_daemon.py`, a real headless
  org.freedesktop.Notifications implementation recording every call as JSONL.
  Asserts name ownership, `GetCapabilities`, and a `notify-send -u critical`
  probe landing in the history with urgency 2 — the exact transport the app's
  notify-rust backend uses.
* **proxy-isolation** — asserts the rig bus address differs from the host's,
  no host desktop names (portals, shell, watcher, notifications) leak into the
  private bus (full name list kept as evidence), a gsettings
  `org.gnome.system.proxy mode=manual` write/read round-trips inside the rig
  (dconf db file copied into evidence), and `HOME` is redirected.

## Isolation boundary matrix

| OS verb | Fully isolatable here? | How / why |
|---|---|---|
| SNI tray + DBusMenu | **yes** | private bus + Watcher (waybar or harness); app runs real ksni |
| DBusMenu click semantics | **yes** | real `Event`/`AboutToShow`/`GetLayout` calls on the app's menu |
| OS notification *delivery* | **yes (transport)** | mako/dunst or harness daemon on the private bus |
| OS notification *visuals/Do-Not-Disturb* | no | needs a real desktop shell; use a VM/CI runner with a real DE |
| App → notification daemon emit | **gap** | demo short-circuits `system_notify` (`src/notify.rs:115`); non-demo needs a running core/WebDAV event — needs an app-side test hook |
| gsettings / dconf writes | **yes** | dconf-service auto-starts on the private bus; db stays in rig XDG (evidence copy) |
| Autostart (.desktop file class) | partial | isolated filesystem is trivially provable, but the app ships no Linux autostart backend at all (Windows-only); rig asserts the SNI-visible flip + revert |
| Single-instance guard | **no** | `single-instance` binds an *abstract unix socket*, which is machine-wide (not per-bus, not per-XDG); the rig detects the conflict (`/proc/net/unix`) and refuses; full isolation needs `unshare -n` (own netns) or a VM |
| TUN mode | no | needs kernel device + root; use a VM |
| Windows / macOS tray & notifications | no | different OS APIs (`muda`/Win32, `NSStatusItem`); use native CI runners |
| Kernel (mihomo) lifecycle, real proxying | no (by design) | rig intentionally runs no kernel; use the runtime page / integration tests |

## Dependencies

Always required: `bash, python3 (dbus + gi), dbus-run-session, busctl, timeout,
mktemp, sha256sum, git`. Tray additionally: `cargo, kwin_wayland, niri` (same
stack as `scripts/capture-iced-matrix.sh`). Nothing is installed by the script;
missing pieces are reported with exit code 2.

Optional, not installed on this machine (as of 2026-08-31): `waybar`
(visual SNI host — `pacman -S waybar`; the harness Watcher substitutes for
protocol assertions), `mako` or `dunst` (visual notification daemon —
`pacman -S mako`; the harness recorder substitutes for transport assertions).

## Operational notes (learned the hard way, encoded in the script)

* **Surviving app instances poison later runs**: the single-instance guard
  makes a second app exit *silently with code 0*; an instant empty-log exit is
  its signature. `pgrep -x` cannot match this binary (kernel comm truncates to
  15 chars) — use `pgrep -f infiltrator-iced` and `/proc/net/unix`. The rig
  refuses to start and verifies socket release on exit.
* The app is launched **without** `setsid`/`timeout` wrappers on purpose:
  `setsid timeout` interposes a wrapper process, and `$!` is then not the app
  pid — which silently breaks the pid-bound SNI identity assertion.
* The 790 MB debug binary on slow storage can take minutes to exec; the stage
  pages it into the cache first (`cat "$APP" >/dev/null`).
* `xdg-document-portal` may FUSE-mount `$XDG_RUNTIME_DIR/doc` inside the rig;
  teardown unmounts (`fusermount -uz`) before `rm -rf` so /tmp stays clean.
* Portal daemons may auto-start on the rig bus and fail (no display) — harmless
  noise; they die with the private bus.
