#!/usr/bin/env python3
"""Keep every build script declared, minimal and process-free by default.

A `build.rs` runs arbitrary code before the crate compiles, outside the
sandbox of the product and before any test can observe it. The workspace
therefore treats each one as a declared surface:

  * every script (auto-detected `build.rs` or a manifest `build = "..."` path)
    must be registered in `build_surface_inventory.json`;
  * the registered package and `[build-dependencies]` must match the manifest,
    so a build script cannot gain a dependency by itself;
  * spawning a process or writing files requires the matching flag and a
    written purpose, which makes network downloads and stray writes visible.

Vendored reference sources are excluded; they are not product code.
"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import sys
import tomllib
import unittest

REPO_ROOT = Path(__file__).resolve().parents[2]
INVENTORY = Path("scripts/quality/build_surface_inventory.json")
SKIPPED_DIRECTORIES = {".git", "target", "node_modules", "dist", "build"}
SCAN_ROOTS = ("crates", "android", "examples", "templates", "sandbox")
EXCLUDED_ROOTS = (Path("crates/infiltrator-slint/lumen-source"),)
PROCESS = re.compile(
    r"\b(?:Command::new|std::process|reqwest|ureq|TcpStream|curl\b|wget\b|git clone)"
)
FILE_WRITES = re.compile(
    r"\b(?:fs::write|fs::copy|fs::create_dir|fs::create_dir_all|fs::remove_file"
    r"|fs::rename|File::create)"
)


def _walk(root: Path, roots: tuple[str, ...]):
    for scan_root in roots:
        base = root / scan_root
        if not base.is_dir():
            continue
        for directory, subdirectories, filenames in os.walk(base):
            subdirectories[:] = sorted(
                name for name in subdirectories if name not in SKIPPED_DIRECTORIES
            )
            for filename in sorted(filenames):
                yield Path(directory) / filename


def manifests(root: Path) -> list[Path]:
    return [path for path in _walk(root, SCAN_ROOTS) if path.name == "Cargo.toml"]


def discover(root: Path) -> dict[str, Path]:
    """Return every build script path mapped to the manifest that owns it."""
    found: dict[str, Path] = {}
    for path in _walk(root, SCAN_ROOTS):
        relative = path.relative_to(root)
        if any(relative.is_relative_to(item) for item in EXCLUDED_ROOTS):
            continue
        if path.name != "build.rs":
            continue
        found[relative.as_posix()] = _owning_manifest(path)
    for manifest in manifests(root):
        relative = manifest.relative_to(root)
        if any(relative.is_relative_to(item) for item in EXCLUDED_ROOTS):
            continue
        try:
            document = tomllib.loads(manifest.read_text(encoding="utf-8"))
        except (OSError, tomllib.TOMLDecodeError):
            continue
        declared = (document.get("package") or {}).get("build")
        if isinstance(declared, str):
            script = (manifest.parent / declared).resolve()
            found[script.relative_to(root.resolve()).as_posix()] = manifest
    return found


def _owning_manifest(script: Path) -> Path:
    directory = script.parent
    while True:
        candidate = directory / "Cargo.toml"
        if candidate.is_file():
            return candidate
        if directory == directory.parent:
            raise AssertionError(f"no manifest above {script}")
        directory = directory.parent


def _manifest_facts(manifest: Path) -> tuple[str, list[str]] | None:
    try:
        document = tomllib.loads(manifest.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError):
        return None
    package = (document.get("package") or {}).get("name")
    dependencies = sorted((document.get("build-dependencies") or {}).keys())
    if not isinstance(package, str):
        return None
    return package, dependencies


def load_inventory(root: Path) -> tuple[dict[str, dict], list[str]]:
    path = root / INVENTORY
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return {}, [f"{INVENTORY}: cannot read the ledger: {error}"]
    if not isinstance(document, dict) or document.get("version") != 1:
        return {}, [f"{INVENTORY}: expected a version 1 object"]
    rows = document.get("build_scripts")
    if not isinstance(rows, list) or not rows:
        return {}, [f"{INVENTORY}: `build_scripts` must be a non-empty array"]

    entries: dict[str, dict] = {}
    problems: list[str] = []
    for index, row in enumerate(rows):
        prefix = f"{INVENTORY}: build_scripts[{index}]"
        if not isinstance(row, dict):
            problems.append(f"{prefix} must be an object")
            continue
        relative = row.get("path")
        if not isinstance(relative, str) or not relative:
            problems.append(f"{prefix}: path must be a non-empty string")
            continue
        if Path(relative).is_absolute() or ".." in Path(relative).parts:
            problems.append(f"{prefix}: path must stay inside the repository: {relative}")
            continue
        if relative in entries:
            problems.append(f"{prefix}: duplicate path `{relative}`")
        if not isinstance(row.get("package"), str) or not row["package"]:
            problems.append(f"{prefix}: package must name the owning crate")
        dependencies = row.get("build_dependencies")
        if not isinstance(dependencies, list) or not all(
            isinstance(item, str) for item in dependencies
        ):
            problems.append(f"{prefix}: build_dependencies must be an array of strings")
        for flag in ("spawns_process", "writes_files"):
            if not isinstance(row.get(flag), bool):
                problems.append(f"{prefix}: {flag} must be true or false")
        if not isinstance(row.get("purpose"), str) or len(row["purpose"].strip()) < 12:
            problems.append(f"{prefix}: purpose must explain what the script does")
        entries[relative] = row
    return entries, problems


def check(root: Path = REPO_ROOT) -> list[str]:
    entries, problems = load_inventory(root)
    if not entries and problems:
        return problems
    discovered = discover(root)

    for relative in sorted(set(discovered) - set(entries)):
        problems.append(
            f"{relative}: build script is not registered in {INVENTORY};"
            " register its package, dependencies and side effects"
        )
    for relative in sorted(set(entries) - set(discovered)):
        problems.append(
            f"{relative}: registered in {INVENTORY} but no build script exists;"
            " delete the stale entry"
        )

    for relative in sorted(set(discovered) & set(entries)):
        entry = entries[relative]
        facts = _manifest_facts(discovered[relative])
        if facts is None:
            problems.append(f"{relative}: cannot read the owning manifest")
            continue
        package, dependencies = facts
        if entry.get("package") != package:
            problems.append(
                f"{relative}: package is `{package}`, ledger records `{entry.get('package')}`"
            )
        if sorted(entry.get("build_dependencies") or []) != dependencies:
            problems.append(
                f"{relative}: [build-dependencies] are {dependencies}, ledger records"
                f" {sorted(entry.get('build_dependencies') or [])}"
            )

        code = _strip_comments((root / relative).read_text(encoding="utf-8", errors="replace"))
        if PROCESS.search(code) and entry.get("spawns_process") is not True:
            problems.append(
                f"{relative}: spawns a process or opens a socket but the ledger says"
                " spawns_process = false"
            )
        if FILE_WRITES.search(code) and entry.get("writes_files") is not True:
            problems.append(
                f"{relative}: writes files but the ledger says writes_files = false"
            )
    return problems


def _strip_comments(text: str) -> str:
    return "\n".join(line for line in text.splitlines() if not line.lstrip().startswith("//"))


class BuildSurfaceSelfTest(unittest.TestCase):
    def test_repository_build_scripts_match_the_ledger(self):
        self.assertEqual(check(REPO_ROOT), [])

    def test_unregistered_build_script_is_rejected(self):
        with _scratch() as root:
            (root / "crates/demo/other.rs").write_text("fn main() {}\n", encoding="utf-8")
            (root / "crates/demo/Cargo.toml").write_text(
                '[package]\nname = "demo"\nversion = "0.1.0"\nbuild = "other.rs"\n\n'
                "[build-dependencies]\n",
                encoding="utf-8",
            )
            problems = check(root)
        self.assertTrue(any("other.rs" in problem for problem in problems))

    def test_undeclared_process_spawn_is_rejected(self):
        with _scratch() as root:
            (root / "crates/demo/build.rs").write_text(
                "fn main() { std::process::Command::new(\"curl\").status().unwrap(); }\n",
                encoding="utf-8",
            )
            problems = check(root)
        self.assertTrue(any("spawns a process" in problem for problem in problems))

    def test_build_dependency_drift_is_rejected(self):
        with _scratch() as root:
            manifest = root / "crates/demo/Cargo.toml"
            manifest.write_text(
                manifest.read_text(encoding="utf-8") + 'coin = "1"\n',
                encoding="utf-8",
            )
            problems = check(root)
        self.assertTrue(any("build-dependencies" in problem for problem in problems))


def _inventory() -> dict:
    return {
        "version": 1,
        "note": "fixture",
        "build_scripts": [
            {
                "path": "crates/demo/build.rs",
                "package": "demo",
                "build_dependencies": [],
                "spawns_process": False,
                "writes_files": False,
                "purpose": "Fixture build script for the guard self-test.",
            }
        ],
    }


class _scratch:
    def __enter__(self) -> Path:
        import tempfile

        self._temporary = tempfile.TemporaryDirectory()
        root = Path(self._temporary.name)
        (root / INVENTORY).parent.mkdir(parents=True, exist_ok=True)
        (root / INVENTORY).write_text(json.dumps(_inventory()), encoding="utf-8")
        (root / "crates/demo").mkdir(parents=True)
        (root / "crates/demo/Cargo.toml").write_text(
            '[package]\nname = "demo"\nversion = "0.1.0"\n\n[build-dependencies]\n',
            encoding="utf-8",
        )
        (root / "crates/demo/build.rs").write_text("fn main() {}\n", encoding="utf-8")
        return root

    def __exit__(self, *_excuses) -> None:
        self._temporary.cleanup()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--mode", choices=["report", "enforce"], default="report")
    arguments = parser.parse_args()

    if arguments.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(BuildSurfaceSelfTest)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1

    problems = check()
    if problems:
        print("\n".join(problems), file=sys.stderr)
        return 1 if arguments.mode == "enforce" else 0
    entries, _ = load_inventory(REPO_ROOT)
    print(
        f"build surface guard: {len(entries)} declared build scripts,"
        f" {sum(1 for entry in entries.values() if entry['writes_files'])} write"
        " inside their own output directory"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
