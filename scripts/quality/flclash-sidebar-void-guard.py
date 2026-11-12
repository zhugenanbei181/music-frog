#!/usr/bin/env python3
"""Pin the navigation-column caps to measured FlClash sidebar pixels.

`docs/SPACE_UTILIZATION_STANDARD.md` §3.1 publishes `chrome_band` 0% and
a `void` cap. The 0% was justified by prose ("the reference sidebar paints only
destinations"); the 12% was inherited from b144, whose stated reason was
"group headings and stretch air must stay at cluster boundaries" - but b164
retired group headings entirely, so the number lost its only rationale.

This guard measures the tracked FlClash desktop reference with the same ink
rule as the judge: substantial runs of non-ground rows inside the sidebar
column. It publishes two facts from the pixels:

* the largest ground run between destination rows, as a share of window
  height (the reference's own `void`);
* the ground below the final destination band, which bounds our exported
  `nav_tail_ground_limit_pct` without outlawing the reference's own air;
* that no ink band lives below the destination stack (the reference's own
  `chrome_band`).

Our shortest tier paints eight destinations in a 480px window while the
reference paints five in 1665px, so a raw share cannot be copied: the same
absolute gap is a much larger percentage of the shorter window. The exported
cap is therefore constrained to a bounded multiple of the reference's own
share, with documented headroom for that scale difference - never a flat
copy of the reference number.

No surface pixels are judged here: `scripts/parity/space_utilization.py`
remains the single judge of published receipts.
"""

from __future__ import annotations

import argparse
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
REFERENCE = ROOT / "docs/reference/flclash-desktop/flclash-desktop-dashboard.png"
EXPORT = ROOT / "crates/infiltrator-contract/assets/page-structure.json"
CONTRACT = ROOT / "crates/infiltrator-contract/src/page_structure.rs"
STANDARD = ROOT / "docs/SPACE_UTILIZATION_STANDARD.md"
GATE = ROOT / "scripts/quality/check-structure.sh"

REFERENCE_SHA256 = "f6406be498613bc0249e9ae2350125a0e670e9620d0ae4b1be1aec7353372bcc"
SIDEBAR_RIGHT_EDGE_PX = 429
SIDEBAR_STACK_BOTTOM_PX = 579
SUBSTANTIAL_RUN_PX = 3
ROW_INK_FRACTION = 0.01
GROUND_DELTA = 14
MEASURED_VOID_PCT = 3.06
MEASURED_TAIL_PCT = 65.17
MAX_TAIL_HEADROOM_PCT = 3.0

# The shortest published tier is 480px tall with eight destinations; the
# reference is 1665px with five. A fixed share copied from the reference
# would penalise the shorter window for the same absolute gap, so the cap is
# allowed a bounded multiple of the reference share, not an equal one.
MIN_SCALE_MULTIPLE = 2.0
MAX_SCALE_MULTIPLE = 4.0


