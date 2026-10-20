#!/usr/bin/env python3
"""Run behavior assertions, activate real controls, capture both peers and accept fresh pixels."""
from __future__ import annotations

import argparse
import csv
import json
import os
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

from resolve_surface_evidence import ROOT, command_json, read_table, EVIDENCE_FIELDS
from visual_receipts import build_receipts, verified_receipts


def capture_failures(run):
    """Business/surface pairs whose capture did not produce a usable scenario."""
    manifest = Path(run) / "capture-manifest.tsv"
    if not manifest.exists():
        return {f"{run.name}/unknown"}
    failures = set()
    with manifest.open(encoding="utf-8", newline="") as file:
        for row in csv.DictReader(file, delimiter="\t"):
            if row.get("status") == "ok":
                continue
            scenario = row.get("scenario", "")
            feature = scenario
            for suffix in ("-standard", "-compact"):
                if feature.endswith(suffix):
                    feature = feature[: -len(suffix)]
                    break
            failures.add(feature)
    return failures


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("features", nargs="?", help="comma-separated implemented interaction tokens")
    parser.add_argument("--all", action="store_true", help="accept the full typed feature registry; incomplete coverage fails")
    args = parser.parse_args()
    if bool(args.features) == args.all:
        parser.error("select explicit features or --all")
    catalogue = command_json(["cargo", "run", "--quiet", "-p", "infiltrator-contract", "--example", "parity_catalogue", "--jobs", "4"])
    specs = {spec["id"]: spec for spec in catalogue}
    features = set(specs) if args.all else set(args.features.split(","))
    if not features <= specs.keys():
        parser.error(f"unknown feature IDs: {sorted(features - specs.keys())}")
    evidence = read_table(ROOT / "scripts/parity/feature_evidence.tsv", EVIDENCE_FIELDS)
    selected = [row for row in evidence if row["feature_id"] in features]
    packages = {"iced": "infiltrator-iced", "bevy": "infiltrator-bevy-ui", "slint": "infiltrator-slint"}
    # A surface without a page carrier cannot present the business, so its cells stay
    # recorded gaps instead of blocking acceptance on the surfaces that do carry it.
    carriers = {feature: [surface for surface in packages if specs[feature][f"{surface}_page"]]
                for feature in features}
    gaps = [f"{row['feature_id']}/{row['surface']}/{row['level']}" for row in selected
            if row["status"] != "anchored" and row["surface"] in carriers[row["feature_id"]]]
    if gaps:
        print(f"acceptance blocked: {len(gaps)} L1/L2 cells lack behavior anchors:\n" + "\n".join(gaps), file=sys.stderr)
        return 1
    uncarried = sorted({f"{feature}/{surface}" for feature in features for surface in packages
                        if surface not in carriers[feature]})
    if uncarried:
        print(f"note: {len(uncarried)} business/surface pairs have no page carrier yet and stay recorded gaps:\n"
              + "\n".join(uncarried), file=sys.stderr)
    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ") + f"_{os.getpid()}"
    output = ROOT / ".evidence/runs" / stamp
    output.mkdir(parents=True)
    discovery = command_json(["cargo", "nextest", "list", "--workspace", "--features",
                               "infiltrator-iced/capture-tools,infiltrator-bevy-ui/capture-tools",
                               "--build-jobs", "4", "--message-format", "json"])
    discovery_path = output / "discovery.json"
    discovery_path.write_text(json.dumps(discovery))
    catalogue_path = output / "catalogue.json"
    catalogue_path.write_text(json.dumps(catalogue))
    # Resolve all declarations from this exact build, including unselected ones.
    discovery_flags = ["--discovery-json", str(discovery_path), "--catalogue-json", str(catalogue_path)]
    subprocess.run([sys.executable, "scripts/parity/resolve_surface_evidence.py", *discovery_flags,
                    "--report-json", str(output / "evidence.json")], cwd=ROOT, check=True)
    # Execute every selected L1/L2 anchor, including shared-crate tests, in the same workspace feature graph.
    metadata = {f"{binary_id}::{name}": (suite["package-name"], suite["binary-name"], name)
                for binary_id, suite in discovery["rust-suites"].items() for name in suite["testcases"]}
    anchors = {test for row in selected if row["status"] == "anchored"
               for test in row["test_id"].split(";") if test and test != "-"}
    filters = []
    for anchor in sorted(anchors):
        package, binary_name, name = metadata[anchor]
        filters.append(f"(package(={package}) & binary(={binary_name}) & test(={name}))")
    filters.append("(package(=infiltrator-contract) & test(parity::parity_test))")
    subprocess.run(["cargo", "nextest", "run", "--workspace", "--features",
                    "infiltrator-iced/capture-tools,infiltrator-bevy-ui/capture-tools",
                    "--build-jobs", "4", "--test-threads", "4",
                    "-E", " | ".join(filters)], cwd=ROOT, check=True)
    (output / "behavior_tests.json").write_text(json.dumps({"test_ids": sorted(anchors), "status": "passed"}, indent=2) + "\n")
    runs = {}
    uncertified = set()
    for surface in packages:
        matrix = output / f"{surface}-scenarios.tsv"
        with matrix.open("w") as file:
            writer = csv.writer(file, delimiter="\t", lineterminator="\n")
            writer.writerow(["name", "page", "skin", "window_size"])
            for feature in sorted(features):
                page = specs[feature][f"{surface}_page"]
                if not page:
                    # The surface has no carrier for this business yet; a capture would
                    # prove a route that the product does not implement.
                    continue
                for viewport, size in (("standard", "1180x780"), ("compact", "720x480")):
                    writer.writerow([f"{feature}-{viewport}", page, "dark", size])
        if not any(surface in carriers[feature] for feature in features):
            # No selected business has a carrier on this surface, so there is
            # nothing to capture; the gap stays recorded in the registry.
            continue
        env = dict(os.environ, INFILTRATOR_CAPTURE_MATRIX=str(matrix), INFILTRATOR_CAPTURE_OUT_DIR=str(output / surface),
                   INFILTRATOR_LANG="en-US", CARGO_BUILD_JOBS="4", INFILTRATOR_CAPTURE_SCENARIOS="")
        evidence_root = ROOT / f".evidence/captures/{surface}"
        previous = set(evidence_root.iterdir()) if evidence_root.exists() else set()
        # A batch may contain businesses whose surface interaction is not
        # implemented yet. Collect every scenario result instead of aborting on
        # the first failure, so the report names every uncertified business.
        capture = subprocess.run(["bash", f"scripts/capture-{surface}-matrix.sh"], cwd=ROOT, env=env)
        fresh = set(evidence_root.iterdir()) - previous
        if len(fresh) != 1:
            raise ValueError(f"expected exactly one fresh {surface} capture run, got {len(fresh)}")
        runs[surface] = fresh.pop()
        failed = capture_failures(runs[surface])
        if failed:
            uncertified.update(f"{feature}/{surface}" for feature in failed)
    receipts = output / "visual_receipts.json"
    receipts.write_text(json.dumps(build_receipts(ROOT, runs), indent=2) + "\n")
    selected_products = {}
    for row in json.loads(receipts.read_text())["receipts"]:
        prior = selected_products.setdefault(row["surface"], row["build_id"])
        if prior != row["build_id"]:
            raise ValueError("one acceptance cannot silently mix product builds")
    product_selection = output / "product-builds.json"
    product_selection.write_text(json.dumps(selected_products, indent=2) + "\n")
    verified = verified_receipts(receipts, ROOT, selected_products)
    expected = {(feature, surface, viewport) for feature in features for surface in carriers[feature]
                for viewport in ("standard", "compact")}
    missing = expected - verified
    if missing:
        uncertified.update(f"{feature}/{surface}" for feature, surface, _ in missing)
    report = output / "certification.json"
    report.write_text(json.dumps({
        "features": sorted(features),
        "certified": sorted(f"{feature}/{surface}" for feature, surface, _ in verified),
        "uncertified": sorted(uncertified),
    }, indent=2) + "\n")
    if uncertified:
        print(
            f"certification incomplete: {len(uncertified)} business/surface pair(s) have no verified "
            f"dual-viewport pixels:\n" + "\n".join(sorted(uncertified)),
            file=sys.stderr,
        )
        print(f"per-business report: {report}", file=sys.stderr)
        return 1
    command = [sys.executable, "scripts/parity/resolve_surface_evidence.py", *discovery_flags, "--receipts", str(receipts),
               "--require-features", ",".join(sorted(features)), "--product-builds", str(product_selection), "--report-json", str(output / "acceptance.json")]
    if args.all:
        command.append("--require-complete")
    subprocess.run(command, cwd=ROOT, check=True)
    print(f"accepted {len(features)} interactions on every carried surface and both viewports: {output}")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        print(f"interaction acceptance failed: {error}", file=sys.stderr)
        sys.exit(1)
