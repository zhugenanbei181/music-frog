#!/usr/bin/env python3
"""Read and verify the repository's pinned Android build toolchain."""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
from collections.abc import Mapping


ROOT = Path(__file__).resolve().parents[1]
ANDROID_ROOT = ROOT / "android"
TOOLCHAIN_FILE = ANDROID_ROOT / "toolchain.properties"
REQUIRED_KEYS = {
    "java.distribution",
    "java.major",
    "java.bootstrap.version",
    "java.bootstrap.directory",
    "java.bootstrap.url",
    "android.compile.sdk",
    "android.sdk.platform",
    "android.min.sdk",
    "android.target.sdk",
    "android.build.tools",
    "android.emulator.api",
    "android.emulator.image",
    "android.ndk.version",
    "android.cmdline.tools.revision",
    "android.cmdline.tools.version",
}
# This is the compatibility guard, not a build input: changing the selected
# plugin/wrapper versions requires a fresh upstream-matrix review.
FULLY_SUPPORTED_ANDROID_GRADLE_TUPLE = ("9.3.1", "2.4.21", "9.7.0")


def read_properties(path: Path = TOOLCHAIN_FILE) -> dict[str, str]:
    values: dict[str, str] = {}
    for line_number, raw_line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        line = raw_line.strip()
        if not line or line.startswith(("#", "!")):
            continue
        key, separator, value = line.partition("=")
        if not separator or not key.strip():
            raise ValueError(f"invalid property at {path}:{line_number}")
        key = key.strip()
        if key in values:
            raise ValueError(f"duplicate property {key!r} at {path}:{line_number}")
        values[key] = value.strip()
    return values


def require_properties(values: Mapping[str, str]) -> None:
    missing = sorted(REQUIRED_KEYS - values.keys())
    if missing:
        raise ValueError(f"toolchain properties are missing: {', '.join(missing)}")
    for key in REQUIRED_KEYS:
        if not values[key]:
            raise ValueError(f"toolchain property {key!r} is empty")
    for key in (
        "java.major",
        "android.compile.sdk",
        "android.min.sdk",
        "android.target.sdk",
        "android.emulator.api",
        "android.cmdline.tools.revision",
    ):
        if not values[key].isdigit():
            raise ValueError(f"toolchain property {key!r} must be a positive integer")
        if int(values[key]) <= 0:
            raise ValueError(f"toolchain property {key!r} must be a positive integer")


def java_major(version_output: str) -> int:
    match = re.search(r"(?:openjdk|java) version \"([^\"]+)\"", version_output, re.IGNORECASE)
    if match is None:
        match = re.search(r"(?m)^(?:openjdk|java)\s+(\d+(?:\.\d+)+)", version_output, re.IGNORECASE)
        version = match.group(1) if match else ""
    else:
        version = match.group(1)
    if not version:
        raise ValueError("could not read Java version output")
    components = version.split(".")
    return int(components[1]) if components[0] == "1" and len(components) > 1 else int(components[0])


def gradle_wrapper_version(properties_text: str) -> str:
    match = re.search(r"distributionUrl=.*?/gradle-([0-9.]+)-(?:bin|all)\.zip", properties_text)
    if match is None:
        raise ValueError("Gradle wrapper distribution URL is missing a pinned version")
    return match.group(1)


def android_gradle_plugin_versions(build_script: str) -> tuple[str, str]:
    agp = re.search(
        r'id\("com\.android\.application"\)\s+version\s+"([0-9.]+)"',
        build_script,
    )
    kotlin = re.findall(
        r'id\("org\.jetbrains\.kotlin\.(?:jvm|plugin\.compose)"\)\s+version\s+"([0-9.]+)"',
        build_script,
    )
    if agp is None or len(kotlin) != 2 or len(set(kotlin)) != 1:
        raise ValueError("Android Gradle and Kotlin plugin versions must be explicit and consistent")
    return agp.group(1), kotlin[0]