def digest(path: pathlib.Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def sidebar_bands(image: np.ndarray) -> tuple[list[tuple[int, int]], int]:
    """Substantial ink bands inside the sidebar, and the window height."""
    height = image.shape[0]
    strip = image[:, 0:SIDEBAR_RIGHT_EDGE_PX].astype(int)
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


def measured_void_pct(image: np.ndarray) -> float:
    bands, height = sidebar_bands(image)
    if len(bands) < 2:
        return 0.0
    voids = [bands[i + 1][0] - bands[i][1] - 1 for i in range(len(bands) - 1)]
    return round(100.0 * max(voids) / height, 2)


def measured_nav_tail_pct(image: np.ndarray) -> float:
    """Ground below the last substantial row of the reference's nav stack."""
    bands, height = sidebar_bands(image)
    if not bands or bands[-1][0] > SIDEBAR_STACK_BOTTOM_PX:
        return 100.0
    return round(100.0 * max(0, height - 1 - bands[-1][1]) / height, 2)


def measured_chrome_band_pct(image: np.ndarray) -> float:
    """The share of the sidebar below the final navigation band that has ink."""
    bands, height = sidebar_bands(image)
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


def findings() -> list[str]:
    found: list[str] = []
    if not REFERENCE.is_file():
        return [f"{REFERENCE.relative_to(ROOT)} is missing"]
    if digest(REFERENCE) != REFERENCE_SHA256:
        found.append("the reference sidebar PNG changed; re-adjudicate its ink bands")
        return found

    with Image.open(REFERENCE) as handle:
        image = np.array(handle.convert("RGB"))
    void = measured_void_pct(image)
    if abs(void - MEASURED_VOID_PCT) > 0.05:
        found.append(
            f"the reference sidebar measures {void}% void, not the documented "
            f"{MEASURED_VOID_PCT}%"
        )
    nav_tail = measured_nav_tail_pct(image)
    if abs(nav_tail - MEASURED_TAIL_PCT) > 0.05:
        found.append(
            f"the reference sidebar measures {nav_tail}% trailing ground, not the "
            f"documented {MEASURED_TAIL_PCT}%"
        )
    chrome_band = measured_chrome_band_pct(image)

    limits = exported_limits()
    if not limits:
        return found + ["the exported table carries no navigation_limits"]
    if limits["chrome_band_limit_pct"] != chrome_band:
        found.append(
            f"the exported chrome_band cap is {limits['chrome_band_limit_pct']}%, "
            f"but the reference measures {chrome_band}% below the nav stack"
        )
    multiple = limits["void_limit_pct"] / MEASURED_VOID_PCT
    if not MIN_SCALE_MULTIPLE <= multiple <= MAX_SCALE_MULTIPLE:
        found.append(
            f"the exported void cap {limits['void_limit_pct']}% is {multiple:.2f}x the "
            f"reference void; the documented scale headroom is "
            f"[{MIN_SCALE_MULTIPLE}, {MAX_SCALE_MULTIPLE}]x"
        )
    headroom = limits["nav_tail_ground_limit_pct"] - nav_tail
    if not 0.0 <= headroom <= MAX_TAIL_HEADROOM_PCT:
        found.append(
            f"the exported nav-tail cap {limits['nav_tail_ground_limit_pct']}% gives "
            f"{headroom:.2f}pp over the reference's {nav_tail}%; legal headroom is "
            f"[0, {MAX_TAIL_HEADROOM_PCT}]pp"
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

    standard = STANDARD.read_text(encoding="utf-8")
    if "3.06" not in standard or "8%" not in standard or "65.17" not in standard:
        found.append("the standard no longer publishes both measured sidebar gaps and their caps")
    for leg in ("flclash-sidebar-void-guard.py --self-test",
                "flclash-sidebar-void-guard.py --mode enforce"):
        if leg not in GATE.read_text(encoding="utf-8"):
            found.append(f"the structure gate is missing '{leg}'")
    return found


class GuardSelfTest(unittest.TestCase):
    def setUp(self) -> None:
        with Image.open(REFERENCE) as handle:
            self.image = np.array(handle.convert("RGB"))

    def test_the_live_reference_and_export_agree(self) -> None:
        self.assertEqual(findings(), [])

    def test_the_reference_void_is_three_point_oh_six_percent(self) -> None:
        self.assertEqual(measured_void_pct(self.image), MEASURED_VOID_PCT)

    def test_the_reference_keeps_bounded_trailing_breathing_room(self) -> None:
        self.assertEqual(measured_nav_tail_pct(self.image), MEASURED_TAIL_PCT)

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

    def test_a_reference_with_a_status_card_below_the_stack_is_not_zero_chrome(self) -> None:
        planted = self.image.copy()
        planted[900:1000, 20:200] = 200
        self.assertEqual(measured_chrome_band_pct(self.image), 0.0)
        self.assertGreater(measured_chrome_band_pct(planted), 0.0)

    def test_a_wrong_reference_digest_is_rejected(self) -> None:
        global REFERENCE_SHA256
        original = REFERENCE_SHA256
        REFERENCE_SHA256 = "0" * 64
        try:
            self.assertTrue(any("PNG changed" in item for item in findings()))
        finally:
            REFERENCE_SHA256 = original

    def test_a_flat_copy_of_the_reference_share_is_rejected(self) -> None:
        global EXPORT
        original_export = EXPORT
        with tempfile.TemporaryDirectory() as directory:
            planted = pathlib.Path(directory) / "page-structure.json"
            document = json.loads(original_export.read_text(encoding="utf-8"))
            document["navigation_limits"]["void_limit_pct"] = MEASURED_VOID_PCT
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
        "flclash sidebar void guard: reference void "
        f"{MEASURED_VOID_PCT}%, exported limits {exported_limits()}, findings={len(problems)}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
