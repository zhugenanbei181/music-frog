#!/usr/bin/env python3
"""Neutral-crate release guard: topology audit, ordered dry-run and publish.

The seven neutral ("Lurker") crates are published to crates.io as a single
release chain:

    lurker-core-contract -> lurker-core-ports -> lurker-engine-runtime
    -> lurker-profile-store -> lurker-ui-presentation
    -> mihomo-driver -> singbox-driver

`scripts/quality/publish-order-guard.py` derives that order from the
dependency graph. This guard adds the release contract on top of it and
provides the local, re-runnable rehearsal entry point that the CI workflow
`.github/workflows/release-neutral-crates.yml` calls.

Modes
-----
structure (default, offline)
    Every crate of the chain exists, stays publishable, carries the
    `MIT OR Apache-2.0` licence, declares the metadata crates.io requires and
    the derived publish order still matches the canonical chain. The result is
    printed as a per-crate table. No network, no cargo, no crates.io.

dry-run
    `cargo publish --dry-run` for every crate in the enforced order. A crate
    whose neutral dependency is not on crates.io yet fails with
    "no matching package named `<crate>`"; that is the expected publish-order
    gate and is reported as `order-gate`, not as a packaging defect. Only a
    genuine packaging/verification failure is reported as `packaging-failure`
    and fails the run.

publish
    The real `cargo publish` in the same order. It refuses to run without a
    crates.io API token in `CARGO_REGISTRY_TOKEN`; there is no anonymous,
    dry-run or debug fallback. After every upload the chain waits until
    crates.io has indexed the new version, because the next crate in the chain
    resolves its dependency from the registry.

Repository secret
-----------------
The crates.io API token is read from the `CARGO_REGISTRY_TOKEN` repository
secret (crates.io token scope `publish-scope`, plus `owner` when the crates
belong to an organisation). The token is never embedded in this file, in the
workflow or in the crate metadata: cargo picks it up from the environment.

Usage
-----
    python3 scripts/quality/neutral-release-guard.py --mode structure
    python3 scripts/quality/neutral-release-guard.py --mode dry-run
    python3 scripts/quality/neutral-release-guard.py --mode publish
    python3 scripts/quality/neutral-release-guard.py --self-test
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
import subprocess
import sys
import time
import tomllib
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
QUALITY_DIR = Path(__file__).resolve().parents[0]

# The authoritative publish chain. `publish-order-guard.py` derives it from
# the dependency graph; this tuple is the reviewed expectation the derived
# order must still match.
CANONICAL_ORDER = (
    "lurker-core-contract",
    "lurker-core-ports",
    "lurker-engine-runtime",
    "lurker-profile-store",
    "lurker-ui-presentation",
    "mihomo-driver",
    "singbox-driver",
)

NEUTRAL_LICENSE = "MIT OR Apache-2.0"
REGISTRY_TOKEN_ENV = "CARGO_REGISTRY_TOKEN"
CRATES_IO_INDEX = "https://index.crates.io"

STATUS_READY = "ready"
STATUS_OK = "ok"
STATUS_ORDER_GATE = "order-gate"
STATUS_ALREADY_PUBLISHED = "already-published"
STATUS_PACKAGING_FAILURE = "packaging-failure"
STATUS_PUBLISHED = "published"
STATUS_MISSING = "missing"

# Cargo/cargo-registry wording that means "the dependency is not on crates.io
# yet" — the expected publish-order gate, not a packaging defect.
ORDER_GATE_SIGNATURES = (
    "no matching package named",
    "could not find crate",
    "failed to select a version for the requirement",
    "no such package",
    "not found in registry",
    "unable to find crate",
)

# Cargo wording that means "this exact version is already on crates.io".
ALREADY_PUBLISHED_SIGNATURES = (
    "already exists",
    "already uploaded",
    "already been published",
)

DEFAULT_COMMAND_TIMEOUT = 1800
DEFAULT_INDEX_TIMEOUT = 600
DEFAULT_PUBLISH_RETRIES = 3


class ReleaseError(RuntimeError):
    """A release precondition that must stop the run loudly."""


def _load_publish_order_guard():
    """Load the sibling publish-order guard (its file name is not importable)."""
    spec = importlib.util.spec_from_file_location(
        "publish_order_guard", QUALITY_DIR / "publish-order-guard.py"
    )
    if spec is None or spec.loader is None:  # pragma: no cover - defensive
        raise ReleaseError("cannot load scripts/quality/publish-order-guard.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


PUBLISH_ORDER_GUARD = _load_publish_order_guard()


# ---------------------------------------------------------------------------
# Topology
# ---------------------------------------------------------------------------


def _read_manifest(path: Path) -> dict:
    return tomllib.loads(path.read_text(encoding="utf-8"))


def _workspace_version(root: Path) -> str | None:
    document = _read_manifest(root / "Cargo.toml")
    version = document.get("workspace", {}).get("package", {}).get("version")
    return version if isinstance(version, str) else None


def _path_dependencies(document: dict) -> dict[str, dict]:
    dependencies: dict[str, dict] = {}
    for section in ("dependencies", "dev-dependencies", "build-dependencies"):
        for name, spec in document.get(section, {}).items():
            if isinstance(spec, dict) and "path" in spec:
                dependencies[name] = spec
    return dependencies


def collect_rows(root: Path = ROOT) -> list[dict]:
    """One row per crate of the canonical chain, in publish order."""
    workspace_version = _workspace_version(root)
    rows: list[dict] = []
    for index, name in enumerate(CANONICAL_ORDER, start=1):
        manifest_path = root / "crates" / name / "Cargo.toml"
        row: dict = {
            "order": index,
            "crate": name,
            "path": f"crates/{name}",
            "present": manifest_path.is_file(),
            "version": None,
            "license": None,
            "description": None,
            "repository": None,
            "publish": None,
            "dependencies": {},
            "status": STATUS_READY,
        }
        if row["present"]:
            document = _read_manifest(manifest_path)
            package = document.get("package", {})
            version = package.get("version")
            row["version"] = version if isinstance(version, str) else workspace_version
            row["license"] = package.get("license")
            row["description"] = package.get("description")
            row["repository"] = package.get("repository")
            row["publish"] = package.get("publish")
            row["dependencies"] = _path_dependencies(document)
        else:
            row["status"] = STATUS_MISSING
        rows.append(row)
    return rows


def version_requirement_matches(requirement: str, version: str | None) -> bool:
    """True when a path dependency's `version` requirement names this release."""
    if not version:
        return False
    wanted = requirement.strip()
    for prefix in ("^", "=", "~"):
        if wanted.startswith(prefix):
            wanted = wanted[len(prefix) :].strip()
    return wanted == version


