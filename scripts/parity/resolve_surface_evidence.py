#!/usr/bin/env python3
"""Resolve declarations against the compiled Rust registry and exact nextest IDs.

No production source scanning. Discovery proves existence, not test success or
interaction depth. Pending cells remain gaps; --require-complete rejects them.
"""
from __future__ import annotations

import argparse
import csv
import json
import subprocess
import sys
from evidence_progress import progress
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SURFACES = {"iced": "infiltrator-iced", "bevy": "infiltrator-bevy-ui", "slint": "infiltrator-slint"}
MANIFEST_FIELDS = ("feature_id", "surface", "status", "reason", "standard_scenario", "compact_scenario")
EVIDENCE_FIELDS = ("feature_id", "surface", "level", "status", "test_id", "paths", "reason")


def read_table(path, fields):
    with Path(path).open(encoding="utf-8", newline="") as file:
        reader = csv.DictReader(file, delimiter="\t")
        if tuple(reader.fieldnames or ()) != fields:
            raise ValueError(f"{path}: expected columns {fields}")
        rows = list(reader)
    if not rows or any(None in row or any(value is None for value in row.values()) for row in rows):
        raise ValueError(f"{path}: empty or malformed table")
    return rows


def discovered_tests(payload):
    """Package + binary + full test name: identical names in other crates cannot satisfy an anchor."""
    suites = payload.get("rust-suites")
    if not isinstance(suites, dict) or not suites:
        raise ValueError("nextest discovery has no rust-suites")
    found = {}
    for binary_id, suite in suites.items():
        package = suite.get("package-name")
        cases = suite.get("testcases")
        if not package or not isinstance(cases, dict):
            raise ValueError(f"malformed nextest suite: {binary_id}")
        # Ignored tests cannot serve as delivered behavior evidence.
        for name, case in cases.items():
            if not case.get("ignored", False):
                found[f"{binary_id}::{name}"] = package
    if not found:
        raise ValueError("nextest discovery contains no runnable tests")
    return found


def discovery_coverage(payload):
    """The packages this discovery could enumerate, if it declares them."""
    covered = payload.get("covered_packages")
    return set(covered) if isinstance(covered, list) else None


