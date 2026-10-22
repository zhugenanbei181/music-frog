#!/usr/bin/env python3
"""Keep stdout/stderr writes in library crates accounted for.

A library that prints has invented an output channel outside its typed
diagnostics: the text is invisible to the caller, unfilterable and untested.
Two rules apply:

  * the inner layers (`infiltrator-contract`, `infiltrator-domain`,
    `infiltrator-ports`, `infiltrator-application`, `infiltrator-presentation`)
    keep a hard zero, because a diagnostic there must travel as a typed value;
  * every other library site is registered in `stdout_inventory.json` with a
    ceiling, so a new print is a deliberate, reviewable edit.

Binary entry points (`src/main.rs`, `src/bin/*.rs`) and the `infiltrator-cli`
crate own their output as the product surface; they are not registered.
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
INVENTORY = Path("scripts/quality/stdout_inventory.json")

sys.path.insert(0, str(QUALITY_ROOT))
import rust_syntax  # noqa: E402  (path bootstrap above is the import contract)

PRINT_MACRO = re.compile(r"\b(?:print|println|eprint|eprintln)\s*!\s*\(")
CATEGORIES = {
    "capture_tooling",
    "crash_diagnostic",
    "platform_diagnostic",
    "test_support",
}
SKIPPED_DIRECTORIES = {".git", "target", "node_modules", "dist", "build"}
SCAN_ROOTS = ("crates", "android", "examples", "templates", "sandbox")
EXCLUDED_ROOTS = (Path("crates/infiltrator-slint/lumen-source"),)
# Crates whose output is the product surface, not a library diagnostic.
OUTPUT_OWNERS = ("crates/infiltrator-cli/",)


@dataclass(frozen=True)
class PrintSite:
    path: str
    sites: int
    category: str
    rationale: str


def _is_binary_entry(path: Path) -> bool:
    return path.name == "main.rs" or "bin" in path.parts


def scan(root: Path) -> dict[str, int]:
    """Count production print macros per library source file."""
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
                posix = relative.as_posix()
                if "/src/" not in f"/{posix}":
                    continue
                if _is_binary_entry(relative):
                    continue
                if any(posix.startswith(prefix) for prefix in OUTPUT_OWNERS):
                    continue
                if any(relative.is_relative_to(item) for item in EXCLUDED_ROOTS):
                    continue
                text = path.read_text(encoding="utf-8", errors="replace")
                production = rust_syntax.mask_test_items(rust_syntax.mask_comments(text))
                sites = len(PRINT_MACRO.findall(production))
                if sites:
                    found[posix] = sites
    return found


def load_inventory(root: Path) -> tuple[dict[str, PrintSite], list[str]]:
    path = root / INVENTORY
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return {}, [f"{INVENTORY}: cannot read the ledger: {error}"]
    if not isinstance(document, dict) or document.get("version") != 1:
        return {}, [f"{INVENTORY}: expected a version 1 object"]
    if document.get("categories") is not None and set(document["categories"]) != CATEGORIES:
        return {}, [f"{INVENTORY}: `categories` must list exactly {sorted(CATEGORIES)}"]

    hard_zero = document.get("hard_zero_crates")
    if not isinstance(hard_zero, list) or not hard_zero:
        return {}, [f"{INVENTORY}: `hard_zero_crates` must be a non-empty array"]
    for crate in hard_zero:
        if not isinstance(crate, str) or not (root / "crates" / crate).is_dir():
            return {}, [f"{INVENTORY}: hard-zero crate is missing: crates/{crate}"]

    rows = document.get("files")
    if not isinstance(rows, list) or not rows:
        return {}, [f"{INVENTORY}: `files` must be a non-empty array"]

    entries: dict[str, PrintSite] = {}
    problems: list[str] = []
    for index, row in enumerate(rows):
        prefix = f"{INVENTORY}: files[{index}]"
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
        if _crate_of(relative) in hard_zero:
            problems.append(
                f"{prefix}: `{relative}` is inside a hard-zero crate and cannot be registered"
            )
        if not isinstance(sites, int) or sites <= 0:
            problems.append(f"{prefix}: sites must be a positive integer")
            continue
        if category not in CATEGORIES:
            problems.append(f"{prefix}: category must be one of {sorted(CATEGORIES)}")
        if not isinstance(rationale, str) or len(rationale.strip()) < 12:
            problems.append(f"{prefix}: rationale must explain why the output exists")
        entries[relative] = PrintSite(relative, sites, str(category), str(rationale))
    return entries, problems


def _crate_of(posix: str) -> str:
    parts = Path(posix).parts
    return parts[1] if len(parts) > 2 and parts[0] == "crates" else ""


def check(root: Path = REPO_ROOT) -> list[str]:
    entries, problems = load_inventory(root)
    if not entries and problems:
        return problems
    document = json.loads((root / INVENTORY).read_text(encoding="utf-8"))
    hard_zero = set(document.get("hard_zero_crates") or [])
    scanned = scan(root)

    for posix, sites in sorted(scanned.items()):
        crate = _crate_of(posix)
        if crate in hard_zero:
            problems.append(
                f"{posix}:{sites} print macro(s) in `{crate}`, which keeps a hard zero;"
                " return a typed diagnostic instead of writing to stdout/stderr"
            )

    for posix in sorted(set(scanned) - set(entries)):
        if _crate_of(posix) in hard_zero:
            continue
        problems.append(
            f"{posix}: {scanned[posix]} print macro(s) are not registered in"
            f" {INVENTORY}; give the diagnostic a typed owner or register the site"
        )
    for posix in sorted(set(entries) - set(scanned)):
        problems.append(
            f"{posix}: registered in {INVENTORY} but no print macro remains;"
            " delete the stale entry"
        )
    for posix in sorted(set(entries) & set(scanned)):
        if scanned[posix] > entries[posix].sites:
            problems.append(
                f"{posix}: {scanned[posix]} print macro(s), ledger allows"
                f" {entries[posix].sites}"
            )
    return problems


class StdoutDisciplineSelfTest(unittest.TestCase):
    def test_repository_print_sites_match_the_ledger(self):
        self.assertEqual(check(REPO_ROOT), [])

    def test_inner_layer_print_is_rejected_even_without_a_ledger_entry(self):
        with _scratch() as root:
            (root / "crates/infiltrator-domain/src/leak.rs").write_text(
                "pub fn f() { println!(\"domain leak\"); }\n", encoding="utf-8"
            )
            problems = check(root)
        self.assertTrue(any("hard zero" in problem for problem in problems))

    def test_unregistered_library_print_is_rejected(self):
        with _scratch() as root:
            (root / "crates/infiltrator-beacon/src/extra.rs").write_text(
                "pub fn f() { eprintln!(\"beacon\"); }\n", encoding="utf-8"
            )
            problems = check(root)
        self.assertTrue(any("not registered" in problem for problem in problems))

    def test_binary_entry_point_and_cli_output_are_not_registered(self):
        with _scratch() as root:
            (root / "crates/infiltrator-cli/src").mkdir(parents=True)
            (root / "crates/infiltrator-beacon/src/main.rs").write_text(
                "fn main() { println!(\"ok\"); }\n", encoding="utf-8"
            )
            (root / "crates/infiltrator-cli/src/output.rs").write_text(
                "pub fn print() { println!(\"ok\"); }\n", encoding="utf-8"
            )
            problems = check(root)
        self.assertEqual(problems, [])


def _inventory(sites: int = 1) -> dict:
    return {
        "version": 1,
        "note": "fixture",
        "categories": sorted(CATEGORIES),
        "hard_zero_crates": ["infiltrator-domain"],
        "files": [
            {
                "path": "crates/infiltrator-beacon/src/lib.rs",
                "sites": sites,
                "category": "platform_diagnostic",
                "rationale": "Fixture entry for the guard self-test.",
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
        (root / "crates/infiltrator-domain/src").mkdir(parents=True)
        (root / "crates/infiltrator-beacon/src").mkdir(parents=True)
        (root / "crates/infiltrator-beacon/src/lib.rs").write_text(
            "pub fn f() { eprintln!(\"beacon\"); }\n", encoding="utf-8"
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
            StdoutDisciplineSelfTest
        )
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1

    problems = check()
    if problems:
        print("\n".join(problems), file=sys.stderr)
        return 1 if arguments.mode == "enforce" else 0
    entries, _ = load_inventory(REPO_ROOT)
    print(
        f"stdout discipline guard: {len(entries)} registered library sites,"
        f" {sum(entry.sites for entry in entries.values())} print macros,"
        " inner layers at zero"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
