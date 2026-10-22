#!/usr/bin/env python3
"""Make every panic boundary explicit and keep it reachable.

A `catch_unwind` is a reliability decision: it says which failure is contained
instead of aborting the process or wedging a worker. `panic_boundary_inventory.json`
registers each one with the failure it contains, and this guard enforces both
directions:

  * an unregistered `catch_unwind` fails the gate, and a registered site that
    disappears must be deleted from the ledger;
  * `panic = "abort"` in any profile is rejected, because aborting skips the
    unwind and silently turns every registered boundary into dead code.

The guard is offline: it reads Rust sources and manifests, it never builds.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import json
import os
from pathlib import Path
import re
import sys
import tomllib
import unittest

QUALITY_ROOT = Path(__file__).resolve().parent
REPO_ROOT = QUALITY_ROOT.parents[1]
INVENTORY = Path("scripts/quality/panic_boundary_inventory.json")

sys.path.insert(0, str(QUALITY_ROOT))
import rust_syntax  # noqa: E402  (path bootstrap above is the import contract)

CATCH_UNWIND = re.compile(r"\bcatch_unwind\s*\(")
CATEGORIES = {"crash_reporting", "runtime_supervision", "ui_event_loop"}
SKIPPED_DIRECTORIES = {".git", "target", "node_modules", "dist", "build"}
SCAN_ROOTS = ("crates", "android", "examples", "templates", "sandbox")
EXCLUDED_ROOTS = (Path("crates/infiltrator-slint/lumen-source"),)


@dataclass(frozen=True)
class Boundary:
    path: str
    sites: int
    category: str
    rationale: str


def scan(root: Path) -> dict[str, int]:
    """Count `catch_unwind` calls per file, ignoring imports and comments."""
    found: dict[str, int] = {}
    for scan_root in SCAN_ROOTS:
        base = root / scan_root
        if not base.is_dir():
            continue
        for directory, subdirectories, filenames in os.walk(base):
            subdirectories[:] = sorted(
                name for name in subdirectories if name not in SKIPPED_DIRECTORIES
            )
            for filename in sorted(filenames):
                if not filename.endswith(".rs"):
                    continue
                path = Path(directory) / filename
                relative = path.relative_to(root)
                if any(relative.is_relative_to(item) for item in EXCLUDED_ROOTS):
                    continue
                text = path.read_text(encoding="utf-8", errors="replace")
                sites = len(CATCH_UNWIND.findall(rust_syntax.mask_noncode(text)))
                if sites:
                    found[relative.as_posix()] = sites
    return found


def _manifests(root: Path) -> list[Path]:
    manifests: list[Path] = []
    for directory, subdirectories, filenames in os.walk(root):
        subdirectories[:] = sorted(
            name for name in subdirectories if name not in SKIPPED_DIRECTORIES
        )
        if "Cargo.toml" in filenames:
            manifests.append(Path(directory) / "Cargo.toml")
    return manifests


def aborting_profiles(root: Path) -> list[str]:
    """Return `manifest: profile` for every profile that aborts on panic."""
    problems: list[str] = []
    for path in _manifests(root):
        try:
            document = tomllib.loads(path.read_text(encoding="utf-8"))
        except (OSError, tomllib.TOMLDecodeError) as error:
            problems.append(f"{path.relative_to(root).as_posix()}: cannot parse: {error}")
            continue
        profiles = document.get("profile")
        if not isinstance(profiles, dict):
            continue
        for name, profile in profiles.items():
            if isinstance(profile, dict) and profile.get("panic") == "abort":
                problems.append(
                    f"{path.relative_to(root).as_posix()}: profile `{name}` sets"
                    " `panic = \"abort\"`, which disables every registered panic boundary"
                )
    return problems


def load_inventory(root: Path) -> tuple[dict[str, Boundary], list[str]]:
    path = root / INVENTORY
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return {}, [f"{INVENTORY}: cannot read the ledger: {error}"]
    if not isinstance(document, dict) or document.get("version") != 1:
        return {}, [f"{INVENTORY}: expected a version 1 object"]
    if document.get("categories") is not None and set(document["categories"]) != CATEGORIES:
        return {}, [f"{INVENTORY}: `categories` must list exactly {sorted(CATEGORIES)}"]

    rows = document.get("boundaries")
    if not isinstance(rows, list) or not rows:
        return {}, [f"{INVENTORY}: `boundaries` must be a non-empty array"]

    entries: dict[str, Boundary] = {}
    problems: list[str] = []
    for index, row in enumerate(rows):
        prefix = f"{INVENTORY}: boundaries[{index}]"
        if not isinstance(row, dict):
            problems.append(f"{prefix} must be an object")
            continue
        relative = row.get("path")
        sites = row.get("sites")
        category = row.get("category")
        rationale = row.get("rationale")
        if not isinstance(relative, str) or not relative:
            problems.append(f"{prefix}: path must be a non-empty string")
            continue
        if Path(relative).is_absolute() or ".." in Path(relative).parts:
            problems.append(f"{prefix}: path must stay inside the repository: {relative}")
            continue
        if relative in entries:
            problems.append(f"{prefix}: duplicate path `{relative}`")
        if not isinstance(sites, int) or sites <= 0:
            problems.append(f"{prefix}: sites must be a positive integer")
            continue
        if category not in CATEGORIES:
            problems.append(f"{prefix}: category must be one of {sorted(CATEGORIES)}")
        if not isinstance(rationale, str) or len(rationale.strip()) < 12:
            problems.append(f"{prefix}: rationale must name the failure being contained")
        entries[relative] = Boundary(relative, sites, str(category), str(rationale))
    return entries, problems


def check(root: Path = REPO_ROOT) -> list[str]:
    entries, problems = load_inventory(root)
    if not entries and problems:
        return problems

    scanned = scan(root)
    for relative in sorted(set(scanned) - set(entries)):
        problems.append(
            f"{relative}: {scanned[relative]} `catch_unwind` site(s) are not registered"
            f" in {INVENTORY}; register the failure boundary or remove the catch"
        )
    for relative in sorted(set(entries) - set(scanned)):
        problems.append(
            f"{relative}: registered in {INVENTORY} but no `catch_unwind` remains;"
            " delete the stale entry"
        )
    for relative in sorted(set(scanned) & set(entries)):
        if scanned[relative] != entries[relative].sites:
            problems.append(
                f"{relative}: {scanned[relative]} `catch_unwind` site(s), ledger records"
                f" {entries[relative].sites}"
            )
    return problems + aborting_profiles(root)


class PanicBoundarySelfTest(unittest.TestCase):
    def test_repository_boundaries_match_the_ledger(self):
        self.assertEqual(check(REPO_ROOT), [])

    def test_unregistered_catch_unwind_is_rejected(self):
        with _scratch() as root:
            (root / "crates/demo/src/extra.rs").write_text(
                "fn f() { let _ = std::panic::catch_unwind(|| {}); }\n", encoding="utf-8"
            )
            problems = check(root)
        self.assertTrue(any("not registered" in problem for problem in problems))

    def test_panic_abort_is_rejected(self):
        with _scratch() as root:
            (root / "crates/demo/Cargo.toml").write_text(
                '[package]\nname = "demo"\nversion = "0.1.0"\n\n'
                '[profile.release]\npanic = "abort"\n',
                encoding="utf-8",
            )
            problems = check(root)
        self.assertTrue(any("panic = \"abort\"" in problem for problem in problems))

    def test_ledger_count_must_match_the_source(self):
        with _scratch(sites=2) as root:
            problems = check(root)
        self.assertTrue(any("ledger records" in problem for problem in problems))


def _inventory(sites: int = 1) -> dict:
    return {
        "version": 1,
        "note": "fixture",
        "categories": sorted(CATEGORIES),
        "boundaries": [
            {
                "path": "crates/demo/src/lib.rs",
                "sites": sites,
                "category": "runtime_supervision",
                "rationale": "Fixture boundary for the guard self-test.",
            }
        ],
    }


class _scratch:
    def __init__(self, sites: int = 1):
        self._sites = sites

    def __enter__(self) -> Path:
        import tempfile

        self._temporary = tempfile.TemporaryDirectory()
        root = Path(self._temporary.name)
        (root / INVENTORY).parent.mkdir(parents=True, exist_ok=True)
        (root / INVENTORY).write_text(json.dumps(_inventory(self._sites)), encoding="utf-8")
        (root / "crates/demo/src").mkdir(parents=True)
        (root / "crates/demo/src/lib.rs").write_text(
            "fn f() { let _ = std::panic::catch_unwind(|| {}); }\n", encoding="utf-8"
        )
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
            PanicBoundarySelfTest
        )
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1

    problems = check()
    if problems:
        print("\n".join(problems), file=sys.stderr)
        return 1 if arguments.mode == "enforce" else 0
    entries, _ = load_inventory(REPO_ROOT)
    print(
        f"panic boundary guard: {len(entries)} registered boundaries,"
        f" {sum(entry.sites for entry in entries.values())} sites, no panic=abort"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
