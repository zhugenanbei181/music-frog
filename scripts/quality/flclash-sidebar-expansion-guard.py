#!/usr/bin/env python3
"""Guard FlClash's pinned desktop sidebar-expansion comparison.

The reference's collapse toggle is owned by a persisted app setting, gated by
the desktop view mode, and projected into 48px/220px sidebars. This offline
guard pins the four source files and checks that the in-repo contract owns one
typed preference, capability, resolver and intent path instead of delegating
the decision to a UI boolean.
"""

from __future__ import annotations

import argparse
import hashlib
import pathlib
import re
import sys
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
LEDGER = ROOT / "docs/reference/flclash-sidebar-expansion.tsv"
CONTRACT = ROOT / "crates/infiltrator-contract/src/shell_sidebar_preference.rs"
LIB = ROOT / "crates/infiltrator-contract/src/lib.rs"
GATE = ROOT / "scripts/quality/check-structure.sh"

COMMIT = "be84736605527034689c95bf27828ff27d912c5e"
FIELDS = ("file", "sha256", "fact", "owner")
COMMIT_RE = re.compile(r"^#\s*upstream:.*@\s*([0-9a-f]{40})\s*$", re.MULTILINE)
DIGEST_RE = re.compile(r"^#\s*([0-9a-f]{64})\s*$", re.MULTILINE)
OWNER_RE = re.compile(
    r"infiltrator-contract::shell_sidebar_preference::([A-Za-z0-9_:]+)"
)
REQUIRED_SYMBOLS = (
    "SIDEBAR_EXPANDED_SETTING_KEY",
    "SIDEBAR_EXPANDED_LEGACY_KEY",
    "enum SidebarExpansionPreference",
    "enum SidebarExpansionCapability",
    "enum SidebarExpansionPresentation",
    "enum SidebarExpansionIntent",
    "fn resolve_sidebar_expansion",
    "fn apply_sidebar_expansion_intent",
    "fn toggled",
)
EXPECTED_ROWS = (
    (
        "lib/common/layout.dart",
        "70317b4ff2f9c9c5cf1b8cc7bca2b5334a303552a77b9c13f8dd78141064fb8f",
        "canExpandSidebar(viewMode)==desktop",
        "infiltrator-contract::shell_sidebar_preference::SidebarExpansionCapability",
    ),
    (
        "lib/manager/app_manager.dart",
        "4bb7d7afdcc0e95907db944ab848778bf2ec11b1eb8db1a71d4550a85a5b1d02",
        "expanded=canExpand&&sidebarExpanded; toggle persists !sidebarExpanded",
        "infiltrator-contract::shell_sidebar_preference::resolve_sidebar_expansion",
    ),
    (
        "lib/models/config.dart",
        "0c97916cbb78687fd62a712469b0d614aedb0675c6f631a8da18f32de66564a0",
        "sidebarExpanded default true; legacy showLabel migration",
        "infiltrator-contract::shell_sidebar_preference::SIDEBAR_EXPANDED_SETTING_KEY",
    ),
    (
        "test/widgets/sidebar_test.dart",
        "0a9cbd924507c22566a5e40ae9f98706107df177c63fd98029a68a1145a57fab",
        "collapsed width 48; expanded width 220; toggle changes glyph and labels",
        "infiltrator-contract::responsive_viewport::SidebarForm::width_px",
    ),
)


def rows_of(text: str) -> list[tuple[str, ...]]:
    rows: list[tuple[str, ...]] = []
    for raw in text.splitlines():
        if not raw or raw.lstrip().startswith("#") or raw.split("\t", 1)[0] == FIELDS[0]:
            continue
        rows.append(tuple(raw.split("\t")))
    return rows


