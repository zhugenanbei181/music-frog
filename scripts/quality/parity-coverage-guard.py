#!/usr/bin/env python3
"""Guard the business × surface × level parity coverage matrix.

`crates/infiltrator-contract/src/parity.rs` is the authoritative business
catalogue. `scripts/parity/feature_evidence.tsv` (business × surface × level)
and `scripts/parity/cross_surface_manifest.tsv` (business × surface) must
assign every business to every product surface (Iced, Bevy, Slint).

This guard states the full expected coverage and fails when
  * a cell is missing (a business is not assigned to a surface),
  * a cell is duplicated,
  * a cell names a surface outside the three product surfaces,
  * an `anchored` cell carries no test anchor,
  * an `anchored` Slint scenario claims a business without a Slint carrier page,
  * a `pending` cell carries no reason,
  * a cell names a business outside the catalogue,
  * a platform leg of the five-platform matrix is not registered.

The catalogue is parsed from the Rust source, so the guard stays offline and
fast; it never compiles the workspace and never trusts a hand-written copy of
the business list.

Usage:
    python3 scripts/quality/parity-coverage-guard.py [--mode report|enforce]
    python3 scripts/quality/parity-coverage-guard.py --self-test

Exit status is 0 when the matrix has no hole (or in report mode), 1 when the
matrix has holes in enforce mode, and 2 when a registry cannot be read.
"""

from __future__ import annotations

import argparse
import csv
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CATALOGUE = Path("crates/infiltrator-contract/src/parity.rs")
EVIDENCE = Path("scripts/parity/feature_evidence.tsv")
MANIFEST = Path("scripts/parity/cross_surface_manifest.tsv")
PLATFORM_MATRIX = Path("docs/PLATFORM_MATRIX.md")

SURFACES = ("iced", "bevy", "slint")
LEVELS = ("contract", "scenario")
EVIDENCE_FIELDS = ("feature_id", "surface", "level", "status", "test_id", "paths", "reason")
MANIFEST_FIELDS = ("feature_id", "surface", "status", "reason", "standard_scenario", "compact_scenario")
EVIDENCE_STATUSES = ("anchored", "pending")
MANIFEST_STATUSES = ("pending", "implemented", "ready")

# The five platform legs of the coverage matrix. The business catalogue is
# platform independent, so every leg inherits the cells of the product
# surfaces it ships; `docs/PLATFORM_MATRIX.md` stays the only authority for
# the platform conclusions and must register every leg.
PLATFORM_LEGS: dict[str, tuple[str, ...]] = {
    "linux": ("iced", "bevy", "slint"),
    "windows": ("iced", "bevy", "slint"),
    "macos": ("iced", "bevy"),
    "android": ("bevy", "slint"),
    "ios": (),
}

CATALOGUE_ENTRY = re.compile(
    r'=>\s*\(\s*"(?P<id>[^"]*)"\s*,\s*"(?P<iced>[^"]*)"\s*,\s*"(?P<bevy>[^"]*)"\s*,\s*'
    r'"(?P<slint>[^"]*)"\s*,\s*\[(?P<paths>[^\]]*)\]\s*\)'
)
CATALOGUE_PATH = re.compile(r'"([^"]*)"')


@dataclass
class Report:
    """The audited coverage state of the parity registries."""

    catalogue: dict[str, dict[str, object]] = field(default_factory=dict)
    cells: dict[tuple[str, str, str], dict[str, str]] = field(default_factory=dict)
    manifest: dict[tuple[str, str], dict[str, str]] = field(default_factory=dict)
    coverage: dict[tuple[str, str], dict[str, object]] = field(default_factory=dict)
    platform_coverage: dict[str, dict[str, object]] = field(default_factory=dict)
    manifest_status: dict[str, int] = field(default_factory=dict)
    problems: list[str] = field(default_factory=list)

    @property
    def total_cells(self) -> int:
        return len(self.catalogue) * len(SURFACES) * len(LEVELS)


