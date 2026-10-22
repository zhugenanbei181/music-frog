#!/usr/bin/env python3
"""Keep the feature matrix of the workspace declared, consistent and resolvable.

Cargo features are a public surface: a feature that appears in a script, in a
CI job or in a dependency edge is part of how the product is built. This guard
keeps `feature_matrix.json` a faithful mirror of the `[features]` tables and
proves that every feature reference in the repository resolves:

  * a crate with a `[features]` table must be registered, and a registered
    crate must still exist with the same features and members;
  * every feature has a kind, and a `default` feature may only enable product
    or platform features, so capture and test tooling stays opt-in;
  * `--features` payloads in scripts and workflows must name a declared
    feature, and a dependency edge that requests a workspace feature must
    request one that exists.

The guard is offline: it parses TOML and the script text.
"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import sys
import tomllib
import unittest

REPO_ROOT = Path(__file__).resolve().parents[2]
MATRIX = Path("scripts/quality/feature_matrix.json")
SKIPPED_DIRECTORIES = {".git", "target", "node_modules", "dist", "build"}
SCAN_ROOTS = ("crates", "android", "examples", "templates", "sandbox")
EXCLUDED_ROOTS = (Path("crates/infiltrator-slint/lumen-source"),)
KINDS = {"capture", "platform", "product", "test"}
FEATURE_FLAG = re.compile(r"--features[=\s]+(?P<payload>\"[^\"]*\"|'[^']*'|[^\s;|&)]+)")
SCRIPT_ROOTS = ("scripts", ".github/workflows")


def _manifest_paths(root: Path) -> list[Path]:
    paths: list[Path] = []
    for scan_root in SCAN_ROOTS:
        base = root / scan_root
        if not base.is_dir():
            continue
        for directory, subdirectories, filenames in os.walk(base):
            subdirectories[:] = sorted(
                name for name in subdirectories if name not in SKIPPED_DIRECTORIES
            )
            if "Cargo.toml" in filenames:
                path = Path(directory) / "Cargo.toml"
                relative = path.relative_to(root)
                if any(relative.is_relative_to(item) for item in EXCLUDED_ROOTS):
                    continue
                paths.append(path)
    return paths


def manifest_features(root: Path) -> dict[str, dict[str, list[str]]]:
    """Return `{package: {feature: members}}` for every crate with features."""
    found: dict[str, dict[str, list[str]]] = {}
    for path in _manifest_paths(root):
        try:
            document = tomllib.loads(path.read_text(encoding="utf-8"))
        except (OSError, tomllib.TOMLDecodeError):
            continue
        features = document.get("features")
        package = (document.get("package") or {}).get("name")
        if not isinstance(features, dict) or not isinstance(package, str):
            continue
        found[package] = {
            name: list(members) if isinstance(members, list) else []
            for name, members in features.items()
        }
    return found


def dependency_feature_requests(root: Path) -> list[tuple[str, str, str]]:
    """Return `(manifest, dependency, feature)` for every requested feature."""
    rows: list[tuple[str, str, str]] = []
    for path in _manifest_paths(root):
        try:
            document = tomllib.loads(path.read_text(encoding="utf-8"))
        except (OSError, tomllib.TOMLDecodeError):
            continue
        relative = path.relative_to(root).as_posix()
        for section in ("dependencies", "dev-dependencies", "build-dependencies"):
            table = document.get(section)
            if not isinstance(table, dict):
                continue
            for alias, entry in table.items():
                if not isinstance(entry, dict):
                    continue
                dependency = entry.get("package") if isinstance(entry.get("package"), str) else alias
                requested = entry.get("features")
                if not isinstance(requested, list):
                    continue
                for feature in requested:
                    if isinstance(feature, str):
                        rows.append((relative, dependency, feature))
    return rows


def script_feature_flags(root: Path) -> list[tuple[str, int, str]]:
    """Return `(file, line, payload)` for every `--features` flag in the tree."""
    rows: list[tuple[str, int, str]] = []
    for scan_root in SCRIPT_ROOTS:
        base = root / scan_root
        if not base.is_dir():
            continue
        for directory, subdirectories, filenames in os.walk(base):
            subdirectories[:] = sorted(
                name for name in subdirectories if name not in SKIPPED_DIRECTORIES
            )
            for filename in sorted(filenames):
                if not filename.endswith((".sh", ".yml", ".yaml")):
                    continue
                path = Path(directory) / filename
                text = path.read_text(encoding="utf-8", errors="replace")
                relative = path.relative_to(root).as_posix()
                for match in FEATURE_FLAG.finditer(text):
                    line = text.count("\n", 0, match.start()) + 1
                    payload = match.group("payload").strip("\"'")
                    payload = re.sub(r"\\[A-Za-z]", "", payload)
                    payload = payload.strip(")]},")
                    rows.append((relative, line, payload))
    return rows


def load_matrix(root: Path) -> tuple[dict[str, dict[str, dict]], list[str]]:
    path = root / MATRIX
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return {}, [f"{MATRIX}: cannot read the matrix: {error}"]
    if not isinstance(document, dict) or document.get("version") != 1:
        return {}, [f"{MATRIX}: expected a version 1 object"]
    if document.get("kinds") is not None and set(document["kinds"]) != KINDS:
        return {}, [f"{MATRIX}: `kinds` must list exactly {sorted(KINDS)}"]

    rows = document.get("crates")
    if not isinstance(rows, list) or not rows:
        return {}, [f"{MATRIX}: `crates` must be a non-empty array"]

    entries: dict[str, dict[str, dict]] = {}
    problems: list[str] = []
    for index, row in enumerate(rows):
        prefix = f"{MATRIX}: crates[{index}]"
        if not isinstance(row, dict):
            problems.append(f"{prefix} must be an object")
            continue
        package = row.get("package")
        if not isinstance(package, str) or not package:
            problems.append(f"{prefix}: package must be a non-empty string")
            continue
        if package in entries:
            problems.append(f"{prefix}: duplicate package `{package}`")
        features = row.get("features")
        if not isinstance(features, list) or not features:
            problems.append(f"{prefix}: features must be a non-empty array")
            continue
        table: dict[str, dict] = {}
        for feature in features:
            if not isinstance(feature, dict):
                problems.append(f"{prefix}: every feature must be an object")
                continue
            name = feature.get("name")
            kind = feature.get("kind")
            members = feature.get("members")
            if not isinstance(name, str) or not name:
                problems.append(f"{prefix}: feature name must be a non-empty string")
                continue
            if kind not in KINDS:
                problems.append(f"{prefix}: feature `{name}` needs a kind of {sorted(KINDS)}")
            if not isinstance(members, list) or not all(
                isinstance(item, str) for item in members
            ):
                problems.append(f"{prefix}: feature `{name}` members must be strings")
                continue
            table[name] = {"kind": str(kind), "members": list(members)}
        entries[package] = table
    return entries, problems


def check(root: Path = REPO_ROOT) -> list[str]:
    entries, problems = load_matrix(root)
    if not entries and problems:
        return problems
    discovered = manifest_features(root)

    for package in sorted(set(discovered) - set(entries)):
        problems.append(
            f"{package}: declares features but is missing from {MATRIX};"
            " register every feature with its kind"
        )
    for package in sorted(set(entries) - set(discovered)):
        problems.append(
            f"{package}: registered in {MATRIX} but declares no features anymore;"
            " delete the stale entry"
        )

    kinds: dict[str, dict[str, str]] = {}
    for package in sorted(set(entries) & set(discovered)):
        registered = entries[package]
        actual = discovered[package]
        kinds[package] = {name: row["kind"] for name, row in registered.items()}
        for name in sorted(set(actual) - set(registered)):
            problems.append(f"{package}: feature `{name}` is not registered in {MATRIX}")
        for name in sorted(set(registered) - set(actual)):
            problems.append(
                f"{package}: feature `{name}` is registered in {MATRIX} but the manifest"
                " does not declare it"
            )
        for name in sorted(set(registered) & set(actual)):
            if sorted(registered[name]["members"]) != sorted(actual[name]):
                problems.append(
                    f"{package}: feature `{name}` enables {sorted(actual[name])}, ledger"
                    f" records {sorted(registered[name]['members'])}"
                )
        default = actual.get("default")
        if default is not None:
            for member in default:
                if "/" in member or member.startswith("dep:"):
                    continue
                member_kind = kinds[package].get(member)
                if member_kind in {"capture", "test"}:
                    problems.append(
                        f"{package}: the default feature enables `{member}`"
                        f" ({member_kind}); capture and test features stay opt-in"
                    )

    for manifest, dependency, feature in dependency_feature_requests(root):
        table = discovered.get(dependency)
        if table is None:
            continue
        if feature not in table:
            problems.append(
                f"{manifest}: dependency `{dependency}` does not declare the feature"
                f" `{feature}`"
            )

    declared_features = {name for table in discovered.values() for name in table}
    for relative, line, payload in script_feature_flags(root):
        if not payload or "$" in payload or "{" in payload:
            continue
        for token in re.split(r"[,\s]+", payload):
            token = token.strip()
            if not token:
                continue
            if "/" in token:
                package, _, feature = token.partition("/")
                if package in discovered and feature not in discovered[package]:
                    problems.append(
                        f"{relative}:{line}: `{token}` is not a feature of `{package}`"
                    )
            elif token not in declared_features:
                problems.append(
                    f"{relative}:{line}: `{token}` is not a declared feature of the workspace"
                )
    return problems


class FeatureMatrixSelfTest(unittest.TestCase):
    def test_repository_matrix_matches_the_manifests(self):
        self.assertEqual(check(REPO_ROOT), [])

    def test_unregistered_feature_is_rejected(self):
        with _scratch(
            manifest='[package]\nname = "demo"\nversion = "0.1.0"\n\n'
            "[features]\ndefault = []\nextra = []\n"
        ) as root:
            problems = check(root)
        self.assertTrue(any("`extra` is not registered" in problem for problem in problems))

    def test_default_feature_may_not_enable_a_capture_feature(self):
        with _scratch(
            manifest='[package]\nname = "demo"\nversion = "0.1.0"\n\n'
            '[features]\ndefault = ["capture-tools"]\ncapture-tools = []\n'
        ) as root:
            problems = check(root)
        self.assertTrue(any("stay opt-in" in problem for problem in problems))

    def test_unknown_script_feature_is_rejected(self):
        with _scratch() as root:
            script = root / "scripts/run.sh"
            script.parent.mkdir(parents=True, exist_ok=True)
            script.write_text("cargo build --features nope\n", encoding="utf-8")
            problems = check(root)
        self.assertTrue(any("is not a declared feature" in problem for problem in problems))


def _matrix() -> dict:
    return {
        "version": 1,
        "note": "fixture",
        "kinds": sorted(KINDS),
        "crates": [
            {
                "package": "demo",
                "features": [
                    {"name": "default", "kind": "product", "members": []},
                    {"name": "capture-tools", "kind": "capture", "members": []},
                ],
            }
        ],
    }


class _scratch:
    def __init__(
        self,
        manifest: str = '[package]\nname = "demo"\nversion = "0.1.0"\n\n'
        "[features]\ndefault = []\ncapture-tools = []\n",
    ):
        self._manifest = manifest

    def __enter__(self) -> Path:
        import tempfile

        self._temporary = tempfile.TemporaryDirectory()
        root = Path(self._temporary.name)
        (root / MATRIX).parent.mkdir(parents=True, exist_ok=True)
        (root / MATRIX).write_text(json.dumps(_matrix()), encoding="utf-8")
        (root / "crates/demo").mkdir(parents=True)
        (root / "crates/demo/Cargo.toml").write_text(self._manifest, encoding="utf-8")
        return root

    def __exit__(self, *_excuses) -> None:
        self._temporary.cleanup()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--mode", choices=["report", "enforce"], default="report")
    arguments = parser.parse_args()

    if arguments.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(FeatureMatrixSelfTest)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1

    problems = check()
    if problems:
        print("\n".join(problems), file=sys.stderr)
        return 1 if arguments.mode == "enforce" else 0
    entries, _ = load_matrix(REPO_ROOT)
    features = sum(len(table) for table in entries.values())
    print(
        f"feature matrix guard: {len(entries)} crates, {features} declared features,"
        " every script flag resolves"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