def find_java(values: Mapping[str, str]) -> Path:
    configured_home = os.environ.get("JAVA_HOME")
    if configured_home:
        candidate = Path(configured_home) / "bin" / "java"
        if candidate.is_file():
            return candidate
        raise ValueError("JAVA_HOME does not contain bin/java")

    candidates = [Path.home() / "android-tools-bevy" / "jdk" / "bin" / "java"]
    java_on_path = shutil.which("java")
    if java_on_path:
        candidates.append(Path(java_on_path))
    for candidate in candidates:
        if candidate.is_file():
            result = subprocess.run(
                [str(candidate), "-version"],
                check=False,
                capture_output=True,
                text=True,
            )
            if result.returncode == 0 and java_major(result.stdout + result.stderr) == int(values["java.major"]):
                return candidate
    raise ValueError(
        f"Java {values['java.major']} is required; set JAVA_HOME to a matching JDK"
    )


def sdk_root() -> Path:
    for variable in ("ANDROID_HOME", "ANDROID_SDK_ROOT"):
        value = os.environ.get(variable)
        if value:
            return Path(value).expanduser()
    local_properties = ANDROID_ROOT / "local.properties"
    if local_properties.is_file():
        for raw_line in local_properties.read_text(encoding="utf-8").splitlines():
            if raw_line.startswith("sdk.dir="):
                return Path(raw_line.partition("=")[2].replace("\\:", ":")).expanduser()
    raise ValueError("set ANDROID_HOME/ANDROID_SDK_ROOT or sdk.dir in android/local.properties")


