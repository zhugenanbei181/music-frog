#!/usr/bin/env python3
"""Cross-check the three font/layout guards against the exported authority."""

from __future__ import annotations

import importlib.util
import json
import pathlib
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[3]
AUTHORITY = ROOT / "crates/infiltrator-contract/assets/design-authority.json"
CATALOG = ROOT / "crates/infiltrator-shared/assets/asset-catalog.json"
FONT_DIR = ROOT / "crates/infiltrator-shared/assets/fonts"


def load(path: pathlib.Path, name: str):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


class DesignResourceGuardTests(unittest.TestCase):
    def setUp(self) -> None:
        self.authority = json.loads(AUTHORITY.read_text(encoding="utf-8"))
        self.generator = load(
            ROOT / "scripts/generate_design_resources.py", "generate_design_resources_test"
        )
        self.design_guard = load(
            ROOT / "scripts/quality/design-resource-authority.py",
            "design_resource_authority_test",
        )
        self.typography_guard = load(
            ROOT / "scripts/quality/slint-typography-role-guard.py",
            "slint_typography_guard_test",
        )
        self.viewport_guard = load(
            ROOT / "scripts/quality/slint-viewport-tier-guard.py",
            "slint_viewport_guard_test",
        )

    def test_seven_faces_and_twenty_six_roles_are_one_inventory(self) -> None:
        self.assertEqual(self.generator.authority_problems(self.authority), [])
        self.assertEqual(len(self.authority["font_faces"]), 7)
        self.assertEqual(len(self.authority["type_roles"]), 26)
        self.assertEqual(self.authority["font_weights"], [400, 500, 600])
        self.assertEqual(
            self.authority["connection_column_weights"], [8, 16, 3, 5, 6, 6, 4]
        )
        self.assertEqual(self.authority["hero_fact_column_weights"], [5, 7])
        catalog_ids = {
            record["id"]
            for record in json.loads(CATALOG.read_text(encoding="utf-8"))["assets"]
            if record.get("format") == "ttf"
        }
        self.assertEqual(
            catalog_ids, {face["catalog_id"] for face in self.authority["font_faces"]}
        )

    def test_every_role_resolves_to_an_authority_face(self) -> None:
        faces = {face["catalog_id"]: face for face in self.authority["font_faces"]}
        for role in self.authority["type_roles"]:
            face = faces[role["catalog_id"]]
            self.assertEqual(face["family_name"], role["family_name"])
            self.assertEqual(face["weight"], role["weight"])
            self.assertIn(role["weight"], {"Regular", "Medium", "SemiBold"})

    def test_cjk_ttf_metadata_matches_the_three_physical_faces(self) -> None:
        records = self.design_guard.font_catalog_records()
        by_id = {
            face["catalog_id"]: face
            for face in self.authority["font_faces"]
            if face["family"] == "Cjk"
        }
        self.assertEqual(set(by_id), {
            "font.CJK-Regular",
            "font.CJK-Medium",
            "font.CJK-SemiBold",
        })
        for catalog_id, face in by_id.items():
            record = records[catalog_id]
            weight, family, subfamily = self.design_guard.font_metadata(record)
            self.assertEqual(weight, self.design_guard.FONT_WEIGHT_VALUES[face["weight"]])
            self.assertEqual(family, "HarmonyOS Sans SC")
            self.assertEqual(record["license"], "HarmonyOS Sans Fonts License Agreement")
            self.assertTrue((FONT_DIR / "HarmonyOS-Sans-LICENSE.txt").is_file())
            self.assertIn(subfamily, {"Regular", "Medium", "Semibold"})

    def test_generated_adapters_and_closed_guards_agree(self) -> None:
        self.assertEqual(self.design_guard.generated_check(), [])
        self.assertEqual(self.design_guard.all_violations(include_freshness=False), [])
        self.assertEqual(self.typography_guard.check(), [])
        self.assertEqual(self.viewport_guard.violations(self.viewport_guard.UI), [])


if __name__ == "__main__":
    unittest.main()
