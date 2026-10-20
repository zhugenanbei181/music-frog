#!/usr/bin/env python3
"""Batch acceptance: real behaviour anchors plus real dual-viewport pixels per business.

One run certifies a list of businesses over the surfaces that carry them.  A
business that fails never stops the batch: every ``(business, surface,
viewport)`` cell ends in a machine-readable report with the reason why it is
not certified.  The run still fails when any carried cell is uncertified.

Evidence per cell
-----------------

* **L1/L2** the declared behaviour anchors of the business run in the same
  build; a missing or failing anchor leaves the cell uncertified.
* **L3** the surface really activates the business: the demo publishes
  ``activated=true`` with native bounds, the compositor pixels match the
  native renderer readback, and the activated region differs from the
  route-only render of the same page (see :mod:`interaction_delta`).
"""
from __future__ import annotations

import argparse
import csv
import json
import os
import re
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

from interaction_delta import changed_pixels, corroborate
from resolve_surface_evidence import ROOT, command_json, read_table, EVIDENCE_FIELDS
from visual_receipts import (build_receipts_tolerant, interaction_bounds, manifest_rows,
                              source_fingerprint, verified_receipts_tolerant)

SURFACE_PACKAGES = {"iced": "infiltrator-iced", "bevy": "infiltrator-bevy-ui", "slint": "infiltrator-slint"}
VIEWPORT_SIZES = {"standard": (1180, 780), "compact": (720, 480)}
VIEWPORTS = tuple(VIEWPORT_SIZES)
# The catalogue names the business page; the capture marker publishes the
# mounted route.  These are the only renames the surfaces apply.
MARKER_ROUTES = {
    "iced": {"mixin": "editor", "filter": "editor", "rules-providers": "rules"},
    "bevy": {"app-routing": "app_routing"},
}
# Route-only captures of one page and viewport.  The second replicate measures
# the noise floor of animated pages.
BASELINE_PREFIX = "baseline"
ANCHOR_LEVELS = ("contract", "scenario")
FAIL_PATTERN = re.compile(r"^\s*FAIL\s+\[\s*[\d.]+s\]\s+(\S+)\s+(\S+)", re.MULTILINE)
BUILD_FAILURE = re.compile(r"could not compile|^error\[E\d+\]", re.MULTILINE)
SUMMARY_PATTERN = re.compile(r"Summary \[.*?\]\s+(\d+) tests? run")


def catalogue_specs():
    """The compiled business registry: the single source of every business id."""
    payload = command_json(["cargo", "run", "--quiet", "-p", "infiltrator-contract",
                             "--example", "parity_catalogue", "--jobs", "4"])
    return {spec["id"]: spec for spec in payload}


def anchor_selection(evidence, features):
    """The declared anchors of the selected businesses and the packages that own them.

    The discovery has to cover every package the registry anchors to (the
    closure resolver rejects an anchor it cannot see), while only the selected
    businesses are actually executed.
    """
    wanted, pending, owners = {}, {}, set()
    for row in evidence:
        if row["status"] != "anchored":
            if row["feature_id"] in features:
                pending.setdefault((row["feature_id"], row["surface"]), []).append(f"{row['level']}: {row['reason']}")
            continue
        for test_id in row["test_id"].split(";"):
            if test_id and test_id != "-":
                package = test_id.split("::", 1)[0]
                owners.add(package)
                if row["feature_id"] in features:
                    wanted.setdefault(package, set()).add(test_id)
    return wanted, pending, owners


def discovery_index(package, attempts=8):
    """The runnable tests of one package: the exact nextest identity of every case."""
    command = ["cargo", "nextest", "list", "-p", package]
    if package in ("infiltrator-iced", "infiltrator-bevy-ui"):
        command += ["--features", "capture-tools"]
    command += ["--build-jobs", "4", "--message-format", "json"]
    last = None
    for attempt in range(attempts):
        try:
            payload = command_json(command)
        except (subprocess.CalledProcessError, ValueError, OSError) as error:
            last = error
            # Another work stream may be compiling the same tree; a transient
            # build failure must not decide the acceptance.
            time.sleep(30)
            continue
        index = {}
        for binary_id, suite in payload.get("rust-suites", {}).items():
            for name, case in suite.get("testcases", {}).items():
                if case.get("ignored", False):
                    continue
                index[f"{binary_id}::{name}"] = (suite.get("binary-name"), name)
        return payload, index
    raise last


