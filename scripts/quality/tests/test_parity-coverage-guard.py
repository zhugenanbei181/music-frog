"""Regression tests for the business × surface × level parity coverage guard."""

import importlib.util
from pathlib import Path
import sys
import tempfile
import unittest

QUALITY = Path(__file__).resolve().parents[1]
REPO = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location(
    "parity_coverage_guard",
    QUALITY / "parity-coverage-guard.py",
)
GUARD = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = GUARD
SPEC.loader.exec_module(GUARD)

CATALOGUE = """//! Synthetic catalogue.

features! {
    Alpha => ("alpha", "home", "home", "home", ["success", "error"]),
    Beta => ("beta", "settings", "settings", "", ["success"]),
}
"""

PLATFORM_MATRIX = """# Platform matrix

| 平台 | 产品与交付 | 当前状态 |
| --- | --- | --- |
| Linux desktop | Iced、Bevy、Slint | pending |
| Windows desktop | Iced、Bevy、Slint | pending |
| macOS desktop | Iced、Bevy | pending |
| Android | Bevy 与 Slint flavor | pending |
| iOS | Rust host seam | blocked |
"""


def evidence_rows() -> list[dict[str, str]]:
    """A registry that satisfies every rule of the guard."""
    rows: list[dict[str, str]] = []
    for feature in ("alpha", "beta"):
        covered = feature == "alpha"
        for surface in GUARD.SURFACES:
            for level in GUARD.LEVELS:
                anchored = covered or surface != "slint" or level == "contract"
                rows.append({
                    "feature_id": feature,
                    "surface": surface,
                    "level": level,
                    "status": "anchored" if anchored else "pending",
                    "test_id": f"pkg::{feature}_{surface}_{level}" if anchored else "-",
                    "paths": "success",
                    "reason": "synthetic" if anchored else "no slint carrier page",
                })
    return rows


def manifest_rows() -> list[dict[str, str]]:
    """A manifest that satisfies every rule of the guard."""
    rows: list[dict[str, str]] = []
    for feature in ("alpha", "beta"):
        covered = feature == "alpha"
        for surface in GUARD.SURFACES:
            rows.append({
                "feature_id": feature,
                "surface": surface,
                "status": "implemented" if covered or surface != "slint" else "pending",
                "reason": "synthetic" if covered or surface != "slint" else "no slint carrier page",
                "standard_scenario": f"{feature}-standard",
                "compact_scenario": f"{feature}-compact",
            })
    return rows