def findings(text: str, contract: str, lib: str, gate: str) -> list[str]:
    found: list[str] = []
    commit = COMMIT_RE.search(text)
    if commit is None or commit.group(1) != COMMIT:
        found.append("the ledger does not pin FlClash v0.9.3")

    rows = rows_of(text)
    for index, row in enumerate(rows):
        if len(row) != len(FIELDS):
            found.append(f"row {index + 1} is not four tab-separated fields")
    if len(rows) == len(EXPECTED_ROWS):
        for index, (row, expected) in enumerate(zip(rows, EXPECTED_ROWS), 1):
            if row != expected:
                found.append(f"row {index} drifted from the pinned upstream source facts")

    digests = [row[1] for row in rows if len(row) == len(FIELDS)]
    combined = hashlib.sha256(("\n".join(digests) + "\n").encode("utf-8")).hexdigest()
    header = DIGEST_RE.search(text)
    if header is None:
        found.append("the ledger header does not pin the combined source sha256")
    elif header.group(1) != combined:
        found.append(
            f"the source hashes combine to {combined}, not the pinned {header.group(1)}"
        )

    owners = [row[3] for row in rows if len(row) == len(FIELDS)]
    for owner in owners:
        if owner.startswith("unknown") or not owner:
            found.append(f"an unadjudicated in-repo owner remains: {owner!r}")
        for symbol in OWNER_RE.findall(owner):
            if symbol not in contract and symbol not in lib:
                found.append(f"the named contract owner is missing: {symbol}")

    for symbol in REQUIRED_SYMBOLS:
        if symbol not in contract:
            found.append(f"the typed contract is missing {symbol}")
    if "pub mod shell_sidebar_preference;" not in lib:
        found.append("the shell sidebar preference module is not exported")
    if "fn toggled(self)" not in contract:
        found.append("the preference toggle no longer has a typed transition")
    if "Tier(ViewportTier)" not in contract or "Fixed(SidebarForm)" not in contract:
        found.append("the capability no longer accepts both a tier and a resolved form")
    if "form: SidebarForm::Rail" not in contract:
        found.append("collapsing no longer resolves to the shared rail form")
    if "flclash-sidebar-expansion-guard.py --self-test" not in gate:
        found.append("the guard self-test is not part of the structure gate")
    if "flclash-sidebar-expansion-guard.py --mode enforce" not in gate:
        found.append("the guard enforce leg is not part of the structure gate")
    return found


def check() -> list[str]:
    if not LEDGER.is_file():
        return [f"{LEDGER.relative_to(ROOT)} is missing"]
    if not CONTRACT.is_file():
        return [f"{CONTRACT.relative_to(ROOT)} is missing"]
    return findings(
        LEDGER.read_text(encoding="utf-8"),
        CONTRACT.read_text(encoding="utf-8"),
        LIB.read_text(encoding="utf-8"),
        GATE.read_text(encoding="utf-8"),
    )


class GuardSelfTest(unittest.TestCase):
    def setUp(self) -> None:
        self.text = LEDGER.read_text(encoding="utf-8")
        self.contract = CONTRACT.read_text(encoding="utf-8")
        self.lib = LIB.read_text(encoding="utf-8")
        self.gate = GATE.read_text(encoding="utf-8")

    def found(self, **changes: str) -> list[str]:
        return findings(
            changes.get("text", self.text),
            changes.get("contract", self.contract),
            changes.get("lib", self.lib),
            changes.get("gate", self.gate),
        )

    def test_live_tree_is_clean(self) -> None:
        self.assertEqual(self.found(), [])

    def test_the_commit_is_pinned(self) -> None:
        planted = self.text.replace(COMMIT, "0" * 40, 1)
        self.assertTrue(any("does not pin FlClash" in item for item in self.found(text=planted)))

    def test_a_drifted_hash_is_caught(self) -> None:
        planted = self.text.replace(EXPECTED_ROWS[0][1], "0" * 64, 1)
        result = self.found(text=planted)
        self.assertTrue(any("drifted from the pinned" in item for item in result), result)
        self.assertTrue(any("combine to" in item for item in result), result)

    def test_a_missing_typed_symbol_is_caught(self) -> None:
        planted = self.contract.replace("enum SidebarExpansionIntent", "struct SidebarExpansionIntent", 1)
        self.assertTrue(any("missing enum SidebarExpansionIntent" in item for item in self.found(contract=planted)))

    def test_an_unexported_module_is_caught(self) -> None:
        planted = self.lib.replace("pub mod shell_sidebar_preference;\n", "", 1)
        self.assertTrue(any("not exported" in item for item in self.found(lib=planted)))

    def test_a_missing_gate_leg_is_caught(self) -> None:
        planted = self.gate.replace("flclash-sidebar-expansion-guard.py --mode enforce\n", "", 1)
        self.assertTrue(any("enforce leg" in item for item in self.found(gate=planted)))

    def test_a_malformed_row_is_caught(self) -> None:
        planted = self.text.replace("\t", " ", 1)
        self.assertTrue(any("four tab-separated" in item for item in self.found(text=planted)))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("report", "enforce"), default="enforce")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(GuardSelfTest)
        result = unittest.TextTestRunner(verbosity=1).run(suite)
        return 0 if result.wasSuccessful() else 1
    problems = check()
    for problem in problems:
        print(f"flclash sidebar expansion guard: {problem}", file=sys.stderr)
    if problems and args.mode == "enforce":
        return 1
    print(
        f"flclash sidebar expansion guard: {len(rows_of(LEDGER.read_text(encoding='utf-8')))} "
        f"upstream source facts adjudicated, findings={len(problems)}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