def parse_catalogue(text: str) -> dict[str, dict[str, object]]:
    """Parse the `features! { … }` catalogue without compiling the crate."""
    start = text.find("features! {")
    if start < 0:
        raise ValueError("the catalogue macro invocation is missing")
    end = text.find("\n}", start)
    if end < 0:
        raise ValueError("the catalogue macro block is not closed")
    block = text[start:end]
    entries = list(CATALOGUE_ENTRY.finditer(block))
    if block.count("=>") != len(entries):
        raise ValueError("the catalogue macro shape changed; update the guard parser")
    catalogue: dict[str, dict[str, object]] = {}
    for entry in entries:
        feature = entry.group("id")
        if not feature:
            raise ValueError("a catalogue entry carries an empty business id")
        if feature in catalogue:
            raise ValueError(f"duplicate catalogue business: {feature}")
        catalogue[feature] = {
            "iced_page": entry.group("iced"),
            "bevy_page": entry.group("bevy"),
            "slint_page": entry.group("slint"),
            "required_paths": CATALOGUE_PATH.findall(entry.group("paths")),
        }
    if not catalogue:
        raise ValueError("the catalogue registers no business")
    return catalogue


def read_table(path: Path, fields: tuple[str, ...]) -> list[dict[str, str]]:
    """Read a tab separated registry and reject any structural drift."""
    with path.open(encoding="utf-8", newline="") as handle:
        reader = csv.DictReader(handle, delimiter="\t")
        if tuple(reader.fieldnames or ()) != fields:
            raise ValueError(f"{path}: expected columns {fields}")
        rows = list(reader)
    if not rows:
        raise ValueError(f"{path}: the registry is empty")
    for row in rows:
        if None in row or any(value is None for value in row.values()):
            raise ValueError(f"{path}: a row does not carry every column")
    return rows


def platform_keys(text: str) -> set[str]:
    """The platform legs registered by the platform matrix table."""
    keys: set[str] = set()
    for line in text.splitlines():
        if not line.startswith("|"):
            continue
        cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
        if len(cells) < 2:
            continue
        token = re.split(r"[^a-z0-9]+", cells[0].lower())[0]
        if token:
            keys.add(token)
    return keys


def _blank(value: str) -> bool:
    """An empty field and the `-` placeholder both carry no information."""
    return not value.strip() or value.strip() == "-"


