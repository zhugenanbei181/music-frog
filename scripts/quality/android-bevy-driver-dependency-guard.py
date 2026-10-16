#!/usr/bin/env python3
"""Keep the Android Bevy driver out of runtime and capture composition.

This is a fast manifest/lock guard, not a `cargo tree` replacement: it must run
before any toolchain install or registry cache exists, so it reads the committed
driver lockfile instead of resolving the graph. The lockfile is a superset of the
Android target closure, which keeps the forbidden-package check fail-closed.
"""

from __future__ import annotations

import argparse
from pathlib import Path
import re
import sys
import tempfile
import tomllib


ROOT = Path(__file__).resolve().parents[2]
DRIVER_MANIFEST = Path("android/bevy-host/driver/Cargo.toml")
DRIVER_LOCK = Path("android/bevy-host/driver/Cargo.lock")
UI_PACKAGE = "infiltrator-bevy-ui"
UI_MANIFEST = Path("crates/infiltrator-bevy-ui/Cargo.toml")
DRIVER_SOURCE = Path("android/bevy-host/driver/src/lib.rs")
FORBIDDEN_PACKAGES = {
    "infiltrator-application",
    "infiltrator-android",
    "infiltrator-composition",
    "mihomo-api",
    "infiltrator-platform",
}
FORBIDDEN_DRIVER_SYMBOLS = {
    "run_demo",
    "DemoOverviewSource",
    "DemoSurfaceSource",
    "DemoCommandSink",
    "MihomoOverviewSource",
    "ControllerConfig",
    "capture_composition",
    "CapturePlugin",
    "InteractionCapture",
    "interaction_capture",
}


def locked_package_names(root: Path) -> set[str]:
    lock = tomllib.loads((root / DRIVER_LOCK).read_text(encoding="utf-8"))
    return {package["name"] for package in lock["package"]}


def dependency_problems(names: set[str]) -> list[str]:
    return [
        f"Android Bevy driver resolved graph contains forbidden package `{name}`"
        for name in sorted(names & FORBIDDEN_PACKAGES)
    ]


def default_feature_problems(features: dict[str, object]) -> list[str]:
    defaults = features.get("default")
    if not isinstance(defaults, list):
        return [f"{UI_MANIFEST}: `[features].default` must be an explicit list"]
    problems = [
        f"{UI_MANIFEST}: capture-only feature `{name}` must not be a product default"
        for name in ("capture-tools", "capture-controller", "fixture-demo")
        if name in defaults
    ]
    if "ui-application-projections" in features:
        problems.append(
            f"{UI_MANIFEST}: retired feature `ui-application-projections` must be removed"
        )
    return problems


def without_rust_comments(source: str) -> str:
    source = re.sub(r"/\*.*?\*/", "", source, flags=re.DOTALL)
    return re.sub(r"//[^\n]*", "", source)


def driver_source_problems(source: str) -> list[str]:
    code = without_rust_comments(source)
    problems = []
    if "run_with_host_surface_and_command_sink" not in code:
        problems.append(
            f"{DRIVER_SOURCE}: product entry must inject the owner-backed host surface and command sink"
        )
    for symbol in sorted(FORBIDDEN_DRIVER_SYMBOLS):
        if re.search(rf"\b{re.escape(symbol)}\b", code):
            problems.append(
                f"{DRIVER_SOURCE}: product entry references capture/controller-only symbol `{symbol}`"
            )
    return problems


def manifest_problems(root: Path) -> list[str]:
    path = root / DRIVER_MANIFEST
    try:
        manifest = tomllib.loads(path.read_text(encoding="utf-8"))
        dependency = manifest["dependencies"][UI_PACKAGE]
    except (OSError, KeyError, tomllib.TOMLDecodeError) as error:
        return [f"{path}: cannot read driver UI dependency: {error}"]
    problems = []
    if dependency.get("default-features") is not False:
        problems.append(f"{path}: UI dependency must set `default-features = false`")
    if dependency.get("features", []) not in (None, []):
        problems.append(
            f"{path}: UI dependency must not enable features in the Android product driver"
        )
    return problems


def ui_default_manifest_problems(root: Path) -> list[str]:
    path = root / UI_MANIFEST
    try:
        manifest = tomllib.loads(path.read_text(encoding="utf-8"))
        features = manifest["features"]
    except (OSError, KeyError, tomllib.TOMLDecodeError) as error:
        return [f"{path}: cannot read UI feature manifest: {error}"]
    return default_feature_problems(features)


def driver_source_manifest_problems(root: Path) -> list[str]:
    path = root / DRIVER_SOURCE
    try:
        source = path.read_text(encoding="utf-8")
    except OSError as error:
        return [f"{path}: cannot read Android product entry: {error}"]
    return driver_source_problems(source)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        return self_test()

    problems = manifest_problems(ROOT)
    problems.extend(ui_default_manifest_problems(ROOT))
    problems.extend(driver_source_manifest_problems(ROOT))
    try:
        names = locked_package_names(ROOT)
    except (OSError, KeyError, tomllib.TOMLDecodeError) as error:
        print(f"Android Bevy driver lock inspection failed: {error}", file=sys.stderr)
        return 1
    problems.extend(dependency_problems(names))
    if problems:
        print("\n".join(problems), file=sys.stderr)
        return 1

    print(
        "Android Bevy driver resolved graph is isolated: "
        "no Application, Android owner, composition, Mihomo API or platform package; "
        "capture/desktop/script features are disabled."
    )
    return 0


def self_test() -> int:
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        lock = root / DRIVER_LOCK
        lock.parent.mkdir(parents=True, exist_ok=True)
        lock.write_text(
            '[[package]]\nname = "infiltrator-bevy-ui"\nversion = "0.0.0"\n',
            encoding="utf-8",
        )
        if locked_package_names(root) != {"infiltrator-bevy-ui"}:
            print("lock parser self-test failed", file=sys.stderr)
            return 1
        if dependency_problems(locked_package_names(root)):
            print("isolated lock allow case self-test failed", file=sys.stderr)
            return 1
        lock.write_text(
            '[[package]]\nname = "infiltrator-bevy-ui"\nversion = "0.0.0"\n\n'
            '[[package]]\nname = "mihomo-api"\nversion = "0.0.0"\n',
            encoding="utf-8",
        )
        if not dependency_problems(locked_package_names(root)):
            print("forbidden package self-test failed", file=sys.stderr)
            return 1
    if default_feature_problems({"default": ["script-engine-boa"]}):
        print("capture-free UI default feature self-test failed", file=sys.stderr)
        return 1
    if not default_feature_problems(
        {
            "default": ["capture-tools", "script-engine-boa"],
            "ui-application-projections": ["dep:infiltrator-application"],
        }
    ):
        print("retired/capture feature rejection self-test failed", file=sys.stderr)
        return 1
    product_source = """
    fn android_main() {
        infiltrator_bevy_ui::run_with_host_surface_and_command_sink(source, sink, insets);
    }
    """
    if driver_source_problems(product_source):
        print("owner-backed product entry source allow case self-test failed", file=sys.stderr)
        return 1
    capture_source = """
    fn android_main() {
        infiltrator_bevy_ui::run_demo();
    }
    """
    if not driver_source_problems(capture_source):
        print("capture source rejection self-test failed", file=sys.stderr)
        return 1
    print("Android Bevy driver dependency/symbol guard self-test passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
