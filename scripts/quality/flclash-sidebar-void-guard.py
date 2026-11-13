#!/usr/bin/env python3
"""Pin the navigation-column caps to *both* tracked FlClash desktop scenes.

`docs/SPACE_UTILIZATION_STANDARD.md` §3.1 publishes `chrome_band` 0% and
a `void` cap. The 0% was justified by prose ("the reference sidebar paints only
destinations"); the 12% was inherited from b144, whose stated reason was
"group headings and stretch air must stay at cluster boundaries" - but b164
retired group headings entirely, so the number lost its only rationale.

This guard measures both tracked FlClash desktop references with the same ink
rule as the judge: substantial runs of non-ground rows inside the sidebar
column. It publishes three facts from every scene's pixels:

* the largest ground run between destination rows, as a share of window
  height (the reference's own `void`);
* the ground below the final destination band, which bounds our exported
  `nav_tail_ground_limit_pct` without outlawing the reference's own air;
* that no ink band lives below the destination stack (the reference's own
  `chrome_band`).

Our shortest tier paints eight destinations in a 480px window while the
references paint five in 1665px and 1949px windows, so a raw share cannot be
copied: the same absolute gap is a much larger percentage of the shorter
window. The exported caps are therefore constrained to a bounded multiple of
the worst reference void and a bounded headroom over the *worst* trailing
ground - never a flat copy of one scene's number. b228 found the concrete
regression this prevents: deriving a 68% tail cap from only the 1665px
dashboard frame would outlaw the tracked 1949px proxies frame itself, whose
identical navigation stack leaves 70.24% legal ground.

No surface pixels are judged here: `scripts/parity/space_utilization.py`
remains the single judge of published receipts.
"""

from __future__ import annotations

import argparse
import dataclasses
import hashlib
import json
import pathlib
import re
import sys
import tempfile
import unittest

import numpy as np
from PIL import Image

ROOT = pathlib.Path(__file__).resolve().parents[2]
EXPORT = ROOT / "crates/infiltrator-contract/assets/page-structure.json"
CONTRACT = ROOT / "crates/infiltrator-contract/src/page_structure.rs"
STANDARD = ROOT / "docs/SPACE_UTILIZATION_STANDARD.md"
REFERENCE_DOC = ROOT / "docs/reference/FLCLASH_DESKTOP_REFERENCE.md"
GATE = ROOT / "scripts/quality/check-structure.sh"


@dataclasses.dataclass(frozen=True)
class ReferenceScene:
    path: pathlib.Path
    sha256: str
    right_edge_px: int
    expected_void_pct: float
    expected_tail_pct: float


REFERENCES = (
    ReferenceScene(
        path=pathlib.Path("docs/reference/flclash-desktop/flclash-desktop-dashboard.png"),
        sha256="f6406be498613bc0249e9ae2350125a0e670e9620d0ae4b1be1aec7353372bcc",
        right_edge_px=429,
        expected_void_pct=3.06,
        expected_tail_pct=65.17,
    ),
    ReferenceScene(
        path=pathlib.Path("docs/reference/flclash-desktop/flclash-desktop-proxies.png"),
        sha256="ebe094bae677d5419cb380e800d5eee74197d20a48891fbe25ba340af7f98e8e",
        right_edge_px=432,
        expected_void_pct=3.49,
        expected_tail_pct=70.24,
    ),
)
SIDEBAR_STACK_BOTTOM_PX = 579
SUBSTANTIAL_RUN_PX = 3
ROW_INK_FRACTION = 0.01
GROUND_DELTA = 14
MAX_TAIL_HEADROOM_PCT = 3.0
EXPECTED_TAIL_LIMIT_PCT = 73.0
EXPECTED_TAIL_BANDS = (
    ("short", 0, 600, 18.0),
    ("medium", 600, 900, 50.0),
    ("tall", 900, 1200, 60.0),
    ("reference", 1200, None, 73.0),
)

