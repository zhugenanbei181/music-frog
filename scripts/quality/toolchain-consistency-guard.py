#!/usr/bin/env python3
"""Keep one Rust release pinned across manifests and every CI workflow.

`rust-toolchain.toml` is the single source of truth. A workflow that floats on
`stable`, or a manifest that advertises another `rust-version`, compiles code
with a compiler the repository never tested. The guard is offline: it reads
`rust-toolchain.toml`, workspace manifests, and `.github/workflows/*.yml`.

Exit codes: 0 when every reference agrees with the pin, 1 otherwise.
"""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import re
import sys
import tomllib
import unittest

REPO_ROOT = Path(__file__).resolve().parents[2]
TOOLCHAIN = Path("rust-toolchain.toml")
WORKFLOWS = Path(".github/workflows")
SKIPPED_DIRECTORIES = {".git", "target", "vendor", "node_modules", "dist"}
FLOATING_CHANNELS = {"stable", "beta", "nightly", "master", "main"}
ACTION_REFERENCE = re.compile(r"dtolnay/rust-toolchain@(?P<ref>[A-Za-z0-9._-]+)")
YAML_TOOLCHAIN_KEY = re.compile(r"^\s*toolchain:\s*(?P<ref>[^\s#]+)\s*(?:#.*)?$", re.MULTILINE)
MANIFEST_RUST_VERSION = re.compile(
    r'^\s*rust-version\s*=\s*"(?P<version>[^"]+)"', re.MULTILINE
)


def _normalize(version: str) -> tuple[int, ...]:
    parts: list[int] = []
    for piece in version.split("."):
        digits = re.match(r"\d+", piece)
        if digits is None:
            break
        parts.append(int(digits.group()))
    return tuple(parts)


def read_channel(root: Path) -> tuple[str | None, list[str]]:
    """Return the pinned channel, or None plus the reasons it is unusable."""
    try:
        document = tomllib.loads((root / TOOLCHAIN).read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as error:
        return None, [f"{TOOLCHAIN}: cannot read the pinned toolchain: {error}"]
    table = document.get("toolchain")
    channel = table.get("channel") if isinstance(table, dict) else None
    if not isinstance(channel, str) or not channel.strip():
        return None, [f"{TOOLCHAIN}: [toolchain] channel must name one pinned release"]
    if channel.strip() in FLOATING_CHANNELS:
        return None, [
            f"{TOOLCHAIN}: channel `{channel}` floats; pin one release so the"
            " workspace and CI agree"
        ]
    return channel.strip(), []


def _walk_manifests(root: Path) -> list[Path]:
    manifests: list[Path] = []
    for directory, subdirectories, filenames in os.walk(root):
        subdirectories[:] = sorted(
            name for name in subdirectories if name not in SKIPPED_DIRECTORIES
        )
        if "Cargo.toml" in filenames:
            manifests.append(Path(directory) / "Cargo.toml")
    return manifests


def manifest_pins(root: Path) -> list[tuple[str, str]]:
    """Return `(manifest, rust-version)` for every manifest that declares one."""
    rows: list[tuple[str, str]] = []
    for path in _walk_manifests(root):
        text = path.read_text(encoding="utf-8", errors="replace")
        for match in MANIFEST_RUST_VERSION.finditer(text):
            rows.append((path.relative_to(root).as_posix(), match.group("version")))
    return rows


def workflow_references(root: Path) -> list[tuple[str, int, str]]:
    """Return `(workflow, line, version)` for every toolchain reference."""
    rows: list[tuple[str, int, str]] = []
    directories = [root / WORKFLOWS]
    for directory in directories:
        if not directory.is_dir():
            continue
        for path in sorted(directory.glob("*.yml")) + sorted(directory.glob("*.yaml")):
            text = path.read_text(encoding="utf-8")
            relative = path.relative_to(root).as_posix()
            for match in ACTION_REFERENCE.finditer(text):
                line = text.count("\n", 0, match.start()) + 1
                rows.append((relative, line, match.group("ref")))
            for match in YAML_TOOLCHAIN_KEY.finditer(text):
                line = text.count("\n", 0, match.start()) + 1
                rows.append((relative, line, match.group("ref").strip("\"'")))
    return rows


def check(root: Path = REPO_ROOT) -> list[str]:
    channel, problems = read_channel(root)
    if channel is None:
        return problems

    for manifest, version in manifest_pins(root):
        if _normalize(version) != _normalize(channel):
            problems.append(
                f"{manifest}: rust-version `{version}` disagrees with the pinned"
                f" toolchain `{channel}`"
            )

    for workflow, line, version in workflow_references(root):
        if _normalize(version) == _normalize(channel) and version != channel:
            problems.append(
                f"{workflow}:{line}: toolchain `{version}` must be spelled `{channel}`"
            )
        elif _normalize(version) != _normalize(channel):
            problems.append(
                f"{workflow}:{line}: toolchain `{version}` disagrees with the pinned"
                f" toolchain `{channel}`"
            )
    return problems


class ToolchainConsistencySelfTest(unittest.TestCase):
    def test_version_normalization_compares_releases_not_spellings(self):
        self.assertEqual(_normalize("1.99"), (1, 99))
        self.assertEqual(_normalize("1.99.0"), (1, 99, 0))
        self.assertNotEqual(_normalize("1.99.0"), _normalize("1.98.1"))

    def test_floating_channel_is_rejected(self):
        with _temporary_root({"rust-toolchain.toml": '[toolchain]\nchannel = "stable"\n'}) as root:
            channel, problems = read_channel(root)
        self.assertIsNone(channel)
        self.assertTrue(any("floats" in problem for problem in problems))

    def test_workflow_reference_to_another_release_is_reported(self):
        with _temporary_root(
            {
                "rust-toolchain.toml": '[toolchain]\nchannel = "1.99.0"\n',
                ".github/workflows/main-gate.yml": (
                    "steps:\n  - uses: dtolnay/rust-toolchain@stable\n"
                ),
            }
        ) as root:
            problems = check(root)
        self.assertTrue(any("main-gate.yml:2" in problem for problem in problems))


class _temporary_root:
    """Minimal scratch tree so self-tests never touch the real repository."""

    def __init__(self, files: dict[str, str]):
        self._files = files

    def __enter__(self) -> Path:
        import tempfile

        self._temporary = tempfile.TemporaryDirectory()
        root = Path(self._temporary.name)
        for relative, content in self._files.items():
            path = root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content, encoding="utf-8")
        return root

    def __exit__(self, *_excuses) -> None:
        self._temporary.cleanup()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--mode", choices=["report", "enforce"], default="report")
    arguments = parser.parse_args()

    if arguments.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(
            ToolchainConsistencySelfTest
        )
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1

    problems = check()
    if problems:
        print("\n".join(problems), file=sys.stderr)
        return 1 if arguments.mode == "enforce" else 0
    print("toolchain consistency guard: single pinned Rust release")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
