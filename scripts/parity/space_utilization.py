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

PAGE_STRUCTURE_FILE = ROOT / "crates/infiltrator-contract/assets/page-structure.json"
RAIL_CHROME_BAND_LIMIT = 0.0
RAIL_VOID_LIMIT = 12.0

# The contract's own limits, mirrored here only so a planted self-test can
# assert the judge read the exported table rather than a local number.
FRAMED_LIMIT = 4.0
CONTENT_LIMIT = 82.0

# The chrome line one frame carries, and where the shell writes it.
#
# The judge keeps NO chrome geometry of its own. Page margin, shell air, the
# reserved navigation column and the end of the painted navigation stack all
# travel with the frame, published by the shell that drew them
# (`infiltrator_contract::shell_receipt`) and read back here per frame. A
# mirrored constant would be a second answer to "what box did this pixel come
# from": change a padding, a skin or a zoom and the judge would measure a box
# no surface painted - or accuse the real one of waste it never had.
CHROME_FILE = "marker.log"
CHROME_PREFIX = "CAPTURE_CHROME"

# The chrome vocabulary a planted export must carry, kept in step with the
# contract by `test_ladder_and_chrome_keys_agree` below.
CHROME_KEYS_FOR_TEST = [
    "nav_form", "nav_column_px", "nav_stack_bottom_px", "nav_content_offset_px",
    "page_margin_px", "shell_top_air_px", "shell_bottom_air_px", "dock_band_px",
    "scale_percent",
]

ROW_INK_FRACTION = 0.04
MIN_CONTENT_RUN = 3
GROUND_DELTA = 18

# A band that paints right up to the edge of the navigation column AND keeps the
# same colour past it cannot be the sidebar's own chrome: a surface that owns a
# column does not paint outside it. Window overlays do - the title bar spans the
# whole window, a notice banner is centred on it and overhangs the rail, a modal
# scrim covers everything - and none of them is the rail spending its height on
# a second content block. The test is geometric instead of a remembered list of
# widget names: sample one row just inside the edge and the same row just outside
# it, and call the band an overlay when most of its rows are ink on the inside
# *and* the same colour over the boundary. A block parked under the nav stack
# does not pass: either it stops short of the edge (no ink there) or it ends at
# the edge in a different colour than the page ground beside it.
EDGE_SAMPLE_PX = 3
EDGE_CONTINUITY_DELTA = 8
EDGE_CONTINUOUS_FRACTION = 0.5


def ladder() -> list[dict]:
    """The exported viewport ladder, smallest tier first.

    The contract owns the boundaries, the navigation form and the fallback
    limit; this reads them. An empty ladder is a bug, not a default: the
    judge that invents its own boundaries is exactly the defect the exported
    ladder exists to prevent, so `ladder_problems()` reports it.
    """
    if not PAGE_STRUCTURE_FILE.is_file():
        return []
    document = json.loads(PAGE_STRUCTURE_FILE.read_text(encoding="utf-8"))
    return list(document.get("tiers", []))


def ladder_problems() -> list[str]:
    """Why the exported ladder cannot judge a frame, if it cannot."""
    tiers = ladder()
    if not tiers:
        return ["page-structure.json carries no viewport ladder; "
                "rerun export_page_structure"]
    problems: list[str] = []
    if not chrome_keys():
        problems.append(
            "page-structure.json carries no chrome vocabulary; the frames' "
            "CAPTURE_CHROME lines cannot be checked against a declared key list")
    if any("nav_leading_limit_px" not in entry for entry in tiers):
        # The leading budget travels with the ladder that owns the column, so
        # an export predating SPACEUTIL-010 cannot judge the axis at all: the
        # judge refuses rather than comparing against a remembered number.
        problems.append(
            "the exported ladder carries no nav_leading_limit_px; "
            "rerun export_page_structure")
    for entry in tiers:
        limit = entry.get("nav_leading_limit_px")
        labelled = bool(entry.get("labelled_sidebar"))
        if (limit is None) or ((limit > 0) != labelled):
            problems.append(
                f"the exported {entry['id']} tier budgets {limit}px of leading "
                f"while its labelled_sidebar is {labelled}")
    if tiers[0]["min_width_px"] != 0:
        problems.append("the exported ladder's smallest tier does not start at 0")
    if tiers[-1]["max_width_px"] is not None:
        problems.append("the exported ladder's widest tier carries an upper bound")
    for pair in zip(tiers, tiers[1:]):
        if pair[0]["max_width_px"] != pair[1]["min_width_px"]:
            problems.append(
                f"the exported ladder leaves widths between {pair[0]['max_width_px']} "
                f"and {pair[1]['min_width_px']} in no tier"
            )
    return problems


def tier(width: int) -> str:
    """The id of the tier a frame width belongs to, from the exported ladder."""
    for entry in ladder():
        upper = entry["max_width_px"]
        if upper is None or width < upper:
            return entry["id"]
    return ladder()[-1]["id"]


def tier_fact(name: str) -> dict:
    for entry in ladder():
        if entry["id"] == name:
            return entry
    raise KeyError(f"{name} is not a tier of the exported ladder")


def painted_nav_column(width: int) -> int:
    """The navigation column a frame of this width actually paints.

    The exported ladder already answers this per tier: 0 where the shell mounts
    a bottom bar, 64 where it mounts the medium tier's icon rail (the form the
    contract chose and Slint now draws, closing SPACEUTIL-006), 240 and 280 for
    the labelled columns. The judge takes the number instead of deciding a
    second time which form a width gets, because a box that disagrees with the
    painted column measures page content as chrome, or chrome as page content.
    """
    return int(tier_fact(tier(width))["nav_column_px"])


