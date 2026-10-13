#!/usr/bin/env python3
"""Keep Bevy desktop product composition outside the UI crate."""

from __future__ import annotations

import argparse
from pathlib import Path
import sys
import tomllib


ROOT = Path(__file__).resolve().parents[2]
UI_DIR = Path("crates/infiltrator-bevy-ui")
UI_MANIFEST = UI_DIR / "Cargo.toml"
HOST_DIR = Path("crates/infiltrator-bevy-desktop-host")
HOST_MANIFEST = HOST_DIR / "Cargo.toml"
RETIRED_UI_HOST_SYMBOLS = (
    "DesktopProductSession",
    "run_with_application_surface_pump",
    "ApplicationSurfaceSource",
    "SurfaceDrainPlugin",
)


def named_dependencies(value: object, name: str) -> bool:
    if isinstance(value, dict):
        return name in value or any(named_dependencies(item, name) for item in value.values())
    if isinstance(value, list):
        return any(named_dependencies(item, name) for item in value)
    return False


def manifest_problems(root: Path) -> list[str]:
    problems: list[str] = []
    ui_path = root / UI_MANIFEST
    host_path = root / HOST_MANIFEST
    try:
        ui = tomllib.loads(ui_path.read_text(encoding="utf-8"))
        host = tomllib.loads(host_path.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as error:
        return [f"Bevy host boundary manifests cannot be read: {error}"]

    ui_features = ui.get("features", {})
    if "desktop-host" in ui_features:
        problems.append(f"{UI_MANIFEST}: retired `desktop-host` feature remains")
    if named_dependencies(ui, "infiltrator-desktop"):
        problems.append(f"{UI_MANIFEST}: UI package depends on the desktop host adapter")

    host_dependencies = host.get("dependencies", {})
    for name in ("infiltrator-bevy-ui", "infiltrator-desktop"):
        if name not in host_dependencies:
            problems.append(f"{HOST_MANIFEST}: host dependency `{name}` is required")
    bins = host.get("bin", [])
    if not any(isinstance(item, dict) and item.get("name") == "infiltrator-bevy-ui" for item in bins):
        problems.append(f"{HOST_MANIFEST}: desktop executable must retain `infiltrator-bevy-ui` identity")
    return problems


def source_problems(root: Path) -> list[str]:
    problems: list[str] = []
    ui_src = root / UI_DIR / "src"
    if (ui_src / "main.rs").exists():
        problems.append(f"{UI_DIR}/src/main.rs: product entrypoint belongs to the host crate")
    if (ui_src / "launch.rs").exists():
        problems.append(f"{UI_DIR}/src/launch.rs: desktop product launcher is retired")

    for source in sorted(ui_src.rglob("*.rs")):
        text = source.read_text(encoding="utf-8")
        for symbol in RETIRED_UI_HOST_SYMBOLS:
            if symbol in text:
                problems.append(f"{source.relative_to(root)}: retired host symbol `{symbol}` remains")

    host_source = root / HOST_DIR / "src" / "desktop.rs"
    try:
        source = host_source.read_text(encoding="utf-8")
    except OSError as error:
        return problems + [f"{HOST_DIR}/src/desktop.rs: cannot read product composition: {error}"]
    for symbol in ("DesktopProductSession", "run_with_host_surface_and_command_sink"):
        if symbol not in source:
            problems.append(f"{host_source.relative_to(root)}: product composition must use `{symbol}`")
    return problems


def self_test() -> int:
    ui = tomllib.loads('[features]\ndefault = []\n[dependencies]\ninfiltrator-contract = "*"\n')
    host = tomllib.loads('[dependencies]\ninfiltrator-bevy-ui = "*"\ninfiltrator-desktop = "*"\n[[bin]]\nname = "infiltrator-bevy-ui"\n')
    if named_dependencies(ui, "infiltrator-desktop") or "desktop-host" in ui["features"]:
        print("UI isolation allow-case self-test failed", file=sys.stderr)
        return 1
    if not named_dependencies(host, "infiltrator-desktop"):
        print("host dependency parser self-test failed", file=sys.stderr)
        return 1
    if not any(item.get("name") == "infiltrator-bevy-ui" for item in host["bin"]):
        print("host product identity self-test failed", file=sys.stderr)
        return 1
    print("Bevy desktop host boundary guard self-test passed")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        return self_test()

    problems = manifest_problems(ROOT) + source_problems(ROOT)
    if problems:
        print("\n".join(problems), file=sys.stderr)
        return 1
    print("Bevy desktop composition is isolated from the UI crate.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