def topology_problems(root: Path = ROOT) -> tuple[list[str], list[dict]]:
    """Release-blocking problems of the neutral chain plus the crate rows."""
    problems, derived = PUBLISH_ORDER_GUARD.audit(root)
    rows = collect_rows(root)

    missing = [row["crate"] for row in rows if not row["present"]]
    for name in missing:
        problems.append(f"crates/{name}: neutral crate is missing from the release chain")

    if not missing and tuple(derived) != CANONICAL_ORDER:
        problems.append(
            "publish order drifted: derived "
            + " -> ".join(derived)
            + "; expected "
            + " -> ".join(CANONICAL_ORDER)
        )

    for row in rows:
        if not row["present"]:
            continue
        crate = row["crate"]
        if row["license"] != NEUTRAL_LICENSE:
            problems.append(
                f"crates/{crate}: licence must be `{NEUTRAL_LICENSE}`, found `{row['license']}`"
            )
        if row["publish"] is False:
            problems.append(
                f"crates/{crate}: neutral crate must stay publishable (`publish = false` forbids crates.io)"
            )
        if not row["description"]:
            problems.append(f"crates/{crate}: crates.io requires a package description")
        if not row["repository"]:
            problems.append(f"crates/{crate}: crates.io requires a package repository URL")
        if not row["version"]:
            problems.append(f"crates/{crate}: neutral crate has no resolvable version")
        for dependency, spec in row["dependencies"].items():
            if dependency not in CANONICAL_ORDER:
                continue
            requirement = spec.get("version")
            if not isinstance(requirement, str):
                problems.append(
                    f"crates/{crate}: path dependency `{dependency}` must carry an explicit `version`"
                )
            elif not version_requirement_matches(requirement, row["version"]):
                problems.append(
                    f"crates/{crate}: dependency `{dependency}` requires version {requirement} "
                    f"but this release publishes {row['version']}"
                )
    return problems, rows


# ---------------------------------------------------------------------------
# Rendering
# ---------------------------------------------------------------------------

TABLE_HEADERS = ("#", "crate", "version", "licence", "neutral deps", "status")