def chrome_keys() -> list[str]:
    """The declared chrome vocabulary, from the contract's export.

    The writer in Rust, the Slint property that fills it and this reader all
    check one list. A key added on one side only is a refusal here, not a
    silently missing number there.
    """
    if not PAGE_STRUCTURE_FILE.is_file():
        return []
    document = json.loads(PAGE_STRUCTURE_FILE.read_text(encoding="utf-8"))
    return list(document.get("chrome_keys", []))


def chrome_of(directory: pathlib.Path) -> dict | None:
    """The chrome the shell published for one frame.

    `None` means the frame did not describe itself, which is a finding, not a
    licence to fall back to remembered numbers.
    """
    marker = directory / CHROME_FILE
    if not marker.is_file():
        return None
    declared = chrome_keys()
    found: dict | None = None
    for line in marker.read_text(encoding="utf-8").splitlines():
        if not line.startswith(CHROME_PREFIX):
            continue
        values: dict[str, object] = {}
        for pair in line.split()[1:]:
            key, sep, value = pair.partition("=")
            if not sep or key not in declared or key in values:
                return None
            values[key] = value if key == "nav_form" else int(value)
        if sorted(values) != sorted(declared):
            return None
        found = values
    return found


def nav_form(chrome: dict) -> str:
    """The typed navigation this frame's shell mounted."""
    return str(chrome["nav_form"])


def owns_nav_column(chrome: dict) -> bool:
    """Whether this frame's shell mounted a vertical navigation column.

    The rail axis judges the column whatever form it takes, so the answer is
    the receipt's own column width, not the form name a caller remembers:
    `nav_column_px` is the one fact both the icon rail and the labelled pane
    publish, and the contract pins it to a measured stack end.
    """
    return int(chrome["nav_column_px"]) > 0


def nav_stack_bottom_px(chrome: dict) -> int:
    """Where the shell measured its navigation stack ending, in page pixels."""
    return int(chrome["nav_stack_bottom_px"])


def nav_content_offset_px(chrome: dict) -> int:
    """The leading the shell gives away inside its column, in page pixels."""
    return int(chrome["nav_content_offset_px"])


def band_is_window_overlay(image: np.ndarray, ink: np.ndarray,
                           band: tuple[int, int], column: int) -> bool:
    """Whether a band is paint laid over the window rather than the column's own.

    Two conditions, both geometric. The band has to be ink AT the column's
    right edge - a block parked under the nav stack usually stops short of it,
    so ground at the edge alone says "this is the column's own content" - and
    the colour has to keep going past the edge, because a surface that owns the
    column does not paint outside it. A notice banner, a title bar and a modal
    scrim satisfy both: they are centred on or sized to the window and simply
    cross the rail on their way. Charging one to the rail would accuse the
    shell of chrome its sidebar never painted.
    """
    top, bottom = band
    height, width, _ = image.shape
    inside_x = max(0, column - EDGE_SAMPLE_PX)
    outside_x = min(width - 1, column + EDGE_SAMPLE_PX)
    inside = image[top:bottom + 1, inside_x].astype(int)
    outside = image[top:bottom + 1, outside_x].astype(int)
    continuous = (np.abs(inside - outside).sum(axis=1) <= EDGE_CONTINUITY_DELTA * 3)
    reaches_edge = ink[top:bottom + 1, inside_x]
    return bool((continuous & reaches_edge).mean() >= EDGE_CONTINUOUS_FRACTION)


def fallback_limit_pct(name: str) -> float:
    """The exported per-tier fallback, used only for a page with no shape."""
    return float(tier_fact(name)["fallback_ground_limit_pct"])


def page_margin_px(chrome: dict) -> int:
    return int(chrome["page_margin_px"])


def shell_top_air_px(chrome: dict) -> int:
    return int(chrome["shell_top_air_px"])


def shell_bottom_air_px(chrome: dict) -> int:
    return int(chrome["shell_bottom_air_px"])


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
    return ladder_problems()


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
    return fallback_limit_pct(tier_name)


def content_box(width: int, height: int, chrome: dict):
    """(ground_x0, ground_x1, content_x0, content_x1, y0, y1).

    The box is the page body: the navigation column this frame reserved, then
    the page margin its chrome paints outside the body, then the shell air it
    keeps above and below. Every one of those numbers is the frame's own, read
    out of the receipt the shell wrote, which is what makes the measured box
    equal the painted box by construction instead of by agreement.
    """
    left = int(chrome["nav_column_px"])
    margin = page_margin_px(chrome)
    return (left + 2, left + margin - 2, left + margin, width - 6,
            shell_top_air_px(chrome), height - 1 - shell_bottom_air_px(chrome))


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


def row_usage(image: np.ndarray, chrome: dict) -> np.ndarray:
    """Per-row fraction of the page body that is not page ground."""
    height, width, _ = image.shape
    ground_x0, ground_x1, x0, x1, y0, y1 = content_box(width, height, chrome)
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