def run_anchors(output, wanted, owners):
    """Run the declared behaviour anchors package by package.

    A package that cannot be built or whose tests fail is a recorded fact for
    the cells that depend on it, never a stop for the whole batch.
    """
    results, index = {}, {}
    (output / "discovery").mkdir(exist_ok=True)
    for package in sorted(owners):
        try:
            payload, package_index = discovery_index(package)
        except (subprocess.CalledProcessError, ValueError, OSError) as error:
            results[package] = {"status": "discovery-failed", "detail": str(error), "tests": {}}
            continue
        (output / "discovery" / f"{package}.json").write_text(json.dumps(payload))
        index[package] = package_index
    for package, tests in sorted(wanted.items()):
        if results.get(package, {}).get("status") == "discovery-failed":
            results[package]["tests"] = {test_id: "unknown" for test_id in sorted(tests)}
            continue
        package_index = index.get(package, {})
        filters, missing = [], []
        for test_id in sorted(tests):
            entry = package_index.get(test_id)
            if entry is None:
                missing.append(test_id)
                continue
            binary_name, testcase = entry
            filters.append(f"(binary(={binary_name}) & test(={testcase}))")
        if not filters:
            results[package] = {"status": "no-matching-tests",
                                 "tests": {test_id: "unknown" for test_id in sorted(tests)}}
            continue
        command = ["cargo", "nextest", "run", "-p", package]
        if package in ("infiltrator-iced", "infiltrator-bevy-ui"):
            command += ["--features", "capture-tools"]
        command += ["--build-jobs", "4", "--test-threads", "4", "--no-fail-fast", "-E", " | ".join(filters)]
        completed = None
        for attempt in range(4):
            completed = subprocess.run(command, cwd=ROOT, capture_output=True, text=True)
            if completed.returncode == 0 or not BUILD_FAILURE.search(completed.stdout + completed.stderr):
                break
            # Another work stream may be compiling the same tree; a transient
            # build failure must not decide the acceptance.
            time.sleep(30)
        log = output / "anchors" / f"{package}.log"
        log.parent.mkdir(exist_ok=True)
        log.write_text(completed.stdout + completed.stderr)
        output_text = completed.stdout + completed.stderr
        failed = {f"{package}::{name}" for _, name in FAIL_PATTERN.findall(completed.stdout)}
        # A package that does not compile runs no test at all: its anchors stay
        # unknown, they are never passed.
        summary = SUMMARY_PATTERN.search(output_text)
        not_run = bool(BUILD_FAILURE.search(output_text)) or "no tests to run" in output_text \
            or (summary is not None and summary.group(1) == "0")
        statuses = {}
        for test_id in sorted(tests):
            if not_run or test_id in missing:
                statuses[test_id] = "unknown"
            elif test_id in failed:
                statuses[test_id] = "failed"
            else:
                statuses[test_id] = "passed"
        results[package] = {"status": "ok" if completed.returncode == 0 else "failed",
                            "returncode": completed.returncode, "tests": statuses}
    return results


def capture_matrix(features, pages, replicates):
    """The rows of one capture matrix: every business scenario plus route-only baselines."""
    rows = []
    for feature in sorted(features):
        for viewport, (width, height) in VIEWPORT_SIZES.items():
            rows.append({"name": f"{feature}-{viewport}", "page": pages[feature], "skin": "dark",
                         "window_size": f"{width}x{height}"})
    for page in sorted(set(pages.values())):
        for viewport, (width, height) in VIEWPORT_SIZES.items():
            for replicate in range(1, replicates + 1):
                prefix = BASELINE_PREFIX if replicate == 1 else f"{BASELINE_PREFIX}-r{replicate}"
                rows.append({"name": f"{prefix}-{page}-{width}x{height}", "page": page, "skin": "dark",
                             "window_size": f"{width}x{height}"})
    return rows


