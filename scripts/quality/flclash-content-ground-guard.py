#!/usr/bin/env python3
"""Pin the `content` space-utilization cap to measured FlClash reference pixels.

`docs/SPACE_UTILIZATION_STANDARD.md` §3.1 allows a content-shaped page to
leave 82% of its body as ground. That number was justified by one sentence:
the FlClash desktop proxies page ends at y357 of a 1849px body (80.6% ground).
Until now that sentence was prose about a gitignored `.private/` image, so
nothing could detect the reference changing, the measurement box drifting, or
the exported cap quietly growing.

This guard measures the tracked reference PNG with the standard's own box:
the sidebar column and the shell's title strip are OUTSIDE the page body, and
one row counts as content only when it is a substantial run of non-ground
pixels. It then asserts that the contract's exported `content` limit equals
the measured ground plus a small, bounded margin.

No surface pixels are judged here. The judge
(`scripts/parity/space_utilization.py`) stays the single entry for published
receipts; this guard only anchors the number that judge consumes.
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
REFERENCE = ROOT / "docs/reference/flclash-desktop/flclash-desktop-proxies.png"
EXPORT = ROOT / "crates/infiltrator-contract/assets/page-structure.json"
CONTRACT = ROOT / "crates/infiltrator-contract/src/page_structure.rs"
STANDARD = ROOT / "docs/SPACE_UTILIZATION_STANDARD.md"
GATE = ROOT / "scripts/quality/check-structure.sh"

REFERENCE_SHA256 = "ebe094bae677d5419cb380e800d5eee74197d20a48891fbe25ba340af7f98e8e"
SIDEBAR_RIGHT_EDGE_PX = 431
BODY_TOP_PX = 100
SUBSTANTIAL_RUN_PX = 3
ROW_INK_PX = 50
GROUND_TOLERANCE = 12
MEASURED_GROUND_PCT = 80.6
MARGIN_LIMIT_PCT = 2.0


def digest(path: pathlib.Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def measured_ground_pct(image: np.ndarray) -> float:
    """The ground below the last substantial content run, in percent.

    The box is the page body: everything right of the sidebar and below the
    shell's title strip. Ground comes from the body's own left margin strip,
    never from the most common colour inside the body.
    """
    body = image[BODY_TOP_PX:, SIDEBAR_RIGHT_EDGE_PX + 1:]
    height = body.shape[0]
    ground = np.median(body[height // 2, 4:10].reshape(-1, 3), axis=0)
    ink = (np.abs(body.astype(int) - ground.astype(int)).max(axis=2)
           > GROUND_TOLERANCE).sum(axis=1)
    runs: list[tuple[int, int]] = []
    start: int | None = None
    for row, count in enumerate(ink):
        hit = count > ROW_INK_PX
        if hit and start is None:
            start = row
        if not hit and start is not None:
            runs.append((start, row - 1))
            start = None
    if start is not None:
        runs.append((start, height - 1))
    substantial = [end for begin, end in runs if end - begin + 1 >= SUBSTANTIAL_RUN_PX]
    if not substantial:
        return 100.0
    last = max(substantial)
    return round(100.0 * (height - 1 - last) / height, 2)


def exported_content_limit() -> float:
    document = json.loads(EXPORT.read_text(encoding="utf-8"))
    for page in document["pages"]:
        if page["structure"] == "content":
            return float(page["ground_limit_pct"])
    raise ValueError("the exported table has no content-shaped page")


def findings() -> list[str]:
    found: list[str] = []
    if not REFERENCE.is_file():
        return [f"{REFERENCE.relative_to(ROOT)} is missing"]
    actual_digest = digest(REFERENCE)
    if actual_digest != REFERENCE_SHA256:
        found.append(
            f"the reference PNG sha256 is {actual_digest}, not the pinned "
            f"{REFERENCE_SHA256}; re-adjudicate the measurement before reusing it"
        )
        return found

    with Image.open(REFERENCE) as handle:
        image = np.array(handle.convert("RGB"))
    ground = measured_ground_pct(image)
    if abs(ground - MEASURED_GROUND_PCT) > 0.05:
        found.append(
            f"the reference body measures {ground}% ground, not the documented "
            f"{MEASURED_GROUND_PCT}%"
        )

    limit = exported_content_limit()
    margin = round(limit - MEASURED_GROUND_PCT, 2)
    if not 0.0 < margin <= MARGIN_LIMIT_PCT:
        found.append(
            f"the exported content limit {limit}% is {margin}pp above the "
            f"measured reference; the margin must stay within "
            f"(0, {MARGIN_LIMIT_PCT}]pp"
        )
    if re.search(rf"\b{MEASURED_GROUND_PCT:.1f}\b", STANDARD.read_text(encoding="utf-8")) is None:
        found.append("the space-utilization standard no longer publishes the measured reference ground")
    if "CONTENT_PAGE_GROUND_LIMIT_PCT" not in CONTRACT.read_text(encoding="utf-8"):
        found.append("the contract lost its single content-limit constant")
    source = CONTRACT.read_text(encoding="utf-8")
    declared = re.search(
        r"pub const CONTENT_PAGE_GROUND_LIMIT_PCT:\s*f64\s*=\s*([0-9.]+)\s*;", source)
    if declared is None:
        found.append("the contract no longer declares a numeric content-limit constant")
    elif float(declared.group(1)) != limit:
        found.append(
            f"the contract source declares {declared.group(1)}% but the export "
            f"publishes {limit}%; rerun export_page_structure"
        )
    standard_text = STANDARD.read_text(encoding="utf-8")
    section = re.search(r"^## 4\..*?(?=^## 5\.)", standard_text, re.MULTILINE | re.DOTALL)
    if section is None:
        found.append("the space-utilization standard lost its §4 no-exemption section")
    else:
        body = section.group(0)
        if "半屏以上留成地面" in body and "b149" not in body:
            found.append(
                "§4 still carries the b144 one-size-fits-all ban on half-screen "
                "ground, which contradicts the shape-based content cap in §3.1"
            )
        if "80.6" not in body or "82" not in body:
            found.append("§4 no longer reconciles its no-exemption rule with the measured content cap")
    for leg in ("flclash-content-ground-guard.py --self-test",
                "flclash-content-ground-guard.py --mode enforce"):
        if leg not in GATE.read_text(encoding="utf-8"):
            found.append(f"the structure gate is missing '{leg}'")
    return found


class GuardSelfTest(unittest.TestCase):
    def setUp(self) -> None:
        with Image.open(REFERENCE) as handle:
            self.image = np.array(handle.convert("RGB"))

    def test_the_live_reference_and_export_agree(self) -> None:
        self.assertEqual(findings(), [])

    def test_the_body_box_is_the_documented_one(self) -> None:
        measured = measured_ground_pct(self.image)
        self.assertAlmostEqual(measured, MEASURED_GROUND_PCT, delta=0.05)
        self.assertEqual(round(measured, 1), MEASURED_GROUND_PCT)

    def test_a_wrong_box_is_not_the_measurement(self) -> None:
        clipped_top = measured_ground_pct(self.image[BODY_TOP_PX * 2:, :])
        self.assertNotAlmostEqual(clipped_top, MEASURED_GROUND_PCT, delta=0.05)

    def test_a_filled_body_has_no_ground(self) -> None:
        filled = np.zeros((400, 1200, 3), dtype=np.uint8)
        filled[:, SIDEBAR_RIGHT_EDGE_PX + 11:, :] = 40
        self.assertEqual(measured_ground_pct(filled), 0.0)

    def test_a_wrong_reference_digest_is_rejected(self) -> None:
        global REFERENCE_SHA256
        original = REFERENCE_SHA256
        REFERENCE_SHA256 = "0" * 64
        try:
            self.assertTrue(any("sha256" in item for item in findings()))
        finally:
            REFERENCE_SHA256 = original

    def test_a_widened_cap_without_a_new_measurement_is_rejected(self) -> None:
        global MARGIN_LIMIT_PCT
        original = MARGIN_LIMIT_PCT
        MARGIN_LIMIT_PCT = 0.0
        try:
            self.assertTrue(any("margin must stay within" in item for item in findings()))
        finally:
            MARGIN_LIMIT_PCT = original

    def test_a_missing_gate_leg_is_rejected(self) -> None:
        global GATE
        original_gate = GATE
        with tempfile.TemporaryDirectory() as directory:
            planted = pathlib.Path(directory) / "check-structure.sh"
            planted.write_text(
                original_gate.read_text(encoding="utf-8").replace(
                    "flclash-content-ground-guard.py --mode enforce\n", "", 1),
                encoding="utf-8")
            GATE = planted
            try:
                self.assertTrue(any("structure gate is missing" in item for item in findings()))
            finally:
                GATE = original_gate

    def test_a_stale_b144_section_four_is_rejected(self) -> None:
        global STANDARD
        original_standard = STANDARD
        with tempfile.TemporaryDirectory() as directory:
            planted = pathlib.Path(directory) / "SPACE_UTILIZATION_STANDARD.md"
            planted.write_text(
                "## 4. 内容少不是豁免\n\n这是本标准最容易被误解之处，必须写死：\n"
                "- 页面仍不得把半屏以上留成地面。\n\n## 5. 超标时的三种收口\n",
                encoding="utf-8")
            STANDARD = planted
            try:
                self.assertTrue(any("b144 one-size-fits-all" in item for item in findings()))
            finally:
                STANDARD = original_standard


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
        print(f"flclash content ground guard: {problem}", file=sys.stderr)
    if problems and args.mode == "enforce":
        return 1
    print(
        "flclash content ground guard: reference measures "
        f"{MEASURED_GROUND_PCT}% ground, exported content cap "
        f"{exported_content_limit()}%, findings={len(problems)}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
