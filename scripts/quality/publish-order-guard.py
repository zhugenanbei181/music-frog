#!/usr/bin/env python3
"""Audit the publish topology of the neutral kernel and product crates.

PUB-006.3 requires that
  * every neutral crate is publishable: each path dependency on another neutral
    crate carries a matching explicit `version`, so `cargo package`/`publish`
    resolves on crates.io instead of the local path;
  * neutral versions stay aligned (they are released together);
  * the neutral dependency graph is acyclic, so a publish order exists;
  * every product crate is `publish = false`, so a product release can never
    accidentally push a product crate to crates.io.

Exit code 0 prints the derived publish order; any violation prints to stderr.
`--self-test` exercises the checks on a synthetic topology.
"""

from __future__ import annotations

import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

# Crates that must never reach crates.io.
PRODUCT_PREFIXES = ("infiltrator-", "mihomo-", "singbox-")
NEUTRAL_EXPLICIT = {"mihomo-driver", "singbox-driver"}


def is_neutral(name: str) -> bool:
    return name.startswith("lurker-") or name in NEUTRAL_EXPLICIT


def manifests(root: Path) -> dict[str, Path]:
    found: dict[str, Path] = {}
    for manifest in sorted(root.glob("crates/**/Cargo.toml")):
        try:
            document = tomllib.loads(manifest.read_text(encoding="utf-8"))
        except tomllib.TOMLDecodeError:
            continue
        name = document.get("package", {}).get("name")
        if name:
            found[name] = manifest
    return found


def path_dependencies(document: dict) -> dict[str, dict]:
    dependencies: dict[str, dict] = {}
    for section in ("dependencies", "dev-dependencies", "build-dependencies"):
        for name, spec in document.get(section, {}).items():
            if isinstance(spec, dict) and "path" in spec:
                dependencies[name] = spec
    return dependencies


def resolved_version(document: dict, workspace_version: str | None) -> str | None:
    package = document.get("package", {})
    if isinstance(package.get("version"), str):
        return package["version"]
    if package.get("version", {}).get("workspace") and workspace_version:
        return workspace_version
    return None


def cycle(nodes: set[str], edges: dict[str, set[str]]) -> list[str] | None:
    state: dict[str, int] = {}
    stack: list[str] = []

    def visit(node: str) -> list[str] | None:
        state[node] = 1
        stack.append(node)
        for peer in sorted(edges.get(node, ())):
            if state.get(peer) == 1:
                return stack[stack.index(peer):] + [peer]
            if state.get(peer, 0) == 0:
                found = visit(peer)
                if found:
                    return found
        stack.pop()
        state[node] = 2
        return None

    for node in sorted(nodes):
        if state.get(node, 0) == 0:
            found = visit(node)
            if found:
                return found
    return None


def publish_order(nodes: set[str], edges: dict[str, set[str]]) -> list[str]:
    order: list[str] = []
    seen: set[str] = set()

    def visit(node: str) -> None:
        if node in seen:
            return
        seen.add(node)
        for peer in sorted(edges.get(node, ())):
            visit(peer)
        order.append(node)

    for node in sorted(nodes):
        visit(node)
    return order


def audit(root: Path = ROOT) -> list[str]:
    problems: list[str] = []
    found = manifests(root)
    workspace_document = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    workspace_version = workspace_document.get("workspace", {}).get("package", {}).get("version")

    neutral = {name: found[name] for name in found if is_neutral(name)}
    versions: dict[str, str | None] = {}
    edges: dict[str, set[str]] = {}
    for name, manifest in neutral.items():
        document = tomllib.loads(manifest.read_text(encoding="utf-8"))
        version = resolved_version(document, workspace_version)
        versions[name] = version
        if not version:
            problems.append(f"{manifest}: neutral crate has no resolvable version")
        dependencies = path_dependencies(document)
        edges[name] = {dep for dep in dependencies if dep in neutral}
        for dependency, spec in dependencies.items():
            if dependency in neutral and "version" not in spec:
                problems.append(
                    f"{manifest}: path dependency `{dependency}` must carry an explicit `version` "
                    "so the crate is publishable"
                )
            if dependency in found and not is_neutral(dependency):
                problems.append(
                    f"{manifest}: neutral crate must not depend on product crate `{dependency}`"
                )

    aligned = {version for version in versions.values() if version}
    if len(aligned) > 1:
        problems.append(
            "neutral crates disagree on version: "
            + ", ".join(f"{name}={versions[name]}" for name in sorted(versions))
        )

    found_cycle = cycle(set(neutral), edges)
    if found_cycle:
        problems.append("neutral dependency cycle: " + " -> ".join(found_cycle))

    for name, manifest in found.items():
        if is_neutral(name) or not name.startswith(PRODUCT_PREFIXES):
            continue
        document = tomllib.loads(manifest.read_text(encoding="utf-8"))
        if document.get("package", {}).get("publish") is not False:
            problems.append(f"{manifest}: product crate must set `publish = false`")

    return problems, publish_order(set(neutral), edges)


def self_test() -> int:
    import tempfile

    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        (root / "crates/one/src").mkdir(parents=True)
        (root / "crates/two/src").mkdir(parents=True)
        (root / "crates/prod/src").mkdir(parents=True)
        (root / "Cargo.toml").write_text(
            '[workspace]\nmembers = []\n[workspace.package]\nversion = "1.0.0"\n', encoding="utf-8"
        )
        for crate, body in (
            ("one", '[package]\nname = "lurker-one"\nversion.workspace = true\n'),
            ("two", '[package]\nname = "lurker-two"\nversion.workspace = true\n'),
            ("prod", '[package]\nname = "infiltrator-prod"\nversion.workspace = true\npublish = false\n'),
        ):
            (root / f"crates/{crate}/Cargo.toml").write_text(body, encoding="utf-8")
        (root / "crates/two/Cargo.toml").write_text(
            '[package]\nname = "lurker-two"\nversion.workspace = true\n'
            '[dependencies]\nlurker-one = { path = "../one", version = "1.0.0" }\n',
            encoding="utf-8",
        )
        problems, order = audit(root)
        assert problems == [], problems
        assert order.index("lurker-one") < order.index("lurker-two"), order

        (root / "crates/two/Cargo.toml").write_text(
            '[package]\nname = "lurker-two"\nversion.workspace = true\n'
            '[dependencies]\nlurker-one = { path = "../one" }\n',
            encoding="utf-8",
        )
        problems, _ = audit(root)
        assert any("explicit `version`" in problem for problem in problems), problems

        (root / "crates/prod/Cargo.toml").write_text(
            '[package]\nname = "infiltrator-prod"\nversion.workspace = true\n', encoding="utf-8"
        )
        problems, _ = audit(root)
        assert any("publish = false" in problem for problem in problems), problems
    print("publish-order guard self-test: PASS")
    return 0


def main() -> int:
    if "--self-test" in sys.argv:
        return self_test()
    problems, order = audit()
    if problems:
        print("\n".join(problems), file=sys.stderr)
        return 1
    print("publish order: " + " -> ".join(order))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
