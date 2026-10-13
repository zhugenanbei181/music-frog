#!/usr/bin/env python3
"""Require a typed support declaration for every CoreOperation per engine adapter."""

from __future__ import annotations

import json
from pathlib import Path
import re
import sys
import tomllib


REPO_ROOT = Path(__file__).resolve().parents[2]
CORE_OPERATIONS_SOURCE = Path("crates/infiltrator-contract/src/core_capabilities.rs")
ENGINE_ADAPTERS = {
    "mihomo": {
        "package": "mihomo-api",
        "declaration": Path(
            "crates/mihomo-api/adapter-operation-declarations.json"
        ),
        "evidence": Path("packaging/mihomo-evidence-assets.json"),
    },
    "sing-box": {
        "package": "singbox-api",
        "declaration": Path(
            "crates/singbox-api/adapter-operation-declarations.json"
        ),
        "evidence": Path("packaging/sing-box-evidence-assets.json"),
    },
}
# Supported is an engine-backed claim, so require both current production
# composition and the matching evidence ID in that engine's target-scoped
# lock. Low-level methods or fixture implementations do not establish this.
PRODUCTION_COMPOSED_SUPPORTED = {
    "mihomo": {
        ("outbound", "list"),
        ("outbound", "select"),
        ("outbound", "probe"),
        ("logs", "stream"),
        ("logs", "set_level"),
        ("proxy_control", "get_routing_intent"),
        ("proxy_control", "set_routing_intent"),
    },
    "sing-box": set(),
}
# EvidenceOpen is reserved for operations that have a current production
# adapter/owner composition but still lack real binary evidence. These are
# intentionally a narrow audit allowlist rather than a claim inferred from
# a low-level method or a test implementation. Unsupported operations can
# move into this set only when production composition is added.
PRODUCTION_COMPOSED_EVIDENCE_OPEN = {
    "mihomo": {
        ("profile", "apply_feature"),
        ("dns", "apply_configuration"),
        ("dns", "query"),
        ("rules", "trace"),
        ("rules", "apply"),
        ("subscription", "refresh"),
        ("subscription", "import"),
        ("subscription", "update"),
        ("provider", "list_rule"),
        ("provider", "update_rule"),
        ("connection", "close"),
        ("geo", "update_databases"),
        ("fake_ip_cache", "flush"),
        ("tun", "configure"),
        ("core_garbage_collection", "trigger"),
    },
    "sing-box": set(),
}
REGISTERED_BINARY_TARGETS = {
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "x86_64-unknown-linux-musl",
    "aarch64-unknown-linux-musl",
    "x86_64-pc-windows-msvc",
    "aarch64-pc-windows-msvc",
    "x86_64-pc-windows-gnu",
    "x86_64-pc-windows-gnullvm",
    "aarch64-pc-windows-gnullvm",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    "x86_64-linux-android",
    "aarch64-linux-android",
    "x86_64-apple-ios",
    "aarch64-apple-ios",
    "aarch64-apple-ios-sim",
    "aarch64-apple-ios-macabi",
}


def camel_to_snake(value: str) -> str:
    first = re.sub(r"(.)([A-Z][a-z]+)", r"\1_\2", value)
    return re.sub(r"([a-z0-9])([A-Z])", r"\1_\2", first).lower()


def core_operation_catalog(source: str) -> tuple[list[dict[str, str]], list[str]]:
    source = re.sub(r"/\*.*?\*/", "", source, flags=re.S)
    source = re.sub(r"//[^\n]*", "", source)
    marker = "pub const CORE_OPERATIONS: &[CoreOperation] = &["
    if marker not in source:
        return [], ["CoreOperation::all catalogue declaration was not found"]
    body = source.split(marker, 1)[1].split("];", 1)[0]
    typed_operations = re.findall(
        r"CoreOperation::([A-Za-z][A-Za-z0-9_]*)\s*\(\s*"
        r"[A-Za-z][A-Za-z0-9_]*::([A-Za-z][A-Za-z0-9_]*)\s*\)",
        body,
    )
    if not typed_operations:
        return [], ["CoreOperation::all catalogue has no typed operations"]
    catalog = [
        {"category": camel_to_snake(category), "operation": camel_to_snake(operation)}
        for category, operation in typed_operations
    ]
    keys = [operation_key(item) for item in catalog]
    if len(keys) != len(set(keys)):
        return [], ["CoreOperation::all catalogue contains duplicate operations"]
    return catalog, []