def run_capture(surface, rows, output, attempts=2):
    """Capture one surface matrix in the private compositor stack."""
    matrix = output / f"{surface}-scenarios.tsv"
    with matrix.open("w") as file:
        writer = csv.writer(file, delimiter="\t", lineterminator="\n")
        writer.writerow(["name", "page", "skin", "window_size"])
        for row in rows:
            writer.writerow([row["name"], row["page"], row["skin"], row["window_size"]])
    env = dict(os.environ,
               INFILTRATOR_CAPTURE_MATRIX=str(matrix),
               INFILTRATOR_CAPTURE_OUT_DIR=str(output / f"{surface}-published"),
               INFILTRATOR_LANG="en-US", CARGO_BUILD_JOBS="4", INFILTRATOR_CAPTURE_SCENARIOS="")
    evidence_root = ROOT / f".evidence/captures/{surface}"
    completed = None
    for attempt in range(attempts):
        previous = set(evidence_root.iterdir()) if evidence_root.exists() else set()
        completed = subprocess.run(["bash", f"scripts/capture-{surface}-matrix.sh"], cwd=ROOT, env=env,
                                   capture_output=True, text=True)
        fresh = sorted(set(evidence_root.iterdir()) - previous, key=lambda path: path.name)
        if fresh and (fresh[-1] / "capture-manifest.tsv").is_file():
            (output / f"{surface}-capture.log").write_text(completed.stdout + completed.stderr)
            return fresh[-1], completed
        # The capture stack or the product build may have been disturbed by
        # another work stream; one clean retry decides the acceptance.
        time.sleep(10)
    (output / f"{surface}-capture.log").write_text(completed.stdout + completed.stderr if completed else "")
    return None, completed


def baseline_images(run, rows):
    """The route-only captures of every page and viewport, plus their noise floor.

    The rows are the capture manifest rows: they name the page and the
    requested window of every captured scenario.
    """
    baselines = {}
    for row in rows:
        name = row["scenario"]
        if not name.startswith(BASELINE_PREFIX):
            continue
        page = row["page"]
        width, height = (int(value) for value in row["requested_window"].split("x"))
        viewport = "standard" if (width, height) == VIEWPORT_SIZES["standard"] else "compact"
        image = run / name / "image.png"
        if not image.is_file():
            continue
        entry = baselines.setdefault((page, viewport), {"primary": None, "replicate": None})
        if entry["primary"] is None:
            entry["primary"] = image
        elif entry["replicate"] is None:
            entry["replicate"] = image
    return baselines


def capture_panic(run, scenario):
    """The panic the surface hit while activating the business, if any."""
    log = run / scenario / "app.log"
    if not log.is_file():
        return ""
    text = log.read_text()
    if "panicked at" not in text:
        return ""
    lines = [line.strip() for line in text.splitlines() if line.strip()]
    for index, line in enumerate(lines):
        if "panicked at" in line:
            return f"{line} {lines[index + 1] if index + 1 < len(lines) else ''}".strip()
    return ""


def capture_diagnostic(run, scenario):
    """The surface's own admission of what it could not show or measure."""
    log = run / scenario / "app.log"
    if not log.is_file():
        return ""
    diagnostics = [line.strip() for line in log.read_text().splitlines()
                   if line.startswith("native capture") or line.startswith("capture geometry")]
    return diagnostics[-1] if diagnostics else ""


