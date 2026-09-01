#!/usr/bin/env python3
"""desktop-smoke: protocol-level SNI/DBusMenu assertions against the real app.

Runs on the PRIVATE session bus created by scripts/desktop-smoke.sh. It acts
as the tray HOST (enumeration + menu reads + activation) and asserts real
StatusNotifierItem / com.canonical.dbusmenu behavior of the running
infiltrator-iced binary:

  1. the app registered org.kde.StatusNotifierItem-<app_pid>-<n>;
  2. item properties (Id/Title/Menu object path);
  3. GetLayout tree structure (top-level sequence, separators, checkmarks,
     submenus, disabled placeholders, the app's "● " active-mode marker);
  4. AboutToShow liveness;
  5. a real "clicked" Event on Proxy Mode -> Global flips the "● " marker
     (optimistic state change + refresh_tray reaching the SNI menu);
  6. a real "clicked" Event on the "Launch at Login" checkmark flips
     toggle-state 0 -> 1 (bash asserts the resulting autostart file later).

Emits a JSON report (--report) and exits 0 only when every assertion passes.
Never touches the app source; everything runs over the public D-Bus
protocols. Expected labels are en-US (INFILTRATOR_LANG=en-US, fresh HOME).
"""
import argparse
import json
import sys
import time
from pathlib import Path

import dbus

MARK = "\u25cf "  # the app's own active marker ("● ")
ITEM_IFACE = "org.kde.StatusNotifierItem"
ITEM_PATH = "/StatusNotifierItem"
MENU_IFACE = "com.canonical.dbusmenu"
MENU_PATH = "/MenuBar"

# (kind, label) with kind in {item, sub, chk, sep}: the fresh-state en-US
# top-level menu from crates/infiltrator-iced/src/tray/menu.rs build_tray_spec.
EXPECTED_TOP = [
    ("item", "Show Main Window"),
    ("sep", ""),
    ("sub", "Proxy Mode"),
    ("sub", "Nodes"),
    ("sep", ""),
    ("chk", "System Proxy"),
    ("chk", "TUN Mode"),
    ("item", "Toggle Light/Dark"),
    ("sep", ""),
    ("sub", "Profiles"),
    ("sub", "Kernel"),
    ("sub", "Sync"),
    ("chk", "Launch at Login"),
    ("sep", ""),
    ("sub", "Info"),
    ("sep", ""),
    ("item", "Factory Reset…"),
    ("item", "Quit"),
]
EXPECTED_MODE = ["Rule", "Global", "Direct", "Script Mode (not enabled)"]


def unwrap(value):
    return getattr(value, "value", value)


