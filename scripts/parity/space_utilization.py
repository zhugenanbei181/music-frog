#!/usr/bin/env python3
"""Judge: the space utilization of a published Slint capture batch.

`docs/SPACE_UTILIZATION_STANDARD.md` owns the rules; this module is the only
measurement that decides them. The standard is a ratio, so the verdict comes
from the real pixels of a published receipt batch, never from a code read.

How a frame is measured
-----------------------
* The box is the **page body**, not the window. The shell's own reserved page
  air (8u above, 72u below on the desktop tier) is adjudicated breathing room
  and sits OUTSIDE the box, so it is never counted as waste.
* The page ground is sampled from the page's own left margin strip - never
  from the most common colour inside the body. A frame-filling list card is
  the majority colour of its own page, and letting it be the reference would
  call the ground around it "content" and the empty card "ground".
* `tail` is the ground below the last substantial content run.
* `band` is the largest ground run strictly between two content runs, i.e.
  the dead band a page pays inside its own span.
* `unused` is the worse of the two: one number per frame, one limit per tier.
* The desktop rail is judged on its own axis against the FlClash desktop
  reference: the sidebar is navigation only. `chrome_band` is the share of
  rail height spent on non-navigation blocks (status card, brand, version),
  and `void` is the largest ground run inside the nav stack. A full-width
  selected row is legitimate breathing room, NOT dead width: measuring a
  "pill tail" was a category error and has been retired.

Modes
-----
    --report            print the distribution and the worst frames
    --enforce           fail on any frame over its tier limit that is not
                        registered in the debt inventory
    --json              machine-readable measurement
    --update-debt       rewrite the inventory from the current measurement
                        (only ever a shrink: new debt is refused)
"""

from __future__ import annotations

import argparse
import csv
import json
import pathlib
import statistics
import subprocess
import sys
import tempfile
import unittest

import numpy as np
from PIL import Image

ROOT = pathlib.Path(__file__).resolve().parents[2]
CAPTURES = ROOT / ".evidence/captures/slint"
METADATA = "capture-metadata.txt"
DEBT_FILE = ROOT / "scripts/quality/space_utilization_debt.json"

RAIL_WIDTH = 228
PAGE_MARGIN = {"desktop": 30, "compact": 16, "phone": 16}
SHELL_TOP_AIR = {"desktop": 8, "compact": 0, "phone": 0}
SHELL_BOTTOM_AIR = {"desktop": 72, "compact": 0, "phone": 0}

# The limits of the standard, per tier, as a percentage of the page body.
#
# A tier limit is only the fallback. The real limit comes from the page's
# declared CONTENT SHAPE, exported by the contract
# (`crates/infiltrator-contract/assets/page-structure.json`): a framed list
# page must fill its body, a dense card grid must fill its rows, and a
# content-shaped page may end where its last block ends - the peer product
# leaves more ground than that on the very same shape, so a single global
# number would judge the reference itself as broken.
UNUSED_LIMIT = {"desktop": 12.0, "compact": 8.0, "phone": 8.0}
PAGE_STRUCTURE_FILE = ROOT / "crates/infiltrator-contract/assets/page-structure.json"
RAIL_CHROME_BAND_LIMIT = 0.0
RAIL_VOID_LIMIT = 12.0

# The contract's own limits, mirrored here only so a planted self-test can
# assert the judge read the exported table rather than a local number.
FRAMED_LIMIT = 4.0
CONTENT_LIMIT = 82.0

ROW_INK_FRACTION = 0.04
MIN_CONTENT_RUN = 3
GROUND_DELTA = 18


def tier(width: int) -> str:
    if width >= 1000:
        return "desktop"
    if width >= 700:
        return "compact"
    return "phone"


