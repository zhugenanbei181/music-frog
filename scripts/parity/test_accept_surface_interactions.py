"""The batch acceptance classifies every cell and never lets one failure hide the rest."""
import csv
import json
import tempfile
import unittest
from pathlib import Path

from accept_surface_interactions import (BASELINE_PREFIX, MARKER_ROUTES, anchor_reason,
                                         anchor_selection, baseline_images, capture_matrix,
                                         classify_cell, surface_registry_state)
from interaction_delta import changed_pixels, corroborate

try:
    from PIL import Image, ImageDraw
    HAS_PIL = True
except ImportError:
    HAS_PIL = False


class MatrixTests(unittest.TestCase):
    def test_every_business_gets_both_viewports_and_a_route_only_baseline_per_page(self):
        rows = capture_matrix(["alpha", "beta"], {"alpha": "settings", "beta": "logs"}, 2)
        names = [row["name"] for row in rows]
        self.assertEqual(names[:4], ["alpha-standard", "alpha-compact", "beta-standard", "beta-compact"])
        baselines = [name for name in names if name.startswith(BASELINE_PREFIX)]
        self.assertEqual(len(baselines), 8)  # two pages, two viewports, two replicates
        self.assertTrue(all("-standard" not in name and "-compact" not in name for name in baselines))
        self.assertTrue(all(row["window_size"] in {"1180x780", "720x480"} for row in rows))

    def test_the_baseline_name_cannot_be_read_as_a_business_interaction(self):
        rows = capture_matrix(["alpha"], {"alpha": "settings"}, 2)
        for row in rows:
            if row["name"].startswith(BASELINE_PREFIX):
                self.assertFalse(row["name"].endswith("-standard"))
                self.assertFalse(row["name"].endswith("-compact"))


class AnchorTests(unittest.TestCase):
    def evidence(self):
        return [
            dict(feature_id="alpha", surface="iced", level="contract", status="anchored",
                 test_id="infiltrator-iced::headless::a", paths="success", reason="-"),
            dict(feature_id="alpha", surface="iced", level="scenario", status="anchored",
                 test_id="infiltrator-iced::headless::b", paths="success", reason="-"),
            dict(feature_id="alpha", surface="bevy", level="contract", status="pending",
                 test_id="-", paths="-", reason="no bevy anchor yet"),
            dict(feature_id="beta", surface="iced", level="contract", status="anchored",
                 test_id="infiltrator-application::doctor::c", paths="success", reason="-"),
        ]

    def test_the_discovery_covers_every_anchored_package_but_only_runs_the_selection(self):
        wanted, pending, owners = anchor_selection(self.evidence(), {"alpha"})
        self.assertEqual(wanted, {"infiltrator-iced": {"infiltrator-iced::headless::a",
                                                        "infiltrator-iced::headless::b"}})
        self.assertEqual(owners, {"infiltrator-iced", "infiltrator-application"})
        self.assertEqual(pending, {("alpha", "bevy"): ["contract: no bevy anchor yet"]})

    def test_a_missing_or_failing_anchor_leaves_the_cell_uncertified(self):
        evidence = self.evidence()
        results = {"infiltrator-iced": {"tests": {"infiltrator-iced::headless::a": "passed",
                                                 "infiltrator-iced::headless::b": "failed"}}}
        reason = anchor_reason(evidence, results, "alpha", "iced")
        self.assertIn("behaviour anchor failed", reason)
        results["infiltrator-iced"]["tests"]["infiltrator-iced::headless::b"] = "passed"
        self.assertEqual(anchor_reason(evidence, results, "alpha", "iced"), "")
        self.assertIn("no behaviour anchor", anchor_reason(evidence, results, "alpha", "bevy"))


