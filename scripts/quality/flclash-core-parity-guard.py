#!/usr/bin/env python3
"""Guard the pinned FlClash CoreMethod audit ledger.

`docs/reference/flclash-core-methods.tsv` is the machine-checkable half of
the b146/b147 "求同存异" audit. Upstream v0.9.3 publishes 56 `CoreMethod`
constants in `core/protocol.go`; every one must appear exactly once in the
ledger with an explicit verdict and a named in-repo truth source. A method
without a verdict is a silent gap, and a method whose name drifts from the
pinned upstream list is an un-reviewed scope change.

The ledger header pins the upstream commit and the SHA-256 of the sorted
method-name list, so re-vendoring requires an explicit decision rather than
a quiet edit. This guard is offline: it reads the TSV only, never clones
upstream and never compiles the workspace.

Usage:
    python3 scripts/quality/flclash-core-parity-guard.py [--mode report|enforce]
    python3 scripts/quality/flclash-core-parity-guard.py --self-test

Exit status: 0 clean (or report mode), 1 findings in enforce mode, 2 unreadable.
"""

from __future__ import annotations

import argparse
import hashlib
import pathlib
import re
import sys
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
LEDGER = ROOT / "docs/reference/flclash-core-methods.tsv"

FIELDS = ("method", "verdict", "owner")
VERDICTS = frozenset(("same", "stronger", "local", "unsupported", "folded"))
METHOD_RE = re.compile(r"^[a-z][a-zA-Z]*\.[a-zA-Z][a-zA-Z]*$")
SHA_LINE = re.compile(r"^#\s+([0-9a-f]{64})\s*$", re.MULTILINE)


def rows_of(text: str) -> list[dict[str, str]]:
    rows: list[dict[str, str]] = []
    for raw in text.splitlines():
        if not raw or raw.lstrip().startswith("#"):
            continue
        if raw.split("\t")[0] == FIELDS[0]:
            # The ledger's own column header, not an upstream method.
            continue
        parts = raw.split("\t")
        if len(parts) != len(FIELDS):
            rows.append({"method": raw, "verdict": "", "owner": "",
                         "_malformed": "true"})
            continue
        rows.append(dict(zip(FIELDS, (part.strip() for part in parts))))
    return rows


def header_digest(text: str) -> str | None:
    match = SHA_LINE.search(text)
    return match.group(1) if match else None


def findings(text: str) -> list[str]:
    found: list[str] = []
    rows = rows_of(text)
    digest = header_digest(text)
    if digest is None:
        found.append("the ledger header does not pin the upstream method-list sha256")
    methods = [row["method"] for row in rows]
    computed = hashlib.sha256(
        ("\n".join(sorted(methods)) + "\n").encode("utf-8")).hexdigest()
    if digest is not None and computed != digest:
        found.append(
            f"the ledger's method list hashes to {computed}, not the pinned {digest}; "
            "re-vendor deliberately and re-adjudicate the new methods")
    if len(methods) != 56:
        found.append(f"the pinned upstream has 56 CoreMethods, the ledger has {len(methods)}")
    duplicates = sorted({name for name in methods if methods.count(name) > 1})
    if duplicates:
        found.append(f"duplicate methods in the ledger: {duplicates}")
    for row in rows:
        name = row["method"]
        if row.get("_malformed"):
            found.append(f"a ledger row is not three tab-separated fields: {name[:60]!r}")
            continue
        if not METHOD_RE.match(name):
            found.append(f"{name!r} is not an area.verb CoreMethod name")
        if row["verdict"] not in VERDICTS:
            found.append(
                f"{name}: verdict {row['verdict']!r} is not one of {sorted(VERDICTS)} - "
                "an unadjudicated upstream method is a silent gap")
        if not row["owner"]:
            found.append(f"{name}: no in-repo truth source named for its verdict")
    return found


def check() -> list[str]:
    if not LEDGER.is_file():
        return [f"{LEDGER.relative_to(ROOT)} is missing"]
    return findings(LEDGER.read_text(encoding="utf-8"))


class GuardSelfTest(unittest.TestCase):
    def setUp(self) -> None:
        self.text = LEDGER.read_text(encoding="utf-8")

    def test_live_ledger_is_clean(self) -> None:
        self.assertEqual(findings(self.text), [])

    def test_the_header_digest_is_the_sorted_method_list(self) -> None:
        methods = [row["method"] for row in rows_of(self.text)]
        digest = hashlib.sha256(
            ("\n".join(sorted(methods)) + "\n").encode("utf-8")).hexdigest()
        self.assertEqual(header_digest(self.text), digest)

    def test_an_unadjudicated_method_is_caught(self) -> None:
        planted = self.text.replace("\tstronger\t", "\tunknown\t", 1)
        self.assertTrue(any("unadjudicated" in item for item in findings(planted)))

    def test_a_dropped_method_is_caught(self) -> None:
        planted = "\n".join(line for line in self.text.splitlines()
                            if not line.startswith("traffic.total\t"))
        found = findings(planted)
        self.assertTrue(any("56 CoreMethods" in item for item in found), found)
        self.assertTrue(any("hashes to" in item for item in found), found)

    def test_a_new_upstream_method_is_caught(self) -> None:
        planted = self.text + "core.newMethod\tstronger\tunknown owner\n"
        found = findings(planted)
        self.assertTrue(any("56 CoreMethods" in item for item in found), found)
        self.assertTrue(any("hashes to" in item for item in found), found)

    def test_a_missing_digest_is_caught(self) -> None:
        planted = "\n".join(line for line in self.text.splitlines()
                            if not re.match(r"^#\s+[0-9a-f]{64}\s*$", line))
        self.assertTrue(any("does not pin" in item for item in findings(planted)))

    def test_a_malformed_row_is_caught(self) -> None:
        planted = self.text.replace("\t", " ", 1)
        self.assertTrue(any("three tab-separated" in item for item in findings(planted)))

    def test_the_guard_is_part_of_the_structure_gate(self) -> None:
        # A guard nobody runs is decorative. Both legs must be inside the one
        # structure gate the CI invokes.
        gate = (ROOT / "scripts/quality/check-structure.sh").read_text(encoding="utf-8")
        self.assertIn("flclash-core-parity-guard.py --self-test", gate)
        self.assertIn("flclash-core-parity-guard.py --mode enforce", gate)


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
        print(f"flclash core parity guard: {problem}", file=sys.stderr)
    if problems and args.mode == "enforce":
        return 1
    print(f"flclash core parity guard: {len(rows_of(LEDGER.read_text(encoding='utf-8')))} "
          f"upstream methods adjudicated, findings={len(problems)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