def evaluate(
    catalogue: dict[str, dict[str, object]],
    evidence_rows: list[dict[str, str]],
    manifest_rows: list[dict[str, str]],
    platform_legs: dict[str, tuple[str, ...]] = PLATFORM_LEGS,
    registered_platforms: set[str] | None = None,
) -> Report:
    """Cross-check both registries against the catalogue and the platform legs."""
    report = Report(catalogue=catalogue)
    problems = report.problems

    for row in manifest_rows:
        feature, surface = row["feature_id"], row["surface"]
        key = (feature, surface)
        if key in report.manifest:
            problems.append(f"duplicated manifest cell: {feature}/{surface}")
        report.manifest[key] = row
        if feature not in catalogue:
            problems.append(f"unknown business in manifest cell: {feature}/{surface}")
        if surface not in SURFACES:
            problems.append(f"unknown surface in manifest cell: {feature}/{surface}")
        if row["status"] not in MANIFEST_STATUSES:
            problems.append(f"invalid status in manifest cell: {feature}/{surface}: {row['status']}")
        if row["status"] == "pending" and _blank(row["reason"]):
            problems.append(f"pending manifest cell without a reason: {feature}/{surface}")
        if surface == "slint" and row["status"] in ("implemented", "ready") \
                and feature in catalogue and not catalogue[feature]["slint_page"]:
            problems.append(f"slint manifest cell without a carrier page: {feature}/{surface}")

    for row in evidence_rows:
        feature, surface, level = row["feature_id"], row["surface"], row["level"]
        key = (feature, surface, level)
        if key in report.cells:
            problems.append(f"duplicated evidence cell: {feature}/{surface}/{level}")
        report.cells[key] = row
        if feature not in catalogue:
            problems.append(f"unknown business in evidence cell: {feature}/{surface}/{level}")
        if surface not in SURFACES:
            problems.append(f"unknown surface in evidence cell: {feature}/{surface}/{level}")
        if level not in LEVELS:
            problems.append(f"unknown level in evidence cell: {feature}/{surface}/{level}")
        if row["status"] not in EVIDENCE_STATUSES:
            problems.append(
                f"invalid status in evidence cell: {feature}/{surface}/{level}: {row['status']}")
        if row["status"] == "anchored" and _blank(row["test_id"]):
            problems.append(f"anchored evidence cell without a test anchor: {feature}/{surface}/{level}")
        if row["status"] == "pending" and _blank(row["reason"]):
            problems.append(f"pending evidence cell without a reason: {feature}/{surface}/{level}")
        if surface == "slint" and level == "scenario" and row["status"] == "anchored" \
                and feature in catalogue and not catalogue[feature]["slint_page"]:
            problems.append(f"slint scenario anchor without a carrier page: {feature}/{surface}/{level}")
        if (feature, surface) not in report.manifest:
            problems.append(f"evidence cell without a manifest cell: {feature}/{surface}/{level}")

    for feature in catalogue:
        for surface in SURFACES:
            for level in LEVELS:
                if (feature, surface, level) not in report.cells:
                    problems.append(f"missing evidence cell: {feature}/{surface}/{level}")
            if (feature, surface) not in report.manifest:
                problems.append(f"missing manifest cell: {feature}/{surface}")

    for (feature, surface), row in sorted(report.manifest.items()):
        if row["status"] != "ready":
            continue
        for level in LEVELS:
            cell = report.cells.get((feature, surface, level))
            if cell is None or cell["status"] != "anchored":
                problems.append(
                    f"ready manifest cell without anchored {level} evidence: {feature}/{surface}")

    for surface in SURFACES:
        for level in LEVELS:
            anchored = pending = invalid = 0
            pending_features: list[str] = []
            missing_features: list[str] = []
            for feature in catalogue:
                cell = report.cells.get((feature, surface, level))
                if cell is None:
                    missing_features.append(feature)
                elif cell["status"] == "anchored":
                    anchored += 1
                elif cell["status"] == "pending":
                    pending += 1
                    pending_features.append(feature)
                else:
                    invalid += 1
            report.coverage[(surface, level)] = {
                "anchored": anchored,
                "pending": pending,
                "invalid": invalid,
                "missing": len(missing_features),
                "pending_features": pending_features,
                "missing_features": missing_features,
            }

    for status in MANIFEST_STATUSES:
        report.manifest_status[status] = sum(
            1 for row in report.manifest.values() if row["status"] == status)

    for leg, surfaces in platform_legs.items():
        totals = {"anchored": 0, "pending": 0, "invalid": 0, "missing": 0}
        for surface in surfaces:
            for level in LEVELS:
                entry = report.coverage[(surface, level)]
                for name in totals:
                    totals[name] += int(entry[name])
        report.platform_coverage[leg] = {"surfaces": surfaces, **totals}
        if registered_platforms is not None and leg not in registered_platforms:
            problems.append(f"platform leg is not registered in {PLATFORM_MATRIX}: {leg}")
        for surface in surfaces:
            if surface not in SURFACES:
                problems.append(f"platform leg {leg} carries an unknown surface: {surface}")

    return report


def check(root: Path = ROOT) -> Report:
    """Audit the repository registries against the authoritative catalogue."""
    catalogue = parse_catalogue((root / CATALOGUE).read_text(encoding="utf-8"))
    evidence = read_table(root / EVIDENCE, EVIDENCE_FIELDS)
    manifest = read_table(root / MANIFEST, MANIFEST_FIELDS)
    registered = platform_keys((root / PLATFORM_MATRIX).read_text(encoding="utf-8"))
    return evaluate(catalogue, evidence, manifest, registered_platforms=registered)


