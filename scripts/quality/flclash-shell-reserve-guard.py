#!/usr/bin/env python3
"""Pin the shell's bottom reserve to measured FlClash reference pixels.

`docs/SPACE_UTILIZATION_STANDARD.md` §2 puts the shell's own air *outside* the
measured page body. That is right for the page's account - a list card that
fills the body must not be charged for the shell's breathing room - and it had
one consequence nobody noticed: **the reserve itself was never measured**. No
cap, no debt entry and no receipt leg ever looked at that band, so the desktop
shell's reserve grew to 72u per side while the tracked reference scene keeps
20dp. At 1180x780 the shell spends 9.2% of every window on a band no standard
has ever judged, which is exactly the "we must set a proportion standard per
situation" the standard exists to answer.

This guard measures the reference's own reserve with the standard's own box
rule - the page body is everything right of the sidebar and below the shell's
title strip, one row counts as content only when it is a substantial run of
non-ground pixels - and then requires:

* the tracked PNG to be the pinned bytes (a swapped reference cannot move the
  number without being named);
* the measured reserve, in device-independent pixels, to equal the contract's
  `SHELL_BOTTOM_AIR_REFERENCE_PX`;
* the exported `shell_air` triple to be that measurement plus a *bounded*
  headroom, so widening the cap without a new measurement fails here rather
  than in a code review;
* the L2 standard to publish the measured number, so the prose and the pixels
  cannot disagree (the defect b207 caught in the other direction);
* both legs of this guard to be wired into the structure gate.

No surface pixels are judged here. `scripts/parity/space_utilization.py` stays
the single entry for published receipts; this guard only anchors the bound
that judge consumes.
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
# The tracked scene's own geometry, measured here rather than remembered: the
# sidebar column ends at x429 and the shell's title strip ends at y100.
SIDEBAR_RIGHT_EDGE_PX = 429
BODY_TOP_PX = 100
SUBSTANTIAL_RUN_PX = 3
ROW_INK_PX = 50
GROUND_TOLERANCE = 12
# 39px of ground below the last substantial content row at 1.95 px/dp.
MEASURED_RESERVE_DP = 20.0
PX_PER_DP = 1.95
HEADROOM_LIMIT_DP = 8.0
# The standard publishes the measurement in this exact spelling. Matching a
# bare "20dp" would also match the "220dp" sidebar width in §3.4, so the leg
# would stay green after the prose lost the number it is supposed to carry.
STANDARD_MEASUREMENT_MARK = "39px = 20.00dp"
STANDARD_SECTION_MARK = "### 3.6"


def digest(path: pathlib.Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def measured_reserve_dp(image: np.ndarray) -> float:
    """The reference's own bottom reserve, in device-independent pixels.

    The box is the page body: right of the sidebar, below the shell's title
    strip. Ground is the body's own left margin strip, never the most common
    colour inside the body - a frame-filling card must not become its own
    reference.
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
        raise AssertionError("the reference page body paints no content row")
    return round((height - 1 - max(substantial)) / PX_PER_DP, 2)


def exported_shell_air() -> dict[str, int]:
    document = json.loads(EXPORT.read_text(encoding="utf-8"))
    air = document.get("shell_air")
    if not isinstance(air, dict):
        return {}
    return {key: int(air[key]) for key in
            ("bottom_reference_px", "bottom_headroom_px", "bottom_limit_px")
            if key in air}


def contract_declaration(name: str) -> int | None:
    match = re.search(
        rf"pub const {name}:\s*u16\s*=\s*(?:([0-9]+)|"
        r"SHELL_BOTTOM_AIR_REFERENCE_PX \+ SHELL_BOTTOM_AIR_HEADROOM_PX)\s*;",
        CONTRACT.read_text(encoding="utf-8"))
    if match is None:
        return None
    if match.group(1) is not None:
        return int(match.group(1))
    reference = contract_declaration("SHELL_BOTTOM_AIR_REFERENCE_PX")
    headroom = contract_declaration("SHELL_BOTTOM_AIR_HEADROOM_PX")
    if reference is None or headroom is None:
        return None
    return reference + headroom


