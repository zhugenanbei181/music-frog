#!/usr/bin/env python3
"""Keep Android Mihomo APK inputs on the hash-verified packaging-lock path."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[2]
ANDROID_GRADLE = ROOT / "android/app/build.gradle.kts"
FETCHER = ROOT / "scripts/fetch-mihomo.py"
LOCK = ROOT / "packaging/mihomo-assets.json"
ANDROID_ASSETS = {
    "aarch64-linux-android": "arm64-v8a",
    "x86_64-linux-android": "x86_64",
}


def source_problems(root: Path = ROOT) -> list[str]:
    problems: list[str] = []
    gradle_path = root / "android/app/build.gradle.kts"
    fetcher_path = root / "scripts/fetch-mihomo.py"
    lock_path = root / "packaging/mihomo-assets.json"

    try:
        gradle = gradle_path.read_text(encoding="utf-8")
    except OSError as error:
        return [f"Android app Gradle file is unavailable: {error}"]
    try:
        fetcher = fetcher_path.read_text(encoding="utf-8")
    except OSError as error:
        return [f"Mihomo asset fetcher is unavailable: {error}"]
    try:
        lock = json.loads(lock_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return [f"Mihomo packaging lock is unreadable: {error}"]

    if not re.fullmatch(r"v[0-9]+\.[0-9]+\.[0-9]+", str(lock.get("version", ""))):
        problems.append("packaging/mihomo-assets.json must pin a semantic vX.Y.Z release")
    locked_assets = {
        asset.get("target"): asset
        for asset in lock.get("assets", [])
        if isinstance(asset, dict)
    }
    for target in ANDROID_ASSETS:
        asset = locked_assets.get(target)
        if asset is None:
            problems.append(f"Mihomo lock has no Android asset for {target}")
        elif not re.fullmatch(r"[0-9a-f]{64}", str(asset.get("binary_sha256", ""))):
            problems.append(f"Mihomo lock has no valid binary SHA-256 for {target}")

    locked_fetcher_requirements = (
        'LOCK = ROOT / "packaging/mihomo-assets.json"',
        'parser.add_argument("--stage"',
        'if sha256(archive) != asset["archive_sha256"]:',
        'if sha256(binary) != asset["binary_sha256"]:',
        'if requested != lock["version"]:',
    )
    for requirement in locked_fetcher_requirements:
        if requirement not in fetcher:
            problems.append(f"scripts/fetch-mihomo.py is missing required lock verification: {requirement}")

    expected_task_calls = (
        ('fetchMihomoArm64Asset', 'aarch64-linux-android', 'arm64-v8a'),
        ('fetchMihomoX86_64Asset', 'x86_64-linux-android', 'x86_64'),
    )
    for task_name, target, abi in expected_task_calls:
        call = re.compile(
            rf'fetchMihomoAsset\(\s*["\']{re.escape(task_name)}["\']\s*,\s*'
            rf'["\']{re.escape(target)}["\']\s*,\s*["\']{re.escape(abi)}["\']\s*,?\s*\)'
        )
        if call.search(gradle) is None:
            problems.append(
                f"Android Gradle must stage {target} through fetchMihomo.py into the {abi} ABI"
            )

    required_gradle_fragments = (
        'rootProject.file("../scripts/fetch-mihomo.py")',
        'commandLine(',
        '"--target"',
        '"--stage"',
        'mihomoStagingDirectory.get().file("$abi/libmihomo.so")',
        'tasks.register<Copy>("prepareMihomoAsset")',
        'dependsOn(fetchMihomoArm64Asset, fetchMihomoX86_64Asset)',
        'from(mihomoStagingDirectory.get().dir("arm64-v8a"))',
        'from(mihomoStagingDirectory.get().dir("x86_64"))',
        'include("libmihomo.so")',
        'into(layout.projectDirectory.dir("src/main/jniLibs"))',
        'dependsOn("prepareMihomoAsset")',
        'dependsOn("verifyAndroidMihomoAssets")',
    )
    for requirement in required_gradle_fragments:
        if requirement not in gradle:
            problems.append(f"Android Gradle is missing locked Mihomo staging wiring: {requirement}")

    if gradle.count('include("libmihomo.so")') != 2:
        problems.append("Android Gradle must copy only one libmihomo.so for each locked Android ABI")

    try:
        gradle_files = [
            *sorted((root / "android").rglob("build.gradle")),
            *sorted((root / "android").rglob("build.gradle.kts")),
        ]
    except OSError as error:
        problems.append(f"cannot scan Android Gradle build files: {error}")
        gradle_files = []
    for path in gradle_files:
        try:
            source = path.read_text(encoding="utf-8")
        except OSError as error:
            problems.append(f"cannot read Android Gradle file {path.relative_to(root)}: {error}")
            continue
        relative = path.relative_to(root)
        if re.search(r"vendor[/\\]mihomo-android-(?:arm64-v8|amd64)", source):
            problems.append(f"{relative} must not read legacy vendor Mihomo binaries")
        if path != gradle_path and any(
            marker in source for marker in ("libmihomo.so", "fetch-mihomo.py", "mihomo-assets.json")
        ):
            problems.append(f"{relative} must not add a second Android Mihomo asset source")

    return problems


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT, help="repository root")
    args = parser.parse_args()
    problems = source_problems(args.root.resolve())
    if problems:
        print("[android-mihomo-assets-guard] ERROR", file=sys.stderr)
        for problem in problems:
            print(f"- {problem}", file=sys.stderr)
        return 1
    print("[android-mihomo-assets-guard] Android Mihomo assets use the hash-verified packaging lock")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