def _table_cells(row: dict) -> tuple[str, ...]:
    dependencies = ", ".join(
        name for name in row.get("dependencies", {}) if name in CANONICAL_ORDER
    )
    return (
        str(row.get("order", "-")),
        str(row.get("crate", "-")),
        str(row.get("version") or "-"),
        str(row.get("license") or "-"),
        dependencies or "-",
        str(row.get("status") or "-"),
    )


def render_table(rows: list[dict]) -> str:
    """Fixed-width per-crate table of the release chain."""
    body = [_table_cells(row) for row in rows]
    widths: list[int] = []
    for index, header in enumerate(TABLE_HEADERS):
        column = [header] + [line[index] for line in body]
        widths.append(max(len(cell) for cell in column))
    lines = ["  ".join(header.ljust(widths[index]) for index, header in enumerate(TABLE_HEADERS)).rstrip()]
    lines.append("  ".join("-" * width for width in widths))
    for line in body:
        lines.append("  ".join(cell.ljust(widths[index]) for index, cell in enumerate(line)).rstrip())
    return "\n".join(lines)


def render_markdown_table(rows: list[dict]) -> str:
    """Markdown variant of the per-crate table (GitHub step summary)."""
    lines = [
        "| " + " | ".join(TABLE_HEADERS) + " |",
        "| " + " | ".join("---" for _ in TABLE_HEADERS) + " |",
    ]
    for row in rows:
        lines.append("| " + " | ".join(_table_cells(row)) + " |")
    return "\n".join(lines)


# ---------------------------------------------------------------------------
# crates.io index
# ---------------------------------------------------------------------------


def index_path(name: str) -> str:
    """Sparse-index path of a crate (crates.io index layout)."""
    lowered = name.lower()
    if len(lowered) == 1:
        return f"1/{lowered}"
    if len(lowered) == 2:
        return f"2/{lowered}"
    if len(lowered) == 3:
        return f"3/{lowered[0]}/{lowered}"
    return f"{lowered[:2]}/{lowered[2:4]}/{lowered}"


def published_versions(name: str, timeout: int = 30, strict: bool = False) -> set[str]:
    """Versions of a crate that crates.io currently indexes.

    `strict` turns a network failure into an error instead of an empty set:
    an empty set means "not indexed", a network failure means "unknown".
    """
    url = f"{CRATES_IO_INDEX}/{index_path(name)}"
    try:
        with urllib.request.urlopen(url, timeout=timeout) as response:
            body = response.read().decode("utf-8", errors="replace")
    except (urllib.error.URLError, OSError, ValueError) as error:
        if strict:
            raise ReleaseError(
                f"cannot reach the crates.io index for `{name}`: {error}"
            ) from error
        return set()
    versions: set[str] = set()
    for line in body.splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            record = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(record, dict) and isinstance(record.get("vers"), str):
            versions.add(record["vers"])
    return versions


def _neutral_dependencies(row: dict) -> list[str]:
    """Neutral crates a crate of the chain depends on."""
    return [
        name for name in row.get("dependencies", {}) if name in CANONICAL_ORDER
    ]


def _unpublished_dependencies(row: dict, probe) -> list[str]:
    """Neutral dependencies of a crate that crates.io does not hold yet."""
    missing: list[str] = []
    for dependency in _neutral_dependencies(row):
        requirement = row["dependencies"][dependency].get("version")
        if not isinstance(requirement, str):
            continue
        if requirement not in probe(dependency):
            missing.append(dependency)
    return missing


# ---------------------------------------------------------------------------
# Failure classification
# ---------------------------------------------------------------------------


def classify_failure(
    output: str, dependencies: list[str], crate: str
) -> tuple[str, str]:
    """Classify a failed cargo publish/dry-run.

    Returns `(status, reason)`. `order-gate` is the expected publish-order
    gate (a neutral dependency of this crate cannot be resolved from
    crates.io yet), `already-published` means the exact version is already
    released, and `packaging-failure` is a genuine defect that must block the
    release. A missing third-party dependency is never an order gate.
    """
    lowered = output.lower()
    for signature in ALREADY_PUBLISHED_SIGNATURES:
        if signature in lowered:
            return STATUS_ALREADY_PUBLISHED, "crates.io already holds this version"

    for dependency in dependencies:
        if dependency.lower() not in lowered:
            continue
        for signature in ORDER_GATE_SIGNATURES:
            if signature in lowered:
                return (
                    STATUS_ORDER_GATE,
                    f"dependency `{dependency}` is not on crates.io yet",
                )

    return (
        STATUS_PACKAGING_FAILURE,
        f"cargo reported a packaging or verification failure for `{crate}`",
    )


