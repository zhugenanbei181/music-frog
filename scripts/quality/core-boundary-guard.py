#!/usr/bin/env python3
"""Guard the dependency boundaries introduced by the 0.30 core split.

The guard checks two things that are easy to regress during a large migration:

* domain/contract/ports/application do not acquire concrete UI, host, or
  transport dependencies;
* Bevy UI does not reintroduce direct Mihomo, Reqwest, or Tokio dependencies.

This is intentionally a small manifest/source guard, not a replacement for
`cargo tree`: it fails fast before a full build and records the architectural
law beside the code that enforces it.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys
import tomllib


REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]

FORBIDDEN_DIRECT = {
    "infiltrator-domain": {
        "tokio",
        "reqwest",
        "bevy",
        "iced",
        "mihomo-api",
        "mihomo-config",
        "mihomo-platform",
        "mihomo-version",
        "infiltrator-core",
        "infiltrator-application",
        "infiltrator-desktop",
        "infiltrator-android",
        "infiltrator-ios",
    },
    "infiltrator-contract": {
        "tokio",
        "reqwest",
        "bevy",
        "iced",
        "mihomo-api",
        "mihomo-config",
        "mihomo-platform",
        "mihomo-version",
        "infiltrator-core",
        "infiltrator-application",
        "infiltrator-desktop",
        "infiltrator-android",
        "infiltrator-ios",
    },
    "infiltrator-ports": {
        "tokio",
        "reqwest",
        "bevy",
        "iced",
        "mihomo-api",
        "mihomo-config",
        "mihomo-platform",
        "mihomo-version",
        "infiltrator-core",
        "infiltrator-application",
        "infiltrator-desktop",
        "infiltrator-android",
        "infiltrator-ios",
    },
    "infiltrator-application": {
        "tokio",
        "reqwest",
        "bevy",
        "iced",
        "mihomo-api",
        "mihomo-config",
        "mihomo-platform",
        "mihomo-version",
        "infiltrator-core",
        "infiltrator-desktop",
        "infiltrator-android",
        "infiltrator-ios",
    },
    "infiltrator-bevy-ui": {
        "tokio",
        "reqwest",
        "mihomo-api",
        "mihomo-config",
        "mihomo-platform",
        "mihomo-version",
        "infiltrator-core",
    },
    "infiltrator-iced": {
        "reqwest",
        "mihomo-api",
        "mihomo-config",
        "mihomo-platform",
        "infiltrator-core",
    },
    "infiltrator-bevy-widgets": {
        "tokio",
        "reqwest",
        "iced",
        "mihomo-api",
        "mihomo-config",
        "mihomo-platform",
        "mihomo-version",
        "infiltrator-core",
        "infiltrator-application",
        "infiltrator-desktop",
        "infiltrator-android",
        "infiltrator-ios",
    },
}

SOURCE_PATTERNS = {
    "infiltrator-domain": re.compile(
        r"\b(?:tokio|reqwest|bevy|iced|mihomo_(?:api|config|platform|version))\s*::"
    ),
    "infiltrator-contract": re.compile(
        r"\b(?:tokio|reqwest|bevy|iced|mihomo_(?:api|config|platform|version))\s*::"
    ),
    "infiltrator-ports": re.compile(
        r"\b(?:tokio|reqwest|bevy|iced|mihomo_(?:api|config|platform|version))\s*::"
    ),
    "infiltrator-bevy-ui": re.compile(
        r"\b(?:tokio|reqwest|mihomo_api)::|\bMihomoClient\b"
    ),
    "infiltrator-iced": re.compile(
        r"\b(?:reqwest|mihomo_api|mihomo_config|mihomo_platform|infiltrator_core)::|\bMihomoClient\b"
    ),
}


def collect_dependency_tables(value: object) -> list[dict[str, object]]:
    tables: list[dict[str, object]] = []
    if not isinstance(value, dict):
        return tables
    for key, child in value.items():
        if key == "dependencies" and isinstance(child, dict):
            tables.append(child)
        elif isinstance(child, dict):
            tables.extend(collect_dependency_tables(child))
    return tables


def direct_dependencies(manifest: pathlib.Path) -> set[str]:
    document = tomllib.loads(manifest.read_text(encoding="utf-8"))
    dependencies: set[str] = set()
    for table in collect_dependency_tables(document):
        dependencies.update(table)
    return dependencies


def strip_comments(text: str) -> str:
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.S)
    return re.sub(r"//[^\n]*", "", text)


def check(repo_root: pathlib.Path) -> list[str]:
    problems: list[str] = []
    for package, forbidden in FORBIDDEN_DIRECT.items():
        manifest = repo_root / "crates" / package / "Cargo.toml"
        if not manifest.is_file():
            problems.append(f"{manifest}: manifest is missing")
            continue
        found = direct_dependencies(manifest) & forbidden
        for dependency in sorted(found):
            problems.append(
                f"{manifest}: direct dependency `{dependency}` violates the 0.30 boundary"
            )

        pattern = SOURCE_PATTERNS.get(package)
        if pattern is None:
            continue
        source_root = manifest.parent / "src"
        for source in sorted(source_root.rglob("*.rs")):
            clean = strip_comments(source.read_text(encoding="utf-8"))
            if pattern.search(clean):
                problems.append(
                    f"{source}: concrete runtime/transport import violates the 0.30 boundary"
                )
    return problems


def self_test() -> int:
    domain_dependencies = direct_dependencies(
        REPO_ROOT / "crates/infiltrator-domain/Cargo.toml"
    )
    assert not domain_dependencies & FORBIDDEN_DIRECT["infiltrator-domain"]
    assert not SOURCE_PATTERNS["infiltrator-domain"].search(
        strip_comments("// tokio::spawn\nuse serde::Serialize;")
    )
    assert SOURCE_PATTERNS["infiltrator-bevy-ui"].search(
        "use mihomo_api::client::MihomoClient;"
    )
    assert SOURCE_PATTERNS["infiltrator-iced"].search(
        "use infiltrator_core::settings_io;"
    )
    print("core boundary guard self-test: PASS")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("report", "enforce"), default="report")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        return self_test()

    problems = check(REPO_ROOT)
    if problems:
        for problem in problems:
            print(f"VIOLATION [{args.mode}]: {problem}", file=sys.stderr)
        print(f"core boundary guard: violations={len(problems)}", file=sys.stderr)
        return 1 if args.mode == "enforce" else 0

    print("core boundary guard: violations=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