def java_version(java: Path) -> int:
    result = subprocess.run(
        [str(java), "-version"],
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise ValueError(f"Java version probe failed: {result.stderr.strip()}")
    return java_major(result.stdout + result.stderr)


def ensure_source_links(values: Mapping[str, str]) -> None:
    requirements = {
        ANDROID_ROOT / "app/build.gradle.kts": (
            'toolchainInt("android.compile.sdk")',
            'toolchainValue("android.ndk.version")',
            'toolchainInt("android.min.sdk")',
            'toolchainInt("android.target.sdk")',
            'toolchainValue("java.major")',
        ),
        ANDROID_ROOT / "bevy-host/build.gradle.kts": (
            'toolchainInt("android.compile.sdk")',
            'toolchainValue("android.ndk.version")',
            'toolchainInt("android.min.sdk")',
        ),
        ANDROID_ROOT / "core-host-tests/build.gradle.kts": (
            'toolchainValue("android.sdk.platform")',
            'toolchainValue("java.major")',
        ),
        ROOT / ".github/workflows/android.yml": (
            "steps.android-toolchain.outputs.java_major",
            "steps.android-toolchain.outputs.android_sdk_platform",
            "steps.android-toolchain.outputs.android_build_tools",
            "steps.android-toolchain.outputs.android_emulator_api",
            "steps.android-toolchain.outputs.android_emulator_image",
            "steps.android-toolchain.outputs.android_ndk_version",
            "steps.android-toolchain.outputs.android_cmdline_tools_revision",
        ),
        ROOT / ".github/workflows/release.yml": (
            "steps.android-toolchain.outputs.java_major",
            "steps.android-toolchain.outputs.android_sdk_platform",
            "steps.android-toolchain.outputs.android_build_tools",
            "steps.android-toolchain.outputs.android_ndk_version",
            "steps.android-toolchain.outputs.android_cmdline_tools_revision",
        ),
        ROOT / "scripts/android-emulator-tests.sh": (
            "scripts/android-toolchain.py",
            'scripts/android-toolchain.py" check',
        ),
        ROOT / "scripts/build-bevy-apk.sh": (
            "scripts/android-toolchain.py",
            'scripts/android-toolchain.py" check',
            "toolchain_value java.bootstrap.url",
            "toolchain_value android.sdk.platform",
            "__ANDROID_MIN_SDK__",
            "__ANDROID_TARGET_SDK__",
        ),
        ROOT / "scripts/android-device-evidence.sh": (
            "scripts/android-toolchain.py",
            "get android.build.tools",
        ),
        ROOT / "scripts/android-check.sh": (
            "android-toolchain.py check",
        ),
    }
    for path, fragments in requirements.items():
        content = path.read_text(encoding="utf-8")
        missing = [fragment for fragment in fragments if fragment not in content]
        if missing:
            raise ValueError(
                f"{path.relative_to(ROOT)} does not consume the pinned toolchain: "
                + ", ".join(missing)
            )

    duplicated_pins = {
        values["android.ndk.version"],
        f"platforms;android-{values['android.sdk.platform']}",
        f"build-tools;{values['android.build.tools']}",
        f"system-images;android-{values['android.emulator.api']};{values['android.emulator.image']}",
        "JavaVersion.VERSION_17",
        "JvmTarget.JVM_17",
        "java-version: '17'",
        f"compileSdk = {values['android.compile.sdk']}",
        f"minSdk = {values['android.min.sdk']}",
        f"targetSdk = {values['android.target.sdk']}",
    }
    active_config_paths = (
        ANDROID_ROOT / "app/build.gradle.kts",
        ANDROID_ROOT / "bevy-host/build.gradle.kts",
        ANDROID_ROOT / "core-host-tests/build.gradle.kts",
        ROOT / ".github/workflows/android.yml",
        ROOT / ".github/workflows/release.yml",
        ROOT / "scripts/android-emulator-tests.sh",
        ROOT / "scripts/android-check.sh",
        ROOT / "scripts/android-device-evidence.sh",
        ROOT / "scripts/build-bevy-apk.sh",
    )
    for path in active_config_paths:
        content = path.read_text(encoding="utf-8")
        duplicates = sorted(pin for pin in duplicated_pins if pin in content)
        if duplicates:
            raise ValueError(
                f"{path.relative_to(ROOT)} duplicates toolchain pins: " + ", ".join(duplicates)
            )

    driver_manifest = (ROOT / "crates/infiltrator-bevy-ui/Cargo.toml").read_text(encoding="utf-8")
    for key in ("min", "target"):
        match = re.search(rf"{key}_sdk_version\s*=\s*(\d+)", driver_manifest)
        expected = values[f"android.{key}.sdk"]
        if match is None or match.group(1) != expected:
            raise ValueError(f"Bevy UI {key} SDK metadata differs from android/toolchain.properties")

    rust_toolchain = (ROOT / "rust-toolchain.toml").read_text(encoding="utf-8")
    rust_match = re.search(r'^channel\s*=\s*"([^"]+)"', rust_toolchain, re.MULTILINE)
    if rust_match is None:
        raise ValueError("rust-toolchain.toml is missing a pinned channel")
    for workflow in (ROOT / ".github/workflows/android.yml", ROOT / ".github/workflows/release.yml"):
        workflow_text = workflow.read_text(encoding="utf-8")
        action_match = re.search(r"dtolnay/rust-toolchain@([0-9.]+)", workflow_text)
        if action_match is None or action_match.group(1) != rust_match.group(1):
            raise ValueError(f"{workflow.relative_to(ROOT)} Rust version differs from rust-toolchain.toml")


def check(values: Mapping[str, str]) -> list[str]:
    require_properties(values)
    ensure_source_links(values)
    checks = ["Android toolchain source links are consistent"]

    java = find_java(values)
    expected_java = int(values["java.major"])
    actual_java = java_version(java)
    if actual_java != expected_java:
        raise ValueError(f"JDK is {actual_java}; expected {expected_java}")
    if not (java.parent / "javac").is_file():
        raise ValueError("JAVA_HOME must point to a JDK containing bin/javac")
    checks.append(f"JDK {actual_java}")

    wrapper_properties = (ANDROID_ROOT / "gradle/wrapper/gradle-wrapper.properties").read_text(
        encoding="utf-8"
    )
    expected_gradle = gradle_wrapper_version(wrapper_properties)
    gradle_environment = os.environ.copy()
    java_home = java.parent.parent
    gradle_environment["JAVA_HOME"] = str(java_home)
    gradle_environment["PATH"] = f"{java.parent}{os.pathsep}{gradle_environment.get('PATH', '')}"
    gradle_result = subprocess.run(
        [str(ANDROID_ROOT / "gradlew"), "--no-daemon", "--version"],
        cwd=ANDROID_ROOT,
        check=False,
        capture_output=True,
        text=True,
        env=gradle_environment,
    )
    if gradle_result.returncode != 0:
        raise ValueError(f"Gradle wrapper version probe failed: {gradle_result.stderr.strip()}")
    output = gradle_result.stdout + gradle_result.stderr
    installed_gradle = re.search(r"(?m)^Gradle\s+([0-9.]+)$", output)
    installed_java = re.search(r"(?m)^Launcher JVM:\s+([0-9]+)", output)
    if installed_gradle is None or installed_gradle.group(1) != expected_gradle:
        raise ValueError("Gradle launcher does not match gradle-wrapper.properties")
    if installed_java is None or int(installed_java.group(1)) != expected_java:
        raise ValueError("Gradle is not running on the pinned JDK major version")
    checks.append(f"Gradle {expected_gradle}")

    agp_version, kotlin_plugin_version = android_gradle_plugin_versions(
        (ANDROID_ROOT / "build.gradle.kts").read_text(encoding="utf-8")
    )
    current_tuple = (agp_version, kotlin_plugin_version, expected_gradle)
    if current_tuple != FULLY_SUPPORTED_ANDROID_GRADLE_TUPLE:
        raise ValueError(
            "Android Gradle compatibility tuple has not been reviewed: "
            f"AGP {agp_version}, KGP {kotlin_plugin_version}, Gradle {expected_gradle}; "
            "review BANDROID-024.1 and update the compatibility guard"
        )
    checks.append(
        f"AGP {agp_version}, Kotlin Gradle Plugin {kotlin_plugin_version}, Gradle {expected_gradle} "
        "match the reviewed fully-supported tuple"
    )

    sdk = sdk_root()
    sdk_platform = values["android.sdk.platform"]
    build_tools = values["android.build.tools"]
    ndk_version = values["android.ndk.version"]
    command_line_tools_version = values["android.cmdline.tools.version"]
    required_paths = (
        sdk / "platforms" / f"android-{sdk_platform}" / "android.jar",
        sdk / "build-tools" / build_tools / "aapt",
        sdk / "ndk" / ndk_version / "source.properties",
        sdk / "cmdline-tools/latest/source.properties",
    )
    missing_paths = [str(path) for path in required_paths if not path.exists()]
    if missing_paths:
        raise ValueError("Android SDK packages are missing: " + ", ".join(missing_paths))
    ndk_properties = read_properties(sdk / "ndk" / ndk_version / "source.properties")
    if ndk_properties.get("Pkg.Revision") != ndk_version:
        raise ValueError("installed NDK revision differs from android/toolchain.properties")
    command_line_tools = read_properties(sdk / "cmdline-tools/latest/source.properties")
    if command_line_tools.get("Pkg.Revision") != command_line_tools_version:
        raise ValueError("installed command-line tools differ from android/toolchain.properties")
    for variable in ("ANDROID_NDK_HOME", "ANDROID_NDK_ROOT"):
        configured_ndk = os.environ.get(variable)
        if configured_ndk:
            configured_properties = read_properties(Path(configured_ndk) / "source.properties")
            if configured_properties.get("Pkg.Revision") != ndk_version:
                raise ValueError(f"{variable} differs from android/toolchain.properties")
    checks.append(
        f"Android platform {sdk_platform}, Build Tools {build_tools}, "
        f"Command-line Tools {command_line_tools_version}, NDK {ndk_version}"
    )

    rust_channel = re.search(
        r'^channel\s*=\s*"([^"]+)"',
        (ROOT / "rust-toolchain.toml").read_text(encoding="utf-8"),
        re.MULTILINE,
    )
    rust_result = subprocess.run(
        ["rustc", "--version"],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    if rust_channel is None or rust_result.returncode != 0 or not rust_result.stdout.startswith(
        f"rustc {rust_channel.group(1)} "
    ):
        raise ValueError("installed rustc does not match rust-toolchain.toml")
    checks.append(f"Rust {rust_channel.group(1)}")

    plugin_pins = (ANDROID_ROOT / "build.gradle.kts").read_text(encoding="utf-8")
    if re.search(r'id\("com\.android\.application"\)\s+version\s+"[0-9.]+"', plugin_pins) is None:
        raise ValueError("Android Gradle Plugin must have an explicit version pin")
    kotlin_versions = re.findall(
        r'id\("org\.jetbrains\.kotlin\.(?:jvm|plugin\.compose)"\)\s+version\s+"([0-9.]+)"',
        plugin_pins,
    )
    if len(kotlin_versions) != 2 or len(set(kotlin_versions)) != 1:
        raise ValueError("JVM and Compose Kotlin Gradle plugins must share one explicit version pin")
    checks.append("Android Gradle Plugin and Kotlin plugin are explicitly pinned")
    return checks


def github_outputs(values: Mapping[str, str], destination: Path) -> None:
    require_properties(values)
    output_values = {
        "java_distribution": values["java.distribution"],
        "java_major": values["java.major"],
        "android_compile_sdk": values["android.compile.sdk"],
        "android_sdk_platform": values["android.sdk.platform"],
        "android_min_sdk": values["android.min.sdk"],
        "android_target_sdk": values["android.target.sdk"],
        "android_build_tools": values["android.build.tools"],
        "android_emulator_api": values["android.emulator.api"],
        "android_emulator_image": values["android.emulator.image"],
        "android_ndk_version": values["android.ndk.version"],
        "android_cmdline_tools_revision": values["android.cmdline.tools.revision"],
    }
    with destination.open("a", encoding="utf-8") as output:
        for key, value in output_values.items():
            output.write(f"{key}={value}\n")


def resolve_java(values: Mapping[str, str]) -> Path:
    java = find_java(values)
    actual = java_version(java)
    expected = int(values["java.major"])
    if actual != expected:
        raise ValueError(f"JDK is {actual}; expected {expected}")
    return java.parent.parent


def self_test() -> None:
    assert java_major('openjdk version "17.0.20.1" 2025-01-01') == 17
    assert java_major("openjdk 25.0.3 2026-04-21") == 25
    assert java_major('java version "1.8.0_432"') == 8
    assert gradle_wrapper_version(
        "distributionUrl=https\\://services.gradle.org/distributions/gradle-9.7.0-bin.zip"
    ) == FULLY_SUPPORTED_ANDROID_GRADLE_TUPLE[2]
    assert android_gradle_plugin_versions(
        'id("com.android.application") version "9.3.1"\n'
        'id("org.jetbrains.kotlin.jvm") version "2.4.21"\n'
        'id("org.jetbrains.kotlin.plugin.compose") version "2.4.21"'
    ) == FULLY_SUPPORTED_ANDROID_GRADLE_TUPLE[:2]
    require_properties(read_properties())
    print("android-toolchain self-test: passed")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("check", help="verify local and source-pinned Android toolchains")
    get_parser = commands.add_parser("get", help="print one property for local scripts")
    get_parser.add_argument("key")
    output_parser = commands.add_parser("github-output", help="write pinned setup values to GITHUB_OUTPUT")
    output_parser.add_argument("path", type=Path)
    commands.add_parser("resolve-java", help="print a locally installed JDK matching the pin")
    commands.add_parser("self-test", help="exercise parser and pin validation")
    args = parser.parse_args()
    try:
        values = read_properties()
        if args.command == "get":
            require_properties(values)
            if args.key not in values:
                raise ValueError(f"unknown toolchain property {args.key!r}")
            print(values[args.key])
        elif args.command == "github-output":
            github_outputs(values, args.path)
        elif args.command == "resolve-java":
            print(resolve_java(values))
        elif args.command == "self-test":
            self_test()
        else:
            for line in check(values):
                print(f"[android-toolchain] OK: {line}")
    except (OSError, ValueError) as error:
        print(f"[android-toolchain] ERROR: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
