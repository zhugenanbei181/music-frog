#!/usr/bin/env python3
"""Protect the Android product manifest's security-relevant declarations.

The Android host is the only place where VPN, foreground-service and boot
permissions are declared. Losing one of these declarations silently breaks the
product (or weakens it) without any Rust compiler or unit test noticing. This
guard parses `android/app/src/main/AndroidManifest.xml` and requires the
permission, service and receiver surface the product depends on.

This checks declarations only; runtime behaviour, platform grants and store
review require independent device/packaging evidence.

Usage:
    python3 scripts/quality/android-manifest-guard.py [--mode report|enforce]
    python3 scripts/quality/android-manifest-guard.py --manifest <path>

Exit status is 0 when every required declaration is present (or in report
mode), 1 when any is missing in enforce mode.
"""
from __future__ import annotations

import argparse
from pathlib import Path
import sys
import xml.etree.ElementTree as ET

ANDROID_NS = "http://schemas.android.com/apk/res/android"
ANDROID = f"{{{ANDROID_NS}}}"

DEFAULT_MANIFEST = "android/app/src/main/AndroidManifest.xml"

REQUIRED_PERMISSIONS = (
    "android.permission.INTERNET",
    "android.permission.ACCESS_NETWORK_STATE",
    "android.permission.FOREGROUND_SERVICE",
    "android.permission.POST_NOTIFICATIONS",
    "android.permission.RECEIVE_BOOT_COMPLETED",
    "android.permission.FOREGROUND_SERVICE_SYSTEM_EXEMPTED",
)

# service name -> (required android:permission, requires foregroundServiceType, intent-filter action)
REQUIRED_SERVICES = {
    ".MihomoVpnService": (
        "android.permission.BIND_VPN_SERVICE",
        True,
        "android.net.VpnService",
    ),
    ".InfiltratorTileService": (
        "android.permission.BIND_QUICK_SETTINGS_TILE",
        False,
        "android.service.quicksettings.action.QS_TILE",
    ),
}

# receiver name -> required intent-filter action
REQUIRED_RECEIVERS = {
    ".BootReceiver": "android.intent.action.BOOT_COMPLETED",
}


def _name(element: ET.Element) -> str:
    return element.get(f"{ANDROID}name", "")


def _actions(element: ET.Element) -> set[str]:
    return {_name(action) for action in element.iter("action")}


def scan(manifest: Path) -> dict[str, list[str]]:
    """Return missing declarations grouped as permission/service/receiver."""
    missing: dict[str, list[str]] = {"permission": [], "service": [], "receiver": []}
    root = ET.parse(manifest).getroot()

    declared_permissions = {
        _name(element) for element in root.findall("uses-permission")
    }
    for permission in REQUIRED_PERMISSIONS:
        if permission not in declared_permissions:
            missing["permission"].append(f"uses-permission {permission}")

    services = {
        _name(element): element for element in root.iter("service")
    }
    for name, (permission, needs_type, action) in REQUIRED_SERVICES.items():
        service = services.get(name)
        if service is None:
            missing["service"].append(f"service {name} is not declared")
            continue
        if service.get(f"{ANDROID}permission") != permission:
            missing["service"].append(
                f"service {name} must declare android:permission={permission}"
            )
        if needs_type and not (service.get(f"{ANDROID}foregroundServiceType") or "").strip():
            missing["service"].append(
                f"service {name} must declare a non-empty android:foregroundServiceType"
            )
        if action not in _actions(service):
            missing["service"].append(
                f"service {name} must declare intent-filter action {action}"
            )

    receivers = {
        _name(element): element for element in root.iter("receiver")
    }
    for name, action in REQUIRED_RECEIVERS.items():
        receiver = receivers.get(name)
        if receiver is None:
            missing["receiver"].append(f"receiver {name} is not declared")
        elif action not in _actions(receiver):
            missing["receiver"].append(
                f"receiver {name} must declare intent-filter action {action}"
            )

    return missing


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("report", "enforce"), default="enforce")
    parser.add_argument(
        "--manifest",
        type=Path,
        default=Path(__file__).resolve().parents[2] / DEFAULT_MANIFEST,
    )
    arguments = parser.parse_args()

    missing = scan(arguments.manifest)
    total = sum(len(items) for items in missing.values())
    if total:
        for group in ("permission", "service", "receiver"):
            for item in missing[group]:
                print(f"MISSING {group}: {item}", file=sys.stderr)
        print(f"Android manifest guard: {total} missing declarations", file=sys.stderr)
        return 1 if arguments.mode == "enforce" else 0
    print("Android manifest guard: 0 missing declarations")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