def classify_cell(run, surface, feature, viewport, page, rows_by_scenario, baselines):
    """Decide one ``(business, surface, viewport)`` cell from the captured evidence."""
    scenario = f"{feature}-{viewport}"
    cell = {"feature_id": feature, "surface": surface, "viewport": viewport, "status": "uncertified",
            "reason": "", "image": "", "sha256": "", "build_id": "", "bounds": [],
            "changed_pixels": 0, "noise_pixels": 0, "required_pixels": 0, "compared_pixels": 0,
            "anchor_note": ""}
    row = rows_by_scenario.get(scenario)
    if row is None:
        cell["reason"] = "capture failed: the capture matrix produced no row for this scenario"
        return cell
    if row["status"] != "ok":
        status = row["status"]
        panic = capture_panic(run, scenario)
        if panic:
            # The surface crashed while activating the business: a product
            # fault, not an environment one.
            cell["reason"] = f"surface panic while activating the business: {panic}"
            return cell
        if status.startswith("failed-marker"):
            # The surface never reached the interaction state of this business:
            # the demo has no activator that brings the business on screen.
            diagnostic = capture_diagnostic(run, scenario)
            cell["reason"] = (f"no demo interaction: the surface never published the activation marker "
                              f"for this business ({status})")
            if diagnostic:
                cell["reason"] += f" - surface diagnostic: {diagnostic}"
        elif status.startswith("failed-window"):
            cell["reason"] = f"capture failed (environment): the window never appeared ({status})"
        else:
            cell["reason"] = f"capture failed: {status}"
        return cell
    marker_path = run / scenario / "marker.log"
    image_path = run / scenario / "image.png"
    if not marker_path.is_file() or not image_path.is_file():
        cell["reason"] = "capture failed: the scenario published no marker or no image"
        return cell
    marker = marker_path.read_text()
    if f"scenario={feature} activated=true" not in marker:
        cell["reason"] = "no demo interaction: the capture published no activation marker"
        return cell
    try:
        bounds = interaction_bounds(marker, feature, viewport)
    except ValueError as error:
        cell["reason"] = f"missing interaction bounds: {error}"
        return cell
    cell["bounds"] = bounds
    expected_route = MARKER_ROUTES.get(surface, {}).get(page, page)
    observed = re.search(r"CAPTURE_READY page=(\S+)", marker)
    if observed is None or observed.group(1) != expected_route:
        cell["reason"] = (f"wrong page: expected {expected_route}, observed "
                          f"{observed.group(1) if observed else 'none'}")
        return cell
    expected_size = VIEWPORT_SIZES[viewport]
    try:
        from PIL import Image
        with Image.open(image_path) as image:
            size = image.size
    except (OSError, ValueError) as error:
        cell["reason"] = f"capture failed: unreadable pixels ({error})"
        return cell
    if size != expected_size:
        cell["reason"] = (f"wrong viewport: expected {expected_size[0]}x{expected_size[1]}, "
                          f"observed {size[0]}x{size[1]}")
        return cell
    baseline = baselines.get((page, viewport))
    if baseline is None or baseline["primary"] is None:
        cell["reason"] = f"no demo interaction: no route-only baseline capture for page {page}"
        return cell
    try:
        noise = 0
        if baseline["replicate"] is not None:
            noise = changed_pixels(baseline["replicate"], baseline["primary"], bounds)[0]
        verdict = corroborate(image_path, baseline["primary"], noise, bounds)
    except ValueError as error:
        cell["reason"] = f"capture failed: {error}"
        return cell
    cell["changed_pixels"] = verdict["changed_pixels"]
    cell["compared_pixels"] = verdict["compared_pixels"]
    cell["noise_pixels"] = verdict["noise_pixels"]
    cell["required_pixels"] = verdict["required_pixels"]
    if not verdict["corroborated"]:
        cell["reason"] = (
            f"no demo interaction: the activated pixels match the route-only render "
            f"({verdict['changed_pixels']} of {verdict['compared_pixels']} region pixels changed, "
            f"noise floor {verdict['noise_pixels']}, required {verdict['required_pixels']})")
        diagnostic = capture_diagnostic(run, scenario)
        if diagnostic:
            cell["reason"] += f" - surface diagnostic: {diagnostic}"
        return cell
    cell["status"] = "certified"
    return cell


def anchor_reason(evidence, anchor_results, feature, surface):
    """The behaviour side of a cell: the declared anchors must exist and pass."""
    levels = {}
    for row in evidence:
        if row["feature_id"] != feature or row["surface"] != surface:
            continue
        levels[row["level"]] = row
    for level in ANCHOR_LEVELS:
        row = levels.get(level)
        if row is None:
            return f"no behaviour anchor: the registry has no {level} cell"
        if row["status"] != "anchored":
            return f"no behaviour anchor ({level}): {row['reason']}"
        for test_id in row["test_id"].split(";"):
            if not test_id or test_id == "-":
                continue
            package = test_id.split("::", 1)[0]
            status = anchor_results.get(package, {}).get("tests", {}).get(test_id)
            if status == "failed":
                return f"behaviour anchor failed: {test_id}"
            if status != "passed":
                return f"behaviour anchor not proven: {test_id} ({status or 'not run'})"
    return ""


