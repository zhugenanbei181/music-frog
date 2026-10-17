#!/usr/bin/env python3
"""Guard the dependency boundaries introduced by the 0.30 core split.

The guard checks two things that are easy to regress during a large migration:

* domain/contract/ports/application do not acquire concrete UI, host, or
  transport dependencies;
* UI and engine-neutral layers do not depend on concrete backend adapters.

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

# Neutral framework crates must stay usable from every engine, platform and UI.
# Any product/engine crate, runtime, transport or UI toolkit would break that.
LURKER_NEUTRAL_CRATES = ("lurker-core-contract", "lurker-core-ports")
NEUTRAL_FORBIDDEN_EXACT = {"tokio", "reqwest", "bevy", "iced", "slint"}
NEUTRAL_FORBIDDEN_PREFIXES = ("infiltrator-", "mihomo-", "singbox-")

ENGINE_ADAPTER_CRATES = {
    "mihomo-api",
    "mihomo-config",
    "mihomo-process",
    "mihomo-version",
    "singbox-api",
    "singbox-config",
    "singbox-process",
    "singbox-version",
}

FORBIDDEN_DIRECT = {
    "infiltrator-domain": {
        "tokio",
        "reqwest",
        "bevy",
        "iced",
        "mihomo-api",
        "mihomo-config",
        "infiltrator-platform",
        "mihomo-version",
        "infiltrator-core",
        "infiltrator-application",
        "infiltrator-desktop",
        "infiltrator-android",
        "infiltrator-ios",
        *ENGINE_ADAPTER_CRATES,
    },
    "infiltrator-contract": {
        "tokio",
        "reqwest",
        "bevy",
        "iced",
        "mihomo-api",
        "mihomo-config",
        "infiltrator-platform",
        "mihomo-version",
        "infiltrator-core",
        "infiltrator-application",
        "infiltrator-desktop",
        "infiltrator-android",
        "infiltrator-ios",
        *ENGINE_ADAPTER_CRATES,
    },
    "infiltrator-ports": {
        "tokio",
        "reqwest",
        "bevy",
        "iced",
        "mihomo-api",
        "mihomo-config",
        "infiltrator-platform",
        "mihomo-version",
        "infiltrator-core",
        "infiltrator-application",
        "infiltrator-desktop",
        "infiltrator-android",
        "infiltrator-ios",
        *ENGINE_ADAPTER_CRATES,
    },
    "infiltrator-application": {
        "tokio",
        "reqwest",
        "bevy",
        "iced",
        "infiltrator-presentation",
        "mihomo-api",
        "mihomo-config",
        "infiltrator-platform",
        "mihomo-version",
        "infiltrator-core",
        "infiltrator-desktop",
        "infiltrator-android",
        "infiltrator-ios",
        *ENGINE_ADAPTER_CRATES,
    },
    "infiltrator-presentation": {
        "tokio",
        "reqwest",
        "bevy",
        "iced",
        "infiltrator-ports",
        "infiltrator-application",
        "infiltrator-composition",
        "infiltrator-platform",
        "infiltrator-core",
        "infiltrator-desktop",
        "infiltrator-android",
        "infiltrator-ios",
        "mihomo-version",
        *ENGINE_ADAPTER_CRATES,
    },
    "infiltrator-bevy-ui": {
        "tokio",
        "reqwest",
        "mihomo-api",
        "mihomo-config",
        "infiltrator-platform",
        "mihomo-version",
        "infiltrator-core",
        *ENGINE_ADAPTER_CRATES,
    },
    "infiltrator-iced": {
        "reqwest",
        "mihomo-api",
        "mihomo-config",
        "infiltrator-platform",
        "infiltrator-core",
        *ENGINE_ADAPTER_CRATES,
    },
    "infiltrator-bevy-widgets": {
        "tokio",
        "reqwest",
        "iced",
        "mihomo-api",
        "mihomo-config",
        "infiltrator-platform",
        "mihomo-version",
        "infiltrator-core",
        "infiltrator-application",
        "infiltrator-desktop",
        "infiltrator-android",
        "infiltrator-ios",
        *ENGINE_ADAPTER_CRATES,
    },
}

NEUTRAL_SOURCE_PATTERN = re.compile(
    r"\b(?:tokio|reqwest|bevy|iced|slint"
    r"|infiltrator_[a-z0-9_]+|mihomo_[a-z0-9_]+|singbox_[a-z0-9_]+)::"
)

SOURCE_PATTERNS = {
    "lurker-core-contract": NEUTRAL_SOURCE_PATTERN,
    "lurker-core-ports": NEUTRAL_SOURCE_PATTERN,
    "infiltrator-domain": re.compile(
        r"\b(?:tokio|reqwest|bevy|iced|mihomo_(?:api|config|process|version)|"
        r"singbox_(?:api|config|process|version)|infiltrator_platform)::"
    ),
    "infiltrator-contract": re.compile(
        r"\b(?:tokio|reqwest|bevy|iced|mihomo_(?:api|config|process|version)|"
        r"singbox_(?:api|config|process|version)|infiltrator_platform)::"
    ),
    "infiltrator-ports": re.compile(
        r"\b(?:tokio|reqwest|bevy|iced|mihomo_(?:api|config|process|version)|"
        r"singbox_(?:api|config|process|version)|infiltrator_platform)::"
    ),
    "infiltrator-bevy-ui": re.compile(
        r"\b(?:tokio|reqwest|mihomo_(?:api|config|process|version)|"
        r"singbox_(?:api|config|process|version)|infiltrator_platform)::|\bMihomoClient\b"
    ),
    "infiltrator-iced": re.compile(
        r"\b(?:reqwest|mihomo_(?:api|config|process|version)|"
        r"singbox_(?:api|config|process|version)|infiltrator_platform|infiltrator_core)::|\bMihomoClient\b"
    ),
    "infiltrator-presentation": re.compile(
        r"\b(?:tokio|reqwest|bevy|iced|mihomo_(?:api|config|process|version)|"
        r"singbox_(?:api|config|process|version)|infiltrator_(?:ports|application|composition|platform|core|desktop|android|ios))::"
    ),
}

PROFILE_WORKSPACE_FIELDS = re.compile(
    r"\bpub\s+sidecar_bytes\s*:\s*Option\s*<\s*"
    r"(?:std\s*::\s*vec\s*::\s*)?Vec\s*<\s*u8\s*>\s*>"
)
PROFILE_PORT_CONCEPTS = re.compile(
    r"\bProfileOptions\b"
    r"|\b(?:[A-Za-z_][A-Za-z_0-9]*)?"
    r"(?:mixin|filter|yaml|yml|mihomo|clash)[A-Za-z_0-9]*\b"
    r"|\b(?:filter|filters|filtering)\b",
    re.IGNORECASE,
)
PROFILE_STORE_OPTIONS_API = re.compile(
    r"\b(?:async\s+)?fn\s+(?:load|save|delete)_options\b",
    re.IGNORECASE,
)


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
        dependencies.update(value["package"] for value in table.values()
                            if isinstance(value, dict) and isinstance(value.get("package"), str))
    return dependencies


def strip_comments(text: str) -> str:
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.S)
    return re.sub(r"//[^\n]*", "", text)


def strip_test_attributes(text: str) -> str:
    """Remove test-runner attributes before checking production imports.

    A `#[tokio::test]` attribute expands only in test builds; treating the
    attribute path itself as a production Tokio dependency is a false positive.
    Runtime calls such as `tokio::spawn` remain visible to the boundary check.
    """
    return re.sub(
        r"#\s*\[\s*tokio\s*::\s*test(?:\s*\([^]]*\))?\s*\]",
        "",
        text,
    )


def check_profile_ports_sources(
    workspace_source: str,
    store_source: str,
    workspace_path: pathlib.Path,
    store_path: pathlib.Path,
) -> list[str]:
    problems: list[str] = []
    clean_workspace = strip_comments(workspace_source)
    clean_store = strip_comments(store_source)

    for struct_name in ("ProfileWorkspace", "ProfileWorkspaceUpdate"):
        struct_match = re.search(
            rf"\bpub\s+struct\s+{struct_name}\s*\{{(?P<body>[^{{}}]*)\}}",
            clean_workspace,
        )
        if struct_match is None:
            problems.append(f"{workspace_path}: missing public `{struct_name}`")
        elif not PROFILE_WORKSPACE_FIELDS.search(struct_match.group("body")):
            problems.append(
                f"{workspace_path}: `{struct_name}` must carry opaque `sidecar_bytes: Option<Vec<u8>>`"
            )

    for source_path, clean in (
        (workspace_path, clean_workspace),
        (store_path, clean_store),
    ):
        if PROFILE_PORT_CONCEPTS.search(clean):
            problems.append(
                f"{source_path}: profile ports must not depend on engine-specific options, mixin/filter, YAML, or Mihomo/Clash concepts"
            )

    if PROFILE_STORE_OPTIONS_API.search(clean_store):
        problems.append(
            f"{store_path}: generic `ProfileStore` must not require an options persistence API"
        )
    return problems


def check(repo_root: pathlib.Path) -> list[str]:
    problems: list[str] = []
    peer_packages = {"infiltrator-iced", "infiltrator-bevy-ui", "infiltrator-bevy-widgets", "iced", "bevy"}
    shared_packages = {"infiltrator-domain", "infiltrator-contract", "infiltrator-ports", "infiltrator-application", "infiltrator-presentation", "infiltrator-composition"}
    for package in shared_packages:
        manifest = repo_root / "crates" / package / "Cargo.toml"
        document = tomllib.loads(manifest.read_text(encoding="utf-8"))
        for dependency in sorted(direct_dependencies(manifest) & peer_packages):
            problems.append(f"{manifest}: shared layer depends on peer renderer `{dependency}`")
        for feature, members in document.get("features", {}).items():
            if feature in peer_packages or feature.startswith(("ui-iced", "ui-bevy", "frontend-")) \
                    or any(member.split("/", 1)[0].removeprefix("dep:") in peer_packages for member in members):
                problems.append(f"{manifest}: frontend-identity feature `{feature}` violates peer neutrality")
        for source in (manifest.parent / "src").rglob("*.rs"):
            clean = strip_comments(source.read_text(encoding="utf-8"))
            if re.search(r'#\s*\[\s*cfg(?:_attr)?\s*\([^\]]*feature\s*=\s*"(?:iced|bevy|frontend-[^"]*|ui-iced[^"]*|ui-bevy[^"]*)"', clean):
                problems.append(f"{source}: frontend-identity compile branch in shared code")
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
            clean = strip_test_attributes(
                strip_comments(source.read_text(encoding="utf-8"))
            )
            if pattern.search(clean):
                problems.append(
                    f"{source}: concrete runtime/transport import violates the 0.30 boundary"
                )

    for package in LURKER_NEUTRAL_CRATES:
        manifest = repo_root / "crates" / package / "Cargo.toml"
        if not manifest.is_file():
            problems.append(f"{manifest}: neutral crate manifest is missing")
            continue
        for dependency in sorted(direct_dependencies(manifest)):
            forbidden = (
                dependency in NEUTRAL_FORBIDDEN_EXACT
                or dependency.startswith(NEUTRAL_FORBIDDEN_PREFIXES)
            )
            if forbidden:
                problems.append(
                    f"{manifest}: neutral crate must not depend on `{dependency}`"
                )
        source_root = manifest.parent / "src"
        for source in sorted(source_root.rglob("*.rs")):
            clean = strip_test_attributes(
                strip_comments(source.read_text(encoding="utf-8"))
            )
            if NEUTRAL_SOURCE_PATTERN.search(clean):
                problems.append(
                    f"{source}: neutral crate references a product/runtime/UI dependency"
                )

    ports_root = repo_root / "crates" / "infiltrator-ports" / "src"
    workspace_source = ports_root / "profile_workspace.rs"
    store_source = ports_root / "profile_store.rs"
    if not workspace_source.is_file() or not store_source.is_file():
        problems.append(f"{ports_root}: profile workspace/store port source is missing")
    else:
        problems.extend(
            check_profile_ports_sources(
                workspace_source.read_text(encoding="utf-8"),
                store_source.read_text(encoding="utf-8"),
                workspace_source,
                store_source,
            )
        )
    return problems


def self_test() -> int:
    domain_dependencies = direct_dependencies(
        REPO_ROOT / "crates/infiltrator-domain/Cargo.toml"
    )
    assert not domain_dependencies & FORBIDDEN_DIRECT["infiltrator-domain"]
    presentation_dependencies = direct_dependencies(
        REPO_ROOT / "crates/infiltrator-presentation/Cargo.toml"
    )
    assert not presentation_dependencies & FORBIDDEN_DIRECT["infiltrator-presentation"]
    assert not SOURCE_PATTERNS["infiltrator-domain"].search(
        strip_comments("// tokio::spawn\nuse serde::Serialize;")
    )
    assert SOURCE_PATTERNS["infiltrator-bevy-ui"].search(
        "use mihomo_api::client::MihomoClient;"
    )
    assert not SOURCE_PATTERNS["infiltrator-presentation"].search(
        "use infiltrator_domain::rules::RuleEntry;"
    )
    assert SOURCE_PATTERNS["infiltrator-presentation"].search(
        "use infiltrator_application::core_application::CoreApplication;"
    )
    assert not SOURCE_PATTERNS["infiltrator-ports"].search(
        strip_test_attributes("#[tokio::test(start_paused = true)] async fn test() {}")
    )
    assert SOURCE_PATTERNS["infiltrator-ports"].search(
        strip_test_attributes("use tokio::runtime::Builder;")
    )
    assert SOURCE_PATTERNS["infiltrator-iced"].search(
        "use infiltrator_core::settings_io;"
    )
    assert not NEUTRAL_SOURCE_PATTERN.search(
        "use lurker_core_contract::identity::EngineId;"
    )
    assert NEUTRAL_SOURCE_PATTERN.search("use tokio::runtime::Builder;")
    assert NEUTRAL_SOURCE_PATTERN.search("use mihomo_api::client::MihomoClient;")
    assert NEUTRAL_SOURCE_PATTERN.search("use infiltrator_contract::snapshot::CoreSnapshot;")
    for name in LURKER_NEUTRAL_CRATES:
        neutral_dependencies = direct_dependencies(
            REPO_ROOT / "crates" / name / "Cargo.toml"
        )
        assert not any(
            dependency in NEUTRAL_FORBIDDEN_EXACT
            or dependency.startswith(NEUTRAL_FORBIDDEN_PREFIXES)
            for dependency in neutral_dependencies
        ), (name, neutral_dependencies)
    ports_root = REPO_ROOT / "crates/infiltrator-ports/src"
    workspace_path = ports_root / "profile_workspace.rs"
    store_path = ports_root / "profile_store.rs"
    current_profile_ports = check_profile_ports_sources(
        workspace_path.read_text(encoding="utf-8"),
        store_path.read_text(encoding="utf-8"),
        workspace_path,
        store_path,
    )
    assert not current_profile_ports, current_profile_ports

    synthetic_workspace = """
        pub struct ProfileWorkspace { pub sidecar_bytes: Option<Vec<u8>> }
        pub struct ProfileWorkspaceUpdate { pub sidecar_bytes: Option<Vec<u8>> }
    """
    synthetic_store = "pub trait ProfileStore {}"
    assert not check_profile_ports_sources(
        synthetic_workspace,
        synthetic_store,
        pathlib.Path("positive/profile_workspace.rs"),
        pathlib.Path("positive/profile_store.rs"),
    )

    negative_workspace = """
        use crate::ProfileOptions;
        pub struct ProfileWorkspace { pub sidecar_bytes: Vec<u8> }
        pub struct ProfileWorkspaceUpdate {}
    """
    negative_workspace_problems = check_profile_ports_sources(
        negative_workspace,
        synthetic_store,
        pathlib.Path("negative/profile_workspace.rs"),
        pathlib.Path("negative/profile_store.rs"),
    )
    assert any("ProfileWorkspace` must carry" in item for item in negative_workspace_problems)
    assert any("ProfileWorkspaceUpdate` must carry" in item for item in negative_workspace_problems)
    assert any("profile ports must not depend" in item for item in negative_workspace_problems)

    for concept in (
        "MixinProfile",
        "FilterRule",
        "parse_yaml",
        "mihomo_api::Client",
        "ClashRuntime",
    ):
        negative_store = f"use {concept};"
        problems = check_profile_ports_sources(
            synthetic_workspace,
            negative_store,
            pathlib.Path("negative/profile_workspace.rs"),
            pathlib.Path("negative/profile_store.rs"),
        )
        assert any("profile ports must not depend" in item for item in problems), (
            concept,
            problems,
        )

    for operation in ("load_options", "save_options", "delete_options"):
        negative_store = f"pub trait ProfileStore {{ fn {operation}(&self); }}"
        problems = check_profile_ports_sources(
            synthetic_workspace,
            negative_store,
            pathlib.Path("negative/profile_workspace.rs"),
            pathlib.Path("negative/profile_store.rs"),
        )
        assert any("must not require an options persistence API" in item for item in problems), (
            operation,
            problems,
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