def render(report: Report) -> str:
    """The human readable coverage table of the current repository state."""
    lines: list[str] = []
    businesses = len(report.catalogue)
    lines.append(
        f"parity coverage: catalogue={businesses} businesses × {len(SURFACES)} surfaces "
        f"× {len(LEVELS)} levels → {report.total_cells} evidence cells, "
        f"{businesses * len(SURFACES)} manifest cells")
    lines.append("")
    lines.append("surface  level     anchored  pending  missing")
    lines.append("-------  --------  --------  -------  ------")
    totals = {"anchored": 0, "pending": 0, "invalid": 0, "missing": 0}
    for surface in SURFACES:
        for level in LEVELS:
            entry = report.coverage[(surface, level)]
            for name in totals:
                totals[name] += int(entry[name])
            lines.append(
                f"{surface:<7}  {level:<8}  {entry['anchored']:>8}  {entry['pending']:>7}  "
                f"{entry['missing']:>6}")
    lines.append(
        f"{'total':<7}  {'':<8}  {totals['anchored']:>8}  {totals['pending']:>7}  "
        f"{totals['missing']:>6}")
    if totals["invalid"]:
        lines.append(f"invalidly classified cells: {totals['invalid']}")
    lines.append("")
    manifest_total = sum(report.manifest_status.values())
    lines.append(
        "manifest cells: "
        + str(manifest_total)
        + " ("
        + ", ".join(f"{status} {report.manifest_status[status]}" for status in MANIFEST_STATUSES)
        + ")")
    lines.append("")
    lines.append("platform legs (the business coverage is platform independent):")
    lines.append("platform   surfaces            anchored  pending  missing")
    lines.append("---------  ------------------  --------  -------  ------")
    for leg, entry in report.platform_coverage.items():
        surfaces = ",".join(entry["surfaces"]) or "-"
        lines.append(
            f"{leg:<9}  {surfaces:<18}  {entry['anchored']:>8}  {entry['pending']:>7}  "
            f"{entry['missing']:>6}")
    lines.append("")
    gaps = [
        f"{surface}/{level}: {len(entry['pending_features'])} pending"
        for surface in SURFACES
        for level in LEVELS
        for entry in [report.coverage[(surface, level)]]
        if entry["pending_features"]
    ]
    gaps.extend(
        f"{surface}/{level}: {len(entry['missing_features'])} missing"
        for surface in SURFACES
        for level in LEVELS
        for entry in [report.coverage[(surface, level)]]
        if entry["missing_features"]
    )
    if gaps:
        lines.append("declared gaps (a pending cell is a gap, never a pass):")
        for gap in gaps:
            lines.append(f"  {gap}")
    else:
        lines.append("declared gaps: none")
    return "\n".join(lines)


def synthetic_catalogue() -> str:
    """A two-business catalogue with one Slint carrier and one without."""
    return """//! Synthetic catalogue.

features! {
    Alpha => ("alpha", "home", "home", "home", ["success", "error"]),
    Beta => ("beta", "settings", "settings", "", ["success"]),
}
"""


def synthetic_tables(catalogue: dict[str, dict[str, object]]) -> tuple[list[dict[str, str]], list[dict[str, str]]]:
    """Registries that satisfy every rule of the guard."""
    evidence: list[dict[str, str]] = []
    manifest: list[dict[str, str]] = []
    for feature, spec in catalogue.items():
        carrier = bool(spec["slint_page"])
        for surface in SURFACES:
            covered = carrier or surface != "slint"
            manifest.append({
                "feature_id": feature,
                "surface": surface,
                "status": "implemented" if covered else "pending",
                "reason": "synthetic" if covered else "no slint carrier page",
                "standard_scenario": f"{feature}-standard",
                "compact_scenario": f"{feature}-compact",
            })
            for level in LEVELS:
                anchored = covered or level == "contract"
                evidence.append({
                    "feature_id": feature,
                    "surface": surface,
                    "level": level,
                    "status": "anchored" if anchored else "pending",
                    "test_id": f"pkg::{feature}_{surface}_{level}" if anchored else "-",
                    "paths": ",".join(spec["required_paths"]),
                    "reason": "synthetic" if anchored else "no slint carrier page",
                })
    return evidence, manifest