def surface_registry_state(evidence, features, surface, specs):
    """What a surface outside the acceptance scope can and cannot certify."""
    carried = sorted(feature for feature in features if specs[feature][f"{surface}_page"])
    anchored = sorted({row["feature_id"] for row in evidence
                       if row["surface"] == surface and row["feature_id"] in features
                       and row["level"] == "scenario" and row["status"] == "anchored"})
    missing = sorted(feature for feature in carried if feature not in anchored)
    reasons = sorted({row["reason"] for row in evidence
                      if row["surface"] == surface and row["feature_id"] in missing
                      and row["level"] == "scenario" and row["reason"] != "-"})
    state = {"carried_businesses": len(carried), "certifiable_businesses": anchored,
             "businesses_without_a_scenario_anchor": missing,
             "missing_capability": reasons[0] if reasons else ""}
    if not anchored:
        state["status"] = "not_certifiable"
        state["reason"] = ("the surface has no scenario anchor for the selected businesses"
                           + (f" ({reasons[0]})" if reasons else ""))
    else:
        state["status"] = "not_in_scope"
        state["reason"] = (f"the surface is outside this acceptance scope; it carries {len(carried)} "
                           f"business(es), {len(anchored)} of them have a scenario anchor")
    return state


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("features", nargs="?", help="comma-separated business ids")
    parser.add_argument("--all", action="store_true", help="accept every business of the typed registry")
    parser.add_argument("--surfaces", default=None,
                        help="comma-separated surfaces in the acceptance scope (default: iced,bevy)")
    parser.add_argument("--replicates", type=int, default=2, help="route-only captures per page and viewport")
    parser.add_argument("--skip-anchors", action="store_true", help="pixels only; do not run the behaviour anchors")
    parser.add_argument("--out", type=Path, help="output directory (default: .evidence/runs/<stamp>)")
    parser.add_argument("--reclassify", type=Path,
                        help="re-derive the report from the pixels of an earlier run (no new capture)")
    parser.add_argument("--rerun-anchors", action="store_true",
                        help="with --reclassify: run the behaviour anchors again")
    args = parser.parse_args()
    if args.reclassify:
        return reclassify(args.reclassify.resolve(), args.out, args.surfaces, args.rerun_anchors)
    if bool(args.features) == args.all:
        parser.error("select explicit features or --all")
    surfaces = tuple(filter(None, (args.surfaces or "iced,bevy").split(",")))
    unknown = set(surfaces) - set(SURFACE_PACKAGES)
    if unknown:
        parser.error(f"unknown surface: {sorted(unknown)}")

    specs = catalogue_specs()
    features = set(specs) if args.all else set(args.features.split(","))
    if not features <= specs.keys():
        parser.error(f"unknown feature IDs: {sorted(features - specs.keys())}")
    evidence = read_table(ROOT / "scripts/parity/feature_evidence.tsv", EVIDENCE_FIELDS)

    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ") + f"_{os.getpid()}"
    output = args.out.resolve() if args.out else ROOT / ".evidence/runs" / stamp
    output.mkdir(parents=True, exist_ok=True)
    (output / "catalogue.json").write_text(json.dumps([specs[feature] for feature in sorted(specs)], indent=2) + "\n")
    (output / "selection.json").write_text(json.dumps({"features": sorted(features), "surfaces": list(surfaces)},
                                                      indent=2) + "\n")

    # ---------------------------------------------------------- behaviour anchors
    wanted, pending_anchors, owners = anchor_selection(evidence, features)
    anchor_results = {}
    if args.skip_anchors:
        anchor_results = {package: {"status": "skipped", "tests": {test_id: "skipped" for test_id in tests}}
                          for package, tests in wanted.items()}
    else:
        anchor_results = run_anchors(output, wanted, owners)
    (output / "anchors.json").write_text(json.dumps(anchor_results, indent=2, sort_keys=True) + "\n")

    # ----------------------------------------------------------------- captures
    runs, cells, capture_reports = {}, [], {}
    for surface in surfaces:
        carried = {feature: specs[feature][f"{surface}_page"] for feature in features if specs[feature][f"{surface}_page"]}
        if not carried:
            capture_reports[surface] = {"status": "no-carrier",
                                        "reason": "no selected business has a carrier page on this surface"}
            continue
        rows = capture_matrix(sorted(carried), carried, args.replicates)
        run, completed = run_capture(surface, rows, output)
        if run is None:
            capture_reports[surface] = {"status": "capture-failed", "returncode": completed.returncode,
                                        "reason": "the capture stack produced no run directory"}
            for feature in carried:
                for viewport in VIEWPORTS:
                    cells.append({"feature_id": feature, "surface": surface, "viewport": viewport,
                                  "status": "uncertified",
                                  "reason": "capture failed: the capture stack produced no run",
                                  "image": "", "sha256": "", "build_id": "", "bounds": [],
                                  "changed_pixels": 0, "noise_pixels": 0, "required_pixels": 0,
                                  "compared_pixels": 0, "anchor_note": ""})
            continue
        runs[surface] = run
        rows_by_scenario = {row["scenario"]: row for row in manifest_rows(run)}
        baselines = baseline_images(run, manifest_rows(run))
        capture_reports[surface] = {"status": "captured", "run": str(run.relative_to(ROOT)),
                                    "scenarios": len(rows_by_scenario),
                                    "failed_scenarios": sorted(name for name, row in rows_by_scenario.items()
                                                               if row["status"] != "ok")}
        for feature, page in sorted(carried.items()):
            for viewport in VIEWPORTS:
                cells.append(classify_cell(run, surface, feature, viewport, page, rows_by_scenario, baselines))

    # --------------------------------------------------------- pixel receipts
    return finalize(output, specs, features, surfaces, evidence, anchor_results, pending_anchors,
                    owners, args.replicates, runs, cells, capture_reports)