def export_is_fresh() -> list[str]:
    """The exported page-shape table must match the contract that owns it.

    The judge reads the JSON instead of keeping numbers of its own, so the
    JSON is a build artefact of the contract: if it drifts, the judge would
    judge by limits nobody declared.
    """
    binary = ROOT / "target/debug/export_page_structure"
    if not binary.is_file():
        return ["page structure exporter is not built"]
    temporary = ROOT / "target/page-structure-check.json"
    result = subprocess.run([str(binary), str(temporary)], cwd=ROOT,
                            capture_output=True, text=True, check=False)
    if result.returncode:
        return ["page structure exporter failed: " + result.stderr.strip()]
    if not PAGE_STRUCTURE_FILE.is_file():
        return ["page-structure.json is missing"]
    if temporary.read_text(encoding="utf-8") != PAGE_STRUCTURE_FILE.read_text(encoding="utf-8"):
        return ["page-structure.json is stale; rerun export_page_structure"]
    return []


def page_structures() -> dict[str, dict]:
    """The exported page-shape table, keyed by capture page slug.

    The table comes from the contract: a slug's shape is a typed fact, not a
    guess the judge makes from a scenario name.
    """
    if not PAGE_STRUCTURE_FILE.is_file():
        return {}
    document = json.loads(PAGE_STRUCTURE_FILE.read_text(encoding="utf-8"))
    return {entry["slug"]: entry for entry in document.get("slugs", [])}


def scenario_pages(run: pathlib.Path) -> dict[str, str]:
    """The capture page slug of every scenario in a published batch."""
    manifest = run / "capture-manifest.tsv"
    if not manifest.is_file():
        return {}
    pages: dict[str, str] = {}
    with manifest.open(encoding="utf-8", newline="") as handle:
        reader = csv.DictReader(handle, delimiter="\t")
        for row in reader:
            pages[row["scenario"]] = row.get("page", "")
    return pages


def limit_for(scenario: str, tier_name: str, structures: dict[str, dict],
              slug: str = "") -> float:
    """The ground limit for one frame, from its page shape when known.

    The slug is the capture page the frame renders; the shape and the limit
    come from the contract's exported table.
    """
    if slug:
        entry = structures.get(slug)
        if entry is not None:
            return float(entry["ground_limit_pct"])
    return UNUSED_LIMIT[tier_name]


def content_box(width: int, height: int):
    """(ground_x0, ground_x1, content_x0, content_x1, y0, y1)."""
    name = tier(width)
    margin = PAGE_MARGIN[name]
    left = RAIL_WIDTH if name == "desktop" else 0
    return (left + 2, left + margin - 2, left + margin, width - 6,
            SHELL_TOP_AIR[name], height - 1 - SHELL_BOTTOM_AIR[name])


def runs(mask: np.ndarray) -> list[tuple[int, int]]:
    """Contiguous True runs of a boolean mask as (first, last) pairs."""
    found: list[tuple[int, int]] = []
    start: int | None = None
    for index, value in enumerate(mask):
        if value and start is None:
            start = index
        elif not value and start is not None:
            found.append((start, index - 1))
            start = None
    if start is not None:
        found.append((start, len(mask) - 1))
    return found


def row_usage(image: np.ndarray) -> np.ndarray:
    """Per-row fraction of the page body that is not page ground."""
    height, width, _ = image.shape
    ground_x0, ground_x1, x0, x1, y0, y1 = content_box(width, height)
    strip = image[y0:y1, ground_x0:ground_x1].astype(int)
    ground = np.median(strip.reshape(-1, 3), axis=0)
    band = image[y0:y1, x0:x1].astype(int)
    return (np.abs(band - ground).sum(axis=2) > GROUND_DELTA).mean(axis=1)


def vertical(rows: np.ndarray) -> dict:
    total = len(rows)
    used = rows > ROW_INK_FRACTION
    content = [run for run in runs(used) if run[1] - run[0] + 1 >= MIN_CONTENT_RUN]
    if not content:
        return dict(box_height=total, tail_pct=100.0, band_pct=0.0)
    first, last = content[0][0], content[-1][1]
    tail = total - 1 - last
    inside = [run for run in runs(~used) if run[0] > first and run[1] < last]
    band = max((run[1] - run[0] + 1 for run in inside), default=0)
    return dict(box_height=total,
                tail_pct=round(tail / total * 100, 1),
                band_pct=round(band / total * 100, 1))