# The shortest published tier is 480px tall with eight destinations; the
# references are 1665px and 1949px with five. A fixed share copied from one
# reference would penalise the shorter window for the same absolute gap, so
# the cap is allowed a bounded multiple of the worst reference share.
MIN_SCALE_MULTIPLE = 2.0
MAX_SCALE_MULTIPLE = 4.0


def digest(scene: ReferenceScene) -> str:
    return hashlib.sha256((ROOT / scene.path).read_bytes()).hexdigest()


def load_scene(scene: ReferenceScene) -> np.ndarray:
    with Image.open(ROOT / scene.path) as handle:
        return np.array(handle.convert("RGB"))


def sidebar_bands(image: np.ndarray, scene: ReferenceScene) -> tuple[list[tuple[int, int]], int]:
    """Substantial ink bands inside one scene's sidebar, and its window height."""
    height = image.shape[0]
    strip = image[:, 0:scene.right_edge_px].astype(int)
    ground = np.median(strip[height // 2, 4:10].reshape(-1, 3), axis=0)
    ink = np.abs(strip - ground).sum(axis=2) > GROUND_DELTA
    row_ink = ink.mean(axis=1) > ROW_INK_FRACTION
    bands: list[tuple[int, int]] = []
    start: int | None = None
    for row, hit in enumerate(row_ink):
        if hit and start is None:
            start = row
        if not hit and start is not None:
            bands.append((start, row - 1))
            start = None
    if start is not None:
        bands.append((start, height - 1))
    return [band for band in bands if band[1] - band[0] + 1 >= SUBSTANTIAL_RUN_PX], height


def measured_void_pct(image: np.ndarray, scene: ReferenceScene) -> float:
    bands, height = sidebar_bands(image, scene)
    if len(bands) < 2:
        return 0.0
    voids = [bands[i + 1][0] - bands[i][1] - 1 for i in range(len(bands) - 1)]
    return round(100.0 * max(voids) / height, 2)


def measured_nav_tail_pct(image: np.ndarray, scene: ReferenceScene) -> float:
    """Ground below the last substantial row of one scene's nav stack."""
    bands, height = sidebar_bands(image, scene)
    if not bands or bands[-1][0] > SIDEBAR_STACK_BOTTOM_PX:
        return 100.0
    return round(100.0 * max(0, height - 1 - bands[-1][1]) / height, 2)


def measured_chrome_band_pct(image: np.ndarray, scene: ReferenceScene) -> float:
    """The share of one sidebar below the final navigation band that has ink."""
    bands, height = sidebar_bands(image, scene)
    below = [end - start + 1 for start, end in bands
             if start > SIDEBAR_STACK_BOTTOM_PX]
    return round(100.0 * max(below, default=0) / height, 2)


def exported_limits() -> dict[str, float]:
    document = json.loads(EXPORT.read_text(encoding="utf-8"))
    limits = document.get("navigation_limits")
    if not isinstance(limits, dict):
        return {}
    keys = ("chrome_band_limit_pct", "void_limit_pct", "nav_tail_ground_limit_pct")
    if any(key not in limits for key in keys):
        return {}
    return {key: float(limits[key]) for key in keys}


def exported_tail_bands(document: dict) -> tuple[list[dict], list[str]]:
    table = document.get("navigation_limits", {}).get("nav_tail_limits")
    if not isinstance(table, dict):
        return [], ["the exported table carries no nav_tail_limits height ladder"]
    bands = table.get("bands")
    if not isinstance(bands, list) or len(bands) != len(EXPECTED_TAIL_BANDS):
        return [], ["the exported nav-tail height ladder no longer has four bands"]
    problems: list[str] = []
    for index, (band, expected) in enumerate(zip(bands, EXPECTED_TAIL_BANDS)):
        name, minimum, maximum, cap = expected
        if (band.get("id"), band.get("min_height_px"),
                band.get("max_height_px"), band.get("limit_pct")) != (
                    name, minimum, maximum, cap):
            problems.append(
                f"the exported nav-tail band {index} no longer adjudicates {expected}")
        if index and bands[index - 1].get("max_height_px") != band.get("min_height_px"):
            problems.append("the exported nav-tail height bands no longer tile the axis")
    if table.get("stack_bottom_px") != SIDEBAR_STACK_BOTTOM_PX:
        problems.append("the exported nav-tail stack bottom no longer matches both scenes")
    return bands, problems


def findings() -> list[str]:
    found: list[str] = []
    if len(REFERENCES) < 2:
        found.append("both tracked FlClash desktop scenes must be measured")
    measurements: list[dict[str, float]] = []
    for scene in REFERENCES:
        path = ROOT / scene.path
        if not path.is_file():
            found.append(f"{scene.path} is missing")
            continue
        if digest(scene) != scene.sha256:
            found.append(f"{scene.path} changed; re-adjudicate its ink bands")
            continue
        image = load_scene(scene)
        values = {
            "void": measured_void_pct(image, scene),
            "nav_tail": measured_nav_tail_pct(image, scene),
            "chrome_band": measured_chrome_band_pct(image, scene),
        }
        measurements.append(values)
        if abs(values["void"] - scene.expected_void_pct) > 0.05:
            found.append(
                f"{scene.path} measures {values['void']}% void, not the documented "
                f"{scene.expected_void_pct}%"
            )
        if abs(values["nav_tail"] - scene.expected_tail_pct) > 0.05:
            found.append(
                f"{scene.path} measures {values['nav_tail']}% trailing ground, not the "
                f"documented {scene.expected_tail_pct}%"
            )
        if values["chrome_band"] != 0.0:
            found.append(
                f"{scene.path} paints {values['chrome_band']}% ink below the nav stack"
            )
    document = json.loads(EXPORT.read_text(encoding="utf-8"))
    limits = exported_limits()
    if not limits:
        return found + ["the exported table carries no navigation_limits"]
    _, band_problems = exported_tail_bands(document)
    found.extend(band_problems)
    if measurements:
        worst_chrome = max(item["chrome_band"] for item in measurements)
        worst_void = max(item["void"] for item in measurements)
        worst_tail = max(item["nav_tail"] for item in measurements)
        if limits["chrome_band_limit_pct"] != worst_chrome:
            found.append(
                f"the exported chrome_band cap is {limits['chrome_band_limit_pct']}%, "
                f"but the references measure {worst_chrome}% below the nav stack"
            )
        multiple = limits["void_limit_pct"] / worst_void
        if not MIN_SCALE_MULTIPLE <= multiple <= MAX_SCALE_MULTIPLE:
            found.append(
                f"the exported void cap {limits['void_limit_pct']}% is {multiple:.2f}x the "
                f"worst reference void; the documented scale headroom is "
                f"[{MIN_SCALE_MULTIPLE}, {MAX_SCALE_MULTIPLE}]x"
            )
        headroom = limits["nav_tail_ground_limit_pct"] - worst_tail
        if not 0.0 <= headroom <= MAX_TAIL_HEADROOM_PCT:
            found.append(
                f"the exported nav-tail cap {limits['nav_tail_ground_limit_pct']}% gives "
                f"{headroom:.2f}pp over the worst reference's {worst_tail}%; legal headroom "
                f"is [0, {MAX_TAIL_HEADROOM_PCT}]pp and cannot outlaw any tracked scene"
            )
        if limits["nav_tail_ground_limit_pct"] != EXPECTED_TAIL_LIMIT_PCT:
            found.append(
                f"the exported nav-tail cap is {limits['nav_tail_ground_limit_pct']}%, "
                f"but both real scenes adjudicate {EXPECTED_TAIL_LIMIT_PCT}%"
            )

    source = CONTRACT.read_text(encoding="utf-8")
    declared = re.search(r"pub const NAV_VOID_LIMIT_PCT:\s*f64\s*=\s*([0-9.]+)\s*;", source)
    if declared is None or float(declared.group(1)) != limits["void_limit_pct"]:
        found.append("the contract's NAV_VOID_LIMIT_PCT and the export disagree")
    declared_tail = re.search(
        r"pub const NAV_TAIL_GROUND_LIMIT_PCT:\s*f64\s*=\s*([0-9.]+)\s*;", source)
    if (declared_tail is None
            or float(declared_tail.group(1)) != limits["nav_tail_ground_limit_pct"]):
        found.append("the contract's NAV_TAIL_GROUND_LIMIT_PCT and the export disagree")
    declared_bands = re.search(
        r"pub const NAV_TAIL_GROUND_LIMIT_BANDS: \[NavTailGroundBand; 4\]", source)
    if declared_bands is None:
        found.append("the contract's typed nav-tail height bands are missing")

    standard = STANDARD.read_text(encoding="utf-8")
    reference_doc = REFERENCE_DOC.read_text(encoding="utf-8")
    facts = ("3.06", "3.49", "65.17", "70.24", "8%", "73%", "18%", "50%", "60%")
    for fact in facts:
        if fact not in standard:
            found.append(f"the standard no longer publishes the measured sidebar fact {fact}")
    for fact in ("70.24", "73%", "flclash-desktop-proxies.png"):
        if fact not in reference_doc:
            found.append(f"the upstream reference no longer publishes the real scene fact {fact}")
    for leg in ("flclash-sidebar-void-guard.py --self-test",
                "flclash-sidebar-void-guard.py --mode enforce"):
        if leg not in GATE.read_text(encoding="utf-8"):
            found.append(f"the structure gate is missing '{leg}'")
    return found


class GuardSelfTest(unittest.TestCase):
    def setUp(self) -> None:
        self.images = {scene.path: load_scene(scene) for scene in REFERENCES}
        self.dashboard, self.proxies = REFERENCES

    def test_the_live_reference_and_export_agree(self) -> None:
        self.assertEqual(findings(), [])

    def test_both_scenes_measure_their_documented_gaps(self) -> None:
        for scene in REFERENCES:
            image = self.images[scene.path]
            self.assertEqual(measured_void_pct(image, scene), scene.expected_void_pct)
            self.assertEqual(measured_nav_tail_pct(image, scene), scene.expected_tail_pct)
            self.assertEqual(measured_chrome_band_pct(image, scene), 0.0)

    def test_a_dashboard_only_tail_cap_would_outlaw_the_proxies_scene(self) -> None:
        global EXPORT
        original_export = EXPORT
        with tempfile.TemporaryDirectory() as directory:
            planted = pathlib.Path(directory) / "page-structure.json"
            document = json.loads(original_export.read_text(encoding="utf-8"))
            document["navigation_limits"]["nav_tail_ground_limit_pct"] = 68.0
            planted.write_text(json.dumps(document), encoding="utf-8")
            EXPORT = planted
            try:
                self.assertTrue(any("nav-tail cap" in item for item in findings()),
                                findings())
                self.assertGreater(self.proxies.expected_tail_pct, 68.0)
            finally:
                EXPORT = original_export

    def test_a_widened_nav_tail_cap_is_rejected(self) -> None:
        global EXPORT
        original_export = EXPORT
        with tempfile.TemporaryDirectory() as directory:
            planted = pathlib.Path(directory) / "page-structure.json"
            document = json.loads(original_export.read_text(encoding="utf-8"))
            document["navigation_limits"]["nav_tail_ground_limit_pct"] = 80.0
            planted.write_text(json.dumps(document), encoding="utf-8")
            EXPORT = planted
            try:
                self.assertTrue(any("nav-tail cap" in item for item in findings()))
            finally:
                EXPORT = original_export

    def test_the_height_banded_tail_table_is_required_and_tight(self) -> None:
        global EXPORT
        original_export = EXPORT
        document = json.loads(original_export.read_text(encoding="utf-8"))
        table = document["navigation_limits"]["nav_tail_limits"]
        self.assertEqual(exported_tail_bands(document)[1], [])
        self.assertLess(table["bands"][0]["limit_pct"],
                        self.proxies.expected_tail_pct)
        with tempfile.TemporaryDirectory() as directory:
            planted = pathlib.Path(directory) / "page-structure.json"
            document["navigation_limits"]["nav_tail_limits"]["bands"][0]["limit_pct"] = 73.0
            planted.write_text(json.dumps(document), encoding="utf-8")
            EXPORT = planted
            try:
                self.assertTrue(any("nav-tail band 0" in item
                                    for item in findings()), findings())
            finally:
                EXPORT = original_export

    def test_a_reference_with_a_status_card_below_the_stack_is_not_zero_chrome(self) -> None:
        planted = self.images[self.proxies.path].copy()
        planted[900:1000, 20:200] = 200
        self.assertEqual(
            measured_chrome_band_pct(self.images[self.proxies.path], self.proxies), 0.0)
        self.assertGreater(measured_chrome_band_pct(planted, self.proxies), 0.0)

    def test_dropping_the_taller_scene_is_rejected(self) -> None:
        global REFERENCES
        original = REFERENCES
        REFERENCES = (self.dashboard,)
        try:
            self.assertTrue(any("both tracked FlClash desktop scenes" in item
                                for item in findings()), findings())
        finally:
            REFERENCES = original

    def test_a_wrong_reference_digest_is_rejected(self) -> None:
        global REFERENCES
        original = REFERENCES
        REFERENCES = (
            dataclasses.replace(self.dashboard, sha256="0" * 64),
            self.proxies,
        )
        try:
            self.assertTrue(any("changed" in item for item in findings()), findings())
        finally:
            REFERENCES = original

    def test_a_flat_copy_of_the_reference_share_is_rejected(self) -> None:
        global EXPORT
        original_export = EXPORT
        with tempfile.TemporaryDirectory() as directory:
            planted = pathlib.Path(directory) / "page-structure.json"
            document = json.loads(original_export.read_text(encoding="utf-8"))
            document["navigation_limits"]["void_limit_pct"] = self.proxies.expected_void_pct
            planted.write_text(json.dumps(document), encoding="utf-8")
            EXPORT = planted
            try:
                self.assertTrue(any("scale headroom" in item for item in findings()))
            finally:
                EXPORT = original_export

    def test_a_widened_cap_is_rejected(self) -> None:
        global EXPORT
        original_export = EXPORT
        with tempfile.TemporaryDirectory() as directory:
            planted = pathlib.Path(directory) / "page-structure.json"
            document = json.loads(original_export.read_text(encoding="utf-8"))
            document["navigation_limits"]["void_limit_pct"] = 20.0
            planted.write_text(json.dumps(document), encoding="utf-8")
            EXPORT = planted
            try:
                self.assertTrue(any("scale headroom" in item for item in findings()))
            finally:
                EXPORT = original_export

    def test_a_missing_gate_leg_is_rejected(self) -> None:
        global GATE
        original_gate = GATE
        with tempfile.TemporaryDirectory() as directory:
            planted = pathlib.Path(directory) / "check-structure.sh"
            planted.write_text(
                original_gate.read_text(encoding="utf-8").replace(
                    "flclash-sidebar-void-guard.py --mode enforce\n", "", 1),
                encoding="utf-8")
            GATE = planted
            try:
                self.assertTrue(any("structure gate is missing" in item for item in findings()))
            finally:
                GATE = original_gate


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("report", "enforce"), default="enforce")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(GuardSelfTest)
        result = unittest.TextTestRunner(verbosity=1).run(suite)
        return 0 if result.wasSuccessful() else 1
    problems = findings()
    for problem in problems:
        print(f"flclash sidebar void guard: {problem}", file=sys.stderr)
    if problems and args.mode == "enforce":
        return 1
    print(
        "flclash sidebar void guard: "
        f"{len(REFERENCES)} real scenes, exported limits {exported_limits()}, "
        f"findings={len(problems)}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