def capture_source_state(run):
    """The source fingerprint the captured pixels were taken from."""
    metadata = run / "capture-metadata.txt"
    if not metadata.is_file():
        return ""
    for line in metadata.read_text().splitlines():
        if line.startswith("source_fingerprint="):
            return line.split("=", 1)[1].strip()
    return ""


def finalize(output, specs, features, surfaces, evidence, anchor_results, pending_anchors, owners,
             replicates, runs, cells, capture_reports):
    """Receipts, per-cell report and registry closure of one acceptance scope."""
    wanted_cells = {(cell["feature_id"], cell["surface"], cell["viewport"]) for cell in cells
                    if cell["status"] == "certified"}
    receipts_path = output / "visual_receipts.json"
    receipt_failures = {}
    receipts = {"schema_version": 4, "receipts": []}
    if runs:
        receipts, receipt_failures = build_receipts_tolerant(ROOT, runs, wanted_cells)
    receipts_path.write_text(json.dumps(receipts, indent=2) + "\n")
    products = {}
    for receipt in receipts["receipts"]:
        products.setdefault(receipt["surface"], receipt["build_id"])
    (output / "product-builds.json").write_text(json.dumps(products, indent=2) + "\n")
    verified, verify_failures = ({}, {})
    if receipts["receipts"]:
        verified, verify_failures = verified_receipts_tolerant(receipts_path, ROOT, products or None)
    for cell in cells:
        key = f"{cell['feature_id']}/{cell['surface']}/{cell['viewport']}"
        if key in verify_failures:
            cell["status"] = "uncertified"
            cell["reason"] = f"pixel receipt rejected: {verify_failures[key]}"
    for receipt in receipts["receipts"]:
        key = (receipt["feature_id"], receipt["surface"], receipt["viewport"])
        if key not in verified:
            continue
        for cell in cells:
            if (cell["feature_id"], cell["surface"], cell["viewport"]) == key:
                cell["image"] = receipt["image"]
                cell["sha256"] = receipt["sha256"]
                cell["build_id"] = receipt["build_id"]

    # ------------------------------------------------ behaviour anchors per cell
    for cell in cells:
        reason = anchor_reason(evidence, anchor_results, cell["feature_id"], cell["surface"])
        if reason:
            cell["anchor_note"] = reason
            if cell["status"] == "certified":
                cell["status"] = "uncertified"
                cell["reason"] = reason

    # ------------------------------------------------------------------ report
    pairs = {}
    for cell in cells:
        key = f"{cell['feature_id']}/{cell['surface']}"
        pair = pairs.setdefault(key, {"feature_id": cell["feature_id"], "surface": cell["surface"],
                                       "status": "certified", "viewports": {}})
        pair["viewports"][cell["viewport"]] = cell["status"]
        if cell["status"] != "certified":
            pair["status"] = "uncertified"
            pair["reason"] = cell["reason"]
    uncertified = [cell for cell in cells if cell["status"] != "certified"]
    certified = [cell for cell in cells if cell["status"] == "certified"]
    certified_pairs = sorted(key for key, pair in pairs.items() if pair["status"] == "certified")
    out_of_scope = {}
    for surface in SURFACE_PACKAGES:
        if surface in surfaces:
            continue
        out_of_scope[surface] = surface_registry_state(evidence, features, surface, specs)
    # The pixels are bound to the archived product build; the behaviour
    # anchors run against the working tree.  When the tree moved in between,
    # the report has to say so instead of pretending both describe one state.
    coherence = []
    tree_fingerprint = source_fingerprint(ROOT)
    for surface, run in sorted(runs.items()):
        captured = capture_source_state(run)
        if captured and captured != tree_fingerprint:
            coherence.append({
                "surface": surface, "capture_source_fingerprint": captured,
                "tree_source_fingerprint": tree_fingerprint,
                "status": "tree_moved_after_the_capture",
                "detail": "the behaviour anchors of this run prove the current tree, "
                          "the captured pixels prove the archived product build",
            })
    report = {
        "schema_version": 1,
        "generated_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "features": sorted(features),
        "surfaces": list(surfaces),
        "replicates": replicates,
        "cells": cells,
        "pairs": pairs,
        "certified_cells": len(certified),
        "uncertified_cells": len(uncertified),
        "certified_pairs": certified_pairs,
        "uncertified_pairs": sorted(key for key, pair in pairs.items() if pair["status"] != "certified"),
        "not_certifiable_surfaces": out_of_scope,
        "source_coherence": coherence,
        "capture_runs": capture_reports,
        "receipt_failures": receipt_failures,
        "receipt_verification_failures": verify_failures,
        "anchor_results": {package: result["status"] for package, result in anchor_results.items()},
        "pending_anchor_cells": {f"{feature}/{surface}": reasons for (feature, surface), reasons in pending_anchors.items()},
    }
    (output / "certification.json").write_text(json.dumps(report, indent=2) + "\n")
    with (output / "certification.tsv").open("w") as file:
        writer = csv.writer(file, delimiter="\t", lineterminator="\n")
        writer.writerow(["feature_id", "surface", "viewport", "status", "reason", "anchor_note", "image",
                         "sha256", "build_id", "bounds", "changed_pixels", "noise_pixels", "required_pixels"])
        for cell in cells:
            writer.writerow([cell["feature_id"], cell["surface"], cell["viewport"], cell["status"],
                             cell["reason"] or "-", cell["anchor_note"] or "-", cell["image"] or "-",
                             cell["sha256"] or "-", cell["build_id"] or "-",
                             ",".join(str(value) for value in cell["bounds"]) or "-",
                             cell["changed_pixels"], cell["noise_pixels"], cell["required_pixels"]])

    # ------------------------------------------------------- registry closure
    closed_features = sorted({cell["feature_id"] for cell in cells
                              if cell["status"] == "certified"
                              and all(pairs[f"{cell['feature_id']}/{surface}"]["status"] == "certified"
                                      for surface in surfaces
                                      if f"{cell['feature_id']}/{surface}" in pairs)})
    resolver = [sys.executable, "scripts/parity/resolve_surface_evidence.py",
                "--catalogue-json", str(output / "catalogue.json"),
                "--receipts", str(receipts_path), "--product-builds", str(output / "product-builds.json"),
                "--accepted-surfaces", ",".join(surfaces), "--require-surfaces", ",".join(surfaces),
                "--require-features", ",".join(closed_features),
                "--report-json", str(output / "acceptance.json")]
    discovery = output / "discovery.json"
    merged = {"rust-suites": {}}
    covered = []
    for package in sorted(owners):
        path = output / "discovery" / f"{package}.json"
        if path.is_file():
            merged["rust-suites"].update(json.loads(path.read_text()).get("rust-suites", {}))
            covered.append(package)
    # The packages the discovery could not enumerate are reported as
    # unverified anchors, not as dangling ones: their build may be broken by
    # another work stream right now.
    merged["covered_packages"] = covered
    merged["undiscovered_packages"] = sorted(set(owners) - set(covered))
    discovery.write_text(json.dumps(merged))
    resolver[2:2] = ["--discovery-json", str(discovery)]
    resolver_status = "skipped"
    if closed_features:
        completed = subprocess.run(resolver, cwd=ROOT, capture_output=True, text=True)
        (output / "acceptance.log").write_text(completed.stdout + completed.stderr)
        resolver_status = "pass" if completed.returncode == 0 else "fail"

    print(f"certified {len(certified)}/{len(cells)} cells "
          f"({len(certified_pairs)}/{len(pairs)} business/surface pairs) on {','.join(surfaces)}")
    for surface, state in out_of_scope.items():
        print(f"{state['status']}: {surface}: {state['reason']}")
    for entry in coherence:
        print(f"source coherence: {entry['surface']}: {entry['status']} - {entry['detail']}")
    for cell in uncertified:
        print(f"  uncertified {cell['feature_id']}/{cell['surface']}/{cell['viewport']}: {cell['reason']}",
              file=sys.stderr)
    print(f"per-cell report: {output / 'certification.json'}")
    print(f"per-cell table:  {output / 'certification.tsv'}")
    if uncertified:
        print(f"certification incomplete: {len(uncertified)} cell(s) have no proven dual-viewport pixels",
              file=sys.stderr)
        return 1
    if resolver_status == "fail":
        print(f"registry closure failed; see {output / 'acceptance.log'}", file=sys.stderr)
        return 1
    print(f"accepted {len(features)} businesses on {','.join(surfaces)}: {output}")
    return 0