# ---------------------------------------------------------------------------
# cargo execution
# ---------------------------------------------------------------------------


def _as_text(value) -> str:
    if value is None:
        return ""
    if isinstance(value, bytes):
        return value.decode("utf-8", errors="replace")
    return str(value)


def run_cargo(
    command: list[str], root: Path, timeout: int, env: dict | None = None
) -> tuple[int, str]:
    """Run a cargo command, capturing its merged output."""
    try:
        completed = subprocess.run(
            command,
            cwd=root,
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
            env={**os.environ, **env} if env else None,
        )
    except subprocess.TimeoutExpired as expired:
        output = _as_text(expired.stdout) + _as_text(expired.stderr)
        return 124, output + f"\ncommand timed out after {timeout}s: {' '.join(command)}"
    except FileNotFoundError as error:
        return 127, f"cannot run {' '.join(command)}: {error}"
    return completed.returncode, (completed.stdout or "") + (completed.stderr or "")


def cargo_publish_command(crate: str, dry_run: bool) -> list[str]:
    command = ["cargo", "publish", "-p", crate, "--locked"]
    if dry_run:
        command.append("--dry-run")
    return command


def default_publish_runner(
    crate: str, dry_run: bool, root: Path, timeout: int, token: str | None = None
):
    """Run cargo publish; the token only ever travels through the environment."""
    env = {REGISTRY_TOKEN_ENV: token} if token else None
    return run_cargo(cargo_publish_command(crate, dry_run), root, timeout, env)


# ---------------------------------------------------------------------------
# Dry-run
# ---------------------------------------------------------------------------


def dry_run_chain(
    root: Path,
    rows: list[dict],
    *,
    runner=None,
    probe=None,
    timeout: int = DEFAULT_COMMAND_TIMEOUT,
) -> list[dict]:
    """Run `cargo publish --dry-run` for every crate in the enforced order."""
    runner = runner or default_publish_runner
    probe = probe or published_versions
    results: list[dict] = []
    for row in rows:
        crate = row["crate"]
        dependencies = _neutral_dependencies(row)
        code, output = runner(crate, True, root, timeout, None)
        result = dict(row)
        result["output"] = output
        if code == 0:
            result["status"] = STATUS_OK
            result["reason"] = "dry-run packaging succeeded"
        else:
            status, reason = classify_failure(output, dependencies, crate)
            if status == STATUS_ORDER_GATE:
                missing = _unpublished_dependencies(row, probe)
                if missing:
                    reason += " (" + ", ".join(f"`{name}`" for name in missing) + " not indexed)"
            result["status"] = status
            result["reason"] = reason
        results.append(result)
    return results


# ---------------------------------------------------------------------------
# Publish
# ---------------------------------------------------------------------------


def require_token(token: str | None) -> str:
    """Refuse to publish without a crates.io API token."""
    if token is None or not token.strip():
        raise ReleaseError(
            "the crates.io API token is missing: set the repository secret "
            f"{REGISTRY_TOKEN_ENV} (crates.io token scope `publish-scope`). "
            "Refusing to publish without a token — there is no anonymous, "
            "dry-run or debug fallback."
        )
    return token.strip()


def wait_for_publication(
    crate: str,
    version: str | None,
    *,
    probe=None,
    sleeper=time.sleep,
    timeout: int = DEFAULT_INDEX_TIMEOUT,
    interval: int = 10,
) -> bool:
    """Wait until crates.io indexes the freshly published version."""
    probe = probe or (lambda name: published_versions(name, strict=True))
    if not version:
        return True
    deadline = time.monotonic() + timeout
    while True:
        if version in probe(crate):
            return True
        if time.monotonic() >= deadline:
            return False
        sleeper(interval)