def findings() -> list[str]:
    found: list[str] = []
    if not REFERENCE.is_file():
        return [f"{REFERENCE.relative_to(ROOT)} is missing"]
    actual_digest = digest(REFERENCE)
    if actual_digest != REFERENCE_SHA256:
        return [
            f"the reference PNG sha256 is {actual_digest}, not the pinned "
            f"{REFERENCE_SHA256}; re-adjudicate the measurement before reusing it"
        ]

    with Image.open(REFERENCE) as handle:
        image = np.array(handle.convert("RGB"))
    reserve = measured_reserve_dp(image)
    if abs(reserve - MEASURED_RESERVE_DP) > 0.05:
        found.append(
            f"the reference page body reserves {reserve}dp below its last "
            f"content row, not the documented {MEASURED_RESERVE_DP}dp"
        )

    air = exported_shell_air()
    if not air:
        return found + ["page-structure.json carries no shell_air; "
                        "rerun export_page_structure"]
    if air["bottom_reference_px"] != round(MEASURED_RESERVE_DP):
        found.append(
            f"the exported bottom reference is {air['bottom_reference_px']}px while "
            f"the tracked scene measures {MEASURED_RESERVE_DP}dp"
        )
    if not 0 < air["bottom_headroom_px"] <= HEADROOM_LIMIT_DP:
        found.append(
            f"the exported headroom {air['bottom_headroom_px']}px is outside "
            f"(0, {HEADROOM_LIMIT_DP}]px; headroom lets a surface round to its "
            "grid, it is not a licence to widen the bound"
        )
    if air["bottom_limit_px"] != air["bottom_reference_px"] + air["bottom_headroom_px"]:
        found.append(
            f"the exported bound {air['bottom_limit_px']}px is not the measured "
            f"{air['bottom_reference_px']}px plus the exported "
            f"{air['bottom_headroom_px']}px headroom"
        )
    for name, key in (("SHELL_BOTTOM_AIR_REFERENCE_PX", "bottom_reference_px"),
                      ("SHELL_BOTTOM_AIR_HEADROOM_PX", "bottom_headroom_px"),
                      ("SHELL_BOTTOM_AIR_LIMIT_PX", "bottom_limit_px")):
        declared = contract_declaration(name)
        if declared is None:
            found.append(f"the contract no longer declares {name} as a numeric constant")
        elif declared != air[key]:
            found.append(
                f"the contract source declares {name} = {declared} but the export "
                f"publishes {air[key]}; rerun export_page_structure"
            )

    standard_text = STANDARD.read_text(encoding="utf-8")
    if STANDARD_MEASUREMENT_MARK not in standard_text:
        found.append(
            "the space-utilization standard no longer publishes the measured "
            f"reference reserve as {STANDARD_MEASUREMENT_MARK}"
        )
    if STANDARD_SECTION_MARK not in standard_text:
        found.append(
            f"the space-utilization standard lost its {STANDARD_SECTION_MARK} "
            "shell-reserve section"
        )
    for leg in ("flclash-shell-reserve-guard.py --self-test",
                "flclash-shell-reserve-guard.py --mode enforce"):
        if leg not in GATE.read_text(encoding="utf-8"):
            found.append(f"the structure gate is missing '{leg}'")
    return found


