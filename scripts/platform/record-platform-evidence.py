#!/usr/bin/env python3
"""Record a real platform run into the platform evidence ledger.

The install smoke (`packaging/linux/smoke-install.sh`) writes a run evidence
file. This script turns such a run into ledger facts: the artifact identity
becomes an `artifact` evidence entry, the run identity becomes a `run` entry,
and the leg status is *derived* from the evidence, never asserted.

A run that did not prove a leg cannot promote it: only a `proven` leg of the
run evidence is recorded, and the ledger status is recomputed from the
recorded coverage. The generated matrix section is rewritten afterwards, so
the document always follows the ledger.

Usage:
    python3 scripts/platform/record-platform-evidence.py --platform linux \\
        --run target/platform-evidence/linux/<run-id>/evidence.json

Exit codes: 0 the ledger was updated, 1 the run evidence is invalid or
contradicts the ledger, 2 the usage is wrong.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from evidence_model import (  # noqa: E402  (the model lives next to this script)
    EVIDENCE_DIR,
    KIND_ARTIFACT,
    KIND_RUN,
    LEG_BUILD,
    LEG_INSTALL,
    LEG_RUN,
    ROOT,
    STATUS_PROVEN,
    STATUS_UNSUPPORTED,
    Evidence,
    Leg,
    PlatformLedger,
    parse_ledger,
)

RUN_STATUSES = ("proven", "failed", "unsupported")


def run_evidence(path: Path) -> dict:
    """Read and check the run evidence file."""
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError(f"{path}: the run evidence must be a JSON object")
    if payload.get("schema_version") != 1:
        raise ValueError(f"{path}: unknown run evidence schema")
    if not payload.get("run_id"):
        raise ValueError(f"{path}: the run evidence carries no run id")
    artifacts = payload.get("artifacts")
    if not isinstance(artifacts, list) or not artifacts:
        raise ValueError(f"{path}: the run evidence carries no artifact")
    for artifact in artifacts:
        for leg in ("install", "run", "remove"):
            status = artifact.get("legs", {}).get(leg, {}).get("status")
            if status not in RUN_STATUSES:
                raise ValueError(f"{path}: the {leg} leg has the unknown status {status!r}")
        if not artifact.get("sha256"):
            raise ValueError(f"{path}: an artifact carries no digest")
        location = artifact.get("path", "")
        candidate = Path(location)
        if not location or candidate.is_absolute() or ".." in candidate.parts:
            raise ValueError(
                f"{path}: the artifact {location!r} is not a repository relative release artifact")
    return payload


def product_of(payload: dict) -> str:
    """The product the run belongs to."""
    product = payload.get("product")
    if not product:
        raise ValueError("the run evidence names no product")
    return product


def recorded_legs(ledger: PlatformLedger, payload: dict) -> list[Leg]:
    """The ledger legs after the run has been recorded."""
    product = product_of(payload)
    run_id = payload["run_id"]
    artifacts = payload["artifacts"]
    origin = f"git {payload.get('git_head', 'unknown')}, worktree {payload.get('worktree', 'unknown')}"

    build_entries = tuple(
        Evidence(
            kind=KIND_ARTIFACT,
            product=product,
            identity=f"sha256:{artifact['sha256']}",
            detail=f"{artifact.get('kind', 'artifact')} {artifact.get('path', '')}".strip(),
        )
        for artifact in artifacts
    )
    install_proven = any(
        artifact.get("legs", {}).get("install", {}).get("status") == "proven"
        for artifact in artifacts
    )
    run_proven = any(
        artifact.get("legs", {}).get("run", {}).get("status") == "proven"
        for artifact in artifacts
    )

    updated: list[Leg] = []
    for leg in ledger.legs:
        if leg.name == LEG_BUILD:
            updated.append(_derive(leg, leg.evidence + build_entries, ledger.products))
            continue
        if leg.name == LEG_INSTALL and install_proven:
            evidence = leg.evidence + (
                Evidence(
                    kind=KIND_RUN,
                    product=product,
                    identity=f"run:{run_id}",
                    detail=f"the install smoke installed the artifact and removed it again ({origin})",
                ),
            )
            updated.append(_derive(leg, evidence, ledger.products))
            continue
        if leg.name == LEG_RUN and run_proven:
            evidence = leg.evidence + (
                Evidence(
                    kind=KIND_RUN,
                    product=product,
                    identity=f"run:{run_id}",
                    detail=f"the installed binary mapped its window in a private virtual display ({origin})",
                ),
            )
            updated.append(_derive(leg, evidence, ledger.products))
            continue
        updated.append(leg)
    return updated


def _derive(leg: Leg, evidence: tuple[Evidence, ...], products: tuple[str, ...]) -> Leg:
    """Recompute the status of a leg from its evidence."""
    candidate = Leg(
        name=leg.name,
        status=leg.status,
        reason=leg.reason,
        command=leg.command,
        evidence=evidence,
    )
    status = candidate.derived_status(products)
    reason = leg.reason
    if status == STATUS_UNSUPPORTED:
        covered = sorted(candidate.covered_products())
        missing = sorted(set(products) - candidate.covered_products())
        if covered and missing:
            reason = (
                f"proven for {', '.join(covered)}; {', '.join(missing)} have no evidence yet"
            )
    else:
        reason = ""
    return Leg(
        name=leg.name,
        status=status,
        reason=reason,
        command=leg.command,
        evidence=evidence,
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--platform", required=True, help="the platform of the run")
    parser.add_argument("--run", required=True, type=Path, help="the run evidence file")
    parser.add_argument("--dry-run", action="store_true", help="only print the new ledger")
    arguments = parser.parse_args()

    ledger_path = ROOT / EVIDENCE_DIR / f"{arguments.platform}.json"
    if not ledger_path.is_file():
        print(f"platform evidence: no ledger for the platform {arguments.platform!r}", file=sys.stderr)
        return 2
    try:
        payload = run_evidence(arguments.run)
        if payload.get("platform") != arguments.platform:
            raise ValueError(
                f"{arguments.run}: the run belongs to the platform {payload.get('platform')!r}")
        ledger = parse_ledger(ledger_path)
        legs = recorded_legs(ledger, payload)
        updated = PlatformLedger(
            platform=ledger.platform,
            label=ledger.label,
            products=ledger.products,
            legs=tuple(legs),
            path=ledger.path,
        )
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"platform evidence: {error}", file=sys.stderr)
        return 1

    document = json.loads(ledger_path.read_text(encoding="utf-8"))
    document["legs"] = [
        {
            "leg": leg.name,
            "status": leg.status,
            "reason": leg.reason,
            "command": leg.command,
            "evidence": [entry.as_dict() for entry in leg.evidence],
        }
        for leg in updated.legs
    ]
    if arguments.dry_run:
        print(json.dumps(document, ensure_ascii=False, indent=2))
        return 0
    ledger_path.write_text(json.dumps(document, ensure_ascii=False, indent=2) + "\n",
                           encoding="utf-8")
    print(f"platform evidence: the ledger {ledger_path} was updated from the run "
          f"{payload['run_id']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