def publish_chain(
    root: Path,
    rows: list[dict],
    token: str,
    *,
    runner=None,
    probe=None,
    sleeper=time.sleep,
    timeout: int = DEFAULT_COMMAND_TIMEOUT,
    retries: int = DEFAULT_PUBLISH_RETRIES,
    index_timeout: int = DEFAULT_INDEX_TIMEOUT,
) -> list[dict]:
    """Publish every crate in the enforced order, waiting for the registry."""
    runner = runner or default_publish_runner
    probe = probe or published_versions
    results: list[dict] = []
    for row in rows:
        crate = row["crate"]
        version = row.get("version")
        result = dict(row)
        result["output"] = ""
        attempts = 0
        while True:
            attempts += 1
            code, output = runner(crate, False, root, timeout, token)
            result["output"] = output
            if code == 0:
                result["status"] = STATUS_PUBLISHED
                result["reason"] = "published to crates.io"
                break
            status, reason = classify_failure(output, _neutral_dependencies(row), crate)
            if status == STATUS_ORDER_GATE:
                missing = _unpublished_dependencies(row, probe)
                if missing:
                    reason += " (" + ", ".join(f"`{name}`" for name in missing) + " not indexed)"
            if status == STATUS_ORDER_GATE and attempts < retries:
                # The predecessor was published a moment ago: wait for the
                # registry to index it and try again.
                result["status"] = STATUS_ORDER_GATE
                result["reason"] = reason
                sleeper(10)
                continue
            result["status"] = status
            result["reason"] = reason
            break
        if result["status"] == STATUS_PUBLISHED and version:
            indexed = wait_for_publication(
                crate,
                version,
                probe=probe,
                sleeper=sleeper,
                timeout=index_timeout,
            )
            if not indexed:
                result["status"] = STATUS_PACKAGING_FAILURE
                result["reason"] = (
                    f"crates.io did not index `{crate} v{version}` in time; "
                    "the next crate in the chain cannot resolve it"
                )
        results.append(result)
        if result["status"] not in (STATUS_PUBLISHED, STATUS_ALREADY_PUBLISHED):
            break
    return results


# ---------------------------------------------------------------------------
# Entry points
# ---------------------------------------------------------------------------


def _print_rows(rows: list[dict], problems: list[str]) -> None:
    print(render_table(rows))
    if problems:
        print("problems:", file=sys.stderr)
        for problem in problems:
            print(f"- {problem}", file=sys.stderr)


def _write_summary(path: str | None, rows: list[dict], title: str) -> None:
    if not path:
        return
    with open(path, "a", encoding="utf-8") as handle:
        handle.write(f"### {title}\n\n")
        handle.write(render_markdown_table(rows))
        handle.write("\n\n")


def _print_cargo_output(row: dict, limit: int = 40) -> None:
    """Show the tail of the cargo output so a failure is diagnosable."""
    output = (row.get("output") or "").strip()
    if not output:
        return
    lines = output.splitlines()
    print("--- cargo output (tail) ---", file=sys.stderr)
    for line in lines[-limit:]:
        print(line, file=sys.stderr)
    print("--- end of cargo output ---", file=sys.stderr)


def run_structure(root: Path, summary: str | None = None) -> int:
    problems, rows = topology_problems(root)
    _print_rows(rows, problems)
    if summary:
        _write_summary(summary, rows, "Neutral crate release topology")
    if problems:
        print(f"neutral release guard: FAILED ({len(problems)} problem(s))", file=sys.stderr)
        return 1
    print(
        f"neutral release guard: OK (publish order: {' -> '.join(CANONICAL_ORDER)})"
    )
    return 0


def run_dry_run(
    root: Path, summary: str | None = None, timeout: int = DEFAULT_COMMAND_TIMEOUT
) -> int:
    problems, rows = topology_problems(root)
    if problems:
        _print_rows(rows, problems)
        print("neutral release guard: FAILED (topology)", file=sys.stderr)
        return 1
    results = dry_run_chain(root, rows, timeout=timeout)
    _print_rows(results, [])
    if summary:
        _write_summary(summary, results, "Neutral crate dry-run")
    failures = [row for row in results if row["status"] == STATUS_PACKAGING_FAILURE]
    gates = [row for row in results if row["status"] == STATUS_ORDER_GATE]
    print(
        f"neutral release guard: dry-run finished "
        f"(ok={len([r for r in results if r['status'] == STATUS_OK])}, "
        f"order-gate={len(gates)}, packaging-failure={len(failures)})"
    )
    if gates:
        print(
            "order-gate crates depend on a neutral crate that is not on "
            "crates.io yet — publish the chain in order and re-run."
        )
    if failures:
        for row in failures:
            print(f"packaging failure: {row['crate']}: {row['reason']}", file=sys.stderr)
            _print_cargo_output(row)
        return 1
    return 0