def self_test() -> int:
    """Exercise every failure mode of the guard against synthetic registries."""
    catalogue = parse_catalogue(synthetic_catalogue())
    evidence, manifest = synthetic_tables(catalogue)
    registered = set(PLATFORM_LEGS)

    clean = evaluate(catalogue, evidence, manifest, registered_platforms=registered)
    assert clean.problems == [], clean.problems

    def problems_with(mutate) -> list[str]:
        rows = [dict(row) for row in evidence]
        cells = [dict(row) for row in manifest]
        mutate(rows, cells)
        return evaluate(catalogue, rows, cells, registered_platforms=registered).problems

    def drop_cell(rows, cells):
        rows.remove(next(row for row in rows
                         if row["feature_id"] == "alpha" and row["surface"] == "iced"
                         and row["level"] == "scenario"))

    assert any("missing evidence cell: alpha/iced/scenario" in problem
               for problem in problems_with(drop_cell))

    def duplicate_cell(rows, cells):
        rows.append(dict(next(row for row in rows
                              if row["feature_id"] == "alpha" and row["surface"] == "iced"
                              and row["level"] == "scenario")))

    assert any("duplicated evidence cell: alpha/iced/scenario" in problem
               for problem in problems_with(duplicate_cell))

    def unknown_surface(rows, cells):
        row = next(row for row in rows if row["feature_id"] == "alpha" and row["level"] == "contract")
        row["surface"] = "compose"

    assert any("unknown surface in evidence cell" in problem
               for problem in problems_with(unknown_surface))

    def unknown_business(rows, cells):
        row = next(row for row in rows if row["surface"] == "iced" and row["level"] == "contract")
        row["feature_id"] = "not-in-the-catalogue"

    assert any("unknown business in evidence cell" in problem
               for problem in problems_with(unknown_business))

    def empty_anchor(rows, cells):
        row = next(row for row in rows if row["feature_id"] == "alpha" and row["surface"] == "iced"
                   and row["level"] == "scenario")
        row["test_id"] = "-"

    assert any("anchored evidence cell without a test anchor: alpha/iced/scenario" in problem
               for problem in problems_with(empty_anchor))

    def pending_without_reason(rows, cells):
        row = next(row for row in rows if row["status"] == "pending")
        row["reason"] = "-"

    assert any("pending evidence cell without a reason" in problem
               for problem in problems_with(pending_without_reason))

    def slint_anchor_without_carrier(rows, cells):
        row = next(row for row in rows if row["feature_id"] == "beta" and row["surface"] == "slint"
                   and row["level"] == "scenario")
        row["status"] = "anchored"
        row["test_id"] = "pkg::beta_slint_scenario"

    assert any("slint scenario anchor without a carrier page: beta/slint/scenario" in problem
               for problem in problems_with(slint_anchor_without_carrier))

    def drop_manifest_cell(rows, cells):
        cells.remove(next(cell for cell in cells
                          if cell["feature_id"] == "alpha" and cell["surface"] == "bevy"))

    assert any("missing manifest cell: alpha/bevy" in problem
               for problem in problems_with(drop_manifest_cell))

    def duplicate_manifest_cell(rows, cells):
        cells.append(dict(next(cell for cell in cells
                                if cell["feature_id"] == "alpha" and cell["surface"] == "bevy")))

    assert any("duplicated manifest cell: alpha/bevy" in problem
               for problem in problems_with(duplicate_manifest_cell))

    def pending_manifest_without_reason(rows, cells):
        cell = next(cell for cell in cells if cell["status"] == "pending")
        cell["reason"] = " "

    assert any("pending manifest cell without a reason" in problem
               for problem in problems_with(pending_manifest_without_reason))

    def ready_without_evidence(rows, cells):
        cell = next(cell for cell in cells if cell["feature_id"] == "alpha" and cell["surface"] == "iced")
        cell["status"] = "ready"
        row = next(row for row in rows if row["feature_id"] == "alpha" and row["surface"] == "iced"
                   and row["level"] == "scenario")
        row["status"] = "pending"
        row["test_id"] = "-"
        row["reason"] = "still a gap"

    assert any("ready manifest cell without anchored scenario evidence: alpha/iced" in problem
               for problem in problems_with(ready_without_evidence))

    def slint_manifest_without_carrier(rows, cells):
        cell = next(cell for cell in cells if cell["feature_id"] == "beta" and cell["surface"] == "slint")
        cell["status"] = "implemented"

    assert any("slint manifest cell without a carrier page: beta/slint" in problem
               for problem in problems_with(slint_manifest_without_carrier))

    unleg = evaluate(catalogue, evidence, manifest, registered_platforms=set())
    assert any("platform leg is not registered" in problem for problem in unleg.problems)

    print("parity coverage guard self-test: PASS")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("report", "enforce"), default="enforce")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        return self_test()

    try:
        report = check()
    except (OSError, ValueError) as error:
        print(f"parity coverage guard: cannot read the parity registries: {error}", file=sys.stderr)
        return 2

    print(render(report))
    if report.problems:
        print(f"parity coverage guard: FAIL ({len(report.problems)} problems)", file=sys.stderr)
        for problem in report.problems:
            print(f"  - {problem}", file=sys.stderr)
        return 1 if args.mode == "enforce" else 0
    print("parity coverage guard: OK (every business is assigned to every surface)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