def validate(manifest, evidence, catalogue=None, discovery=None, require_complete=False, receipts=None,
             require_features=(), require_surfaces=(), accepted_surfaces=None, judge_pixels=True,
             covered_packages=None):
    invalid, dangling, pending = [], [], []
    unverified = []
    out_of_scope_findings = []
    cells, anchors = {}, {}
    # An acceptance may cover a subset of the surfaces (a peer that is not
    # certifiable yet must not silently pass as closed).  The scope decides
    # which closed cells count for the acceptance; the global picture stays.
    scope = set(accepted_surfaces) if accepted_surfaces else set(SURFACES)
    # An anchor in a package the discovery could not enumerate is unverified,
    # not dangling: the test may exist in a build that does not compile now.
    # A discovery that does not declare its coverage judges every anchor.
    covered = set(covered_packages) if covered_packages is not None else None
    specs = {spec["id"]: spec for spec in catalogue} if catalogue is not None else None
    if catalogue is not None and (not specs or len(specs) != len(catalogue)):
        invalid.append("compiled registry is empty or contains duplicate identities")
    for row in manifest:
        key = (row["feature_id"], row["surface"])
        if key in cells:
            invalid.append(f"duplicate manifest cell: {key}")
        cells[key] = row
        if row["surface"] not in SURFACES or row["status"] not in {"pending", "implemented", "ready"}:
            invalid.append(f"invalid surface or status: {key}")
        if row["status"] == "pending":
            pending.append(list(key))
            if not row["reason"].strip() or row["reason"] == "-":
                invalid.append(f"pending cell needs a reason: {key}")
        if specs is not None and key[0] not in specs:
            invalid.append(f"unknown feature: {key[0]}")
        if row["standard_scenario"] != f"{key[0]}-standard" or row["compact_scenario"] != f"{key[0]}-compact":
            invalid.append(f"scenario identity differs from feature: {key}")
    ids = specs.keys() if specs is not None else {key[0] for key in cells}
    expected = {(feature, surface) for feature in ids for surface in SURFACES}
    for key in expected - cells.keys():
        invalid.append(f"missing peer manifest cell: {key}")
    for row in evidence:
        key = (row["feature_id"], row["surface"], row["level"])
        if key in anchors:
            invalid.append(f"duplicate evidence cell: {key}")
        anchors[key] = row
        if key[:2] not in cells or row["level"] not in {"contract", "scenario"}:
            invalid.append(f"unregistered evidence cell: {key}")
        if row["status"] == "pending":
            if row["test_id"] != "-" or row["paths"] != "-" or not row["reason"].strip() or row["reason"] == "-":
                invalid.append(f"pending evidence cannot carry an anchor: {key}")
            continue
        if row["status"] != "anchored" or row["test_id"] in {"", "-"}:
            invalid.append(f"invalid evidence status or empty anchor: {key}")
            continue
        tests = row["test_id"].split(";")
        if len(set(tests)) != len(tests) or any(not test.strip() for test in tests):
            invalid.append(f"duplicate or empty test ID: {key}")
        if discovery is not None:
            for test in tests:
                owner = discovery.get(test)
                if owner is None:
                    package = test.split("::", 1)[0]
                    if covered is not None and package not in covered:
                        unverified.append(f"{key}: {test} (package {package} was not discovered)")
                    else:
                        dangling.append(f"{key}: {test}")
                elif owner != SURFACES.get(row["surface"]) and not (row["level"] == "contract" and owner in {"infiltrator-contract", "lurker-core-contract", "infiltrator-domain", "infiltrator-application", "infiltrator-core", "infiltrator-presentation", "lurker-ui-presentation", "mihomo-driver", "singbox-driver"}):
                    invalid.append(f"wrong package for {key}: {owner}")
        if specs is not None and row["level"] == "scenario" and key[0] in specs:
            if set(row["paths"].split(",")) != set(specs[key[0]]["required_paths"]):
                invalid.append(f"incomplete or unknown interaction paths: {key}")
        if specs is not None and row["surface"] == "slint" and row["level"] == "scenario" \
                and row["status"] == "anchored" and not specs.get(key[0], {}).get("slint_page"):
            invalid.append(f"slint scenario anchor without a carrier page: {key}")
    expected_anchors = {(feature, surface, level) for feature, surface in expected for level in ("contract", "scenario")}
    for key in expected_anchors - anchors.keys():
        invalid.append(f"missing evidence cell: {key}")
    for key, row in cells.items():
        if row["status"] != "ready":
            continue
        if key[1] not in scope:
            # A certified cell of a surface outside this acceptance scope is
            # reported, not judged here: this run cannot hold its receipts.
            for level in ("contract", "scenario"):
                if anchors.get((*key, level), {}).get("status") != "anchored":
                    out_of_scope_findings.append(f"ready without {level} evidence: {key}")
            if judge_pixels:
                for viewport in ("standard", "compact"):
                    if receipts is None or (*key, viewport) not in receipts:
                        out_of_scope_findings.append(f"ready without verified {viewport} pixels: {key}")
            continue
        for level in ("contract", "scenario"):
            if anchors.get((*key, level), {}).get("status") != "anchored":
                invalid.append(f"ready without {level} evidence: {key}")
        if not judge_pixels:
            # A structure-only run carries no pixel receipts, so the pixel
            # layer cannot be judged here; the acceptance run is its
            # authority and still rejects a cell without verified pixels.
            continue
        for viewport in ("standard", "compact"):
            if receipts is None or (*key, viewport) not in receipts:
                invalid.append(f"ready without verified {viewport} pixels: {key}")
    aligned = {key for key in cells if discovery is not None and receipts is not None
               and all(anchors.get((*key, level), {}).get("status") == "anchored" for level in ("contract", "scenario"))
               and all((*key, viewport) in receipts for viewport in ("standard", "compact"))}
    unclosed = set(cells) - aligned
    if require_complete and unclosed:
        invalid.append(f"complete parity required: {len(unclosed)} unclosed surface cells")
    scope_aligned = {key for key in aligned if key[1] in scope}
    required_surfaces = tuple(require_surfaces) if require_surfaces else tuple(SURFACES)
    for feature in require_features:
        if specs is None or feature not in specs:
            invalid.append(f"unknown required feature: {feature}")
        for surface in required_surfaces:
            if (feature, surface) not in scope_aligned:
                invalid.append(f"required feature lacks L1/L2/L3 closure: {(feature, surface)}")
    return {"status": "fail" if invalid or dangling else "pass", "invalid": sorted(invalid),
            "dangling": sorted(dangling), "pending": sorted(pending),
            "unverified": sorted(unverified),
            "out_of_scope_findings": sorted(out_of_scope_findings),
            "cells": len(cells), "anchored": sum(row["status"] == "anchored" for row in evidence),
            "aligned": sorted([list(key) for key in aligned]), "unclosed": len(unclosed),
            "accepted_surfaces": sorted(scope),
            "scope_aligned": sorted([list(key) for key in scope_aligned]),
            "complete": not unclosed and not invalid and not dangling,
            "discovery_verified": discovery is not None, "registry_verified": specs is not None}


