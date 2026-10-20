#!/usr/bin/env python3
"""Summarise one acceptance run: the certified matrix and the missing capabilities.

The verdict comes from the acceptance run alone.  This helper only *names*
what an uncertified cell still needs, by reading the surfaces' own activator
declarations; that annotation never changes a certified or uncertified
status.
"""
from __future__ import annotations

import argparse
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
# Where a surface declares that it really activates a business.
ACTIVATOR_SOURCES = {
    "iced": ("crates/infiltrator-iced/src/capture_geometry.rs",
             "crates/infiltrator-iced/src/demo/interaction.rs"),
    "bevy": ("crates/infiltrator-bevy-ui/src/interaction_capture.rs",),
}


def declared_activators(surface):
    """The businesses the surface itself declares a dedicated interaction for."""
    found = set()
    for relative in ACTIVATOR_SOURCES.get(surface, ()):
        path = ROOT / relative
        if not path.is_file():
            continue
        text = path.read_text()
        for match in re.finditer(r"FeatureId::(\w+)", text):
            found.add(match.group(1))
    return found


def feature_variant(feature_id):
    """`proxies-search-highlight` -> `ProxiesSearchHighlight`."""
    return "".join(part.capitalize() for part in feature_id.split("-"))


def missing_capability(cell, activators):
    """What the surface still has to implement for this cell."""
    reason = cell.get("reason", "")
    surface = cell["surface"]
    variant = feature_variant(cell["feature_id"])
    if reason.startswith("no behaviour anchor"):
        return ("the registry has no proven behaviour anchor for this business on this surface; "
                "the surface needs a scenario test that drives the real control")
    if reason.startswith("behaviour anchor"):
        return ("the declared behaviour anchor does not pass in this build; "
                "the surface's scenario test has to drive the real interaction")
    if reason.startswith("surface panic"):
        return (f"the surface crashed while activating the business: {reason}; "
                "the interaction capture must not panic when the capability provider is unavailable")
    if reason.startswith("capture failed (environment)"):
        return (f"the private capture stack could not produce the pixels: {reason}; "
                "this is an environment fault, not a product verdict")
    if reason.startswith("capture failed"):
        return (f"the private capture stack could not produce the pixels: {reason}; "
                "this is an environment or capture-stack fault, not a product verdict")
    if reason.startswith("missing interaction bounds") or reason.startswith("wrong page") \
            or reason.startswith("wrong viewport") or reason.startswith("route-only"):
        return (f"the surface published no usable activation for this business: {reason}; "
                "the surface has to activate the business and report its native bounds")
    if reason.startswith("no demo interaction"):
        if variant in activators:
            return (f"{reason}; the surface declares an activator for this business but the "
                    "activated pixels are indistinguishable from the route-only render")
        return (f"{reason}; the surface has no dedicated activator for this business and falls back "
                f"to the page-level observation, so {cell['feature_id']} is never really exercised")
    return reason


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("run", type=Path, help="the acceptance run directory")
    parser.add_argument("--markdown", action="store_true", help="print a markdown table")
    args = parser.parse_args()
    report = json.loads((args.run / "certification.json").read_text())
    activators = {surface: declared_activators(surface) for surface in report["surfaces"]}

    pairs = report["pairs"]
    certified = [pair for pair in pairs.values() if pair["status"] == "certified"]
    uncertified = [pair for pair in pairs.values() if pair["status"] != "certified"]
    cells = {(cell["feature_id"], cell["surface"], cell["viewport"]): cell for cell in report["cells"]}

    print(f"run: {args.run}")
    print(f"certified cells {report['certified_cells']}/{len(report['cells'])}; "
          f"certified business/surface pairs {len(certified)}/{len(pairs)}")
    for surface, state in report.get("not_certifiable_surfaces", {}).items():
        print(f"not certifiable: {surface}: {state['reason']}")

    print("\n## certified matrix (receipt identities)")
    for key in report["certified_pairs"]:
        feature, surface = key.split("/")
        for viewport in ("standard", "compact"):
            cell = cells[(feature, surface, viewport)]
            print(f"{key}\t{viewport}\t{cell['sha256']}\t{cell['build_id']}\t{cell['image']}")
    if args.markdown:
        print("\n| business | surface | standard | compact |")
        print("| --- | --- | --- | --- |")
        for key in report["certified_pairs"]:
            feature, surface = key.split("/")
            standard = cells[(feature, surface, "standard")]
            compact = cells[(feature, surface, "compact")]
            print(f"| {feature} | {surface} | `{standard['sha256'][:16]}…` | `{compact['sha256'][:16]}…` |")

    print("\n## not-certified matrix (missing capability)")
    for key in report["uncertified_pairs"]:
        feature, surface = key.split("/")
        cell = cells.get((feature, surface, "standard"))
        reason = cell.get("reason", "") if cell else ""
        print(f"{key}\t{reason}")
        print(f"\t{missing_capability(cell, activators.get(surface, set())) if cell else reason}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
