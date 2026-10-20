"""Report historical accepted baselines separately from the selected release candidate."""
from __future__ import annotations
import json
from pathlib import Path

SURFACES = ("iced", "bevy", "slint")


def historical_coverage(history, registered_features):
    accepted, references = set(), {}
    roots = [Path(history), Path(history).parent.parent / ".evidence/runs"]
    reports = {path for directory in roots for path in directory.rglob("acceptance.json")}
    for path in sorted(reports):
        try:
            report = json.loads(path.read_text())
        except (OSError, ValueError):
            continue
        if report.get("status") != "pass" or not report.get("registry_verified") or not report.get("discovery_verified"):
            continue
        for cell in report.get("aligned", []):
            if not isinstance(cell, list) or len(cell) != 2:
                continue
            feature, surface = cell
            if feature not in registered_features or surface not in SURFACES:
                continue
            key = (feature, surface)
            accepted.add(key)
            references.setdefault(f"{feature}/{surface}", []).append(str(path))
    paired = [feature for feature in sorted(registered_features)
              if all((feature, surface) in accepted for surface in SURFACES)]
    return {"scope": "historical acceptance reports; not current artifact revalidation",
            "accepted_surface_cells": len(accepted), "accepted_peer_scenarios": len(paired),
            "peer_scenarios": paired, "report_references": references}


def progress(manifest, evidence, history, registered_features, selected_report):
    features = set(registered_features)
    implemented = {(row["feature_id"], row["surface"]) for row in manifest
                   if row["status"] in {"implemented", "ready"} and row["feature_id"] in features}
    historical = historical_coverage(history, features)
    return {"targets": {"scenarios": len(features), "surface_cells": len(features) * len(SURFACES),
                        "contract_cells": len(features) * len(SURFACES), "scenario_cells": len(features) * len(SURFACES),
                        "visual_receipts": len(features) * len(SURFACES) * 2},
            "implemented_peer_scenarios": sum(all((feature, surface) in implemented for surface in SURFACES)
                                              for feature in features),
            "test_anchors": {level: sum(row["level"] == level and row["status"] == "anchored" for row in evidence)
                             for level in ("contract", "scenario")},
            "historical_baselines": historical,
            "selected_candidate": {"explicitly_selected": selected_report["acceptance_scope"] == "selected-products",
                                   "aligned_surface_cells": len(selected_report["aligned"]) if selected_report["acceptance_scope"] == "selected-products" else None,
                                   "release_complete": selected_report["release_complete"]}}
