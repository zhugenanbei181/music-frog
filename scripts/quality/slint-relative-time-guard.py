#!/usr/bin/env python3
"""SLINT-018: the profile age is one typed fact with two shared templates.

FlClash's `profiles.dart` renders `profileUpdatedAt(age)` at normal density and
the bare age once it measures that the row has no room for the prefix. MusicFrog
resolves both strings in the shared presentation layer and publishes them as two
typed `DashboardFacts` fields. Slint has no owner-published text-width fact, so
its desktop card consumes the detailed template and never reconstructs an
absolute "更新于 <timestamp>" string; the compact template remains available for
a surface that owns the measured decision.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys
import unittest
import unittest.mock

ROOT = pathlib.Path(__file__).resolve().parents[2]
MODELS = ROOT / "crates/infiltrator-slint/ui/components/models.slint"
GRID = ROOT / "crates/infiltrator-slint/ui/pages/home_desktop_grid.slint"
RUST = ROOT / "crates/infiltrator-slint/src/lumen_projection/dashboard.rs"
DETAILED_FIELD = "profile-updated-relative"
COMPACT_FIELD = "profile-updated-relative-compact"


def findings(models: str, grid: str, rust: str) -> list[str]:
    found: list[str] = []
    for field in (DETAILED_FIELD, COMPACT_FIELD):
        if len(re.findall(rf"^\s*{re.escape(field)}:\s*string,", models, re.MULTILINE)) != 1:
            found.append(f"models.slint: {field} must be declared exactly once")
        if len(re.findall(rf"^\s*{re.escape(field)}:\s*\"—\",", models, re.MULTILINE)) != 1:
            found.append(f"models.slint: {field} needs one unobserved default")
    if "profile_updated_relative: updated_relative.into()," not in rust:
        found.append("dashboard.rs: detailed age copy is not projected")
    if "profile_updated_relative_compact: updated_relative_compact.into()," not in rust:
        found.append("dashboard.rs: compact age copy is not projected")
    if "RelativeTimePresentation::Detailed" not in rust:
        found.append("dashboard.rs: detailed template is not requested")
    if "RelativeTimePresentation::Compact" not in rust:
        found.append("dashboard.rs: compact template is not requested")
    if "text: root.dashboard-facts.profile-updated-relative;" not in grid:
        found.append("home_desktop_grid.slint: the card must consume the detailed template")
    if "profile-updated-relative-compact" in grid:
        found.append("home_desktop_grid.slint: text-width selection is not an owner fact")
    if re.search(r"更新于\s*\+|format!\(\"更新于", grid + rust):
        found.append("the surface must not reconstruct the absolute update copy")
    return found


def live_findings() -> list[str]:
    return findings(
        MODELS.read_text(encoding="utf-8"),
        GRID.read_text(encoding="utf-8"),
        RUST.read_text(encoding="utf-8"),
    )


class GuardSelfTest(unittest.TestCase):
    def setUp(self) -> None:
        self.models = MODELS.read_text(encoding="utf-8")
        self.grid = GRID.read_text(encoding="utf-8")
        self.rust = RUST.read_text(encoding="utf-8")

    def test_live_tree_is_clean(self) -> None:
        self.assertEqual(findings(self.models, self.grid, self.rust), [])

    def test_a_private_width_decision_is_caught(self) -> None:
        planted = self.grid.replace(
            "text: root.dashboard-facts.profile-updated-relative;",
            "text: root.compact-metrics ? root.dashboard-facts.profile-updated-relative-compact : root.dashboard-facts.profile-updated-relative;",
            1,
        )
        self.assertTrue(
            any("text-width selection is not an owner fact" in item for item in findings(self.models, planted, self.rust))
        )

    def test_a_missing_owner_projection_is_caught(self) -> None:
        planted = self.rust.replace(
            "        profile_updated_relative_compact: updated_relative_compact.into(),\n",
            "",
            1,
        )
        self.assertTrue(
            any("compact age copy is not projected" in item for item in findings(self.models, self.grid, planted))
        )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("enforce", "report"), default="enforce")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(GuardSelfTest)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1
    found = live_findings()
    for item in found:
        print(f"SLINT-018 [{args.mode}]: {item}")
    print(f"slint relative time guard: findings={len(found)}")
    return 1 if found and args.mode == "enforce" else 0


if __name__ == "__main__":
    sys.exit(main())
