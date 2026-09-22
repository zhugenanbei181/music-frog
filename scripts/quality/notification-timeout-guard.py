#!/usr/bin/env python3
"""Fail-closed guard: OS notifications must always have a bounded lifetime.

Regression context: the desktop `SystemNotifier` sent subscription
notifications with `notify-send -u critical` and no `-t`, and the Iced
notify-rust backend never set a timeout. The freedesktop `critical` urgency is
defined as "the user must acknowledge this", so KDE Plasma pinned those
notifications on screen forever. Every notification must now carry an explicit
bounded expiry, must never use the persistent urgency, and the Linux path must
be able to force-close the notification it created.
"""

from __future__ import annotations

import argparse
import pathlib
import sys


ROOT = pathlib.Path(__file__).resolve().parents[2]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def require(violations: list[str], path: str, *markers: str) -> None:
    text = read(path)
    compact_text = " ".join(text.split())
    for marker in markers:
        compact_marker = " ".join(marker.split())
        rustfmt_marker = compact_marker.replace(" }", ", }")
        if (
            marker not in text
            and compact_marker not in compact_text
            and rustfmt_marker not in compact_text
        ):
            violations.append(f"{path} missing {marker!r}")


def forbid(violations: list[str], path: str, *markers: str) -> None:
    text = read(path)
    for marker in markers:
        if marker in text:
            violations.append(f"{path} still contains persistent notification {marker!r}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--mode", choices=["report", "enforce"], default="enforce")
    args = parser.parse_args()
    violations: list[str] = []

    require(
        violations,
        "crates/infiltrator-desktop/src/notify.rs",
        "display_timeout_ms",
        "--print-id",
        "spawn_linux_notification_closer",
        "CloseNotification",
        "test_notification_display_time_is_always_bounded",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/notify.rs",
        "Timeout::Milliseconds",
        "Urgency::Normal",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/notify_tests.rs",
        "daemon_urgency_never_persists_and_timeout_is_bounded",
    )
    # The persistent urgency hints must never come back.
    forbid(violations, "crates/infiltrator-desktop/src/notify.rs", 'Self::Error => "critical"')
    forbid(violations, "crates/infiltrator-iced/src/notify.rs", "notify_rust::Urgency::Critical")
    forbid(violations, "crates/infiltrator-iced/src/notify.rs", "Timeout::Never")
    require(
        violations,
        "scripts/test.sh",
        "notification-timeout-guard.py --mode enforce",
    )
    require(
        violations,
        "scripts/test-bevy.sh",
        'notification-timeout-guard.py" --mode enforce',
    )

    if violations:
        for violation in violations:
            print(f"notification-timeout-guard: {violation}", file=sys.stderr)
        print(
            f"notification-timeout-guard: violations={len(violations)}",
            file=sys.stderr,
        )
        return 1 if args.mode == "enforce" else 0
    print("notification-timeout-guard: bounded display markers=complete violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