def operation_key(operation: object) -> tuple[str, str] | None:
    if not isinstance(operation, dict) or set(operation) != {"category", "operation"}:
        return None
    category = operation.get("category")
    name = operation.get("operation")
    if not isinstance(category, str) or not isinstance(name, str):
        return None
    if not category or not name:
        return None
    return category, name


def evidence_operation_id(operation: tuple[str, str]) -> str:
    category, name = operation
    if category == "proxy_control" and name == "get_routing_intent":
        return "routing.get_intent"
    if category == "proxy_control" and name == "set_routing_intent":
        return "routing.set_intent"
    return f"{category}.{name}"


def owner_for_operation(operation: tuple[str, str]) -> str | None:
    category, name = operation
    if category == "lifecycle":
        return "core_lifecycle"
    if category == "version" and name in {"inspect", "rollback"}:
        return "version_management"
    return None


def manifest_evidence_ids(manifest: object) -> tuple[set[str], list[str]]:
    if not isinstance(manifest, dict) or manifest.get("schema_version") != 2:
        return set(), ["evidence lock must be an object with target-scoped schema_version 2"]
    assets = manifest.get("assets")
    if not isinstance(assets, list) or not assets:
        return set(), ["evidence lock must contain a non-empty assets array"]

    registered: set[str] = set()
    problems: list[str] = []
    seen_scopes: set[tuple[str, str, int]] = set()
    for index, asset in enumerate(assets):
        if not isinstance(asset, dict):
            problems.append(f"evidence asset {index} must be an object")
            continue
        version = asset.get("version")
        if not isinstance(version, str) or not version.strip():
            problems.append(f"evidence asset {index} has no version")
        binary_target = asset.get("binary_target")
        if binary_target not in REGISTERED_BINARY_TARGETS:
            problems.append(f"evidence asset {index} has an unregistered binary_target")
        adapter_revision = asset.get("adapter_revision")
        if type(adapter_revision) is not int or adapter_revision <= 0:
            problems.append(f"evidence asset {index} has no positive adapter_revision")
        elif isinstance(version, str) and binary_target in REGISTERED_BINARY_TARGETS:
            scope = (version, binary_target, adapter_revision)
            if scope in seen_scopes:
                problems.append(f"evidence asset {index} duplicates a version/target/revision scope")
            seen_scopes.add(scope)

        operations = asset.get("operations")
        if not isinstance(operations, list) or any(
            not isinstance(operation, str) or not operation.strip()
            for operation in operations
        ):
            problems.append(f"evidence asset {index} operations must be typed IDs")
            continue
        if len(operations) != len(set(operations)):
            problems.append(f"evidence asset {index} repeats an operation evidence ID")
            continue
        registered.update(operations)
    if not assets:
        return set(), problems or ["evidence lock has no valid assets"]
    return registered, problems


