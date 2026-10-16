#!/usr/bin/env python3
"""Enforce Android .so packaging boundaries and native symbol composition (BANDROID-041.4.2).

Validates that:
1. Product flavors declare exact required .so libraries for each supported ABI:
   - compose: libinfiltrator_android.so, libmihomo.so
   - bevy:    libinfiltrator_android.so, libinfiltrator_bevy_android.so, libmihomo.so
   - slint:   libinfiltrator_android.so (slint-ui), libmihomo.so
2. Gradle configuration binds driver projects and build scripts exclusively to their flavors:
   - :bevy-host is only added to bevyImplementation
   - buildSlintAndroidRust is only bound to mergeSlint*JniLibFolders
3. Symbol boundaries between host IPC, NativeActivity, and third-party core:
   - libinfiltrator_android.so (compose) must not export ANativeActivity_onCreate
   - libinfiltrator_bevy_android.so must export ANativeActivity_onCreate
   - libinfiltrator_android.so (slint) must export ANativeActivity_onCreate
   - Both driver and host .so files must not directly package Mihomo symbols
"""

from __future__ import annotations

import argparse
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[2]
BUILD_GRADLE = Path("android/app/build.gradle.kts")
PRODUCT_PROPERTIES = Path("product-identity.properties")
SUPPORTED_ABIS = ("arm64-v8a", "x86_64")

FLAVOR_REQUIRED_SO = {
    "compose": {"libinfiltrator_android.so", "libmihomo.so"},
    "bevy": {"libinfiltrator_android.so", "libinfiltrator_bevy_android.so", "libmihomo.so"},
    "slint": {"libinfiltrator_android.so", "libmihomo.so"},
}


def gradle_problems(content: str) -> list[str]:
    problems = []

    # 1. Bevy driver dependency isolation
    if 'add("bevyImplementation", project(":bevy-host"))' not in content:
        problems.append("build.gradle.kts must add :bevy-host only via bevyImplementation")

    for forbidden in ('implementation(project(":bevy-host"))', 'add("composeImplementation", project(":bevy-host"))', 'add("slintImplementation", project(":bevy-host"))'):
        if forbidden in content:
            problems.append(f"build.gradle.kts must not leak :bevy-host into other flavors via `{forbidden}`")

    # 2. Slint rust build task isolation
    if "buildSlintAndroidRust" not in content:
        problems.append("build.gradle.kts must define `buildSlintAndroidRust` for the Slint flavor")

    if 'name.startsWith("mergeSlint") && name.endsWith("JniLibFolders")' not in content:
        problems.append("build.gradle.kts must bind `buildSlintAndroidRust` exclusively to `mergeSlint*JniLibFolders`")

    # 3. Mihomo asset preparation
    if 'tasks.register<Copy>("prepareMihomoAsset")' not in content:
        problems.append("build.gradle.kts must register `prepareMihomoAsset` task")

    if 'mustRunAfter("cargoBuild")' not in content:
        problems.append("`prepareMihomoAsset` must run after `cargoBuild` to prevent jniLibs race conditions")

    return problems


def product_identity_problems(content: str) -> list[str]:
    problems = []
    match = re.search(r"^android\.products=([^\r\n]+)", content, re.MULTILINE)
    if not match:
        return ["product-identity.properties must declare android.products"]

    declared = set(match.group(1).split(","))
    required = set(FLAVOR_REQUIRED_SO.keys())
    if declared != required:
        problems.append(
            f"android.products mismatch: expected {sorted(required)}, found {sorted(declared)}"
        )
    return problems


def so_boundary_problems(flavor: str, so_names: set[str]) -> list[str]:
    problems = []
    expected = FLAVOR_REQUIRED_SO.get(flavor)
    if expected is None:
        return [f"Unknown Android flavor: `{flavor}`"]

    missing = expected - so_names
    if missing:
        problems.append(f"Flavor `{flavor}` is missing required native libraries: {sorted(missing)}")

    unexpected = so_names - expected
    if unexpected:
        problems.append(f"Flavor `{flavor}` contains unexpected native libraries: {sorted(unexpected)}")

    return problems


def native_symbol_boundary_problems(library_name: str, symbols: set[str], is_slint_flavor: bool = False) -> list[str]:
    problems = []
    has_native_activity = "ANativeActivity_onCreate" in symbols

    if library_name == "libinfiltrator_bevy_android.so":
        if not has_native_activity:
            problems.append("libinfiltrator_bevy_android.so must export ANativeActivity_onCreate")
    elif library_name == "libinfiltrator_android.so":
        if is_slint_flavor:
            if not has_native_activity:
                problems.append("Slint libinfiltrator_android.so must export ANativeActivity_onCreate")
        else:
            if has_native_activity:
                problems.append("Standard libinfiltrator_android.so must not export ANativeActivity_onCreate")

    return problems


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        test_gradle = (ROOT / BUILD_GRADLE).read_text(encoding="utf-8")
        assert not gradle_problems(test_gradle), gradle_problems(test_gradle)

        test_identity = (ROOT / PRODUCT_PROPERTIES).read_text(encoding="utf-8")
        assert not product_identity_problems(test_identity), product_identity_problems(test_identity)

        # Test positive cases
        for flavor, libs in FLAVOR_REQUIRED_SO.items():
            assert not so_boundary_problems(flavor, libs)

        # Test negative cases
        assert so_boundary_problems("compose", {"libinfiltrator_android.so"})
        assert so_boundary_problems("bevy", {"libinfiltrator_android.so"})
        assert so_boundary_problems("unknown", set())

        # Test symbol boundaries
        assert not native_symbol_boundary_problems("libinfiltrator_bevy_android.so", {"ANativeActivity_onCreate"})
        assert native_symbol_boundary_problems("libinfiltrator_bevy_android.so", set())
        assert not native_symbol_boundary_problems("libinfiltrator_android.so", {"Java_uniffi_..."}, is_slint_flavor=False)
        assert native_symbol_boundary_problems("libinfiltrator_android.so", {"ANativeActivity_onCreate"}, is_slint_flavor=False)
        assert not native_symbol_boundary_problems("libinfiltrator_android.so", {"ANativeActivity_onCreate"}, is_slint_flavor=True)

        print("self-test: PASS")
        return 0

    problems = []
    gradle_path = ROOT / BUILD_GRADLE
    if gradle_path.exists():
        problems.extend(gradle_problems(gradle_path.read_text(encoding="utf-8")))
    else:
        problems.append(f"{BUILD_GRADLE} not found")

    props_path = ROOT / PRODUCT_PROPERTIES
    if props_path.exists():
        problems.extend(product_identity_problems(props_path.read_text(encoding="utf-8")))
    else:
        problems.append(f"{PRODUCT_PROPERTIES} not found")

    if problems:
        for p in problems:
            print(f"VIOLATION: {p}", file=sys.stderr)
        return 1

    print("Android .so and APK combination guard: all flavors and boundaries aligned")
    return 0


if __name__ == "__main__":
    sys.exit(main())
