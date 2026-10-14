#!/usr/bin/env python3
"""Keep the Android Bevy driver out of runtime and capture composition."""

from __future__ import annotations

import argparse
from pathlib import Path
import re
import subprocess
import sys
import tomllib


ROOT = Path(__file__).resolve().parents[2]
DRIVER_MANIFEST = Path("android/bevy-host/driver/Cargo.toml")
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
FORBIDDEN_UI_FEATURES = {
    "default",
    "ui-application-projections",
    "capture-tools",
    "capture-controller",
    "desktop-host",
    "script-engine-boa",
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


def package_names(tree: str) -> set[str]:
    return {
        match.group(1)
        for line in tree.splitlines()
        if (match := re.search(r"(?:^|[├└]── )([A-Za-z0-9_-]+) v[0-9]", line))
    }


def ui_features(tree: str) -> set[str]:
    return {
        match.group(1)
        for line in tree.splitlines()
        if (match := re.search(r'infiltrator-bevy-ui feature "([^"]+)"', line))
    }


def dependency_problems(names: set[str]) -> list[str]:
    return [
        f"Android Bevy driver normal closure contains forbidden package `{name}`"
        for name in sorted(names & FORBIDDEN_PACKAGES)
    ]


def feature_problems(features: set[str]) -> list[str]:
    return [
        f"Android Bevy driver enables forbidden UI feature `{name}`"
        for name in sorted(features & FORBIDDEN_UI_FEATURES)
    ]


def default_feature_problems(features: dict[str, object]) -> list[str]:
    defaults = features.get("default")
    if not isinstance(defaults, list):
        return [f"{UI_MANIFEST}: `[features].default` must be an explicit list"]
    problems = [
        f"{UI_MANIFEST}: capture-only feature `{name}` must not be a product default"
        for name in ("capture-tools", "capture-controller")
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


def inspect_closure(root: Path) -> tuple[set[str], set[str]]:
    command = [
        "cargo",
        "tree",
        "--manifest-path",
        str(root / DRIVER_MANIFEST),
        "--offline",
        "--locked",
        "--target",
        "aarch64-linux-android",
        "-e",
        "features",
    ]
    result = subprocess.run(
        command,
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
        timeout=120,
    )
    return package_names(result.stdout), ui_features(result.stdout)


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
        names, features = inspect_closure(ROOT)
    except (OSError, subprocess.SubprocessError) as error:
        print(f"Android Bevy driver closure check failed: {error}", file=sys.stderr)
        return 1
    problems.extend(dependency_problems(names))
    problems.extend(feature_problems(features))
    if problems:
        print("\n".join(problems), file=sys.stderr)
        return 1

    print(
        "Android Bevy driver runtime closure is isolated: "
        "no Application, Android owner, composition, Mihomo API or platform package; "
        "capture/desktop/script features are disabled."
    )
    return 0


def self_test() -> int:
    tree = """\
├── infiltrator-bevy-ui v0.41.0
"""
    if package_names(tree) != {"infiltrator-bevy-ui"}:
        print("package parser self-test failed", file=sys.stderr)
        return 1
    if ui_features(tree):
        print("feature parser self-test failed", file=sys.stderr)
        return 1
    if dependency_problems(package_names(tree)):
        print("isolated UI closure allow case self-test failed", file=sys.stderr)
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
    forbidden = package_names(tree + "└── mihomo-api v0.1.0")
    if "mihomo-api" not in forbidden or not dependency_problems(forbidden):
        print("forbidden package self-test failed", file=sys.stderr)
        return 1
    bad_features = ui_features(
        tree + '└── infiltrator-bevy-ui feature "capture-tools"'
    )
    if "capture-tools" not in bad_features or not feature_problems(bad_features):
        print("forbidden feature self-test failed", file=sys.stderr)
        return 1
    print("Android Bevy driver dependency/symbol guard self-test passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
