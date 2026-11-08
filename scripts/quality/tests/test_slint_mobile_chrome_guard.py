"""Mutation checks for the typed mobile dock-band guard."""

import importlib.util
import pathlib
import shutil
import sys
import tempfile
import unittest


QUALITY = pathlib.Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "slint_mobile_chrome_guard",
    QUALITY / "slint-mobile-chrome-guard.py",
)
GUARD = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = GUARD
SPEC.loader.exec_module(GUARD)


class SlintMobileChromeGuardPlantTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = pathlib.Path(self.temporary.name)
        self.ui = self.root / "ui"
        shutil.copytree(GUARD.UI, self.ui)
        self.contract = self.root / "mobile_chrome.rs"
        self.projection = self.root / "mobile_chrome_projection.rs"
        shutil.copyfile(GUARD.CONTRACT, self.contract)
        shutil.copyfile(GUARD.PRESENTATION, self.projection)

    def tearDown(self):
        self.temporary.cleanup()

    def findings(self):
        return GUARD.violations(self.ui, self.contract, self.projection)

    def test_clean_tree_has_no_typed_state_findings(self):
        self.assertEqual(
            [item for item in self.findings() if "typed" in item or "dock band" in item],
            [],
        )

    def test_two_truth_flags_are_caught(self):
        target = self.ui / "lumen-app.slint"
        target.write_text(
            target.read_text(encoding="utf-8")
            + "\nproperty <bool> bottom-bar-present: true;\n",
            encoding="utf-8",
        )
        self.assertTrue(any("retired dock stage flags" in item for item in self.findings()))

    def test_transparent_band_without_page_ground_is_caught(self):
        target = self.ui / "components/app_chrome.slint"
        target.write_text(
            target.read_text(encoding="utf-8").replace(
                "export component MobileDockPageGround inherits Rectangle {",
                "export component DisabledMobileDockPageGround inherits Rectangle {",
                1,
            ),
            encoding="utf-8",
        )
        self.assertTrue(any("page-ground owner" in item for item in self.findings()))

    def test_overlay_can_plant_an_opaque_backdrop(self):
        target = self.ui / "components/app_chrome.slint"
        target.write_text(
            target.read_text(encoding="utf-8").replace(
                "export component AppBottomBar inherits Rectangle {",
                "dock-backdrop := Rectangle { background: LumenTheme.zinc-0; }\n"
                "export component AppBottomBar inherits Rectangle {",
                1,
            ),
            encoding="utf-8",
        )
        self.assertTrue(any("opaque dock backdrop" in item for item in self.findings()))

    def test_old_bool_geometry_reflow_is_caught(self):
        target = self.ui / "components/app_body.slint"
        target.write_text(
            target.read_text(encoding="utf-8").replace(
                "padding-bottom: root.dock-band-height;",
                "padding-bottom: root.bottom-bar-present ? root.dock-band-height : 0 * LumenUiScale.unit;",
                1,
            ),
            encoding="utf-8",
        )
        findings = self.findings()
        self.assertTrue(
            any("retired dock stage flags" in item for item in findings),
            findings,
        )
        self.assertTrue(
            any("subtracts exactly one" in item for item in findings),
            findings,
        )