def reclassify(previous, output=None, surfaces=None, rerun_anchors=False):
    """Re-derive the certification report from the pixels of an earlier run.

    The captured pixels, their markers and the behaviour anchors are facts of
    the earlier run; only the classification rules are re-applied.  This keeps
    a verdict reproducible without paying for another capture.  A surface that
    the earlier run never captured is captured now.
    """
    report = json.loads((previous / "certification.json").read_text())
    selection = json.loads((previous / "selection.json").read_text())
    features = set(selection["features"])
    surfaces = tuple(filter(None, surfaces.split(","))) if surfaces else tuple(selection["surfaces"])
    specs = catalogue_specs()
    evidence = read_table(ROOT / "scripts/parity/feature_evidence.tsv", EVIDENCE_FIELDS)
    anchor_results = json.loads((previous / "anchors.json").read_text())
    wanted, pending_anchors, owners = anchor_selection(evidence, features)
    output = output.resolve() if output else previous
    output.mkdir(parents=True, exist_ok=True)
    (output / "catalogue.json").write_text(json.dumps([specs[feature] for feature in sorted(specs)], indent=2) + "\n")
    (output / "selection.json").write_text(json.dumps({"features": sorted(features), "surfaces": list(surfaces)},
                                                       indent=2) + "\n")
    if rerun_anchors:
        anchor_results = run_anchors(output, wanted, owners)
    (output / "anchors.json").write_text(json.dumps(anchor_results, indent=2, sort_keys=True) + "\n")
    # A package that gained anchors since the earlier run has to be discovered
    # again, otherwise its anchors would count as dangling.
    (output / "discovery").mkdir(exist_ok=True)
    for package in sorted(owners):
        if (output / "discovery" / f"{package}.json").is_file():
            continue
        try:
            payload, _ = discovery_index(package)
        except (subprocess.CalledProcessError, ValueError, OSError):
            continue
        (output / "discovery" / f"{package}.json").write_text(json.dumps(payload))
    runs, cells, capture_reports = {}, [], {}
    for surface in surfaces:
        state = report.get("capture_runs", {}).get(surface, {})
        relative = state.get("run")
        carried = {feature: specs[feature][f"{surface}_page"] for feature in features
                   if specs[feature][f"{surface}_page"]}
        if not relative:
            if not carried:
                capture_reports[surface] = {"status": "no-carrier",
                                            "reason": "no selected business has a carrier page on this surface"}
                continue
            # The earlier run never captured this surface: capture it now so
            # the scope is complete.
            rows = capture_matrix(sorted(carried), carried, report.get("replicates", 2))
            run, _ = run_capture(surface, rows, output)
            if run is None:
                capture_reports[surface] = {"status": "capture-failed",
                                            "reason": "the capture stack produced no run directory"}
                continue
            relative = str(run.relative_to(ROOT))
        run = ROOT / relative
        rows = manifest_rows(run)
        rows_by_scenario = {row["scenario"]: row for row in rows}
        baselines = baseline_images(run, rows)
        capture_reports[surface] = dict(state, run=relative)
        runs[surface] = run
        for feature, page in sorted(carried.items()):
            for viewport in VIEWPORTS:
                cells.append(classify_cell(run, surface, feature, viewport, page, rows_by_scenario, baselines))
    return finalize(output, specs, features, surfaces, evidence, anchor_results, pending_anchors,
                    owners, report.get("replicates", 2), runs, cells, capture_reports)


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        print(f"interaction acceptance failed: {error}", file=sys.stderr)
        sys.exit(1)
