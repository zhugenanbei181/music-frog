#!/usr/bin/env python3
"""Validate Mihomo real-binary evidence identity and provider wiring."""

from __future__ import annotations

import argparse
import copy
import json
from pathlib import Path
import re
import sys


REPO_ROOT = Path(__file__).resolve().parents[2]
EVIDENCE_LOCK = Path("packaging/mihomo-evidence-assets.json")
PRODUCT_LOCK = Path("packaging/mihomo-assets.json")
PROVIDER_SOURCE = Path("crates/mihomo-api/src/core_capability_provider.rs")
OPERATION_DECLARATION_SOURCE = Path(
    "crates/infiltrator-contract/src/adapter_operation_declaration.rs"
)
SMOKE_SOURCE = Path("scripts/core-backend-smoke-mihomo.sh")
REAL_PROCESS_TEST = Path("crates/mihomo-api/tests/real_process_smoke.rs")
REAL_PROFILE_TEST = Path("crates/infiltrator-core/tests/real_process_profile_smoke.rs")
REAL_TUN_CONFIG_TEST = Path(
    "crates/infiltrator-composition/tests/mihomo_tun_configuration_real_process_smoke.rs"
)
REAL_RULE_TRACE_TEST = Path(
    "crates/infiltrator-composition/tests/mihomo_rule_trace_real_process_smoke.rs"
)
REAL_PROFILE_FEATURE_TEST = Path(
    "crates/infiltrator-composition/tests/mihomo_profile_feature_real_process_smoke.rs"
)
CORE_APPLY_SOURCE = Path("crates/infiltrator-core/src/apply.rs")
CORE_PROFILE_TRANSACTION_SOURCE = Path("crates/infiltrator-core/src/profile_configuration_transaction.rs")
MIHOMO_CLIENT_SOURCE = Path("crates/mihomo-api/src/client.rs")
MIHOMO_API_SOURCE = Path("crates/mihomo-api/src/api.rs")
REQUIRED_VERSIONS = {"v1.19.18", "v1.19.32"}
REQUIRED_OPERATIONS = (
    "profile.validate",
    "profile.apply",
    "profile.apply_feature",
    "dns.apply_configuration",
    "dns.query",
    "rules.apply",
    "rules.trace",
    "connection.close",
    "fake_ip_cache.flush",
    "provider.list_rule",
    "provider.update_rule",
    "core_garbage_collection.trigger",
    "outbound.list",
    "outbound.select",
    "outbound.probe",
    "logs.set_level",
    "logs.stream",
    "routing.set_intent",
    "routing.get_intent",
    "tun.configure",
)
LINUX_AMD64_TARGET = "x86_64-unknown-linux-gnu"


def is_canonical_release_version(version: object) -> bool:
    if not isinstance(version, str) or re.fullmatch(r"v[0-9]+\.[0-9]+\.[0-9]+", version) is None:
        return False
    return all(
        component == "0" or not component.startswith("0")
        for component in version[1:].split(".")
    )


