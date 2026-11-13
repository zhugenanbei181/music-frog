#!/usr/bin/env python3
"""Pin the `dense` space-utilization cap to the tracked FlClash dashboard scene.

`docs/SPACE_UTILIZATION_STANDARD.md` §3.1 allows a dense (card-grid) page to
leave 12% of its body as ground. Its published 依据 used to read "the
reference dashboard leaves 0.0%", which is the *tail* axis, and not even the
truthful tail: measured with the standard's own box the reference dashboard's
worst axis is its **interior** dead band, and the tail is only 0.0% once the
reference's own shell reserve is excluded. A 依据 that quotes the wrong axis
is how a cap stops being checkable -- b201 fixed the `content` cap the same
way, and `dense` was simply never measured.

This guard measures the tracked dashboard PNG with the judge's own ruler
(`scripts/parity/space_utilization.py`), so the number published in the
standard is the number the pixel leg enforces, and neither keeps a private
copy of the box:

* the page body excludes the sidebar column and the shell's title strip (§2);
* the shell's own bottom reserve is excluded, taken from the digest-pinned
  reserve guard's measurement rather than a second literal;
* ground is sampled from the body's own left margin strip, never from the
  majority colour inside the box.

It then requires:

* the tracked PNG to be the pinned bytes;
* the measured `unused` (worse of tail and band) to equal what the standard
  publishes;
* the exported `dense` cap to sit above that measurement by a bounded margin;
* the contract source and the export to agree on that cap;
* the standard's §3.1 dense row to publish both axis numbers, so the prose
  cannot silently return to quoting only one of them;
* both legs of this guard to be wired into the structure gate.

No surface pixels are judged here. `space_utilization.py` stays the single
entry for published receipts; this guard only anchors the bound it consumes.
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import pathlib
import re
import sys
import tempfile
import unittest

import hashlib
import numpy as np
from PIL import Image

ROOT = pathlib.Path(__file__).resolve().parents[2]
JUDGE = ROOT / "scripts/parity/space_utilization.py"
RESERVE_GUARD = ROOT / "scripts/quality/flclash-shell-reserve-guard.py"
REFERENCE = ROOT / "docs/reference/flclash-desktop/flclash-desktop-dashboard.png"
EXPORT = ROOT / "crates/infiltrator-contract/assets/page-structure.json"
CONTRACT = ROOT / "crates/infiltrator-contract/src/page_structure.rs"
STANDARD = ROOT / "docs/SPACE_UTILIZATION_STANDARD.md"
GATE = ROOT / "scripts/quality/check-structure.sh"

# The tracked dashboard scene, measured here rather than remembered: its
# sidebar column ends at x429 and the shell's title strip ends at y100, the
# same two edges both sibling reference guards use.
SIDEBAR_RIGHT_EDGE_PX = 429
BODY_TOP_PX = 100
# The published 依据 must state the worst axis (the interior band) AND the
# tail it used to quote alone, so a later edit cannot drop back to one number.
MEASURED_UNUSED_PCT = 4.7
MEASURED_TAIL_PCT = 0.0
MARGIN_LIMIT_PCT = 8.0


def _load(name: str, path: pathlib.Path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


def judge_module():
    """The single measurement entry, consumed rather than reimplemented."""
    return _load("space_utilization_under_guard", JUDGE)


def reserve_module():
    """The digest-pinned shell reserve, consumed rather than copied."""
    return _load("flclash_shell_reserve_under_guard", RESERVE_GUARD)


def reference_reserve_px() -> int:
    """The reserve the reference itself keeps, in scene pixels.

    Derived through the reserve guard's own dp measurement so this file holds
    no second copy of 39px, 1.95 or 20dp; the reserve guard is what pins that
    figure to the tracked pixels.
    """
    guard = reserve_module()
    with Image.open(REFERENCE) as handle:
        image = np.array(handle.convert("RGB"))
    return round(guard.measured_reserve_dp(image) * guard.PX_PER_DP)



def exported_page_padding_px() -> int:
    """The page gutter the contract publishes, read from the export.

    The box needs one gutter value; taking it from the exported table keeps
    this file free of a second copy of the number. The reading is insensitive
    to it anyway (4.7% reproduces at 16px and at the scene's own ~31px gutter)
    because ground is sampled inside the margin strip either way.
    """
    document = json.loads(EXPORT.read_text(encoding="utf-8"))
    return int(document["page_padding"]["px"])


def measured_axes(image: np.ndarray, exclude_reserve: bool = True) -> dict:
    """The reference dashboard's page-body axes, through the judge's ruler."""
    judge = judge_module()
    height, width, _ = image.shape
    chrome = dict(
        nav_form="standard",
        nav_column_px=SIDEBAR_RIGHT_EDGE_PX,
        nav_stack_bottom_px=579,
        nav_content_offset_px=0,
        page_margin_px=exported_page_padding_px(),
        shell_top_air_px=BODY_TOP_PX,
        shell_bottom_air_px=reference_reserve_px() if exclude_reserve else 0,
        dock_band_px=0,
        scale_percent=100,
        sidebar_expansion="expanded",
        touch_surface=0,
        sidebar_toggle_px=0,
    )
    axes = judge.vertical(judge.row_usage(image, chrome))
    axes["unused_pct"] = max(axes["tail_pct"], axes["band_pct"])
    return axes


def digest(path: pathlib.Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def pinned_reference_digest() -> str:
    """The digest the reserve guard already pins; no second copy here."""
    return reserve_module().REFERENCE_SHA256


def exported_dense_limit() -> float:
    document = json.loads(EXPORT.read_text(encoding="utf-8"))
    for page in document["pages"]:
        if page["structure"] == "dense":
            return float(page["ground_limit_pct"])
    raise ValueError("the exported table has no dense-shaped page")


def findings() -> list[str]:
    found: list[str] = []
    if not REFERENCE.is_file():
        return [f"{REFERENCE.relative_to(ROOT)} is missing"]
    pinned = pinned_reference_digest()
    actual_digest = digest(REFERENCE)
    if actual_digest != pinned:
        found.append(
            f"the reference PNG sha256 is {actual_digest}, not the pinned "
            f"{pinned}; re-adjudicate the measurement before reusing it"
        )
        return found

    axes = measured_axes(np.array(Image.open(REFERENCE).convert("RGB")))
    if abs(axes["unused_pct"] - MEASURED_UNUSED_PCT) > 0.05:
        found.append(
            f"the reference dashboard body measures {axes['unused_pct']}% unused, "
            f"not the published {MEASURED_UNUSED_PCT}%"
        )
    if abs(axes["tail_pct"] - MEASURED_TAIL_PCT) > 0.05:
        found.append(
            f"the reference dashboard tail measures {axes['tail_pct']}%, not the "
            f"published {MEASURED_TAIL_PCT}%"
        )
    if axes["band_pct"] <= axes["tail_pct"]:
        found.append(
            "the dense 依据's premise is gone: the interior band is no longer the "
            f"worst axis (band {axes['band_pct']}% vs tail {axes['tail_pct']}%), so "
            "the standard must stop naming both in that order"
        )

    limit = exported_dense_limit()
    margin = round(limit - MEASURED_UNUSED_PCT, 2)
    if not 0.0 < margin <= MARGIN_LIMIT_PCT:
        found.append(
            f"the exported dense limit {limit}% is {margin}pp above the measured "
            f"reference; the margin must stay within (0, {MARGIN_LIMIT_PCT}]pp"
        )

    source = CONTRACT.read_text(encoding="utf-8")
    declared = re.search(
        r"pub const DENSE_PAGE_GROUND_LIMIT_PCT:\s*f64\s*=\s*([0-9.]+)\s*;", source)
    if declared is None:
        found.append("the contract no longer declares a numeric dense-limit constant")
    elif float(declared.group(1)) != limit:
        found.append(
            f"the contract source declares {declared.group(1)}% but the export "
            f"publishes {limit}%; rerun export_page_structure"
        )

    standard_text = STANDARD.read_text(encoding="utf-8")
    # The §3.1 row wraps onto a continuation line, so a match that stops at the
    # first newline would let the measured numbers move off the judged line and
    # out of this leg's reach. A logical row runs to the next `|` row.
    row = re.search(r"^\| `dense` \|.*?(?=^\| |\Z)", standard_text,
                    re.MULTILINE | re.DOTALL)
    if row is None:
        found.append("the standard lost its §3.1 dense row")
    else:
        body = row.group(0)
        if f"{MEASURED_UNUSED_PCT}%" not in body or f"{MEASURED_TAIL_PCT}%" not in body:
            found.append(
                "the §3.1 dense row no longer publishes both measured axes "
                f"({MEASURED_UNUSED_PCT}% worst, {MEASURED_TAIL_PCT}% tail); "
                "quoting only one is how this cap lost its 依据"
            )
        if str(int(limit)) not in body:
            found.append(
                f"the §3.1 dense row no longer publishes the exported {limit}% cap"
            )
        if "0.0%" in body and "4.7" not in body:
            found.append(
                "the §3.1 dense row is back to quoting the tail alone as if it were "
                "the whole measurement"
            )
    for leg in ("flclash-dashboard-ground-guard.py --self-test",
                "flclash-dashboard-ground-guard.py --mode enforce"):
        if leg not in GATE.read_text(encoding="utf-8"):
            found.append(f"the structure gate is missing '{leg}'")
    return found


class GuardSelfTest(unittest.TestCase):
    def setUp(self) -> None:
        with Image.open(REFERENCE) as handle:
            self.image = np.array(handle.convert("RGB"))

    def test_the_live_reference_and_export_agree(self) -> None:
        self.assertEqual(findings(), [])

    def test_the_worst_axis_is_the_band_not_the_tail(self) -> None:
        """The published number is `unused`, and it is not the tail.

        b253 found the old 依据 had quoted the tail as if it were the whole
        measurement; this pins which axis actually constrains a card grid.
        """
        axes = measured_axes(self.image)
        self.assertGreater(axes["band_pct"], axes["tail_pct"])
        self.assertEqual(axes["unused_pct"], axes["band_pct"])

    def test_the_reserve_changes_the_reading(self) -> None:
        """The §2 box excludes the shell's own reserve, and that is measurable.

        Without the exclusion the reference's tail (2.4%) is charged to its
        page body; with it the tail is zero and only the interior band counts.
        """
        with_reserve = measured_axes(self.image, exclude_reserve=True)
        without = measured_axes(self.image, exclude_reserve=False)
        self.assertEqual(with_reserve["tail_pct"], MEASURED_TAIL_PCT)
        self.assertGreater(without["tail_pct"], MEASURED_TAIL_PCT)

    def test_a_swapped_reference_is_rejected(self) -> None:
        """Another tracked scene cannot stand in for this one.

        The proxies scene is a sparse `content` page whose worst axis is its
        80.6% tail; measuring it here would report a dense page as if the
        reference left no ground at all, which is the error this guard exists
        to prevent. The digest leg must catch the swap before measurement.
        """
        global REFERENCE
        original = REFERENCE
        other = ROOT / "docs/reference/flclash-desktop/flclash-desktop-proxies.png"
        self.assertNotEqual(digest(other), pinned_reference_digest())
        REFERENCE = other
        try:
            self.assertTrue(any("sha256" in item for item in findings()))
        finally:
            REFERENCE = original

    def test_a_wrong_box_is_not_the_measurement(self) -> None:
        clipped = measured_axes(self.image[BODY_TOP_PX * 3:, :])
        self.assertNotAlmostEqual(clipped["unused_pct"], MEASURED_UNUSED_PCT, delta=0.05)

    def test_a_widened_cap_without_a_new_measurement_is_rejected(self) -> None:
        global MARGIN_LIMIT_PCT
        original = MARGIN_LIMIT_PCT
        MARGIN_LIMIT_PCT = 0.0
        try:
            self.assertTrue(any("margin must stay within" in item for item in findings()))
        finally:
            MARGIN_LIMIT_PCT = original

    def test_a_source_that_drops_the_measured_axis_is_rejected(self) -> None:
        global STANDARD
        original = STANDARD
        with tempfile.TemporaryDirectory() as directory:
            planted = pathlib.Path(directory) / "SPACE_UTILIZATION_STANDARD.md"
            text = original.read_text(encoding="utf-8")
            row = re.search(r"^\| `dense` \|.*?(?=^\| |\Z)", text,
                            re.MULTILINE | re.DOTALL).group(0)
            # Every copy of the measured worst axis leaves the row, not just the
            # one in the summary clause: the row also names it next to the
            # executor, and a mutation that left either copy would prove
            # nothing about this leg.
            planted.write_text(
                text.replace(row, row.replace("4.7", "12", 99), 1),
                encoding="utf-8")
            STANDARD = planted
            try:
                self.assertTrue(
                    any("both measured axes" in item for item in findings()))
            finally:
                STANDARD = original

    def test_a_source_back_to_quoting_the_tail_alone_is_rejected(self) -> None:
        global STANDARD
        original = STANDARD
        with tempfile.TemporaryDirectory() as directory:
            planted = pathlib.Path(directory) / "SPACE_UTILIZATION_STANDARD.md"
            text = original.read_text(encoding="utf-8")
            row = re.search(r"^\| `dense` \|.*?(?=^\| |\Z)", text,
                            re.MULTILINE | re.DOTALL).group(0)
            planted.write_text(
                text.replace(row, "| `dense` | 密铺卡片网格（概览） | **12%** | "
                                  "参考产品仪表盘实测页身空地 **0.0%** |\n", 1),
                encoding="utf-8")
            STANDARD = planted
            try:
                self.assertTrue(any("tail alone" in item for item in findings()))
            finally:
                STANDARD = original

    def test_a_missing_gate_leg_is_rejected(self) -> None:
        global GATE
        original_gate = GATE
        with tempfile.TemporaryDirectory() as directory:
            planted = pathlib.Path(directory) / "check-structure.sh"
            planted.write_text(
                original_gate.read_text(encoding="utf-8").replace(
                    "flclash-dashboard-ground-guard.py --mode enforce\n", "", 1),
                encoding="utf-8")
            GATE = planted
            try:
                self.assertTrue(any("structure gate is missing" in item
                                    for item in findings()))
            finally:
                GATE = original_gate

    def test_the_export_and_contract_cannot_disagree(self) -> None:
        """A widened contract constant without a new export must be caught."""
        global EXPORT
        original = EXPORT
        with tempfile.TemporaryDirectory() as directory:
            document = json.loads(original.read_text(encoding="utf-8"))
            for page in document["pages"]:
                if page["structure"] == "dense":
                    page["ground_limit_pct"] = 30
            planted = pathlib.Path(directory) / "page-structure.json"
            planted.write_text(json.dumps(document), encoding="utf-8")
            EXPORT = planted
            try:
                self.assertTrue(
                    any("rerun export_page_structure" in item for item in findings()))
            finally:
                EXPORT = original


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
        print(f"flclash dashboard ground guard: {problem}", file=sys.stderr)
    if problems and args.mode == "enforce":
        return 1
    print(
        "flclash dashboard ground guard: reference dashboard measures "
        f"{MEASURED_UNUSED_PCT}% unused (band; tail {MEASURED_TAIL_PCT}%), "
        f"exported dense cap {exported_dense_limit()}%, findings={len(problems)}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