def command_json(command):
    result = subprocess.run(command, cwd=ROOT, text=True, stdout=subprocess.PIPE, check=True)
    return json.loads(result.stdout)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=ROOT / "scripts/parity/cross_surface_manifest.tsv")
    parser.add_argument("--evidence", type=Path, default=ROOT / "scripts/parity/feature_evidence.tsv")
    parser.add_argument("--structure-only", action="store_true", help="fast schema checks; does not verify the registry or discover tests")
    parser.add_argument("--discovery-json", type=Path, help="use a saved nextest JSON from this build")
    parser.add_argument("--catalogue-json", type=Path, help="use a saved output of the compiled parity_catalogue example")
    parser.add_argument("--receipts", type=Path)
    parser.add_argument("--product-builds", type=Path, help="explicit iced/bevy build IDs selected for this acceptance")
    parser.add_argument("--require-complete", action="store_true")
    parser.add_argument("--require-features", default="", help="comma-separated features requiring both peers and both viewports")
    parser.add_argument("--require-surfaces", default="",
                        help="comma-separated surfaces a required feature must close on (default: every surface)")
    parser.add_argument("--accepted-surfaces", default="",
                        help="comma-separated surfaces inside this acceptance scope (default: every surface)")
    parser.add_argument("--report-json", type=Path)
    args = parser.parse_args()
    accepted_surfaces = tuple(filter(None, args.accepted_surfaces.split(",")))
    require_surfaces = tuple(filter(None, args.require_surfaces.split(",")))
    unknown = (set(accepted_surfaces) | set(require_surfaces)) - set(SURFACES)
    if unknown:
        parser.error(f"unknown surface: {sorted(unknown)}")
    if args.require_complete and args.product_builds is None:
        parser.error("release acceptance requires explicit --product-builds; baseline coverage is a separate result")
    if args.product_builds and not args.receipts:
        parser.error("product build selection requires visual receipts")
    if args.structure_only and (args.require_complete or args.require_features or args.discovery_json or args.catalogue_json or args.receipts or args.product_builds):
        parser.error("structure-only cannot be used for evidence acceptance")
    try:
        catalogue, discovery, receipts = None, None, None
        covered_packages = None
        if not args.structure_only:
            catalogue = json.loads(args.catalogue_json.read_text()) if args.catalogue_json else command_json([
                "cargo", "run", "--quiet", "-p", "infiltrator-contract", "--example", "parity_catalogue"])
            payload = json.loads(args.discovery_json.read_text()) if args.discovery_json else command_json([
                "cargo", "nextest", "list", "--workspace", "--features", "infiltrator-iced/capture-tools,infiltrator-bevy-ui/capture-tools", "--build-jobs", "4", "--message-format", "json"])
            discovery = discovered_tests(payload)
            covered_packages = discovery_coverage(payload)
            if args.receipts:
                from visual_receipts import verified_receipts
                selected_builds = json.loads(args.product_builds.read_text()) if args.product_builds else None
                if selected_builds is not None:
                    scope = set(accepted_surfaces) if accepted_surfaces else set(SURFACES)
                    if not set(selected_builds) <= set(SURFACES) or not scope <= set(selected_builds):
                        raise ValueError(f"product selection must bind every accepted surface ({sorted(scope)}) to concrete build IDs")
                    if any(not isinstance(value, str) or len(value) != 64 or any(char not in "0123456789abcdef" for char in value)
                           for value in selected_builds.values()):
                        raise ValueError("product selection must bind every accepted surface to concrete build IDs")
                receipts = verified_receipts(args.receipts, ROOT, selected_builds)
        manifest = read_table(args.manifest, MANIFEST_FIELDS)
        evidence = read_table(args.evidence, EVIDENCE_FIELDS)
        report = validate(manifest, evidence,
                          catalogue, discovery, args.require_complete, receipts,
                          tuple(filter(None, args.require_features.split(","))),
                          require_surfaces, accepted_surfaces,
                          judge_pixels=receipts is not None,
                          covered_packages=covered_packages)
        report["acceptance_scope"] = "selected-products" if args.product_builds else "baseline-coverage"
        report["release_complete"] = bool(args.product_builds and report["complete"])
        if catalogue is not None:
            report["progress"] = progress(manifest, evidence, ROOT / "target/parity",
                                          {spec["id"] for spec in catalogue}, report)
        if args.report_json:
            args.report_json.parent.mkdir(parents=True, exist_ok=True)
            args.report_json.write_text(json.dumps(report, indent=2) + "\n")
        print(f"evidence: status={report['status']} registry={report['registry_verified']} "
              f"discovery={report['discovery_verified']} anchors={report['anchored']} "
              f"cells={report['cells']} aligned={len(report['aligned'])} unclosed={report['unclosed']} "
              f"scope={','.join(report['accepted_surfaces'])} scope_aligned={len(report['scope_aligned'])} "
              f"complete={report['complete']}")
        for finding in report["invalid"] + report["dangling"]:
            print(f"  {finding}", file=sys.stderr)
        for finding in report.get("unverified", []):
            print(f"  unverified anchor: {finding}", file=sys.stderr)
        for finding in report.get("out_of_scope_findings", []):
            print(f"  out of acceptance scope: {finding}", file=sys.stderr)
        return 0 if report["status"] == "pass" else 1
    except (ValueError, OSError, KeyError, TypeError, subprocess.CalledProcessError) as error:
        print(f"evidence discovery/IO error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
