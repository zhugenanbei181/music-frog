"""Regression checks for the feature matrix guard."""

import importlib.util
import json
from pathlib import Path
import sys
import unittest


QUALITY = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "feature_matrix_guard",
    QUALITY / "feature-matrix-guard.py",
)
GUARD = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = GUARD
SPEC.loader.exec_module(GUARD)


class FeatureMatrixGuardTests(unittest.TestCase):
    def test_repository_matrix_matches_the_manifests(self):
        self.assertEqual(GUARD.check(GUARD.REPO_ROOT), [])

    def test_every_feature_carries_a_kind(self):
        document = json.loads(
            (GUARD.REPO_ROOT / GUARD.MATRIX).read_text(encoding="utf-8")
        )

        for crate in document["crates"]:
            for feature in crate["features"]:
                self.assertIn(feature["kind"], GUARD.KINDS)
                self.assertIsInstance(feature["members"], list)

    def test_default_features_enable_no_capture_or_test_feature(self):
        document = json.loads(
            (GUARD.REPO_ROOT / GUARD.MATRIX).read_text(encoding="utf-8")
        )

        for crate in document["crates"]:
            kinds = {feature["name"]: feature["kind"] for feature in crate["features"]}
            default = next(
                (
                    feature
                    for feature in crate["features"]
                    if feature["name"] == "default"
                ),
                None,
            )
            if default is None:
                continue
            for member in default["members"]:
                self.assertNotIn(kinds.get(member), {"capture", "test"})


if __name__ == "__main__":
    unittest.main()
