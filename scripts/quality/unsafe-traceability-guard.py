#!/usr/bin/env python3
"""Keep every unsafe operation in the repository accounted for.

Unsafe code is legitimate here — libc file locks, Windows APIs, JNI and
volatile secret erasure have no safe substitute — but it must never appear
silently. `unsafe_inventory.json` is a ledger with two guarantees:

  * `operations` matches the source exactly, so adding or removing an unsafe
    operation is always a deliberate, reviewable ledger edit;
  * `undocumented` may only fall, so the `SAFETY:` debt is a ratchet. New
    unsafe code must carry its own justification at the call site.

Vendored reference sources are outside the scan; they are not product code.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import json
import os
from pathlib import Path
import re
import sys
import unittest

QUALITY_ROOT = Path(__file__).resolve().parent
REPO_ROOT = QUALITY_ROOT.parents[1]
INVENTORY = Path("scripts/quality/unsafe_inventory.json")

sys.path.insert(0, str(QUALITY_ROOT))
import rust_syntax  # noqa: E402  (path bootstrap above is the import contract)

UNSAFE_OPERATION = re.compile(r"\bunsafe\s*(?:\{|fn\b|impl\b|trait\b)")
SAFETY_NOTE = re.compile(r"\bSAFETY\b")
CATEGORIES = {"ffi", "jni", "env", "zeroize", "test_support"}
SKIPPED_DIRECTORIES = {".git", "target", "node_modules", "dist", "build"}


@dataclass(frozen=True)
class ScannedFile:
    operations: int
    undocumented: int


def scan(root: Path, scan_roots: list[str], excluded: list[str]) -> dict[str, ScannedFile]:
    """Count unsafe operations and the undocumented remainder per file."""
    excluded_paths = [Path(item) for item in excluded]
    found: dict[str, ScannedFile] = {}
    for scan_root in scan_roots:
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
                if any(
                    relative == excluded_path or relative.is_relative_to(excluded_path)
                    for excluded_path in excluded_paths
                ):
                    continue
                text = path.read_text(encoding="utf-8", errors="replace")
                operations = len(UNSAFE_OPERATION.findall(rust_syntax.mask_noncode(text)))
                if operations == 0:
                    continue
                documented = min(operations, len(SAFETY_NOTE.findall(text)))
                found[relative.as_posix()] = ScannedFile(
                    operations=operations,
                    undocumented=operations - documented,
                )
    return found


def load_inventory(root: Path) -> tuple[dict[str, dict], list[str]]:
    path = root / INVENTORY
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return {}, [f"{INVENTORY}: cannot read the ledger: {error}"]
    if not isinstance(document, dict) or document.get("version") != 1:
        return {}, [f"{INVENTORY}: expected a version 1 object"]

    problems: list[str] = []
    rows = document.get("unsafe_files")
    if not isinstance(rows, list) or not rows:
        return {}, [f"{INVENTORY}: `unsafe_files` must be a non-empty array"]
    if document.get("categories") is not None and set(document["categories"]) != CATEGORIES:
        problems.append(
            f"{INVENTORY}: `categories` must list exactly {sorted(CATEGORIES)}"
        )

    entries: dict[str, dict] = {}
    for index, row in enumerate(rows):
        prefix = f"{INVENTORY}: unsafe_files[{index}]"
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
        operations = row.get("operations")
        max_undocumented = row.get("max_undocumented")
        if not isinstance(operations, int) or operations <= 0:
            problems.append(f"{prefix}: operations must be a positive integer")
            continue
        if not isinstance(max_undocumented, int) or not 0 <= max_undocumented <= operations:
            problems.append(f"{prefix}: max_undocumented must be between 0 and operations")
            continue
        if row.get("category") not in CATEGORIES:
            problems.append(f"{prefix}: category must be one of {sorted(CATEGORIES)}")
        if not isinstance(row.get("rationale"), str) or len(row["rationale"].strip()) < 12:
            problems.append(f"{prefix}: rationale must explain why the unsafe code exists")
        entries[relative] = row
    return entries, problems


def check(root: Path = REPO_ROOT) -> list[str]:
    entries, problems = load_inventory(root)
    if not entries and problems:
        return problems
    document = json.loads((root / INVENTORY).read_text(encoding="utf-8"))
    scanned = scan(
        root,
        document.get("scan_roots") or [],
        document.get("excluded_roots") or [],
    )

    for relative in sorted(set(scanned) - set(entries)):
        problems.append(
            f"{relative}: {scanned[relative].operations} unsafe operation(s) are not"
            f" registered in {INVENTORY}; add the entry and a `SAFETY:` note"
        )
    for relative in sorted(set(entries) - set(scanned)):
        problems.append(
            f"{relative}: registered in {INVENTORY} but no unsafe operation remains;"
            " delete the stale entry"
        )
    for relative in sorted(set(scanned) & set(entries)):
        actual = scanned[relative]
        registered = entries[relative]
        if actual.operations != registered["operations"]:
            problems.append(
                f"{relative}: {actual.operations} unsafe operation(s), ledger records"
                f" {registered['operations']}; update the ledger deliberately"
            )
        if actual.undocumented > registered["max_undocumented"]:
            problems.append(
                f"{relative}: {actual.undocumented} unsafe operation(s) without a"
                f" `SAFETY:` note, ceiling is {registered['max_undocumented']};"
                " document the new operation instead of raising the ceiling"
            )
    return problems


class UnsafeTraceabilitySelfTest(unittest.TestCase):
    def test_repository_ledger_matches_the_source(self):
        self.assertEqual(check(REPO_ROOT), [])

    def test_new_unsafe_operation_without_ledger_entry_is_rejected(self):
        ledger = _inventory(path="crates/demo/src/other.rs")
        with _scratch(ledger, create_source=False) as root:
            (root / "crates/demo/src/lib.rs").write_text(
                "pub fn f() { unsafe { libc::close(1) }; }\n", encoding="utf-8"
            )
            problems = check(root)
        self.assertTrue(any("not registered" in problem for problem in problems))

    def test_raising_the_documented_ceiling_requires_a_safety_note(self):
        with _scratch(_inventory(operations=2, max_undocumented=1)) as root:
            (root / "crates/demo/src/lib.rs").write_text(
                "pub fn f() {\n    unsafe { libc::close(1) };\n    unsafe { libc::close(2) };\n}\n",
                encoding="utf-8",
            )
            problems = check(root)
        self.assertTrue(any("ceiling" in problem for problem in problems))

    def test_safety_note_lowers_the_undocumented_count(self):
        with _scratch(_inventory(operations=2, max_undocumented=0)) as root:
            (root / "crates/demo/src/lib.rs").write_text(
                "// SAFETY: first closed descriptor.\n"
                "// SAFETY: second closed descriptor.\n"
                "pub fn f() {\n    unsafe { libc::close(1) };\n    unsafe { libc::close(2) };\n}\n",
                encoding="utf-8",
            )
            problems = check(root)
        self.assertEqual(problems, [])


def _inventory(
    operations: int = 1,
    max_undocumented: int = 1,
    path: str = "crates/demo/src/lib.rs",
) -> dict:
    return {
        "version": 1,
        "note": "fixture",
        "categories": sorted(CATEGORIES),
        "scan_roots": ["crates"],
        "excluded_roots": [],
        "unsafe_files": [
            {
                "path": path,
                "operations": operations,
                "max_undocumented": max_undocumented,
                "category": "ffi",
                "rationale": "Fixture entry for the guard self-test.",
            }
        ],
    }


class _scratch:
    def __init__(self, inventory: dict, create_source: bool = True):
        self._inventory = inventory
        self._create_source = create_source

    def __enter__(self) -> Path:
        import tempfile

        self._temporary = tempfile.TemporaryDirectory()
        root = Path(self._temporary.name)
        (root / INVENTORY).parent.mkdir(parents=True, exist_ok=True)
        (root / INVENTORY).write_text(json.dumps(self._inventory), encoding="utf-8")
        (root / "crates/demo/src").mkdir(parents=True)
        if self._create_source:
            (root / "crates/demo/src/lib.rs").write_text(
                "pub fn f() { unsafe { libc::close(1) }; }\n", encoding="utf-8"
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
            UnsafeTraceabilitySelfTest
        )
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1

    problems = check()
    if problems:
        print("\n".join(problems), file=sys.stderr)
        return 1 if arguments.mode == "enforce" else 0
    entries, _ = load_inventory(REPO_ROOT)
    operations = sum(entry["operations"] for entry in entries.values())
    undocumented = sum(entry["max_undocumented"] for entry in entries.values())
    print(
        f"unsafe traceability guard: {len(entries)} files, {operations} operations,"
        f" {undocumented} documented-debt ceiling"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