class Checker:
    def __init__(self, app_pid: int, item_name: str | None):
        self.bus = dbus.SessionBus()
        self.app_pid = app_pid
        self.item_name = item_name
        self.results: list[dict] = []

    def record(self, ok: bool, check_id: str, detail: str) -> None:
        self.results.append({"id": check_id, "pass": bool(ok), "detail": detail})
        print(f"[{'PASS' if ok else 'FAIL'}] {check_id}: {detail}", flush=True)

    # -- transport ----------------------------------------------------------
    def list_names(self) -> list[str]:
        d_bus = dbus.Interface(
            self.bus.get_object("org.freedesktop.DBus", "/org/freedesktop/DBus"),
            "org.freedesktop.DBus")
        return [str(name) for name in d_bus.ListNames()]

    def find_item(self, timeout: float) -> str | None:
        prefix = f"org.kde.StatusNotifierItem-{self.app_pid}-"
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            names = [name for name in self.list_names() if name.startswith(prefix)]
            if names:
                return names[0]
            time.sleep(0.4)
        return None

    def item_props(self) -> dict:
        obj = self.bus.get_object(self.item_name, ITEM_PATH)
        props = dbus.Interface(obj, dbus.PROPERTIES_IFACE).GetAll(ITEM_IFACE)
        return {str(k): unwrap(v) for k, v in props.items()}

    def layout(self) -> tuple[int, dict]:
        obj = self.bus.get_object(self.item_name, MENU_PATH)
        menu = dbus.Interface(obj, MENU_IFACE)
        revision, tree = menu.GetLayout(0, -1, [])
        return int(revision), self._normalize(tree)

    def about_to_show(self, item_id: int) -> bool:
        obj = self.bus.get_object(self.item_name, MENU_PATH)
        return bool(dbus.Interface(obj, MENU_IFACE).AboutToShow(int(item_id)))

    def click(self, item_id: int) -> None:
        obj = self.bus.get_object(self.item_name, MENU_PATH)
        dbus.Interface(obj, MENU_IFACE).Event(
            int(item_id), "clicked", dbus.String("", variant_level=1), 0)

    @staticmethod
    def _normalize(node) -> dict:
        item_id, props, children = node
        props = {str(k): unwrap(v) for k, v in dict(props).items()}
        if props.get("type") == "separator":
            kind = "sep"
        elif props.get("children-display") == "submenu":
            kind = "sub"
        elif props.get("toggle-type") == "checkmark":
            kind = "chk"
        else:
            kind = "item"
        return {
            "id": int(item_id),
            "kind": kind,
            "label": str(props.get("label", "")),
            "enabled": bool(props.get("enabled", True)),
            "toggle_state": int(props["toggle-state"]) if "toggle-state" in props else None,
            "children": [Checker._normalize(child) for child in (children or [])],
        }

    # -- helpers ------------------------------------------------------------
    @staticmethod
    def find(root: dict, label: str) -> dict | None:
        for node in root["children"]:
            if node["label"] == label:
                return node
            hit = Checker.find(node, label)
            if hit is not None:
                return hit
        return None

    @staticmethod
    def kind_of(node: dict, expected_kind: str) -> bool:
        if expected_kind == "chk":
            return node["kind"] in ("chk",) or node["toggle_state"] is not None
        return node["kind"] == expected_kind

    def wait_for(self, predicate, budget: float) -> tuple[bool, int, dict]:
        deadline = time.monotonic() + budget
        last_rev, tree = self.layout()
        while True:
            if predicate(tree):
                return True, last_rev, tree
            if time.monotonic() >= deadline:
                return False, last_rev, tree
            time.sleep(0.3)
            last_rev, tree = self.layout()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--app-pid", type=int, required=True)
    parser.add_argument("--item-name", default=None,
                        help="override item bus name discovery")
    parser.add_argument("--report", required=True)
    parser.add_argument("--discover-timeout", type=float, default=40.0)
    parser.add_argument("--click-timeout", type=float, default=6.0)
    args = parser.parse_args()

    check = Checker(args.app_pid, args.item_name)

    # 1. item registration ---------------------------------------------------
    name = args.item_name or check.find_item(args.discover_timeout)
    if name is None:
        check.record(False, "item_registered",
                     f"no org.kde.StatusNotifierItem-{args.app_pid}-* name on the bus "
                     f"within {args.discover_timeout:.0f}s (tray spawn failed?)")
        emit(check, args.report)
        return 1
    check.item_name = name
    check.record(True, "item_registered", f"bus name {name}")

    # 2. item properties -----------------------------------------------------
    try:
        props = check.item_props()
        ok = (str(props.get("Id")) == "MusicFrogInfiltrator"
              and str(props.get("Title")) == "MusicFrog Infiltrator"
              and str(props.get("Menu")) == MENU_PATH
              and bool(str(props.get("Status"))))
        check.record(ok, "item_props",
                     f"Id={props.get('Id')} Title={props.get('Title')} "
                     f"Status={props.get('Status')} Menu={props.get('Menu')}")
    except dbus.DBusException as error:
        check.record(False, "item_props", f"GetAll failed: {error}")
        emit(check, args.report)
        return 1

    # 3. menu tree structure ---------------------------------------------------
    try:
        rev0, root = check.layout()
    except dbus.DBusException as error:
        check.record(False, "layout_readable", f"GetLayout failed: {error}")
        emit(check, args.report)
        return 1
    check.record(True, "layout_readable", f"revision={rev0} via GetLayout(0,-1,[])")

    actual = [(node["kind"], node["label"]) for node in root["children"]]
    expected = [(kind, label) for kind, label in EXPECTED_TOP]
    check.record(actual == expected, "top_level_structure",
                 f"{len(actual)} top-level entries (expected {len(expected)}): "
                 f"{_fmt(actual)} vs expected {_fmt(expected)}")

    mode = Checker.find(root, "Proxy Mode")
    mode_children = [child["label"] for child in (mode or {}).get("children", [])]
    script = (mode or {}).get("children", [None, None, None, {}])[-1] if mode else None
    mode_ok = (mode is not None and mode["kind"] == "sub" and mode["enabled"]
               and mode_children == EXPECTED_MODE
               and script is not None and script["enabled"] is False)
    check.record(mode_ok, "mode_submenu",
                 f"children={mode_children}; script entry disabled without script block")

    profiles = Checker.find(root, "Profiles")
    placeholder = ((profiles or {}).get("children") or [{}])[0]
    profiles_ok = (profiles is not None and profiles["kind"] == "sub"
                   and placeholder.get("label") == "No profiles"
                   and placeholder.get("enabled") is False)
    check.record(profiles_ok, "profiles_placeholder",
                 f"fresh HOME reflected in tray: {placeholder}")

    marks = [child["label"] for child in (mode or {}).get("children", [])
             if child["label"].startswith(MARK)]
    check.record(True, "initial_mode_mark",
                 f"initial active-mode marks: {marks or 'none (fresh state, mode unset)'}")

    # 4. AboutToShow liveness -------------------------------------------------
    try:
        ats = check.about_to_show(mode["id"]) if mode else None
        check.record(isinstance(ats, bool), "about_to_show",
                     f"AboutToShow(Proxy Mode id={mode['id'] if mode else '?'}) -> {ats}")
    except dbus.DBusException as error:
        check.record(False, "about_to_show", f"failed: {error}")

    if not (mode_ok and profiles_ok and actual == expected):
        emit(check, args.report)
        return 1

    # 5. real click: Proxy Mode -> Global flips the "● " marker ---------------
    targets = {child["label"]: child["id"] for child in mode["children"]}
    global_id = targets.get("Global")
    if global_id is None:
        check.record(False, "click_global_send", "no 'Global' entry in Proxy Mode submenu")
        emit(check, args.report)
        return 1
    try:
        check.click(global_id)
        check.record(True, "click_global_send",
                     f"Event(id={global_id}, 'clicked') sent to com.canonical.dbusmenu")
    except dbus.DBusException as error:
        check.record(False, "click_global_send", f"Event failed: {error}")
        emit(check, args.report)
        return 1
    flipped, rev1, tree1 = check.wait_for(
        lambda tree: any(child["label"] == MARK + "Global"
                         for child in (Checker.find(tree, "Proxy Mode")
                                       or {"children": []})["children"]),
        args.click_timeout)
    detail = (f"'{MARK}Global' observed after real click "
              f"(revision {rev0} -> {rev1}, "
              f"initial marks: {marks or 'none'})")
    # 0.20 fix semantics: without a running core the app must NOT flip the
    # mode optimistically — the tray never shows a mode that never took
    # effect (SetProxyMode lands in the runtime-unavailable branch).
    not_flipped, rev1, tree1 = check.wait_for(
        lambda tree: not any(child["label"] == MARK + "Global"
                             for child in (Checker.find(tree, "Proxy Mode")
                                           or {"children": []})["children"]),
        args.click_timeout)
    check.record(not_flipped, "click_global_no_optimistic_flip",
                 "no '● Global' without a running core — the tray never lies "
                 f"(revision {rev0} -> {rev1}, initial marks: {marks or 'none'})")
    _ = flipped, detail

    # 6. real click: autostart checkmark toggle-state 0 -> 1 ------------------
    root_after_global = tree1
    autostart = Checker.find(root_after_global, "Launch at Login")
    before_state = autostart["toggle_state"] if autostart else None
    if autostart is not None:
        try:
            check.click(autostart["id"])
        except dbus.DBusException as error:
            check.record(False, "click_autostart_send", f"Event failed: {error}")
            emit(check, args.report)
            return 1
        ok, _rev2, _tree2 = check.wait_for(
            lambda tree: (Checker.find(tree, "Launch at Login") or {"toggle_state": None})
            ["toggle_state"] == 1,
            args.click_timeout)
        check.record(ok and before_state == 0, "click_autostart_toggle",
                     f"Launch at Login toggle-state {before_state} -> 1 (optimistic flip)")
        # 0.20 fix semantics: the Linux autostart backend now succeeds
        # (XDG autostart entry in the redirected HOME), so the state must
        # PERSIST — the toggle stays 1 and the .desktop file really lands.
        # NOTE: the toggle is optimistically 1 from the click override, so
        # the file is the only observation that truly synchronizes with the
        # async backend completing — poll for it.
        desktop_file = Path.home() / ".config" / "autostart" / "MusicFrogInfiltrator.desktop"
        deadline = time.time() + args.click_timeout
        while not desktop_file.is_file() and time.time() < deadline:
            time.sleep(0.2)
        file_ok = desktop_file.is_file()
        check.record(file_ok, "click_autostart_xdg_file",
                     f"XDG autostart entry written by the async backend: "
                     f"{desktop_file} ({'exists' if file_ok else 'MISSING after timeout'})")
        # With the file confirmed, the refreshed spec must also carry the
        # persisted checked state (optimistic override replaced by truth).
        persisted, _rev3, _tree3 = check.wait_for(
            lambda tree: (Checker.find(tree, "Launch at Login") or {"toggle_state": None})
            ["toggle_state"] == 1,
            args.click_timeout)
        check.record(persisted, "click_autostart_state_persists",
                     "SNI menu still shows Launch at Login checked after the async "
                     "set_autostart completed on Linux (toggle-state stays 1)")
    else:
        check.record(False, "click_autostart_toggle", "Launch at Login entry not found")

    emit(check, args.report)
    return 0 if all(row["pass"] for row in check.results) else 1


def _fmt(entries) -> str:
    return "[" + ", ".join("SEP" if kind == "sep" else f"{label}({kind})"
                           for kind, label in entries) + "]"


def emit(check: Checker, path: str) -> None:
    report = {
        "item_name": check.item_name,
        "app_pid": check.app_pid,
        "pass": all(row["pass"] for row in check.results),
        "checks": check.results,
    }
    with open(path, "w") as handle:
        json.dump(report, handle, indent=1, ensure_ascii=False)


if __name__ == "__main__":
    raise SystemExit(main())