class SurfaceStateTests(unittest.TestCase):
    def test_a_surface_without_scenario_anchors_is_not_certifiable(self):
        evidence = [dict(feature_id="alpha", surface="slint", level="scenario", status="pending",
                         test_id="-", paths="-", reason="the slint surface has no scenario anchor")]
        specs = {"alpha": {"slint_page": ""}}
        state = surface_registry_state(evidence, {"alpha"}, "slint", specs)
        self.assertEqual(state["status"], "not_certifiable")
        self.assertIn("no scenario anchor", state["reason"])

    def test_a_carried_surface_reports_what_it_can_and_cannot_certify(self):
        evidence = [dict(feature_id="alpha", surface="slint", level="scenario", status="anchored",
                         test_id="infiltrator-slint::a", paths="success", reason="-"),
                    dict(feature_id="beta", surface="slint", level="scenario", status="pending",
                         test_id="-", paths="-", reason="no sheet yet")]
        specs = {"alpha": {"slint_page": "profiles"}, "beta": {"slint_page": "profiles"}}
        state = surface_registry_state(evidence, {"alpha", "beta"}, "slint", specs)
        self.assertEqual(state["status"], "not_in_scope")
        self.assertEqual(state["certifiable_businesses"], ["alpha"])
        self.assertEqual(state["businesses_without_a_scenario_anchor"], ["beta"])
        self.assertEqual(state["missing_capability"], "no sheet yet")

    def test_the_marker_route_rename_is_explicit(self):
        self.assertEqual(MARKER_ROUTES["iced"]["filter"], "editor")
        self.assertEqual(MARKER_ROUTES["iced"]["rules-providers"], "rules")
        self.assertEqual(MARKER_ROUTES["bevy"]["app-routing"], "app_routing")


@unittest.skipUnless(HAS_PIL, "Pillow is required for pixel classification tests")
class ClassificationTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.run = Path(self.directory.name)
        self.scenario = self.run / "alpha-standard"
        self.scenario.mkdir()
        self.baseline = self.run / "baseline-settings-1180x780"
        self.baseline.mkdir()
        Image.new("RGB", (1180, 780), "#202020").save(self.baseline / "image.png")
        Image.new("RGB", (1180, 780), "#202020").save(self.scenario / "image.png")
        self.rows = {"alpha-standard": {"scenario": "alpha-standard", "page": "settings", "skin": "dark",
                                        "requested_window": "1180x780", "app_pid": "7", "window_id": "3",
                                        "width": "1180", "height": "780", "bytes": "9",
                                        "sha256": "x", "status": "ok"}}
        self.baselines = {("settings", "standard"): {"primary": self.baseline / "image.png",
                                                     "replicate": None}}

    def marker(self, text):
        (self.scenario / "marker.log").write_text(text)

    def classify(self):
        return classify_cell(self.run, "iced", "alpha", "standard", "settings", self.rows, self.baselines)

    def test_a_failed_capture_is_reported_with_the_capture_reason(self):
        self.rows["alpha-standard"]["status"] = "failed-marker (no CAPTURE_READY within 120x0.25s)"
        cell = self.classify()
        self.assertEqual(cell["status"], "uncertified")
        self.assertIn("no demo interaction", cell["reason"])

    def test_a_missing_window_is_an_environment_fault_not_a_product_verdict(self):
        self.rows["alpha-standard"]["status"] = "failed-window (no pid+identity match via niri IPC)"
        cell = self.classify()
        self.assertEqual(cell["status"], "uncertified")
        self.assertIn("environment", cell["reason"])

    def test_a_surface_panic_is_a_product_fault(self):
        self.rows["alpha-standard"]["status"] = "failed-window (no pid+identity match via niri IPC)"
        (self.scenario / "app.log").write_text(
            "thread 'main' panicked at crates/infiltrator-bevy-ui/src/interaction_capture/logs.rs:137:22:\n"
            "actual isolated logs lifecycle: Failure { code: NotReady }\n")
        cell = self.classify()
        self.assertEqual(cell["status"], "uncertified")
        self.assertIn("surface panic", cell["reason"])
        self.assertIn("logs.rs", cell["reason"])

    def test_a_route_only_capture_without_an_activation_marker_is_not_certified(self):
        self.marker("CAPTURE_READY page=settings skin=dark\n")
        cell = self.classify()
        self.assertEqual(cell["status"], "uncertified")
        self.assertIn("no demo interaction", cell["reason"])

    def test_the_page_must_be_the_carrier_page_of_the_business(self):
        self.marker("CAPTURE_READY page=overview skin=dark scenario=alpha activated=true bounds=10,10,100,40\n")
        cell = self.classify()
        self.assertEqual(cell["status"], "uncertified")
        self.assertIn("wrong page", cell["reason"])

    def test_a_publish_that_changed_nothing_is_not_an_interaction(self):
        self.marker("CAPTURE_READY page=settings skin=dark scenario=alpha activated=true bounds=10,10,100,40\n")
        cell = self.classify()
        self.assertEqual(cell["status"], "uncertified")
        self.assertIn("no demo interaction", cell["reason"])
        self.assertEqual(cell["changed_pixels"], 0)

    def test_a_real_interaction_is_certified(self):
        with Image.open(self.scenario / "image.png") as image:
            draw = ImageDraw.Draw(image)
            draw.rectangle((10, 10, 110, 50), fill="#ffffff")
            image.save(self.scenario / "image.png")
        self.marker("CAPTURE_READY page=settings skin=dark scenario=alpha activated=true bounds=10,10,100,40\n")
        cell = self.classify()
        self.assertEqual(cell["status"], "certified")
        self.assertGreater(cell["changed_pixels"], 0)

    def test_the_noise_floor_of_an_animated_page_is_measured_between_two_baselines(self):
        with Image.open(self.baseline / "image.png") as image:
            draw = ImageDraw.Draw(image)
            draw.rectangle((10, 10, 110, 50), fill="#ffffff")
            image.save(self.baseline / "image.png")
        replicate = self.run / "baseline-r2-settings-1180x780"
        replicate.mkdir()
        with Image.open(self.baseline / "image.png") as image:
            draw = ImageDraw.Draw(image)
            draw.rectangle((10, 10, 110, 50), fill="#202020")
            draw.rectangle((60, 10, 160, 50), fill="#ffffff")
            image.save(replicate / "image.png")
        baselines = {("settings", "standard"): {"primary": self.baseline / "image.png",
                                                 "replicate": replicate / "image.png"}}
        self.marker("CAPTURE_READY page=settings skin=dark scenario=alpha activated=true bounds=10,10,100,40\n")
        cell = classify_cell(self.run, "iced", "alpha", "standard", "settings", self.rows, baselines)
        self.assertEqual(cell["status"], "uncertified")
        self.assertIn("no demo interaction", cell["reason"])
        self.assertGreater(cell["noise_pixels"], 0)