def run_publish(
    root: Path, summary: str | None = None, timeout: int = DEFAULT_COMMAND_TIMEOUT
) -> int:
    token = require_token(os.environ.get(REGISTRY_TOKEN_ENV))
    problems, rows = topology_problems(root)
    if problems:
        _print_rows(rows, problems)
        print("neutral release guard: FAILED (topology)", file=sys.stderr)
        return 1
    results = publish_chain(root, rows, token, timeout=timeout)
    _print_rows(results, [])
    if summary:
        _write_summary(summary, results, "Neutral crate publish")
    failures = [
        row
        for row in results
        if row["status"] not in (STATUS_PUBLISHED, STATUS_ALREADY_PUBLISHED)
    ]
    if failures:
        for row in failures:
            print(f"publish failed: {row['crate']}: {row['reason']}", file=sys.stderr)
            _print_cargo_output(row)
        return 1
    print("neutral release guard: publish finished")
    return 0


def self_test() -> int:
    order_gate = (
        "error: failed to select a version for the requirement "
        "`lurker-core-contract = \"^0.50.0\"`\n"
        "no matching package named `lurker-core-contract` found\n"
        "location searched: crates.io index\n"
    )
    status, _ = classify_failure(order_gate, ["lurker-core-contract"], "lurker-core-ports")
    assert status == STATUS_ORDER_GATE, status

    status, _ = classify_failure(order_gate, [], "lurker-core-ports")
    assert status == STATUS_PACKAGING_FAILURE, status

    third_party = (
        "error: failed to select a version for the requirement `anyhow = \"^1.0\"`\n"
        "no matching package named `anyhow` found\n"
    )
    status, _ = classify_failure(third_party, ["lurker-core-contract"], "lurker-core-ports")
    assert status == STATUS_PACKAGING_FAILURE, status

    already = (
        "error: failed to publish to registry at https://crates.io\n"
        "the remote server responded with an error (status 403 Forbidden): "
        "crate `lurker-core-contract v0.50.0` already exists\n"
    )
    status, _ = classify_failure(already, [], "lurker-core-contract")
    assert status == STATUS_ALREADY_PUBLISHED, status

    broken = "error: failed to prepare local package for uploading\n"
    status, _ = classify_failure(broken, ["lurker-core-contract"], "lurker-core-contract")
    assert status == STATUS_PACKAGING_FAILURE, status

    assert version_requirement_matches("0.50.0", "0.50.0")
    assert version_requirement_matches("^0.50.0", "0.50.0")
    assert not version_requirement_matches("0.49.0", "0.50.0")

    assert index_path("lurker-core-contract") == "lu/rk/lurker-core-contract"
    assert index_path("mihomo-driver") == "mi/ho/mihomo-driver"
    assert index_path("ab") == "2/ab"

    try:
        require_token("")
    except ReleaseError as error:
        assert REGISTRY_TOKEN_ENV in str(error)
    else:  # pragma: no cover - defensive
        raise AssertionError("a missing token must be rejected")

    assert require_token(" token ") == "token"

    rows = [
        {
            "order": 1,
            "crate": "lurker-core-contract",
            "version": "0.50.0",
            "license": NEUTRAL_LICENSE,
            "dependencies": {},
            "status": STATUS_READY,
        }
    ]
    table = render_table(rows)
    assert "lurker-core-contract" in table and "0.50.0" in table
    assert "| lurker-core-contract |" in render_markdown_table(rows)

    print("neutral release guard self-test: PASS")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--mode",
        choices=("structure", "dry-run", "publish"),
        default="structure",
        help="structure: offline topology audit; dry-run: cargo publish --dry-run; publish: real upload",
    )
    parser.add_argument("--root", default=str(ROOT), help="repository root to audit")
    parser.add_argument("--summary", help="append the per-crate table to this file")
    parser.add_argument(
        "--timeout",
        type=int,
        default=DEFAULT_COMMAND_TIMEOUT,
        help="per-command timeout in seconds",
    )
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        return self_test()

    root = Path(args.root).resolve()
    try:
        if args.mode == "structure":
            return run_structure(root, args.summary)
        if args.mode == "dry-run":
            return run_dry_run(root, args.summary, args.timeout)
        return run_publish(root, args.summary, args.timeout)
    except ReleaseError as error:
        print(f"neutral release guard: FAILED: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