def validate_declaration(
    engine: str,
    declaration: object,
    catalog: list[dict[str, str]],
    manifest: object,
) -> list[str]:
    problems: list[str] = []
    where = f"{engine} adapter declaration"
    if not isinstance(declaration, dict):
        return [f"{where} must be an object"]
    if declaration.get("schema_version") != 1:
        problems.append(f"{where} must use schema_version 1")
    if declaration.get("engine") != engine:
        problems.append(f"{where} engine must be `{engine}`")
    if isinstance(manifest, dict) and manifest.get("engine") != engine:
        problems.append(f"{where}: evidence lock engine must be `{engine}`")
    if set(declaration) != {"schema_version", "engine", "operations"}:
        problems.append(f"{where} has unexpected or missing top-level fields")

    registered_ids, manifest_problems = manifest_evidence_ids(manifest)
    problems.extend(f"{where}: {issue}" for issue in manifest_problems)

    expected = {operation_key(item) for item in catalog}
    records = declaration.get("operations")
    if not isinstance(records, list):
        return problems + [f"{where} operations must be an array"]

    seen: set[tuple[str, str]] = set()
    for index, record in enumerate(records):
        record_where = f"{where} operation {index}"
        if not isinstance(record, dict) or set(record) != {"operation", "support"}:
            problems.append(
                f"{record_where} must contain exactly a typed operation and support variant"
            )
            continue
        operation = operation_key(record.get("operation"))
        if operation is None:
            problems.append(
                f"{record_where} operation must match the CoreOperation tagged enum shape"
            )
            continue
        if operation not in expected:
            problems.append(f"{record_where} is not in CoreOperation::all")
        if operation in seen:
            problems.append(f"{record_where} duplicates {operation[0]}.{operation[1]}")
        seen.add(operation)

        support = record.get("support")
        if not isinstance(support, dict):
            problems.append(f"{record_where} support must be a tagged enum object")
            continue
        status = support.get("status")
        if not isinstance(status, str):
            problems.append(f"{record_where} support status must be an enum string")
            continue
        if engine == "sing-box" and status in {"supported", "evidence_open"}:
            problems.append(
                f"{record_where} has no production EngineBackendFactory composition; use Unsupported"
            )
        if status == "supported":
            if (
                engine in PRODUCTION_COMPOSED_SUPPORTED
                and operation not in PRODUCTION_COMPOSED_SUPPORTED[engine]
            ):
                problems.append(
                    f"{record_where} has no audited production composition for Supported; use Unsupported or EvidenceOpen"
                )
            if set(support) != {"status", "evidence_id"}:
                problems.append(
                    f"{record_where} Supported requires exactly one registered evidence_id"
                )
                continue
            evidence_id = support.get("evidence_id")
            if not isinstance(evidence_id, str) or not evidence_id.strip():
                problems.append(f"{record_where} Supported evidence_id must be non-empty")
                continue
            if operation is not None and evidence_id != evidence_operation_id(operation):
                problems.append(
                    f"{record_where} evidence_id does not match its typed operation"
                )
            if evidence_id not in registered_ids:
                problems.append(
                    f"{record_where} evidence_id `{evidence_id}` is not registered in the engine lock"
                )
        elif status == "owner_managed":
            if set(support) != {"status", "owner"}:
                problems.append(f"{record_where} owner_managed requires exactly one typed owner")
                continue
            owner = support.get("owner")
            expected_owner = owner_for_operation(operation) if operation is not None else None
            if expected_owner is None:
                problems.append(
                    f"{record_where} operation has no owner-managed authority"
                )
            elif owner != expected_owner:
                problems.append(
                    f"{record_where} owner `{owner}` does not match `{expected_owner}`"
                )
        elif status in {"unsupported", "evidence_open"}:
            if set(support) != {"status", "reason"}:
                problems.append(
                    f"{record_where} {status} requires exactly one reason"
                )
                continue
            reason = support.get("reason")
            if not isinstance(reason, str) or not reason.strip():
                problems.append(f"{record_where} {status} reason must be non-empty")
            if (
                status == "evidence_open"
                and engine in PRODUCTION_COMPOSED_EVIDENCE_OPEN
                and operation not in PRODUCTION_COMPOSED_EVIDENCE_OPEN[engine]
            ):
                problems.append(
                    f"{record_where} has no audited production composition for EvidenceOpen; use Unsupported"
                )
        else:
            problems.append(
                f"{record_where} support status must be supported, owner_managed, unsupported, or evidence_open"
            )

    missing = expected - seen
    if missing:
        problems.append(
            f"{where} omits CoreOperation entries: "
            + ", ".join(f"{category}.{name}" for category, name in sorted(missing))
        )
    return problems


def discovered_adapter_packages(repo_root: Path) -> tuple[set[str], list[str]]:
    packages: set[str] = set()
    problems: list[str] = []
    known_packages = {
        value["package"] for value in ENGINE_ADAPTERS.values()
    }
    for manifest_path in sorted((repo_root / "crates").glob("*/Cargo.toml")):
        try:
            manifest = tomllib.loads(manifest_path.read_text(encoding="utf-8"))
        except (OSError, tomllib.TOMLDecodeError) as error:
            problems.append(f"{manifest_path}: cannot read package manifest: {error}")
            continue
        package = manifest.get("package")
        name = package.get("name") if isinstance(package, dict) else None
        if not isinstance(name, str):
            continue
        if name.endswith("-adapter") or name in known_packages:
            packages.add(name)
    return packages, problems