class GuardSelfTest(unittest.TestCase):
    def setUp(self) -> None:
        with Image.open(REFERENCE) as handle:
            self.image = np.array(handle.convert("RGB"))

    def test_the_live_reference_and_export_agree(self) -> None:
        self.assertEqual(findings(), [])

    def test_the_reserve_is_measured_in_the_documented_box(self) -> None:
        self.assertAlmostEqual(measured_reserve_dp(self.image),
                               MEASURED_RESERVE_DP, delta=0.05)

    def test_a_box_that_ignores_the_title_strip_is_not_the_measurement(self) -> None:
        # A box whose ground comes from the body's majority colour lets the
        # card be its own reference: the reference's last card then reads as
        # ground, the reserve collapses to nothing, and an oversized shell
        # band would look compliant. The standard forbids exactly this
        # self-certification, so the guard must refuse it too.
        body = self.image[BODY_TOP_PX:, SIDEBAR_RIGHT_EDGE_PX + 1:]
        height = body.shape[0]
        values, counts = np.unique(body.reshape(-1, 3), axis=0, return_counts=True)
        inside_ground = values[counts.argmax()]
        ink = (np.abs(body.astype(int) - inside_ground.astype(int)).max(axis=2)
               > GROUND_TOLERANCE).sum(axis=1)
        runs = [row for row, count in enumerate(ink) if count > ROW_INK_PX]
        self.assertEqual(round((height - 1 - max(runs)) / PX_PER_DP, 2), 0.0)
        self.assertNotEqual(measured_reserve_dp(self.image), 0.0)

    def test_a_body_painted_to_its_last_row_reserves_nothing(self) -> None:
        filled = np.zeros((400, 1200, 3), dtype=np.uint8)
        filled[:, SIDEBAR_RIGHT_EDGE_PX + 11:, :] = 40
        self.assertEqual(measured_reserve_dp(filled), 0.0)

    def test_a_wrong_reference_digest_is_rejected(self) -> None:
        global REFERENCE_SHA256
        original = REFERENCE_SHA256
        REFERENCE_SHA256 = "0" * 64
        try:
            self.assertTrue(any("sha256" in item for item in findings()))
        finally:
            REFERENCE_SHA256 = original

    def test_a_widened_bound_without_a_new_measurement_is_rejected(self) -> None:
        global HEADROOM_LIMIT_DP
        original = HEADROOM_LIMIT_DP
        HEADROOM_LIMIT_DP = 0.0
        try:
            self.assertTrue(any("headroom" in item for item in findings()))
        finally:
            HEADROOM_LIMIT_DP = original

    def test_an_export_that_disagrees_with_the_contract_is_rejected(self) -> None:
        global EXPORT
        original_export = EXPORT
        with tempfile.TemporaryDirectory() as directory:
            document = json.loads(original_export.read_text(encoding="utf-8"))
            document["shell_air"]["bottom_limit_px"] += 1
            planted = pathlib.Path(directory) / "page-structure.json"
            planted.write_text(json.dumps(document), encoding="utf-8")
            EXPORT = planted
            try:
                self.assertTrue(any("rerun export_page_structure" in item
                                    for item in findings()))
            finally:
                EXPORT = original_export

    def test_a_missing_gate_leg_is_rejected(self) -> None:
        global GATE
        original_gate = GATE
        with tempfile.TemporaryDirectory() as directory:
            planted = pathlib.Path(directory) / "check-structure.sh"
            planted.write_text(
                original_gate.read_text(encoding="utf-8").replace(
                    "flclash-shell-reserve-guard.py --mode enforce\n", "", 1),
                encoding="utf-8")
            GATE = planted
            try:
                self.assertTrue(any("structure gate is missing" in item
                                    for item in findings()))
            finally:
                GATE = original_gate

    def test_a_standard_that_dropped_the_measurement_is_rejected(self) -> None:
        global STANDARD
        original_standard = STANDARD
        with tempfile.TemporaryDirectory() as directory:
            planted = pathlib.Path(directory) / "SPACE_UTILIZATION_STANDARD.md"
            planted.write_text(original_standard.read_text(encoding="utf-8").replace(
                STANDARD_MEASUREMENT_MARK, "39px = 18.00dp"), encoding="utf-8")
            STANDARD = planted
            try:
                self.assertTrue(any("no longer publishes" in item for item in findings()))
            finally:
                STANDARD = original_standard

    def test_a_standard_that_lost_the_section_is_rejected(self) -> None:
        global STANDARD
        original_standard = STANDARD
        with tempfile.TemporaryDirectory() as directory:
            text = original_standard.read_text(encoding="utf-8")
            planted = pathlib.Path(directory) / "SPACE_UTILIZATION_STANDARD.md"
            planted.write_text(text.replace(STANDARD_SECTION_MARK, "### 9.6"),
                               encoding="utf-8")
            STANDARD = planted
            try:
                self.assertTrue(any("shell-reserve section" in item
                                    for item in findings()))
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
        print(f"flclash shell reserve guard: {problem}", file=sys.stderr)
    if problems and args.mode == "enforce":
        return 1
    air = exported_shell_air()
    print(
        "flclash shell reserve guard: reference reserves "
        f"{MEASURED_RESERVE_DP}dp, exported bound {air.get('bottom_limit_px')}px, "
        f"findings={len(problems)}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