def validate_documents(evidence: object, product: object) -> list[str]:
    problems: list[str] = []
    if not isinstance(evidence, dict) or evidence.get("schema_version") != 2:
        return ["evidence lock must be an object with target-scoped schema_version 2"]
    if evidence.get("engine") != "mihomo":
        problems.append("evidence lock engine must be mihomo")

    assets = evidence.get("assets")
    if not isinstance(assets, list):
        return problems + ["evidence lock assets must be an array"]
    assets_by_version: dict[str, list[dict[str, object]]] = {}
    seen_scopes: set[tuple[str, str, int]] = set()
    for index, asset in enumerate(assets):
        where = f"evidence asset {index}"
        if not isinstance(asset, dict):
            problems.append(f"{where} must be an object")
            continue
        version = asset.get("version")
        if not is_canonical_release_version(version):
            problems.append(f"{where} has a non-canonical release version")
            continue
        assets_by_version.setdefault(version, []).append(asset)

        binary_target = asset.get("binary_target")
        if binary_target != LINUX_AMD64_TARGET:
            problems.append(
                f"{where} binary_target must match the locked {LINUX_AMD64_TARGET} product target until other targets are evidenced"
            )
        adapter_revision = asset.get("adapter_revision")
        if type(adapter_revision) is not int or adapter_revision <= 0:
            problems.append(f"{where} adapter_revision must be a positive integer")
        elif isinstance(binary_target, str):
            scope = (version, binary_target, adapter_revision)
            if scope in seen_scopes:
                problems.append(f"{where} duplicates a version/target/adapter revision scope")
            seen_scopes.add(scope)

        expected_release = f"https://github.com/MetaCubeX/mihomo/releases/tag/{version}"
        if asset.get("release_url") != expected_release:
            problems.append(f"{where} release_url does not identify the official release")
        if asset.get("archive") != f"mihomo-linux-amd64-v1-{version}.gz":
            problems.append(f"{where} archive does not match the official Linux amd64 asset name")
        if asset.get("binary") != "mihomo-linux-amd64":
            problems.append(f"{where} binary must be mihomo-linux-amd64")
        for digest_key in ("archive_sha256", "binary_sha256"):
            digest = asset.get(digest_key)
            if not isinstance(digest, str) or re.fullmatch(r"[0-9a-f]{64}", digest) is None:
                problems.append(f"{where} has an invalid {digest_key}")
        operations = asset.get("operations")
        if not isinstance(operations, list) or tuple(operations) != REQUIRED_OPERATIONS:
            problems.append(f"{where} operations do not match the twenty verified behaviors")

    if set(assets_by_version) != REQUIRED_VERSIONS:
        problems.append(
            "evidence lock must contain exactly v1.19.18 and v1.19.32 canonical releases"
        )

    if not isinstance(product, dict):
        problems.append("product Mihomo lock must be an object")
        return problems
    product_version = product.get("version")
    evidence_product_assets = [
        asset
        for asset in assets_by_version.get(product_version, [])
        if asset.get("binary_target") == LINUX_AMD64_TARGET
    ]
    if len(evidence_product_assets) != 1:
        problems.append("evidence lock must include the product lock's current version")
        evidence_product_asset = None
    else:
        evidence_product_asset = evidence_product_assets[0]
    product_assets = product.get("assets")
    if not isinstance(product_assets, list):
        problems.append("product Mihomo lock assets must be an array")
        return problems
    matching_product_assets = [
        item for item in product_assets
        if isinstance(item, dict) and item.get("target") == LINUX_AMD64_TARGET
    ]
    if len(matching_product_assets) != 1:
        problems.append("product lock must contain exactly one Linux amd64 asset")
    elif evidence_product_asset is not None:
        product_asset = matching_product_assets[0]
        for evidence_key, product_key in (
            ("archive", "archive"),
            ("archive_sha256", "archive_sha256"),
            ("binary", "binary"),
            ("binary_sha256", "binary_sha256"),
        ):
            if evidence_product_asset.get(evidence_key) != product_asset.get(product_key):
                problems.append(
                    f"evidence product asset {evidence_key} does not match packaging/mihomo-assets.json"
                )
    return problems


def load_json(path: Path) -> object:
    return json.loads(path.read_text(encoding="utf-8"))