def write_table(path: Path, fields: tuple[str, ...], rows: list[dict[str, str]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    lines = ["\t".join(fields)]
    lines.extend("\t".join(row[field] for field in fields) for row in rows)
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def write_repository(
    root: Path,
    *,
    evidence: list[dict[str, str]] | None = None,
    manifest: list[dict[str, str]] | None = None,
    platform_matrix: str = PLATFORM_MATRIX,
) -> Path:
    """Materialise a synthetic repository that the guard can audit."""
    catalogue = root / GUARD.CATALOGUE
    catalogue.parent.mkdir(parents=True, exist_ok=True)
    catalogue.write_text(CATALOGUE, encoding="utf-8")
    write_table(root / GUARD.EVIDENCE, GUARD.EVIDENCE_FIELDS,
                evidence_rows() if evidence is None else evidence)
    write_table(root / GUARD.MANIFEST, GUARD.MANIFEST_FIELDS,
                manifest_rows() if manifest is None else manifest)
    matrix = root / GUARD.PLATFORM_MATRIX
    matrix.parent.mkdir(parents=True, exist_ok=True)
    matrix.write_text(platform_matrix, encoding="utf-8")
    return root


class RepositoryCoverage(unittest.TestCase):
    def test_repository_registries_cover_every_business_cell(self):
        report = GUARD.check(REPO)
        self.assertEqual(report.problems, [])
        self.assertEqual(report.total_cells, 354)
        self.assertEqual(len(report.manifest), 177)

    def test_repository_coverage_table_lists_every_surface(self):
        rendered = GUARD.render(GUARD.check(REPO))
        for surface in GUARD.SURFACES:
            self.assertIn(surface, rendered)
        self.assertIn("anchored", rendered)
        self.assertIn("pending", rendered)
        self.assertIn("missing", rendered)


class SyntheticCoverage(unittest.TestCase):
    def audit(self, **kwargs) -> list[str]:
        with tempfile.TemporaryDirectory() as directory:
            root = write_repository(Path(directory), **kwargs)
            return GUARD.check(root).problems

    def test_synthetic_registries_pass(self):
        self.assertEqual(self.audit(), [])

    def test_missing_cell_fails(self):
        evidence = evidence_rows()
        evidence.remove(next(row for row in evidence
                              if row["feature_id"] == "alpha" and row["surface"] == "iced"
                              and row["level"] == "scenario"))
        problems = self.audit(evidence=evidence)
        self.assertTrue(any("missing evidence cell: alpha/iced/scenario" in problem
                            for problem in problems))

    def test_duplicated_cell_fails(self):
        evidence = evidence_rows()
        evidence.append(dict(next(row for row in evidence
                                  if row["feature_id"] == "alpha" and row["surface"] == "iced"
                                  and row["level"] == "scenario")))
        problems = self.audit(evidence=evidence)
        self.assertTrue(any("duplicated evidence cell: alpha/iced/scenario" in problem
                            for problem in problems))

    def test_pending_cell_without_reason_fails(self):
        evidence = evidence_rows()
        row = next(row for row in evidence if row["status"] == "pending")
        row["reason"] = "-"
        problems = self.audit(evidence=evidence)
        self.assertTrue(any("pending evidence cell without a reason" in problem
                            for problem in problems))

    def test_anchored_scenario_without_carrier_fails(self):
        evidence = evidence_rows()
        row = next(row for row in evidence if row["feature_id"] == "beta"
                   and row["surface"] == "slint" and row["level"] == "scenario")
        row["status"] = "anchored"
        row["test_id"] = "pkg::beta_slint_scenario"
        problems = self.audit(evidence=evidence)
        self.assertTrue(any("slint scenario anchor without a carrier page: beta/slint/scenario"
                            in problem for problem in problems))

    def test_anchored_cell_without_test_anchor_fails(self):
        evidence = evidence_rows()
        row = next(row for row in evidence if row["feature_id"] == "alpha"
                   and row["surface"] == "iced" and row["level"] == "scenario")
        row["test_id"] = ""
        problems = self.audit(evidence=evidence)
        self.assertTrue(any("anchored evidence cell without a test anchor" in problem
                            for problem in problems))

    def test_cell_outside_the_product_surfaces_fails(self):
        evidence = evidence_rows()
        row = next(row for row in evidence if row["feature_id"] == "alpha"
                   and row["level"] == "contract")
        row["surface"] = "compose"
        problems = self.audit(evidence=evidence)
        self.assertTrue(any("unknown surface in evidence cell" in problem
                            for problem in problems))

    def test_cell_outside_the_catalogue_fails(self):
        evidence = evidence_rows()
        row = next(row for row in evidence if row["surface"] == "iced"
                   and row["level"] == "contract")
        row["feature_id"] = "not-in-the-catalogue"
        problems = self.audit(evidence=evidence)
        self.assertTrue(any("unknown business in evidence cell" in problem
                            for problem in problems))

    def test_missing_manifest_cell_fails(self):
        manifest = manifest_rows()
        manifest.remove(next(row for row in manifest if row["feature_id"] == "alpha"
                             and row["surface"] == "bevy"))
        problems = self.audit(manifest=manifest)
        self.assertTrue(any("missing manifest cell: alpha/bevy" in problem
                            for problem in problems))

    def test_duplicated_manifest_cell_fails(self):
        manifest = manifest_rows()
        manifest.append(dict(next(row for row in manifest if row["feature_id"] == "alpha"
                                  and row["surface"] == "bevy")))
        problems = self.audit(manifest=manifest)
        self.assertTrue(any("duplicated manifest cell: alpha/bevy" in problem
                            for problem in problems))

    def test_pending_manifest_cell_without_reason_fails(self):
        manifest = manifest_rows()
        row = next(row for row in manifest if row["status"] == "pending")
        row["reason"] = " "
        problems = self.audit(manifest=manifest)
        self.assertTrue(any("pending manifest cell without a reason" in problem
                            for problem in problems))

    def test_ready_manifest_cell_without_evidence_fails(self):
        manifest = manifest_rows()
        row = next(row for row in manifest if row["feature_id"] == "alpha"
                   and row["surface"] == "iced")
        row["status"] = "ready"
        evidence = evidence_rows()
        cell = next(row for row in evidence if row["feature_id"] == "alpha"
                    and row["surface"] == "iced" and row["level"] == "scenario")
        cell["status"] = "pending"
        cell["test_id"] = "-"
        cell["reason"] = "still a gap"
        problems = self.audit(evidence=evidence, manifest=manifest)
        self.assertTrue(any("ready manifest cell without anchored scenario evidence: alpha/iced"
                            in problem for problem in problems))

    def test_slint_manifest_cell_without_carrier_fails(self):
        manifest = manifest_rows()
        row = next(row for row in manifest if row["feature_id"] == "beta"
                   and row["surface"] == "slint")
        row["status"] = "implemented"
        problems = self.audit(manifest=manifest)
        self.assertTrue(any("slint manifest cell without a carrier page: beta/slint" in problem
                            for problem in problems))

    def test_unregistered_platform_leg_fails(self):
        problems = self.audit(platform_matrix=PLATFORM_MATRIX.replace("| iOS |", "| iPad |"))
        self.assertTrue(any("platform leg is not registered" in problem for problem in problems))


if __name__ == "__main__":
    unittest.main()