def rail_of(image: np.ndarray, chrome: dict) -> dict | None:
    """The labelled sidebar's own utilization, or None where there is none.

    The reference sidebar is navigation only: the selected row is a full-width
    wash plus a narrow indicator, and the empty right side is breathing room.
    What IS waste is a second content block (a status card, brand tile or
    version line) living in the rail below the nav stack - those facts already
    have a single owner in the title bar and the dashboard.

    The line the stack ends at is the receipt's measured `nav_stack_bottom_px`,
    which the sidebar took from the last destination row's own geometry, so a
    pane whose rows reach further down a short window is not mistaken for a
    chrome card. Bands above that line are navigation by definition; bands
    below it are chrome unless they paint straight across the column edge - the
    title bar, a notice banner and a modal scrim are window overlays that
    happen to cross the rail, not blocks the rail spent its height on.
    """
    height, width, _ = image.shape
    column = int(chrome["nav_column_px"])
    bottom = nav_stack_bottom_px(chrome)
    if column == 0 or bottom <= 0:
        return None
    strip = image[:, 0:column].astype(int)
    ground = np.median(strip[height // 2, 4:10].reshape(-1, 3), axis=0)
    ink = np.abs(strip - ground).sum(axis=2) > 14
    row_ink = ink.mean(axis=1) > 0.01
    bands = runs(row_ink)
    if not bands:
        return None
    voids = [bands[i + 1][0] - bands[i][1] - 1 for i in range(len(bands) - 1)]
    # Navigation is every band above the measured stack end; below it, only a
    # band that stays inside the column is the rail's own chrome.
    chrome_band = 0.0
    below = [band for band in bands
             if band[0] >= bottom and not band_is_window_overlay(image, ink, band, column)]
    if below:
        chrome_band = round(
            (max(band[1] for band in below) - min(band[0] for band in below) + 1)
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
        chrome = chrome_of(scenario)
        entry = dict(scenario=scenario.name,
                     page=pages.get(scenario.name, ""),
                     tier=tier(image.shape[1]),
                     size=f"{image.shape[1]}x{image.shape[0]}",
                     chrome=chrome)
        if chrome is None:
            # No self-description, no box, no verdict: `chrome_findings`
            # reports the frame instead of the judge inventing a box for it.
            out.append(entry)
            continue
        split = vertical(row_usage(image, chrome))
        entry.update(tail_pct=split["tail_pct"],
                     band_pct=split["band_pct"],
                     unused_pct=round(max(split["tail_pct"], split["band_pct"]), 1))
        rail_info = rail_of(image, chrome)
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


def chrome_findings(results: list[dict]) -> list[str]:
    """Every frame must describe its own chrome, and agree with the ladder.

    Four refusals, all of the same kind: a frame that says nothing about its
    chrome cannot be measured, a frame whose numbers contradict the ladder
    measured a decision no contract made (the column the shell reserved against
    the column the tier owns), and a frame that reserved a bottom band while
    mounting a form with no bar is the exact defect SPACEUTIL-006 was written
    for. The fourth is SPACEUTIL-010: a labelled column states the leading it
    gives away, and the tier's exported budget - not a number this judge
    remembers - says how much of the column that may be. None of these are
    judged by remembered numbers.
    """
    found: list[str] = []
    for entry in results:
        chrome = entry.get("chrome")
        if chrome is None:
            found.append(
                f"{entry['scenario']}: the frame carries no {CHROME_PREFIX} line, so its "
                "measurement box would be a number the judge remembered instead of the "
                "chrome that painted these pixels"
            )
            continue
        fact = tier_fact(entry["tier"])
        form = nav_form(chrome)
        if int(chrome["nav_column_px"]) != int(fact["nav_column_px"]):
            found.append(
                f"{entry['scenario']}: the shell reserved {chrome['nav_column_px']}px of "
                f"navigation column but the {entry['tier']} tier owns {fact['nav_column_px']}px"
            )
        if nav_form(chrome) != fact["nav_form"]:
            found.append(
                f"{entry['scenario']}: the shell mounted {nav_form(chrome)} while the "
                f"{entry['tier']} tier says {fact['nav_form']}"
            )
        band = int(chrome["dock_band_px"])
        if (band > 0) != (nav_form(chrome) == "bottom_nav"):
            found.append(
                f"{entry['scenario']}: a {band}px dock band with form "
                f"{nav_form(chrome)} - a band belongs to a bar, not to a page density"
            )
        bottom = int(chrome["nav_stack_bottom_px"])
        if (bottom > 0) != (int(chrome["nav_column_px"]) > 0):
            found.append(
                f"{entry['scenario']}: a {chrome['nav_column_px']}px navigation column "
                f"with a stack bottom of {bottom}px - the column and the stack end are "
                "one fact seen from two edges, and a receipt that states one without the "
                "other describes geometry it cannot account for"
            )
        offset = nav_content_offset_px(chrome)
        limit = int(fact.get("nav_leading_limit_px", 0))
        if bool(fact.get("labelled_sidebar")):
            if offset <= 0:
                found.append(
                    f"{entry['scenario']}: the {form} column labels its rows but states "
                    "no leading, so how much of the column it gives away before the "
                    "first glyph is unmeasured"
                )
            elif offset > limit:
                found.append(
                    f"{entry['scenario']}: the {form} column gives away {offset}px of "
                    f"leading inside its {fact['nav_column_px']}px column, but the tier "
                    f"budgets {limit}px - ground the column already paid for twice"
                )
        elif offset > 0:
            found.append(
                f"{entry['scenario']}: a {offset}px leading with form {form}, which "
                "centres its glyphs in the column instead of indenting them"
            )
        if int(chrome["scale_percent"]) != 100:
            found.append(
                f"{entry['scenario']}: painted at {chrome['scale_percent']}% page scale, so its "
                "chrome numbers are not the pixels being measured"
            )
    return found


def frame_findings(results: list[dict], debt: dict) -> list[str]:
    found: list[str] = []
    registered = debt["frames"]
    structures = page_structures()
    for entry in results:
        if "unused_pct" not in entry:
            # Not measured, therefore not judged here: `chrome_findings`
            # reports why such a frame has no box at all.
            continue
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
    """Judge the navigation column of every frame that owns one.

    Which frames own a column is decided by `rail_of`, i.e. by the exported
    ladder and the pixels, not by a width the judge remembers.
    """
    found: list[str] = []
    registered = debt["rail"]
    columned = [entry for entry in results if "chrome_band_pct" in entry]
    if not columned:
        return found
    worst_chrome = max((entry.get("chrome_band_pct", 0.0) for entry in columned))
    worst_void = max((entry.get("void_pct", 0.0) for entry in columned))
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
    for name in [entry["id"] for entry in ladder()]:
        group = [entry for entry in results if entry["tier"] == name]
        if not group:
            continue
        label = name
        group = [entry for entry in group if "unused_pct" in entry]
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
    columned = [entry for entry in results if "chrome_band_pct" in entry]
    if columned:
        chrome = max(entry.get("chrome_band_pct", 0.0) for entry in columned)
        void = max(entry.get("void_pct", 0.0) for entry in columned)
        print(f"\n=== rail: worst non-nav chrome {chrome}% (limit {RAIL_CHROME_BAND_LIMIT}%), "
              f"worst nav void {void}% (limit {RAIL_VOID_LIMIT}%) ===")
    described = [entry for entry in results if entry.get("chrome")]
    print(f"\n=== chrome: {len(described)}/{len(results)} frames published their own "
          f"geometry; column "
          f"{sorted({int(entry['chrome']['nav_column_px']) for entry in described})}px, "
          f"margin {sorted({page_margin_px(entry['chrome']) for entry in described})}px, "
          f"band {sorted({int(entry['chrome']['dock_band_px']) for entry in described})}px ===")


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
    column = painted_nav_column(1180)
    image = _canvas(1180, 780)
    _fill(image, 0, 780, 0, column, (241, 246, 237))
    _fill(image, 0, 780, column, 1180, GROUND)
    return image


def _desktop_body_dark() -> np.ndarray:
    """The same frame on a dark rail, so a luminance-cut judge cannot hide."""
    column = painted_nav_column(1180)
    image = _canvas(1180, 780)
    _fill(image, 0, 780, 0, column, (20, 23, 27))
    _fill(image, 0, 780, column, 1180, (11, 13, 16))
    return image


def _chrome(width: int, height: int, **override) -> dict:
    """The chrome a planted frame of this size says it painted.

    The defaults are what the published shell really draws at that width -
    `chrome_findings` refuses a frame whose column, form, band or scale
    contradicts the exported ladder, so a test cannot plant a geometry the
    contract would reject without the planted frame saying so.
    """
    fact = tier_fact(tier(width))
    labelled = bool(fact["labelled_sidebar"])
    form = str(fact["nav_form"])
    chrome = {
        "nav_form": form,
        "nav_column_px": int(fact["nav_column_px"]),
        "nav_stack_bottom_px": 0 if form == "bottom_nav" else (488 if labelled else 398),
        "nav_content_offset_px": 12 if labelled else 0,
        "page_margin_px": 30 if labelled else 16,
        "shell_top_air_px": 8 if labelled else 0,
        "shell_bottom_air_px": 72 if labelled else 0,
        "dock_band_px": 72 if form == "bottom_nav" else 0,
        "scale_percent": 100,
    }
    chrome.update(override)
    return chrome


def _chrome_line(chrome: dict) -> str:
    return (CHROME_PREFIX + " "
            + " ".join(f"{key}={chrome[key]}" for key in chrome_keys()) + "\n")


def _write_run(root: pathlib.Path, frames: dict[str, np.ndarray],
               pages: dict[str, str] | None = None,
               chrome: dict | bool | None = None) -> pathlib.Path:
    """One published batch: pixels, and the chrome line the shell wrote beside them."""
    run = root / "batch"
    run.mkdir(parents=True, exist_ok=True)
    (run / METADATA).write_text(f"scenario_count={len(frames)}\n", encoding="utf-8")
    for name, image in frames.items():
        directory = run / name
        directory.mkdir(parents=True, exist_ok=True)
        Image.fromarray(image).save(directory / "image.png")
        # A dict that carries `nav_form` is the frame's chrome; any other dict
        # maps scenario names to their own chrome (or False to withhold it).
        facts = (chrome if isinstance(chrome, dict) and "nav_form" in chrome
                 else chrome.get(name) if isinstance(chrome, dict) else chrome)
        if facts is False:
            continue
        if facts is None:
            facts = _chrome(image.shape[1], image.shape[0])
        (directory / CHROME_FILE).write_text(
            f"CAPTURE_READY page=overview skin=dark\n{_chrome_line(facts)}",
            encoding="utf-8")
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

    def measure_one(self, image: np.ndarray, page: str = "",
                    chrome: dict | bool | None = None) -> dict:
        pages = {"planted": page} if page else None
        run = _write_run(self.root, {"planted": image}, pages, chrome)
        return measure(run)[0]

    def test_a_frame_filling_card_is_not_waste(self) -> None:
        image = _desktop_body()
        _fill(image, 96, 708, 258, 1150, CARD)
        for row in range(110, 700, 44):
            _fill(image, row, row + 18, 270, 1140, INK)
        entry = self.measure_one(image)
        self.assertLessEqual(entry["unused_pct"], fallback_limit_pct(tier(1180)), entry)

    def test_a_short_card_over_empty_ground_is_waste(self) -> None:
        image = _desktop_body()
        _fill(image, 96, 300, 258, 1150, CARD)
        for row in range(110, 290, 44):
            _fill(image, row, row + 18, 270, 1140, INK)
        entry = self.measure_one(image)
        self.assertGreater(entry["unused_pct"], fallback_limit_pct(tier(1180)), entry)
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
        self.assertEqual(limit, fallback_limit_pct(tier(1180)), limit)

    def test_the_shell_air_is_never_waste(self) -> None:
        # A card that fills the page body exactly, leaving only the shell's
        # reserved 8u/72u air, must read as full.
        image = _desktop_body()
        _fill(image, 8 + 1, 780 - 72, 258, 1150, CARD)
        _fill(image, 40, 58, 270, 1140, INK)
        _fill(image, 600, 618, 270, 1140, INK)
        entry = self.measure_one(image)
        self.assertLessEqual(entry["unused_pct"], fallback_limit_pct(tier(1180)), entry)

    def test_edge_noise_cannot_disguise_a_dead_page(self) -> None:
        image = _desktop_body()
        _fill(image, 96, 200, 258, 1150, CARD)
        _fill(image, 110, 128, 270, 1140, INK)
        # one stray row of ink at the very bottom of the box
        _fill(image, 706, 707, 258, 1150, INK)
        entry = self.measure_one(image)
        self.assertGreater(entry["unused_pct"], fallback_limit_pct(tier(1180)), entry)

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
        self.assertLess(entry["tail_pct"], fallback_limit_pct(tier(1180)), entry)
        self.assertGreater(entry["band_pct"], 30.0, entry)
        self.assertGreater(entry["unused_pct"], fallback_limit_pct(tier(1180)), entry)

    def test_the_card_is_never_its_own_ground_reference(self) -> None:
        # A card that is the MAJORITY colour of the frame, with ground below
        # it. Sampling the ground from inside the box would read the card as
        # ground and the ground as content, and hide the whole dead tail.
        image = _desktop_body()
        _fill(image, 96, 520, 258, 1150, CARD)
        _fill(image, 110, 128, 270, 1140, INK)
        _fill(image, 500, 512, 270, 1140, INK)
        entry = self.measure_one(image)
        self.assertGreater(entry["unused_pct"], fallback_limit_pct(tier(1180)), entry)

    def test_an_empty_interior_inside_a_frame_card_is_not_page_waste(self) -> None:
        # The b134 frame contract: a list card fills the page body and its
        # rows scroll inside it. Sparse rows inside that frame are the
        # frame's own viewport, not ground, and must not be reported as waste.
        image = _desktop_body()
        _fill(image, 96, 708, 258, 1150, CARD)
        for row in (140, 190, 240):
            _fill(image, row, row + 18, 270, 1140, INK)
        entry = self.measure_one(image)
        self.assertLessEqual(entry["unused_pct"], fallback_limit_pct(tier(1180)), entry)

    def test_unregistered_debt_fails_and_registered_debt_passes(self) -> None:
        image = _desktop_body()
        _fill(image, 96, 300, 258, 1150, CARD)
        _fill(image, 110, 128, 270, 1140, INK)
        entry = self.measure_one(image)
        self.assertNotEqual(frame_findings([entry], {"frames": {}, "rail": {}}), [])
        debt = {"frames": {"planted": entry["unused_pct"]}, "rail": {}}
        self.assertEqual(frame_findings([entry], debt), [])

    def test_debt_may_only_shrink(self) -> None:
        entry = dict(scenario="planted", tier="expanded", unused_pct=30.0,
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
        # The rows end above the receipt's measured stack bottom (488 on the
        # planted desktop chrome), so every band is navigation by definition.
        for y in (100, 180, 260, 340, 420):
            _fill(image, y, y + 40, 0, 200, (220, 238, 226))
            _fill(image, y + 12, y + 28, 34, 140, INK)

    def test_a_status_card_below_the_nav_stack_is_caught(self) -> None:
        image = _desktop_body()
        self._nav_only_rail(image)
        _fill(image, 617, 712, 18, 210, CARD)
        entry = self.measure_one(image, chrome=_chrome(1180, 780))
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
        for y in (100, 180, 260, 340, 420):
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
        entry = self.measure_one(image, chrome=_chrome(1180, 780))
        self.assertEqual(entry["chrome_band_pct"], 0.0, entry)
        self.assertEqual(rail_findings([entry], {"frames": {}, "rail": {}}), [])

    def test_the_rail_verdict_does_not_depend_on_the_skin(self) -> None:
        light = _desktop_body()
        self._nav_only_rail(light)
        _fill(light, 617, 712, 18, 210, CARD)
        dark = _desktop_body_dark()
        for y in (100, 180, 260, 340, 420):
            _fill(dark, y, y + 40, 0, 200, (6, 73, 91))
            _fill(dark, y + 12, y + 28, 34, 140, (213, 241, 248))
        _fill(dark, 617, 712, 18, 210, (29, 33, 38))
        light_entry = self.measure_one(light, chrome=_chrome(1180, 780))
        dark_entry = self.measure_one(dark, chrome=_chrome(1180, 780))
        self.assertEqual(light_entry["chrome_band_pct"], dark_entry["chrome_band_pct"],
                         (light_entry, dark_entry))

    def test_a_window_overlay_across_the_column_edge_is_not_rail_chrome(self) -> None:
        # The notice banner is laid out by the window and overhangs the rail;
        # its colour keeps going past the column's edge, so it is not a block
        # the rail spent its height on and must not be charged to it.
        image = _desktop_body()
        self._nav_only_rail(image)
        _fill(image, 617, 712, 0, 1180, CARD)
        entry = self.measure_one(image, chrome=_chrome(1180, 780))
        self.assertEqual(entry["chrome_band_pct"], 0.0, entry)

    def test_nav_rows_below_the_old_70pct_line_are_not_accused(self) -> None:
        # The shape-blind line this judge used to guess (70% of the height)
        # accused real nav rows in short windows. The receipt's measured bottom
        # is the only line now: rows end above it, so nothing is accused.
        image = _desktop_body()
        for y in (100, 180, 260, 340, 420):
            _fill(image, y, y + 44, 0, 200, (220, 238, 226))
            _fill(image, y + 14, y + 30, 34, 140, INK)
        entry = self.measure_one(image, chrome=_chrome(1180, 780))
        self.assertEqual(entry["chrome_band_pct"], 0.0, entry)

    def test_rail_rows_below_the_old_70pct_line_are_not_accused(self) -> None:
        # The exact frame shape that broke the fraction: on the 720x480 rail
        # frame the 70% line is 336 while the measured stack bottom is 398, so
        # an extra destination row landing in between is navigation, not the
        # bottom chrome card the old judge read it as.
        image = _canvas(720, 480)
        self._icon_rail(image)
        _fill(image, 350, 386, 24, 40, INK)
        entry = self.measure_one(image)
        self.assertEqual(entry["chrome_band_pct"], 0.0, entry)

    def test_a_stack_end_without_a_column_is_reported(self) -> None:
        # The column and the stack end are one fact seen from two edges; a
        # receipt that states one without the other is a shell describing
        # geometry it cannot account for, reported rather than judged.
        entry = self.measure_one(_canvas(400, 800),
                                 chrome=_chrome(400, 800, nav_stack_bottom_px=398))
        found = chrome_findings([entry])
        self.assertTrue(any("one fact seen from two edges" in item for item in found),
                        found)

    def test_a_column_without_a_stack_end_is_reported(self) -> None:
        # The other half of the pair: a columned frame that states no bottom
        # would leave the rail axis blind again, so the receipt is refused.
        entry = self.measure_one(_desktop_body(),
                                 chrome=_chrome(1180, 780, nav_stack_bottom_px=0))
        found = chrome_findings([entry])
        self.assertTrue(any("one fact seen from two edges" in item for item in found),
                        found)

    def test_a_leading_over_the_exported_budget_is_reported(self) -> None:
        # SPACEUTIL-010: a labelled column may not give away more of its own
        # width than the tier budgets. 32px was the value b158 measured by hand
        # (13.3% of 240 against a 8% budget), so it must fail here.
        entry = self.measure_one(_desktop_body(),
                                 chrome=_chrome(1180, 780, nav_content_offset_px=32))
        found = chrome_findings([entry])
        self.assertTrue(any("already paid for twice" in item for item in found), found)
        budget = int(tier_fact("expanded")["nav_leading_limit_px"])
        keeps = self.measure_one(_desktop_body(),
                                 chrome=_chrome(1180, 780, nav_content_offset_px=budget))
        self.assertEqual(chrome_findings([keeps]), [])

    def test_a_labelled_column_without_a_leading_is_reported(self) -> None:
        # Half of the pair: a pane that labels its rows states the leading it
        # gives away, so stating none leaves that axis unmeasured.
        entry = self.measure_one(_desktop_body(),
                                 chrome=_chrome(1180, 780, nav_content_offset_px=0))
        found = chrome_findings([entry])
        self.assertTrue(any("states no leading" in item for item in found), found)

    def test_a_rail_may_not_claim_a_leading(self) -> None:
        # The other half: an icon rail centres its glyphs in the column, so a
        # nonzero leading describes geometry the form does not own.
        entry = self.measure_one(_canvas(720, 480), chrome=_chrome(720, 480,
                                                                  nav_content_offset_px=12))
        found = chrome_findings([entry])
        self.assertTrue(any("centres its glyphs" in item for item in found), found)

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

    def _with_export(self, document: dict):
        """Point the judge at a planted export for the body of a test."""
        path = self.root / "planted-structure.json"
        path.write_text(json.dumps(document), encoding="utf-8")
        return path

    def test_the_ladder_boundaries_come_from_the_export(self) -> None:
        # A ladder with different boundaries must move the tiers with it.
        # This is the proof that the judge reads the exported table instead
        # of the boundaries it used to remember (700 and 1000).
        planted = self._with_export({
            "version": 4, "pages": [], "slugs": [],
            "chrome_keys": CHROME_KEYS_FOR_TEST,
            "tiers": [
                {"id": "compact", "min_width_px": 0, "max_width_px": 500,
                 "nav_form": "bottom_nav", "nav_column_px": 0,
                 "max_nav_column_px": 0, "nav_leading_limit_px": 0,
                 "labelled_sidebar": False, "proxy_node_columns": 1,
                 "overview_card_columns": 1, "fallback_ground_limit_pct": 8.0},
                {"id": "expanded", "min_width_px": 500, "max_width_px": None,
                 "nav_form": "standard", "nav_column_px": 200, "max_nav_column_px": 200,
                 "nav_leading_limit_px": 16,
                 "labelled_sidebar": True, "proxy_node_columns": 3,
                 "overview_card_columns": 2, "fallback_ground_limit_pct": 12.0},
            ],
        })
        global PAGE_STRUCTURE_FILE
        original = PAGE_STRUCTURE_FILE
        PAGE_STRUCTURE_FILE = planted
        try:
            self.assertEqual(tier(480), "compact")
            self.assertEqual(tier(520), "expanded")
            self.assertEqual(painted_nav_column(480), 0)
            self.assertEqual(painted_nav_column(520), 200)
            self.assertEqual(fallback_limit_pct("expanded"), 12.0)
            self.assertEqual(ladder_problems(), [])
        finally:
            PAGE_STRUCTURE_FILE = original

    def test_a_ladder_without_a_tier_table_is_refused(self) -> None:
        # The judge keeps no boundaries of its own, so an export that carries
        # no ladder cannot be judged at all rather than silently judged by
        # remembered numbers.
        planted = self._with_export({"version": 1, "pages": [], "slugs": []})
        global PAGE_STRUCTURE_FILE
        original = PAGE_STRUCTURE_FILE
        PAGE_STRUCTURE_FILE = planted
        try:
            self.assertEqual(ladder_problems(),
                             ["page-structure.json carries no viewport ladder; "
                              "rerun export_page_structure"])
        finally:
            PAGE_STRUCTURE_FILE = original

    def test_a_gapped_ladder_is_refused(self) -> None:
        # Half-open spans must tile the axis. A ladder with a gap would leave
        # a width that belongs to no tier, and the judge would pick a limit
        # for it anyway.
        planted = self._with_export({
            "version": 4, "pages": [], "slugs": [],
            "chrome_keys": CHROME_KEYS_FOR_TEST,
            "tiers": [
                {"id": "compact", "min_width_px": 0, "max_width_px": 500,
                 "nav_form": "bottom_nav", "nav_column_px": 0,
                 "labelled_sidebar": False, "proxy_node_columns": 1,
                 "overview_card_columns": 1, "fallback_ground_limit_pct": 8.0},
                {"id": "expanded", "min_width_px": 600, "max_width_px": None,
                 "nav_form": "standard", "nav_column_px": 200, "max_nav_column_px": 200,
                 "labelled_sidebar": True, "proxy_node_columns": 3,
                 "overview_card_columns": 2, "fallback_ground_limit_pct": 12.0},
            ],
        })
        global PAGE_STRUCTURE_FILE
        original = PAGE_STRUCTURE_FILE
        PAGE_STRUCTURE_FILE = planted
        try:
            self.assertTrue(any("in no tier" in item for item in ladder_problems()),
                            ladder_problems())
        finally:
            PAGE_STRUCTURE_FILE = original

    def test_a_narrow_tier_frame_is_never_measured_as_a_sidebar_frame(self) -> None:
        # The defect this retires: 800..999 was judged as if it had no
        # sidebar while the surface drew a 240px column, and 1000+ was
        # measured as a sidebar frame even in a tier that draws none. The
        # answer now comes from one exported fact per tier.
        self.assertEqual(tier(820), "medium")
        self.assertEqual(tier(1180), "expanded")
        self.assertEqual(shell_bottom_air_px(_chrome(1180, 780)), 72)
        self.assertEqual(shell_bottom_air_px(_chrome(820, 480)), 0)
        entry = self.measure_one(_canvas(820, 480))
        self.assertNotIn("chrome_band_pct", entry, entry)

    def test_the_medium_tier_pays_its_rail_and_keeps_its_bottom(self) -> None:
        # SPACEUTIL-006, measured rather than asserted: a 720x480 desktop frame
        # reserves the exported 64px icon rail, so its page body starts at the
        # rail's edge, and because that form owns no bar the shell reserves no
        # dock band - the 72 rows this window used to spend on a phone's bottom
        # bar belong to the page now. The labelled column keeps its own numbers.
        medium = _chrome(820, 480)
        self.assertEqual(painted_nav_column(820), 64)
        self.assertTrue(owns_nav_column(medium))
        self.assertEqual(content_box(820, 480, medium)[0], 66)
        self.assertEqual(content_box(820, 480, medium)[2], 64 + page_margin_px(medium))
        self.assertEqual(content_box(820, 480, medium)[5], 479)
        desktop = _chrome(1180, 780)
        self.assertEqual(painted_nav_column(1180), 240)
        self.assertEqual(content_box(1180, 780, desktop)[0], 242)
        self.assertTrue(owns_nav_column(desktop))
        # And no column at all where the exported form is a bottom bar.
        phone = _chrome(400, 800)
        self.assertEqual(painted_nav_column(400), 0)
        self.assertFalse(owns_nav_column(phone))
        self.assertEqual(content_box(400, 800, phone)[0], 2)
        self.assertTrue(phone["dock_band_px"] > 0)

    def test_a_frame_that_says_nothing_about_its_chrome_is_not_measured(self) -> None:
        # The whole point of the receipt: without it the judge would have to
        # remember a padding, which is the second source SPACEUTIL-007 retires.
        entry = self.measure_one(_desktop_body(), chrome=False)
        self.assertIsNone(entry["chrome"], entry)
        self.assertNotIn("unused_pct", entry, entry)
        found = chrome_findings([entry])
        self.assertTrue(any("no CAPTURE_CHROME line" in item for item in found), found)
        # And it is not silently judged against a remembered box either.
        self.assertEqual(frame_findings([entry], {"frames": {}, "rail": {}}), [])

    def test_the_measurement_box_follows_the_receipt(self) -> None:
        # A shell that paints a deeper margin gets a deeper box, with no edit
        # to this file: the box is the frame's, not the judge's.
        remembered = content_box(1180, 780, _chrome(1180, 780))
        deeper = content_box(1180, 780, _chrome(1180, 780, page_margin_px=64))
        self.assertEqual(remembered[2], 240 + 30)
        self.assertEqual(deeper[2], 240 + 64)
        self.assertEqual(deeper[4], remembered[4])

    def test_a_column_that_disagrees_with_the_ladder_is_reported(self) -> None:
        # The private 228 this surface used to keep is caught per frame now,
        # not by pinning two source files against each other.
        entry = self.measure_one(_desktop_body(),
                                 chrome=_chrome(1180, 780, nav_column_px=228))
        found = chrome_findings([entry])
        self.assertTrue(any("reserved 228px" in item for item in found), found)

    def test_a_dock_band_without_a_bar_is_reported(self) -> None:
        # SPACEUTIL-006's defect, seen from the receipt side: a desktop frame
        # that pays a band while mounting a column reserves air for nothing.
        entry = self.measure_one(_desktop_body(),
                                 chrome=_chrome(1180, 780, dock_band_px=72))
        found = chrome_findings([entry])
        self.assertTrue(any("a band belongs to a bar" in item for item in found), found)

    def test_an_icon_rail_describes_itself_and_is_not_accused(self) -> None:
        # The medium frame says rail/64/no band, which is exactly what the
        # exported ladder owns for that tier, so nothing is reported - and the
        # rail axis judges it by the same receipt rule as a labelled pane: this
        # planted canvas paints no bands at all, so there is no rail verdict,
        # and the icon rail's real frames are judged below via the medium tier.
        entry = self.measure_one(_canvas(720, 480))
        self.assertEqual(nav_form(entry["chrome"]), "rail", entry)
        self.assertEqual(chrome_findings([entry]), [], entry)
        self.assertNotIn("chrome_band_pct", entry, entry)

    def _icon_rail(self, image: np.ndarray) -> None:
        """A medium frame's rail: background, one selected wash, eight glyphs."""
        _fill(image, 0, image.shape[0], 0, 64, (241, 246, 237))
        for y in (18, 62, 106, 150, 194, 238, 282, 326):
            _fill(image, y + 10, y + 26, 24, 40, INK)
        _fill(image, 18, 54, 0, 64, (220, 238, 226))

    def test_an_icon_rail_frame_is_judged_by_the_same_receipt_rule(self) -> None:
        # SPACEUTIL-009 opens the rail axis to the icon rail: the receipt says
        # where the stack ends, so the medium tier is judged on the same terms
        # as the labelled pane - rows above the measured bottom are navigation,
        # and a card parked below it is the rail spending itself on chrome.
        image = _canvas(720, 480)
        self._icon_rail(image)
        _fill(image, 404, 463, 12, 52, CARD)
        entry = self.measure_one(image)
        self.assertGreater(entry["chrome_band_pct"], RAIL_CHROME_BAND_LIMIT, entry)
        self.assertNotEqual(rail_findings([entry], {"frames": {}, "rail": {}}), [])

    def test_a_toast_over_the_icon_rail_is_window_overlay(self) -> None:
        # The compact notice banner is centred by the window and overhangs the
        # 64px rail; its paint continues across the column edge, so it is not
        # charged to a rail that never painted it.
        image = _canvas(720, 480)
        self._icon_rail(image)
        _fill(image, 404, 463, 0, 720, CARD)
        entry = self.measure_one(image)
        self.assertEqual(entry["chrome_band_pct"], 0.0, entry)

    def test_the_exported_column_never_breaches_its_own_ceiling(self) -> None:
        # The ratio standard is checked against the real export, so a tier that
        # grabbed more width than the standard allows fails here rather than
        # winning an argument about how the reference product looks.
        for fact in ladder():
            self.assertLessEqual(
                fact["nav_column_px"],
                fact["max_nav_column_px"],
                f"{fact['id']} reserves {fact['nav_column_px']}px, ceiling "
                f"{fact['max_nav_column_px']}px",
            )
            self.assertEqual(
                fact["touch_nav_form"], "bottom_nav",
                "a touch surface navigates with a bar at every width")


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
    # No ladder, no verdict: the judge keeps no boundaries of its own, so a
    # missing or gapped export must stop the pixel leg instead of silently
    # falling back to remembered numbers.
    problems = ladder_problems()
    if problems:
        for problem in problems:
            print(f"space-utilization: {problem}", file=sys.stderr)
        return 1
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
        measured = [entry for entry in results if "unused_pct" in entry]
        frames = {entry["scenario"]: entry["unused_pct"] for entry in measured
                  if entry["unused_pct"] > limit_for(entry["scenario"], entry["tier"],
                                                    structures, entry.get("page", ""))}
        columned = [entry for entry in results if "chrome_band_pct" in entry]
        rail = {}
        if columned:
            rail["chrome_band_pct"] = max(
                entry.get("chrome_band_pct", 0.0) for entry in columned)
            rail["void_pct"] = max(entry.get("void_pct", 0.0) for entry in columned)
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

    findings = (chrome_findings(results) + frame_findings(results, debt)
                + rail_findings(results, debt))
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