def rail_of(image: np.ndarray) -> dict | None:
    """The desktop rail's own utilization, or None off the desktop tier.

    The reference sidebar is navigation only: the selected row is a full-width
    wash plus a narrow indicator, and the empty right side is breathing room.
    What IS waste is a second content block (a status card, brand tile or
    version line) living in the rail below the nav stack - those facts already
    have a single owner in the title bar and the dashboard. We measure it as
    the ink below the last navigation row, which the nav stack's own stride
    identifies.
    """
    height, width, _ = image.shape
    if width < 1000:
        return None
    strip = image[:, 0:RAIL_WIDTH].astype(int)
    ground = np.median(strip[height // 2, 4:10].reshape(-1, 3), axis=0)
    ink = np.abs(strip - ground).sum(axis=2) > 14
    row_ink = ink.mean(axis=1) > 0.01
    bands = runs(row_ink)
    if not bands:
        return None
    voids = [bands[i + 1][0] - bands[i][1] - 1 for i in range(len(bands) - 1)]
    # Navigation rows are the compact bands in the upper 70% of the rail. A
    # bottom chrome card is a tall band (or a cluster of lines) below them.
    nav = [band for band in bands if band[0] < height * 0.70]
    chrome_band = 0.0
    if nav:
        nav_bottom = max(band[1] for band in nav)
        lower = [band for band in bands if band[0] > nav_bottom]
        if lower:
            chrome_band = round(
                (max(band[1] for band in lower) - min(band[0] for band in lower) + 1)
                / height * 100, 1)
    return dict(void_pct=round(max(voids, default=0) / height * 100, 1),
                chrome_band_pct=chrome_band)


def measure(run: pathlib.Path) -> list[dict]:
    out: list[dict] = []
    pages = scenario_pages(run)
    for scenario in sorted(p for p in run.iterdir() if p.is_dir()):
        image_path = scenario / "image.png"
        if not image_path.exists():
            continue
        image = np.asarray(Image.open(image_path).convert("RGB"))
        split = vertical(row_usage(image))
        entry = dict(scenario=scenario.name,
                     page=pages.get(scenario.name, ""),
                     tier=tier(image.shape[1]),
                     size=f"{image.shape[1]}x{image.shape[0]}",
                     tail_pct=split["tail_pct"],
                     band_pct=split["band_pct"],
                     unused_pct=round(max(split["tail_pct"], split["band_pct"]), 1))
        rail_info = rail_of(image)
        if rail_info:
            entry.update(rail_info)
        out.append(entry)
    return out


def published_run(captures: pathlib.Path = CAPTURES) -> pathlib.Path | None:
    """The newest full-matrix publication; diagnostic runs never judge."""
    if not captures.is_dir():
        return None
    size = scenario_count(captures)
    candidates = []
    for run in captures.iterdir():
        metadata = run / METADATA
        if not run.is_dir() or not metadata.is_file():
            continue
        facts = dict(line.split("=", 1) for line in metadata.read_text(encoding="utf-8").splitlines()
                     if "=" in line)
        if size is None or facts.get("scenario_count") != str(size):
            continue
        candidates.append((run.name, run))
    return max(candidates)[1] if candidates else None


def scenario_count(captures: pathlib.Path = CAPTURES) -> int | None:
    """The size of a full matrix: the largest scenario count ever published.

    This is a count, not the count of the newest run: a diagnostic run that
    happens to sort last must not redefine what a full matrix is.
    """
    counts = []
    for run in captures.iterdir() if captures.is_dir() else []:
        metadata = run / METADATA
        if not metadata.is_file():
            continue
        facts = dict(line.split("=", 1) for line in metadata.read_text(encoding="utf-8").splitlines()
                     if "=" in line)
        if "scenario_count" in facts:
            counts.append(int(facts["scenario_count"]))
    return max(counts) if counts else None


def read_debt(path: pathlib.Path = DEBT_FILE) -> dict:
    if not path.is_file():
        return {"version": 1, "frames": {}, "rail": {}}
    document = json.loads(path.read_text(encoding="utf-8"))
    document.setdefault("version", 1)
    document.setdefault("frames", {})
    document.setdefault("rail", {})
    return document


def write_debt(document: dict, path: pathlib.Path = DEBT_FILE) -> None:
    path.write_text(json.dumps(document, indent=2, ensure_ascii=False, sort_keys=True) + "\n",
                    encoding="utf-8")


def frame_findings(results: list[dict], debt: dict) -> list[str]:
    found: list[str] = []
    registered = debt["frames"]
    structures = page_structures()
    for entry in results:
        limit = limit_for(entry["scenario"], entry["tier"], structures, entry.get("page", ""))
        if entry["unused_pct"] <= limit:
            continue
        allowed = registered.get(entry["scenario"])
        if allowed is None:
            found.append(
                f"{entry['scenario']}: unused {entry['unused_pct']}% of the page body "
                f"exceeds the {limit}% limit for its page shape and is not registered debt"
            )
            continue
        if entry["unused_pct"] > float(allowed):
            found.append(
                f"{entry['scenario']}: unused {entry['unused_pct']}% grew past its "
                f"registered debt {allowed}%; debt may only shrink"
            )
    for name in sorted(registered):
        if not any(entry["scenario"] == name for entry in results):
            found.append(f"{name}: registered debt for a scenario that is not in the batch")
    return found


def rail_findings(results: list[dict], debt: dict) -> list[str]:
    found: list[str] = []
    registered = debt["rail"]
    desktop = [entry for entry in results if entry["tier"] == "desktop"]
    if not desktop:
        return found
    worst_chrome = max((entry.get("chrome_band_pct", 0.0) for entry in desktop))
    worst_void = max((entry.get("void_pct", 0.0) for entry in desktop))
    allowed_chrome = float(registered.get("chrome_band_pct", 0.0))
    allowed_void = float(registered.get("void_pct", 0.0))
    if worst_chrome > RAIL_CHROME_BAND_LIMIT:
        if worst_chrome > allowed_chrome:
            found.append(
                f"rail: {worst_chrome}% of the rail height is non-navigation chrome, "
                f"over the {RAIL_CHROME_BAND_LIMIT}% limit"
                + (f" and over its registered debt {allowed_chrome}%" if allowed_chrome else "")
            )
    if worst_void > RAIL_VOID_LIMIT:
        if worst_void > allowed_void:
            found.append(
                f"rail: the nav stack keeps a {worst_void}% void, over the "
                f"{RAIL_VOID_LIMIT}% limit"
                + (f" and over its registered debt {allowed_void}%" if allowed_void else "")
            )
    return found


def report(results: list[dict]) -> None:
    for name, label in (("desktop", "page body"), ("compact", "compact"), ("phone", "phone")):
        group = [entry for entry in results if entry["tier"] == name]
        if not group:
            continue
        values = [entry["unused_pct"] for entry in group]
        structures = page_structures()
        limits = {entry["scenario"]: limit_for(entry["scenario"], name, structures,
                                                entry.get("page", ""))
                  for entry in group}
        over = [entry for entry in group
                if entry["unused_pct"] > limits[entry["scenario"]]]
        print(f"\n=== {label} ({len(group)} frames, limit by page shape) "
              f"median {statistics.median(values):.1f}% p90 {sorted(values)[int(len(values)*0.9)]:.1f}% "
              f"max {max(values):.1f}% | over limit {len(over)} ===")
        for entry in sorted(over, key=lambda e: -e["unused_pct"])[:14]:
            print(f"  unused {entry['unused_pct']:5.1f}% (tail {entry['tail_pct']:5.1f} "
                  f"band {entry['band_pct']:5.1f}) limit {limits[entry['scenario']]:5.1f}% "
                  f" {entry['scenario']}")
    desktop = [entry for entry in results if entry["tier"] == "desktop"]
    if desktop:
        chrome = max(entry.get("chrome_band_pct", 0.0) for entry in desktop)
        void = max(entry.get("void_pct", 0.0) for entry in desktop)
        print(f"\n=== rail: worst non-nav chrome {chrome}% (limit {RAIL_CHROME_BAND_LIMIT}%), "
              f"worst nav void {void}% (limit {RAIL_VOID_LIMIT}%) ===")


# ---------------------------------------------------------------------------
# Self-test: the judge must be able to see the defect it claims to see.
# ---------------------------------------------------------------------------

GROUND = (225, 235, 217)
CARD = (248, 251, 245)
INK = (20, 25, 20)


def _canvas(width: int, height: int) -> np.ndarray:
    return np.zeros((height, width, 3), dtype=np.uint8)


def _fill(image: np.ndarray, y0: int, y1: int, x0: int, x1: int, colour: tuple) -> None:
    image[y0:y1, x0:x1] = colour


def _desktop_body() -> np.ndarray:
    """A 1180x780 desktop frame: rail, page ground, one full-width card."""
    image = _canvas(1180, 780)
    _fill(image, 0, 780, 0, RAIL_WIDTH, (241, 246, 237))
    _fill(image, 0, 780, RAIL_WIDTH, 1180, GROUND)
    return image


def _desktop_body_dark() -> np.ndarray:
    """The same frame on a dark rail, so a luminance-cut judge cannot hide."""
    image = _canvas(1180, 780)
    _fill(image, 0, 780, 0, RAIL_WIDTH, (20, 23, 27))
    _fill(image, 0, 780, RAIL_WIDTH, 1180, (11, 13, 16))
    return image


def _write_run(root: pathlib.Path, frames: dict[str, np.ndarray],
               pages: dict[str, str] | None = None) -> pathlib.Path:
    run = root / "batch"
    run.mkdir(parents=True, exist_ok=True)
    (run / METADATA).write_text(f"scenario_count={len(frames)}\n", encoding="utf-8")
    for name, image in frames.items():
        directory = run / name
        directory.mkdir(parents=True, exist_ok=True)
        Image.fromarray(image).save(directory / "image.png")
    if pages:
        rows = ["scenario\tpage\tskin\trequested_window\tapp_pid\twindow_id\twidth"
                "\theight\tbytes\tsha256\tstatus"]
        for name, slug in pages.items():
            rows.append(f"{name}\t{slug}\tdark\t1180x780\t0\t0\t1180\t780\t0\t0\tok")
        (run / "capture-manifest.tsv").write_text("\n".join(rows) + "\n", encoding="utf-8")
    return run


class SelfTest(unittest.TestCase):
    """Planted frames, so the thresholds are known to bite."""

    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = pathlib.Path(self.temporary.name)
        self.addCleanup(self.temporary.cleanup)

    def measure_one(self, image: np.ndarray, page: str = "") -> dict:
        pages = {"planted": page} if page else None
        run = _write_run(self.root, {"planted": image}, pages)
        return measure(run)[0]

    def test_a_frame_filling_card_is_not_waste(self) -> None:
        image = _desktop_body()
        _fill(image, 96, 708, 258, 1150, CARD)
        for row in range(110, 700, 44):
            _fill(image, row, row + 18, 270, 1140, INK)
        entry = self.measure_one(image)
        self.assertLessEqual(entry["unused_pct"], UNUSED_LIMIT["desktop"], entry)

    def test_a_short_card_over_empty_ground_is_waste(self) -> None:
        image = _desktop_body()
        _fill(image, 96, 300, 258, 1150, CARD)
        for row in range(110, 290, 44):
            _fill(image, row, row + 18, 270, 1140, INK)
        entry = self.measure_one(image)
        self.assertGreater(entry["unused_pct"], UNUSED_LIMIT["desktop"], entry)
        self.assertNotEqual(frame_findings([entry], {"frames": {}, "rail": {}}), [])

    def test_a_framed_page_may_not_leave_ground_below_its_frame(self) -> None:
        # A framed list page fills the page body; ground below the frame is
        # waste, and the limit for this shape is the strict one.
        image = _desktop_body()
        _fill(image, 96, 300, 258, 1150, CARD)
        for row in range(110, 290, 44):
            _fill(image, row, row + 18, 270, 1140, INK)
        entry = self.measure_one(image, page="logs")
        limit = limit_for(entry["scenario"], entry["tier"], page_structures(), "logs")
        self.assertEqual(limit, FRAMED_LIMIT, limit)
        self.assertGreater(entry["unused_pct"], limit, entry)
        self.assertNotEqual(frame_findings([entry], {"frames": {}, "rail": {}}), [])

    def test_a_content_page_may_end_where_its_last_block_ends(self) -> None:
        # The peer product leaves more ground than this on the same shape, so
        # judging it by the framed rule would fail the reference itself.
        image = _desktop_body()
        _fill(image, 96, 300, 258, 1150, CARD)
        for row in range(110, 290, 44):
            _fill(image, row, row + 18, 270, 1140, INK)
        entry = self.measure_one(image, page="about")
        limit = limit_for(entry["scenario"], entry["tier"], page_structures(), "about")
        self.assertEqual(limit, CONTENT_LIMIT, limit)
        self.assertLessEqual(entry["unused_pct"], limit, entry)
        self.assertEqual(frame_findings([entry], {"frames": {}, "rail": {}}), [])

    def test_an_unknown_page_falls_back_to_the_tier_limit(self) -> None:
        image = _desktop_body()
        _fill(image, 96, 300, 258, 1150, CARD)
        entry = self.measure_one(image)
        limit = limit_for(entry["scenario"], entry["tier"], page_structures(), "")
        self.assertEqual(limit, UNUSED_LIMIT["desktop"], limit)

    def test_the_shell_air_is_never_waste(self) -> None:
        # A card that fills the page body exactly, leaving only the shell's
        # reserved 8u/72u air, must read as full.
        image = _desktop_body()
        _fill(image, 8 + 1, 780 - 72, 258, 1150, CARD)
        _fill(image, 40, 58, 270, 1140, INK)
        _fill(image, 600, 618, 270, 1140, INK)
        entry = self.measure_one(image)
        self.assertLessEqual(entry["unused_pct"], UNUSED_LIMIT["desktop"], entry)

    def test_edge_noise_cannot_disguise_a_dead_page(self) -> None:
        image = _desktop_body()
        _fill(image, 96, 200, 258, 1150, CARD)
        _fill(image, 110, 128, 270, 1140, INK)
        # one stray row of ink at the very bottom of the box
        _fill(image, 706, 707, 258, 1150, INK)
        entry = self.measure_one(image)
        self.assertGreater(entry["unused_pct"], UNUSED_LIMIT["desktop"], entry)

    def test_an_interior_ground_band_between_two_cards_is_waste(self) -> None:
        # The doctor shape: a card, a band of page ground, another card. The
        # tail is zero because the last card reaches the bottom, so only the
        # interior-band half of the metric can see this defect.
        image = _desktop_body()
        _fill(image, 96, 200, 258, 1150, CARD)
        _fill(image, 110, 128, 270, 1140, INK)
        _fill(image, 600, 700, 258, 1150, CARD)
        _fill(image, 610, 628, 270, 1140, INK)
        entry = self.measure_one(image)
        self.assertLess(entry["tail_pct"], UNUSED_LIMIT["desktop"], entry)
        self.assertGreater(entry["band_pct"], 30.0, entry)
        self.assertGreater(entry["unused_pct"], UNUSED_LIMIT["desktop"], entry)

    def test_the_card_is_never_its_own_ground_reference(self) -> None:
        # A card that is the MAJORITY colour of the frame, with ground below
        # it. Sampling the ground from inside the box would read the card as
        # ground and the ground as content, and hide the whole dead tail.
        image = _desktop_body()
        _fill(image, 96, 520, 258, 1150, CARD)
        _fill(image, 110, 128, 270, 1140, INK)
        _fill(image, 500, 512, 270, 1140, INK)
        entry = self.measure_one(image)
        self.assertGreater(entry["unused_pct"], UNUSED_LIMIT["desktop"], entry)

    def test_an_empty_interior_inside_a_frame_card_is_not_page_waste(self) -> None:
        # The b134 frame contract: a list card fills the page body and its
        # rows scroll inside it. Sparse rows inside that frame are the
        # frame's own viewport, not ground, and must not be reported as waste.
        image = _desktop_body()
        _fill(image, 96, 708, 258, 1150, CARD)
        for row in (140, 190, 240):
            _fill(image, row, row + 18, 270, 1140, INK)
        entry = self.measure_one(image)
        self.assertLessEqual(entry["unused_pct"], UNUSED_LIMIT["desktop"], entry)

    def test_unregistered_debt_fails_and_registered_debt_passes(self) -> None:
        image = _desktop_body()
        _fill(image, 96, 300, 258, 1150, CARD)
        _fill(image, 110, 128, 270, 1140, INK)
        entry = self.measure_one(image)
        self.assertNotEqual(frame_findings([entry], {"frames": {}, "rail": {}}), [])
        debt = {"frames": {"planted": entry["unused_pct"]}, "rail": {}}
        self.assertEqual(frame_findings([entry], debt), [])

    def test_debt_may_only_shrink(self) -> None:
        entry = dict(scenario="planted", tier="desktop", unused_pct=30.0,
                     tail_pct=30.0, band_pct=1.0)
        debt = {"frames": {"planted": 20.0}, "rail": {}}
        found = frame_findings([entry], debt)
        self.assertTrue(any("grew past its registered debt" in item for item in found), found)

    def test_debt_for_a_missing_scenario_is_a_violation(self) -> None:
        debt = {"frames": {"gone": 40.0}, "rail": {}}
        found = frame_findings([], debt)
        self.assertTrue(any("not in the batch" in item for item in found), found)

    def _nav_only_rail(self, image: np.ndarray) -> None:
        """Five selected/nav rows in the upper rail, no bottom chrome."""
        for y in (140, 230, 320, 410, 500):
            _fill(image, y, y + 40, 0, 200, (220, 238, 226))
            _fill(image, y + 12, y + 28, 34, 140, INK)

    def test_a_status_card_below_the_nav_stack_is_caught(self) -> None:
        image = _desktop_body()
        self._nav_only_rail(image)
        _fill(image, 617, 712, 18, 210, CARD)
        entry = self.measure_one(image)
        self.assertGreater(entry["chrome_band_pct"], RAIL_CHROME_BAND_LIMIT, entry)
        self.assertNotEqual(rail_findings([entry], {"frames": {}, "rail": {}}), [])

    def test_a_navigation_only_rail_passes(self) -> None:
        image = _desktop_body()
        self._nav_only_rail(image)
        entry = self.measure_one(image)
        self.assertEqual(entry["chrome_band_pct"], 0.0, entry)
        self.assertEqual(rail_findings([entry], {"frames": {}, "rail": {}}), [])

    def test_a_dark_status_card_is_caught_too(self) -> None:
        image = _desktop_body_dark()
        for y in (140, 230, 320, 410, 500):
            _fill(image, y, y + 40, 0, 200, (6, 73, 91))
            _fill(image, y + 12, y + 28, 34, 140, (213, 241, 248))
        _fill(image, 617, 712, 18, 210, (29, 33, 38))
        entry = self.measure_one(image)
        self.assertGreater(entry["chrome_band_pct"], RAIL_CHROME_BAND_LIMIT, entry)

    def test_a_full_width_selected_row_is_not_dead_width(self) -> None:
        # Regression for the retired `pill_tail` metric: a full-width
        # selected band is the reference's navigation geometry. Only the
        # presence of a non-navigation block below the nav stack is waste.
        image = _desktop_body()
        _fill(image, 140, 180, 0, 200, (220, 238, 226))
        _fill(image, 152, 168, 34, 140, INK)
        entry = self.measure_one(image)
        self.assertEqual(entry["chrome_band_pct"], 0.0, entry)
        self.assertEqual(rail_findings([entry], {"frames": {}, "rail": {}}), [])

    def test_the_rail_verdict_does_not_depend_on_the_skin(self) -> None:
        light = _desktop_body()
        self._nav_only_rail(light)
        _fill(light, 617, 712, 18, 210, CARD)
        dark = _desktop_body_dark()
        for y in (140, 230, 320, 410, 500):
            _fill(dark, y, y + 40, 0, 200, (6, 73, 91))
            _fill(dark, y + 12, y + 28, 34, 140, (213, 241, 248))
        _fill(dark, 617, 712, 18, 210, (29, 33, 38))
        light_entry = self.measure_one(light)
        dark_entry = self.measure_one(dark)
        self.assertEqual(light_entry["chrome_band_pct"], dark_entry["chrome_band_pct"],
                         (light_entry, dark_entry))

    def test_a_published_batch_is_required_to_judge(self) -> None:
        empty = self.root / "nothing"
        empty.mkdir()
        self.assertIsNone(published_run(empty))

    def test_diagnostic_runs_never_judge(self) -> None:
        captures = self.root / "captures"
        full = captures / "full"
        _write_run(full, {"a": _desktop_body()})
        (full / METADATA).write_text("scenario_count=197\n", encoding="utf-8")
        partial = captures / "partial"
        _write_run(partial, {"a": _desktop_body()})
        (partial / METADATA).write_text("scenario_count=6\n", encoding="utf-8")
        chosen = published_run(captures)
        self.assertEqual(chosen, full)


def self_test(argv: list[str] | None = None) -> int:
    suite = unittest.TestLoader().loadTestsFromTestCase(SelfTest)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    return 0 if result.wasSuccessful() else 1




def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--run", type=pathlib.Path)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--report", action="store_true")
    parser.add_argument("--enforce", action="store_true")
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--update-debt", action="store_true")
    parser.add_argument("--bootstrap", action="store_true",
                        help="allow the first debt baseline to be written")
    parser.add_argument("--check-export", action="store_true",
                        help="check that the exported page-shape table is current")
    args = parser.parse_args(argv)

    if args.self_test:
        return self_test()
    if args.check_export:
        problems = export_is_fresh()
        for problem in problems:
            print(f"space-utilization: {problem}", file=sys.stderr)
        if problems:
            return 1
        print("space-utilization: the exported page-shape table is current")
        return 0
    run = args.run or published_run()
    if run is None or not run.is_dir():
        print("space-utilization: no published full-matrix batch; the pixel leg "
              "cannot judge until a capture batch exists", file=sys.stderr)
        return 0
    results = measure(run)
    debt = read_debt()

    if args.json:
        print(json.dumps(dict(run=run.name, results=results), ensure_ascii=False))
        return 0
    if args.report:
        report(results)
        return 0
    if args.update_debt:
        first_baseline = not DEBT_FILE.is_file()
        if first_baseline and not args.bootstrap:
            print("space-utilization: the debt inventory does not exist; pass "
                  "--bootstrap once to write the first baseline, afterwards "
                  "--update-debt may only shrink it", file=sys.stderr)
            return 1
        structures = page_structures()
        frames = {entry["scenario"]: entry["unused_pct"] for entry in results
                  if entry["unused_pct"] > limit_for(entry["scenario"], entry["tier"],
                                                    structures, entry.get("page", ""))}
        desktop = [entry for entry in results if entry["tier"] == "desktop"]
        rail = {}
        if desktop:
            rail["chrome_band_pct"] = max(entry.get("chrome_band_pct", 0.0) for entry in desktop)
            rail["void_pct"] = max(entry.get("void_pct", 0.0) for entry in desktop)
        grew = sorted(name for name, value in frames.items()
                      if float(debt["frames"].get(name, -1.0)) < value)
        if grew and not first_baseline:
            print(f"space-utilization: refusing to register new debt for {grew}", file=sys.stderr)
            return 1
        if first_baseline:
            print(f"space-utilization: writing the first debt baseline "
                  f"({len(frames)} frame(s))")
        lost = sorted(set(debt["frames"]) - set(frames))
        document = dict(version=1, frames=frames, rail=rail)
        write_debt(document)
        print(f"space-utilization: debt inventory now {len(frames)} frame(s)"
              + (f", closed {len(lost)}: {lost}" if lost else ""))
        return 0

    findings = frame_findings(results, debt) + rail_findings(results, debt)
    if args.enforce:
        if findings:
            for finding in findings:
                print(f"space-utilization violation: {finding}", file=sys.stderr)
            return 1
        print(f"space-utilization: {len(results)} frames judged against "
              f"{run.name}, violations={len(findings)}")
        return 0
    report(results)
    for finding in findings:
        print(f"  finding: {finding}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