def unregistered_adapter_packages(packages: set[str]) -> list[str]:
    registered = {value["package"] for value in ENGINE_ADAPTERS.values()}
    return sorted(packages - registered)


def load_json(path: Path) -> object:
    return json.loads(path.read_text(encoding="utf-8"))


def source_problems(repo_root: Path = REPO_ROOT) -> list[str]:
    problems: list[str] = []
    catalog_path = repo_root / CORE_OPERATIONS_SOURCE
    if not catalog_path.is_file():
        return [f"{catalog_path}: CoreOperation catalogue source is missing"]
    catalog, catalog_problems = core_operation_catalog(
        catalog_path.read_text(encoding="utf-8")
    )
    problems.extend(f"{catalog_path}: {problem}" for problem in catalog_problems)

    packages, package_problems = discovered_adapter_packages(repo_root)
    problems.extend(package_problems)
    expected_packages = {value["package"] for value in ENGINE_ADAPTERS.values()}
    for package in unregistered_adapter_packages(packages):
        problems.append(
            f"crates/{package}: engine adapter has no registered declaration mapping"
        )
    for package in sorted(expected_packages - packages):
        problems.append(f"crates/{package}: registered engine adapter package is missing")

    for engine, spec in ENGINE_ADAPTERS.items():
        declaration_path = repo_root / spec["declaration"]
        evidence_path = repo_root / spec["evidence"]
        if not declaration_path.is_file():
            problems.append(f"{declaration_path}: engine adapter declaration is missing")
            continue
        if not evidence_path.is_file():
            problems.append(f"{evidence_path}: engine evidence lock is missing")
            continue
        try:
            declaration = load_json(declaration_path)
            evidence = load_json(evidence_path)
        except (OSError, json.JSONDecodeError) as error:
            problems.append(f"{declaration_path}: declaration/evidence JSON is invalid: {error}")
            continue
        problems.extend(
            f"{declaration_path}: {problem}"
            for problem in validate_declaration(engine, declaration, catalog, evidence)
        )
        if engine == "mihomo" and ("logs", "stream") in PRODUCTION_COMPOSED_SUPPORTED[engine]:
            scoped_runtime_path = repo_root / "crates/infiltrator-desktop/src/runtime/ports.rs"
            mihomo_stream_path = repo_root / "crates/mihomo-api/src/runtime_stream.rs"
            try:
                scoped_runtime = scoped_runtime_path.read_text(encoding="utf-8")
                mihomo_stream = mihomo_stream_path.read_text(encoding="utf-8")
            except OSError as error:
                problems.append(f"Mihomo scoped log stream production source is unavailable: {error}")
            else:
                for required in (
                    "impl RuntimeGateway for MihomoRuntime",
                    "fn runtime_stream_port(&self) -> Arc<dyn RuntimeStreamPort>",
                    "Arc::new(MihomoRuntimeStreamPort::new(",
                ):
                    if required not in scoped_runtime:
                        problems.append(
                            f"{scoped_runtime_path}: supported logs.stream lost `{required}`"
                        )
                if "async fn logs(" not in mihomo_stream:
                    problems.append(f"{mihomo_stream_path}: supported logs.stream has no typed stream method")
                if mihomo_stream.count("self.validate_scope(&scope, Capability::Logs)?;") < 2:
                    problems.append(
                        f"{mihomo_stream_path}: supported logs.stream must validate scope before and after opening"
                    )
                for required in (
                    ".stream_logs_events(level.as_deref())",
                    "Ok(scope_stream(",
                    "Ok(active_scope) if active_scope == scope =>",
                ):
                    if required not in mihomo_stream:
                        problems.append(
                            f"{mihomo_stream_path}: supported logs.stream lost `{required}`"
                        )
    return problems


def main() -> int:
    problems = source_problems()
    if problems:
        print("adapter operation declaration guard: FAILED", file=sys.stderr)
        for problem in problems:
            print(f" - {problem}", file=sys.stderr)
        return 1
    print("adapter operation declaration guard: all engines declare the full typed CoreOperation set")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
