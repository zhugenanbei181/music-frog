#!/usr/bin/env python3
"""Ordered crates.io rehearsal that stops at the first real defect.

This driver is a thin shell around the repository's own release guard
(`scripts/quality/neutral-release-guard.py`). There is exactly one classifier:
the guard decides whether a failed `cargo publish --dry-run` is the expected
publish-order gate (a neutral dependency is not on crates.io yet) or a real
packaging defect. This module never re-implements that decision.

The only behaviour it adds is the stop-at-the-first-defect rule: the guard's
own `dry-run` mode walks the whole chain and reports every crate, while this
driver stops at the first crate with a genuine `packaging-failure`, because a
broken package must never reach crates.io and every later crate would only
repeat the same defect.

Modes
-----
dry-run     `cargo publish --dry-run` for every crate of the chain, in the
            enforced order, stopping at the first real packaging failure.
checksums  `cargo package --no-verify` for every crate of the chain and the
            SHA-256 checksum manifest of the produced `.crate` artifacts. A
            crate that is still behind the publish-order gate cannot be
            packaged; that is reported, not hidden.

Usage
-----
    python3 scripts/blocked/publish_chain.py --mode dry-run
    python3 scripts/blocked/publish_chain.py --mode checksums
    python3 scripts/blocked/publish_chain.py --self-test
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
GUARD_PATH = ROOT / "scripts" / "quality" / "neutral-release-guard.py"

# The release candidate baseline the board freezes the seven neutral crates on.
RC_BASELINE_VERSION = "1.0.0-rc.1"


def load_guard():
    """Load the repository's release guard as a module."""
    spec = importlib.util.spec_from_file_location("neutral_release_guard", GUARD_PATH)
    if spec is None or spec.loader is None:  # pragma: no cover - defensive
        raise SystemExit(f"cannot load {GUARD_PATH}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def run_dry_run(guard, root: Path, timeout: int, stop_at_first: bool) -> tuple[int, list[dict]]:
    problems, rows = guard.topology_problems(root)
    if problems:
        for problem in problems:
            print(f"topology problem: {problem}", file=sys.stderr)
        return 1, []

    results: list[dict] = []
    for row in rows:
        crate = row["crate"]
        dependencies = guard._neutral_dependencies(row)
        code, output = guard.default_publish_runner(crate, True, root, timeout, None)
        result = dict(row)
        result["output"] = output
        if code == 0:
            result["status"] = guard.STATUS_OK
            result["reason"] = "dry-run packaging succeeded"
        else:
            status, reason = guard.classify_failure(output, dependencies, crate)
            if status == guard.STATUS_ORDER_GATE:
                missing = guard._unpublished_dependencies(row, guard.published_versions)
                if missing:
                    reason += " (" + ", ".join(f"`{name}`" for name in missing) + " not indexed)"
            result["status"] = status
            result["reason"] = reason
        results.append(result)
        if result["status"] == guard.STATUS_PACKAGING_FAILURE and stop_at_first:
            print(
                f"stopping at the first real defect: {crate} ({result['reason']})",
                file=sys.stderr,
            )
            break
    return 0, results


def run_checksums(guard, root: Path, rows: list[dict], timeout: int) -> tuple[int, list[dict]]:
    records: list[dict] = []
    failures = 0
    for row in rows:
        crate = row["crate"]
        version = row.get("version")
        artifact = root / "target" / "package" / f"{crate}-{version}.crate"
        command = ["cargo", "package", "-p", crate, "--locked", "--no-verify"]
        try:
            completed = subprocess.run(
                command,
                cwd=root,
                capture_output=True,
                text=True,
                timeout=timeout,
                check=False,
            )
            output = (completed.stdout or "") + (completed.stderr or "")
        except subprocess.TimeoutExpired:
            output = f"cargo package timed out after {timeout}s"
            completed = None

        record = {
            "crate": crate,
            "version": version,
            "artifact": str(artifact.relative_to(root)),
            "sha256": sha256_file(artifact) if artifact.is_file() else None,
            "size_bytes": artifact.stat().st_size if artifact.is_file() else 0,
            "status": "ok" if artifact.is_file() else "order-gate",
            "reason": "",
        }
        if record["status"] == "ok":
            record["reason"] = "packaged"
        else:
            lowered = output.lower()
            gated = any(
                signature in lowered
                for signature in guard.ORDER_GATE_SIGNATURES
            )
            record["status"] = "order-gate" if gated else "packaging-failure"
            record["reason"] = (
                "a neutral dependency is not on crates.io yet"
                if gated
                else "cargo package failed"
            )
            failures += 1 if record["status"] == "packaging-failure" else 0
        records.append(record)
    return failures, records


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("dry-run", "checksums"), default="dry-run")
    parser.add_argument("--timeout", type=int, default=1800)
    parser.add_argument("--json", type=Path, default=None, help="write the result as JSON")
    parser.add_argument(
        "--continue-on-defect",
        action="store_true",
        help="do not stop at the first real packaging defect",
    )
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        assert sha256_file(ROOT / "Cargo.toml") == sha256_file(ROOT / "Cargo.toml")
        guard = load_guard()
        assert guard.CANONICAL_ORDER[0] == "lurker-core-contract"
        assert "no matching package named" in guard.ORDER_GATE_SIGNATURES
        print("publish chain self-test: PASS")
        return 0

    guard = load_guard()
    problems, rows = guard.topology_problems(ROOT)

    if args.mode == "dry-run":
        code, results = run_dry_run(guard, ROOT, args.timeout, not args.continue_on_defect)
        if code:
            return code
        print(guard.render_table(results))
        failures = [row for row in results if row["status"] == guard.STATUS_PACKAGING_FAILURE]
        gates = [row for row in results if row["status"] == guard.STATUS_ORDER_GATE]
        ok = [row for row in results if row["status"] == guard.STATUS_OK]
        print(
            "publish chain dry-run: "
            f"ok={len(ok)} order-gate={len(gates)} packaging-failure={len(failures)}"
        )
        if gates:
            print(
                "order-gate: the chain is not on crates.io yet; publish it in the enforced "
                "order and re-run."
            )
        for row in failures:
            print(f"packaging failure: {row['crate']}: {row['reason']}", file=sys.stderr)
            tail = (row.get("output") or "").strip().splitlines()[-25:]
            for line in tail:
                print(line, file=sys.stderr)
        payload = {
            "mode": "dry-run",
            "results": [
                {key: row.get(key) for key in ("crate", "version", "status", "reason")}
                for row in results
            ],
        }
        if args.json:
            args.json.write_text(json.dumps(payload, indent=2), encoding="utf-8")
        return 1 if failures else 0

    # checksums
    if problems:
        for problem in problems:
            print(f"topology problem: {problem}", file=sys.stderr)
        return 1
    failures, records = run_checksums(guard, ROOT, rows, args.timeout)
    print(f"{'crate':<26} {'version':<10} {'status':<18} sha256")
    for record in records:
        print(
            f"{record['crate']:<26} {str(record['version']):<10} "
            f"{record['status']:<18} {record['sha256'] or '-'}"
        )
    frozen = [row for row in rows if row.get("version") == RC_BASELINE_VERSION]
    print(
        f"release-candidate baseline {RC_BASELINE_VERSION}: "
        f"{len(frozen)}/{len(rows)} neutral crates frozen"
    )
    payload = {"mode": "checksums", "records": records}
    if args.json:
        args.json.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