def source_problems(repo_root: Path) -> list[str]:
    problems: list[str] = []
    provider_path = repo_root / PROVIDER_SOURCE
    operation_declaration_path = repo_root / OPERATION_DECLARATION_SOURCE
    smoke_path = repo_root / SMOKE_SOURCE
    real_process_test_path = repo_root / REAL_PROCESS_TEST
    real_profile_test_path = repo_root / REAL_PROFILE_TEST
    real_tun_config_test_path = repo_root / REAL_TUN_CONFIG_TEST
    real_rule_trace_test_path = repo_root / REAL_RULE_TRACE_TEST
    real_profile_feature_test_path = repo_root / REAL_PROFILE_FEATURE_TEST
    core_apply_path = repo_root / CORE_APPLY_SOURCE
    core_profile_transaction_path = repo_root / CORE_PROFILE_TRANSACTION_SOURCE
    mihomo_client_path = repo_root / MIHOMO_CLIENT_SOURCE
    mihomo_api_path = repo_root / MIHOMO_API_SOURCE
    if not provider_path.is_file():
        problems.append(f"{provider_path}: capability provider source is missing")
    if not operation_declaration_path.is_file():
        problems.append(
            f"{operation_declaration_path}: typed operation declaration resolver is missing"
        )
    if not smoke_path.is_file():
        problems.append(f"{smoke_path}: binary smoke source is missing")
    if not real_process_test_path.is_file():
        problems.append(f"{real_process_test_path}: typed adapter smoke test is missing")
    if not real_profile_test_path.is_file():
        problems.append(f"{real_profile_test_path}: real-process Profile transaction test is missing")
    if not real_tun_config_test_path.is_file():
        problems.append(f"{real_tun_config_test_path}: real-process TUN configuration test is missing")
    if not real_rule_trace_test_path.is_file():
        problems.append(f"{real_rule_trace_test_path}: real-process Rule.Trace test is missing")
    if not real_profile_feature_test_path.is_file():
        problems.append(f"{real_profile_feature_test_path}: real-process Profile.ApplyFeature test is missing")
    for source_path in (
        core_apply_path,
        core_profile_transaction_path,
        mihomo_client_path,
        mihomo_api_path,
    ):
        if not source_path.is_file():
            problems.append(f"{source_path}: live config transaction source is missing")
    if problems:
        return problems

    provider = provider_path.read_text(encoding="utf-8")
    operation_declaration = operation_declaration_path.read_text(encoding="utf-8")
    smoke = smoke_path.read_text(encoding="utf-8")
    real_process_test = real_process_test_path.read_text(encoding="utf-8")
    real_profile_test = real_profile_test_path.read_text(encoding="utf-8")
    real_tun_config_test = real_tun_config_test_path.read_text(encoding="utf-8")
    real_rule_trace_test = real_rule_trace_test_path.read_text(encoding="utf-8")
    real_profile_feature_test = real_profile_feature_test_path.read_text(encoding="utf-8")
    live_transaction_sources = {
        core_apply_path: core_apply_path.read_text(encoding="utf-8"),
        core_profile_transaction_path: core_profile_transaction_path.read_text(encoding="utf-8"),
        mihomo_client_path: mihomo_client_path.read_text(encoding="utf-8"),
        mihomo_api_path: mihomo_api_path.read_text(encoding="utf-8"),
    }
    if 'include_str!("../../../packaging/mihomo-evidence-assets.json")' not in provider:
        problems.append(f"{provider_path}: provider must read the evidence manifest at compile time")
    if (
        "MIHOMO_OPERATION_DECLARATION" not in provider
        or "ValidatedAdapterOperationDeclarations::validate" not in provider
        or "declarations.engine_decision_for_scope(" not in provider
    ):
        problems.append(
            f"{provider_path}: capability decisions must delegate to the validated typed declaration's scoped resolver"
        )
    for required in (
        "pub fn engine_decision_for_scope(",
        "match self.support(operation)",
        "self.has_verified_support_for_target(",
        "self.has_verified_support_for_scope(",
        "self.evidence_lock",
        "contains_for_scope(",
        "contains_for_target(",
    ):
        if required not in operation_declaration:
            problems.append(
                f"{operation_declaration_path}: scoped capability resolution must validate `{required}`"
            )
    if "has_registered_target" not in provider or "binary_target.host_kind()" not in provider:
        problems.append(
            f"{provider_path}: capabilities must fail closed for target/host mismatch or unregistered target evidence"
        )
    for retired in (
        "has_exact_active_version_evidence",
        "has_host_target_evidence",
        "host_evidence_platform",
    ):
        if retired in provider:
            problems.append(f"{provider_path}: retired unscoped evidence helper `{retired}` remains")
    if "mihomo-evidence-assets.json" not in smoke:
        problems.append(f"{smoke_path}: smoke must load the evidence asset lock")
    for required in ("archive_sha256", "binary_sha256", "curl", "gzip"):
        if required not in smoke:
            problems.append(f"{smoke_path}: smoke must verify locked {required} data")
    if "log-level: debug" not in smoke:
        problems.append(
            f"{smoke_path}: CoreGarbageCollection.Trigger evidence must start Mihomo with log-level debug"
        )
    if "mihomo-evidence-guard.py" not in smoke:
        problems.append(f"{smoke_path}: smoke must run the dedicated evidence guard")
    for required in ("cargo nextest run", "--test real_process_smoke", "--run-ignored only"):
        if required not in smoke:
            problems.append(f"{smoke_path}: explicit Rust adapter smoke must use `{required}`")
    if "Profile, DNS and Rules transaction apply/readback/restore" not in smoke:
        problems.append(
            f"{smoke_path}: real-process Profile smoke must exercise typed DNS and Rules apply/readback/restore"
        )
    for required in (
        "-p infiltrator-composition",
        "--test mihomo_tun_configuration_real_process_smoke",
        "TUN stack typed apply/readback/restore kept the TUN disabled",
    ):
        if required not in smoke:
            problems.append(
                f"{smoke_path}: real-process TUN smoke must include `{required}`"
            )
    for required in (
        "-p infiltrator-composition",
        "--test mihomo_rule_trace_real_process_smoke",
        "read-only Rule.Trace resolved the live profile rule against Mihomo outbound evidence",
    ):
        if required not in smoke:
            problems.append(
                f"{smoke_path}: real-process Rule.Trace smoke must include `{required}`"
            )
    for required in (
        "-p infiltrator-composition",
        "--test mihomo_profile_feature_real_process_smoke",
        "Profile.ApplyFeature IPv6 write/readback/restore kept the host TUN disabled",
    ):
        if required not in smoke:
            problems.append(
                f"{smoke_path}: real-process Profile.ApplyFeature smoke must include `{required}`"
            )
    if "tun:\n  enable: false\n  stack: system" not in smoke:
        problems.append(
            f"{smoke_path}: TUN configuration evidence must keep the live TUN disabled"
        )
    if "ipv6: false" not in smoke:
        problems.append(
            f"{smoke_path}: Profile.ApplyFeature evidence must start from IPv6 disabled"
        )
    if "rules:\n  - DOMAIN,core-smoke-active.test,CORE-SMOKE-HTTP\n  - RULE-SET,CORE-SMOKE-RULE,DIRECT" not in smoke:
        problems.append(
            f"{smoke_path}: the Rule.Trace fixture must place the directly evaluable live rule before RULE-SET"
        )
    for required in (
        "MIHOMO_REAL_CONTROLLER_URL",
        "MIHOMO_REAL_CONTROLLER_SECRET",
        "MIHOMO_REAL_VERSION",
    ):
        if required not in smoke or required not in real_process_test:
            problems.append(
                f"{smoke_path}: typed adapter smoke must pass `{required}` to the ignored test"
            )
    if "#[ignore" not in real_process_test:
        problems.append(
            f"{real_process_test_path}: real-process test must be ignored in ordinary nextest runs"
        )
    if "#[ignore" not in real_tun_config_test:
        problems.append(
            f"{real_tun_config_test_path}: real-process test must be ignored in ordinary nextest runs"
        )
    for required in (
        "RuntimeConfigurationApplication",
        "set_tun_stack(TunStack::Mixed)",
        "set_tun_stack(TunStack::System)",
        "ObservedValue::Reported(false)",
        "get_version()",
    ):
        if required not in real_tun_config_test:
            problems.append(
                f"{real_tun_config_test_path}: real TUN evidence must exercise `{required}`"
            )
    if "#[ignore" not in real_rule_trace_test:
        problems.append(
            f"{real_rule_trace_test_path}: real-process test must be ignored in ordinary nextest runs"
        )
    for required in (
        "RuleTracerApplication",
        "ProfileApplication",
        "MihomoOutboundAdapter",
        "RuleTraceOutboundFacts::Observed",
        "RuleTraceRuntimeMode::Live",
        "core-smoke-active.test",
        "CORE-SMOKE-HTTP",
        "the read-only rule-trace smoke does not compose rule override",
    ):
        if required not in real_rule_trace_test:
            problems.append(
                f"{real_rule_trace_test_path}: real Rule.Trace evidence must exercise `{required}`"
            )
    if "#[ignore" not in real_profile_feature_test:
        problems.append(
            f"{real_profile_feature_test_path}: real-process test must be ignored in ordinary nextest runs"
        )
    for required in (
        "RuntimeConfigurationApplication",
        "set_ipv6_routing(true)",
        "set_ipv6_routing(false)",
        "ObservedValue::Reported(false)",
        "get_version()",
    ):
        if required not in real_profile_feature_test:
            problems.append(
                f"{real_profile_feature_test_path}: real Profile.ApplyFeature evidence must exercise `{required}`"
            )
    for required in (
        "MihomoOutboundAdapter",
        "MihomoRoutingIntentAdapter",
        "OutboundPort",
        "RoutingIntentPort",
        "stream_logs_events",
        "StreamEvent::Connected",
        "CoreGarbageCollection.Trigger",
        "only when log-level is debug at process startup",
    ):
        if required not in real_process_test:
            problems.append(
                f"{real_process_test_path}: smoke must drive typed adapter `{required}`"
            )
    for required in (
        "MIHOMO_REAL_CONTROLLER_URL",
        "MIHOMO_REAL_CONTROLLER_SECRET",
        "MIHOMO_REAL_VERSION",
        "MIHOMO_REAL_BINARY",
        "MIHOMO_REAL_CONFIGS_ROOT",
        "MIHOMO_REAL_PROFILE_PATH",
        "MIHOMO_REAL_DNS_ALT_SERVER",
        "EndpointConfigReloader",
        "ProfileEndpointSource",
        "MihomoProfileConfigurationTransactionAdapter",
        "profile_dns_rules_apply_readback_and_restore_use_the_live_mihomo_transaction",
        "ProfileConfigurationTransactionPort",
        "ProfileConfigurationTransactionRequest",
        "ProfileConfigurationTransactionOutcome::Committed",
        "ProfileConfigurationTransactionOutcome::RolledBack",
        "ProfileConfigurationTransactionOutcome::RollbackFailed",
        "ProfileStore::load_active_workspace",
        "get_config",
        "log_level",
        "let restored =",
        "restored_config",
        "ProfileConfigurationUpdate::DnsSettings",
        "ProfileConfigurationUpdate::Rules",
        "dns_codec_readback",
        "rules_codec_readback",
        "DnsQueryPort::query",
        "get_rules()",
        "core-smoke-dns-updated.test",
        "core-smoke-dns-restored.test",
        "core-smoke-rule-apply.test",
        "192.0.2.53",
    ):
        if required not in real_profile_test:
            problems.append(
                f"{real_profile_test_path}: real Profile evidence must exercise `{required}`"
            )
    if "MIHOMO_REAL_DNS_ALT_SERVER" not in smoke:
        problems.append(
            f"{smoke_path}: the locked profile smoke must supply its alternate DNS fixture"
        )
    retired_live_preflight = "preflight" + "_check"
    retired_force_switch = "reload_config_with_" + "force"
    retired_force_false = "force=" + "false"
    for source_path, source in live_transaction_sources.items():
        if retired_live_preflight in source or retired_force_false in source:
            problems.append(
                f"{source_path}: unsupported Mihomo live preflight path remains"
            )
        if source_path in (mihomo_client_path, mihomo_api_path) and retired_force_switch in source:
            problems.append(
                f"{source_path}: untyped reload force switch remains; expose only documented reload"
            )
    if "#[ignore" not in real_profile_test:
        problems.append(
            f"{real_profile_test_path}: real-process Profile test must be ignored in ordinary nextest runs"
        )
    for required in (
        "MIHOMO_REAL_CONTROLLER_URL",
        "MIHOMO_REAL_CONTROLLER_SECRET",
        "MIHOMO_REAL_VERSION",
        "MIHOMO_REAL_CONFIGS_ROOT",
        "MIHOMO_REAL_PROFILE_PATH",
        "-p infiltrator-core",
        "--test real_process_profile_smoke",
        "--run-ignored only",
    ):
        if required not in smoke:
            problems.append(
                f"{smoke_path}: locked real-process Profile smoke must pass `{required}`"
            )
    for source_path, source in (
        (provider_path, provider),
        (smoke_path, smoke),
        (real_profile_test_path, real_profile_test),
    ):
        if re.search(r"\bv1\.19\.(?:18|32)\b", source) or re.search(r"\b[0-9a-f]{64}\b", source):
            problems.append(f"{source_path}: version/hash literals must stay in the evidence manifest")
    return problems


