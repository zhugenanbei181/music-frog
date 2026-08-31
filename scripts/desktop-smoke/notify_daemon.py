#!/usr/bin/env python3
"""desktop-smoke: recording org.freedesktop.Notifications daemon.

Fallback notification daemon for the PRIVATE session bus created by
scripts/desktop-smoke.sh, used when mako/dunst are not installed. It is a
real (headless) implementation of the freedesktop notifications protocol:
notify-rust (the app's backend, crates/infiltrator-iced/src/notify.rs) and
notify-send both talk to it exactly as they would to mako.

Every Notify/CloseNotification call is appended as one JSON line to
--history, which the stage asserts against. Prints NOTIFY_READY when the
bus name is owned.
"""
import argparse
import json
import signal
import sys
import threading
import time

import dbus
import dbus.service
from dbus.mainloop.glib import DBusGMainLoop
from gi.repository import GLib

IFACE = "org.freedesktop.Notifications"
PATH = "/org/freedesktop/Notifications"
SERVER = ("desktop-smoke-recorder", "music-frog desktop-smoke harness", "1.0", "1.2")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--history", required=True, help="JSONL history file")
    parser.add_argument("--timeout", type=int, default=600)
    args = parser.parse_args()

    DBusGMainLoop(set_as_default=True)
    loop = GLib.MainLoop()
    bus = dbus.SessionBus()
    lock = threading.Lock()
    next_id = [0]

    def append(record: dict) -> None:
        with lock:
            with open(args.history, "a") as handle:
                handle.write(json.dumps(record, ensure_ascii=False) + "\n")

    class Daemon(dbus.service.Object):
        @dbus.service.method(IFACE, in_signature="susssasa{sv}i", out_signature="u")
        def Notify(self, app_name, replaces_id, app_icon, summary, body,
                   actions, hints, expire_timeout):
            next_id[0] += 1
            notification_id = next_id[0]
            hints_plain = {}
            for key, value in dict(hints).items():
                value = unwrap(value)
                hints_plain[str(key)] = value if isinstance(value, (int, float, bool, str)) \
                    else str(value)
            append({
                "ts": time.time(),
                "kind": "notify",
                "app_name": str(app_name),
                "summary": str(summary),
                "body": str(body),
                "actions": [str(action) for action in actions],
                "urgency": hints_plain.get("urgency"),
                "hints": hints_plain,
                "id": notification_id,
            })
            return dbus.UInt32(notification_id)

        @dbus.service.method(IFACE, in_signature="u", out_signature="")
        def CloseNotification(self, notification_id):
            append({"ts": time.time(), "kind": "close", "id": int(notification_id)})

        @dbus.service.method(IFACE, in_signature="", out_signature="as")
        def GetCapabilities(self):
            return ["actions", "body", "body-markup", "icon-static", "urgency"]

        @dbus.service.method(IFACE, in_signature="", out_signature="ssss")
        def GetServerInformation(self):
            return SERVER

    def unwrap(value):
        return getattr(value, "value", value)

    daemon = Daemon(bus, PATH)
    bus.request_name(IFACE)
    print("NOTIFY_READY", flush=True)

    def die(_signum, _frame):
        loop.quit()

    signal.signal(signal.SIGTERM, die)
    signal.signal(signal.SIGINT, die)
    GLib.timeout_add_seconds(args.timeout, loop.quit)
    loop.run()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