@unittest.skipUnless(HAS_PIL, "Pillow is required for pixel delta tests")
class DeltaTests(unittest.TestCase):
    def test_identical_captures_change_nothing_and_differing_captures_do(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            first = root / "first.png"
            second = root / "second.png"
            Image.new("RGB", (100, 100), "#101010").save(first)
            Image.new("RGB", (100, 100), "#101010").save(second)
            self.assertEqual(changed_pixels(first, second), (0, 10000))
            with Image.open(second) as image:
                ImageDraw.Draw(image).rectangle((0, 0, 20, 20), fill="#ffffff")
                image.save(second)
            changed, compared = changed_pixels(first, second)
            self.assertEqual(compared, 10000)
            self.assertGreater(changed, 0)
            self.assertFalse(corroborate(first, first, 0)["corroborated"])
            self.assertTrue(corroborate(second, first, 0)["corroborated"])

    def test_the_noise_floor_raises_the_bar(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            observed = root / "observed.png"
            baseline = root / "baseline.png"
            Image.new("RGB", (100, 100), "#101010").save(baseline)
            image = Image.new("RGB", (100, 100), "#101010")
            ImageDraw.Draw(image).rectangle((0, 0, 8, 8), fill="#ffffff")
            image.save(observed)
            self.assertTrue(corroborate(observed, baseline, 0)["corroborated"])
            self.assertFalse(corroborate(observed, baseline, 100)["corroborated"])


if __name__ == "__main__":
    unittest.main()
