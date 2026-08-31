#!/usr/bin/env python3
"""desktop-smoke: minimal SNI StatusNotifierWatcher (harness fallback host).

Runs on the PRIVATE session bus created by dbus-run-session (scripts/
desktop-smoke.sh). It implements the org.kde.StatusNotifierWatcher protocol
so the app's ksni tray (crates/infiltrator-iced/src/tray/ksni_backend.rs)
can register its StatusNotifierItem when no real tray host (waybar etc.)
is installed. This is test-harness infrastructure, not an app mock: the
item, its menu and its state all live in the real application.

Contract:
  * prints WATCHER_READY once the bus name is owned;
  * writes the live item list to --items (JSON) on every change;
  * exits when the bus disconnects or on SIGTERM.

Real hosts (waybar with the tray module) are preferred; see
docs/DESKTOP_SMOKE.md. This fallback exists so the protocol-level
assertions can run on machines without a tray host installed.
"""
import argparse
import json
import signal
import sys

import dbus
import dbus.service
from dbus.mainloop.glib import DBusGMainLoop
from gi.repository import GLib

WATCHER_IFACE = "org.kde.StatusNotifierWatcher"
WATCHER_PATH = "/StatusNotifierWatcher"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--status", required=True,
                        help="marker file written ('ready') once the name is owned")
    parser.add_argument("--items", required=True,
                        help="JSON file kept in sync with registered items")
    parser.add_argument("--timeout", type=int, default=900,
                        help="hard self-termination after N seconds")
    args = parser.parse_args()

    DBusGMainLoop(set_as_default=True)
    loop = GLib.MainLoop()
    bus = dbus.SessionBus()

    state = {"items": []}

    def flush_items() -> None:
        try:
            with open(args.items, "w") as handle:
                json.dump({"items": state["items"]}, handle)
        except OSError as error:  # pragma: no cover - harness diagnostics only
            print(f"watcher: cannot write {args.items}: {error}", file=sys.stderr)

    class Watcher(dbus.service.Object):
        @dbus.service.method(WATCHER_IFACE, in_signature="s", out_signature="")
        def RegisterStatusNotifierItem(self, service):
            service = str(service)
            if service not in state["items"]:
                state["items"].append(service)
                flush_items()
                self.StatusNotifierItemRegistered(service)

        @dbus.service.method(WATCHER_IFACE, in_signature="s", out_signature="")
        def UnregisterStatusNotifierItem(self, service):
            service = str(service)
            if service in state["items"]:
                state["items"].remove(service)
                flush_items()
                self.StatusNotifierItemUnregistered(service)

        @dbus.service.signal(WATCHER_IFACE, signature="s")
        def StatusNotifierItemRegistered(self, service):
            pass

        @dbus.service.signal(WATCHER_IFACE, signature="s")
        def StatusNotifierItemUnregistered(self, service):
            pass

        @dbus.service.signal(WATCHER_IFACE)
        def StatusNotifierHostRegistered(self):
            pass

        # -- org.freedesktop.DBus.Properties ---------------------------------
        @dbus.service.method(dbus.PROPERTIES_IFACE, in_signature="ss",
                             out_signature="v")
        def Get(self, interface, prop):
            return self.GetAll(interface)[prop]

        @dbus.service.method(dbus.PROPERTIES_IFACE, in_signature="s",
                             out_signature="a{sv}")
        def GetAll(self, interface):
            if interface != WATCHER_IFACE:
                raise dbus.exceptions.DBusException(
                    f"org.freedesktop.DBus.Error.UnknownInterface: {interface}")
            return {
                "RegisteredStatusNotifierItems":
                    dbus.Array(state["items"], signature="s"),
                "IsStatusNotifierHostRegistered": dbus.Boolean(True),
                "HasStatusNotifierHostRegistered": dbus.Boolean(True),
                # The harness itself is the (headless) host: sni_check.py
                # enumerates items and reads/activates their menus.
                "ProtocolVersion": dbus.Int32(0),
            }

        @dbus.service.method(dbus.PROPERTIES_IFACE, in_signature="ssv",
                             out_signature="")
        def Set(self, interface, prop, value):  # pragma: no cover
            raise dbus.exceptions.DBusException(
                "org.freedesktop.DBus.Error.PropertyReadOnly")

    watcher = Watcher(bus, WATCHER_PATH)
    bus.request_name(WATCHER_IFACE)
    watcher.StatusNotifierHostRegistered()
    flush_items()
    with open(args.status, "w") as handle:
        handle.write("ready\n")
    print("WATCHER_READY", flush=True)

    # Drop items whose bus owner vanished (app exited without unregistering).
    def on_name_owner_changed(name, _old, new):
        if str(new) == "" and str(name) in state["items"]:
            state["items"].remove(str(name))
            flush_items()
            watcher.StatusNotifierItemUnregistered(str(name))

    bus.add_signal_receiver(
        on_name_owner_changed,
        signal_name="NameOwnerChanged",
        dbus_interface="org.freedesktop.DBus",
    )

    def die(_signum, _frame):
        loop.quit()

    signal.signal(signal.SIGTERM, die)
    signal.signal(signal.SIGINT, die)
    GLib.timeout_add_seconds(args.timeout, loop.quit)
    loop.run()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
