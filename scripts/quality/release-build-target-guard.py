#!/usr/bin/env python3
"""Keep every release build step pointing at a package that owns the named bin.

`cargo build -p <package> --bin <name>` silently succeeds at review time and
fails only on the release runner when `<name>` belongs to a different package.
This guard pairs each `-p <package>` with its `--bin <name>` in
`.github/workflows/release.yml` and checks the workspace manifests declare that
bin for that package.
"""

from __future__ import annotations

import re
import sys
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
RELEASE_WORKFLOW = Path(".github/workflows/release.yml")
BUILD_COMMAND = re.compile(r"-p\s+([A-Za-z0-9_-]+)([^\n]*)--bin\s+([A-Za-z0-9_-]+)")


def _workspace_members(root: Path) -> list[Path]:
    manifest = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    members = manifest.get("workspace", {}).get("members", [])
    return [root / member for member in members if isinstance(member, str)]


def _declared_bins(member: Path) -> tuple[str, set[str]]:
    manifest = tomllib.loads((member / "Cargo.toml").read_text(encoding="utf-8"))
    package = manifest["package"]["name"]
    bins = {
        target["name"]
        for target in manifest.get("bin", [])
        if isinstance(target, dict) and isinstance(target.get("name"), str)
    }
    if not bins and (member / "src/main.rs").is_file():
        bins.add(package)
    return package, bins


def check(root: Path = ROOT) -> list[str]:
    bins_by_package: dict[str, set[str]] = {}
    for member in _workspace_members(root):
        package, bins = _declared_bins(member)
        bins_by_package[package] = bins

    workflow = (root / RELEASE_WORKFLOW).read_text(encoding="utf-8")
    problems: list[str] = []
    for match in BUILD_COMMAND.finditer(workflow):
        package, _, binary = match.groups()
        if package not in bins_by_package:
            problems.append(
                f"{RELEASE_WORKFLOW}: release build references unknown package `{package}`"
            )
            continue
        if binary not in bins_by_package[package]:
            owners = sorted(name for name, bins in bins_by_package.items() if binary in bins)
            owner_hint = f" (owned by {', '.join(owners)})" if owners else " (no package declares it)"
            problems.append(
                f"{RELEASE_WORKFLOW}: package `{package}` does not declare bin `{binary}`{owner_hint}"
            )
    if not problems and not BUILD_COMMAND.search(workflow):
        problems.append(f"{RELEASE_WORKFLOW}: no release build step declares a `-p <package> --bin <name>`")
    return problems


def self_test() -> int:
    assert BUILD_COMMAND.search("cargo build -p foo --bin bar")
    assert BUILD_COMMAND.search("cargo zigbuild --release -p foo --features x --bin bar")
    assert not BUILD_COMMAND.search("cargo build --bin bar")
    print("release build target guard self-test: PASS")
    return 0


def main() -> int:
    if "--self-test" in sys.argv:
        return self_test()
    problems = check()
    if problems:
        print("\n".join(problems), file=sys.stderr)
        return 1
    print("release build target guard: OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