def check(repo_root: Path) -> list[str]:
    try:
        evidence = load_json(repo_root / EVIDENCE_LOCK)
        product = load_json(repo_root / PRODUCT_LOCK)
    except (OSError, json.JSONDecodeError) as error:
        return [f"cannot load Mihomo evidence/product lock: {error}"]
    return validate_documents(evidence, product) + source_problems(repo_root)


def self_test(repo_root: Path) -> int:
    for version in sorted(REQUIRED_VERSIONS):
        assert is_canonical_release_version(version)
    for version in (
        "1.19.32",
        "v01.19.32",
        "v1.19.032",
        "v1.19.32-rc.1",
        "v1.19.32+build.1",
        "v1.19.33.1",
    ):
        assert not is_canonical_release_version(version), version

    evidence = load_json(repo_root / EVIDENCE_LOCK)
    product = load_json(repo_root / PRODUCT_LOCK)
    assert not validate_documents(evidence, product)

    unsupported = copy.deepcopy(evidence)
    unsupported["assets"][1]["version"] = "v1.19.33"
    assert any("exactly v1.19.18 and v1.19.32" in issue
               for issue in validate_documents(unsupported, product))

    prerelease = copy.deepcopy(evidence)
    prerelease["assets"][1]["version"] = "v1.19.18-rc.1"
    assert any("non-canonical release version" in issue
               for issue in validate_documents(prerelease, product))

    mismatched = copy.deepcopy(evidence)
    mismatched["assets"][0]["binary_sha256"] = "0" * 64
    assert any("binary_sha256 does not match packaging/mihomo-assets.json" in issue
               for issue in validate_documents(mismatched, product))
    print("Mihomo evidence guard self-test: PASS")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        return self_test(REPO_ROOT)

    problems = check(REPO_ROOT)
    if problems:
        for problem in problems:
            print(f"VIOLATION: {problem}", file=sys.stderr)
        print(f"Mihomo evidence guard: violations={len(problems)}", file=sys.stderr)
        return 1
    print("Mihomo evidence guard: valid")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
