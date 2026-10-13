"""Regression tests for the real Mihomo evidence lock and provider wiring."""

import copy
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[3]
QUALITY = ROOT / "scripts/quality"
SPEC = importlib.util.spec_from_file_location(
    "mihomo_evidence_guard",
    QUALITY / "mihomo-evidence-guard.py",
)
GUARD = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(GUARD)
EVIDENCE = json.loads((ROOT / GUARD.EVIDENCE_LOCK).read_text(encoding="utf-8"))
PRODUCT = json.loads((ROOT / GUARD.PRODUCT_LOCK).read_text(encoding="utf-8"))


def source_fixture(root: Path, real_process_test: str) -> Path:
    provider = root / GUARD.PROVIDER_SOURCE
    operation_declaration = root / GUARD.OPERATION_DECLARATION_SOURCE
    smoke = root / GUARD.SMOKE_SOURCE
    integration = root / GUARD.REAL_PROCESS_TEST
    profile_integration = root / GUARD.REAL_PROFILE_TEST
    tun_integration = root / GUARD.REAL_TUN_CONFIG_TEST
    rule_trace_integration = root / GUARD.REAL_RULE_TRACE_TEST
    profile_feature_integration = root / GUARD.REAL_PROFILE_FEATURE_TEST
    subscription_integration = root / GUARD.REAL_SUBSCRIPTION_TEST
    geo_update_integration = root / GUARD.REAL_GEO_UPDATE_TEST
    core_apply = root / GUARD.CORE_APPLY_SOURCE
    core_profile_transaction = root / GUARD.CORE_PROFILE_TRANSACTION_SOURCE
    mihomo_client = root / GUARD.MIHOMO_CLIENT_SOURCE
    mihomo_api = root / GUARD.MIHOMO_API_SOURCE
    for path in (
        provider,
        operation_declaration,
        smoke,
        integration,
        profile_integration,
        tun_integration,
        rule_trace_integration,
        profile_feature_integration,
        subscription_integration,
        geo_update_integration,
        core_apply,
        core_profile_transaction,
        mihomo_client,
        mihomo_api,
    ):
        path.parent.mkdir(parents=True, exist_ok=True)
    provider.write_text(
        'include_str!("../../../packaging/mihomo-evidence-assets.json");\n'
        'const MIHOMO_OPERATION_DECLARATION: &str = include_str!("../adapter-operation-declarations.json");\n'
        "ValidatedAdapterOperationDeclarations::validate(declaration, evidence_lock);\n"
        "declarations.engine_decision_for_scope(operation, version, binary_target, revision);\n"
        "declarations.has_registered_target(binary_target, revision);\n"
        "binary_target.host_kind();\n",
        encoding="utf-8",
    )
    operation_declaration.write_text(
        "pub fn engine_decision_for_scope(\n"
        "    match self.support(operation) {\n"
        "        self.has_verified_support_for_target(operation, target, revision);\n"
        "        self.has_verified_support_for_scope(operation, version, target, revision);\n"
        "    }\n"
        "    self.evidence_lock.contains_for_scope(version, target, revision);\n"
        "    self.evidence_lock.contains_for_target(target, revision);\n"
        "}\n",
        encoding="utf-8",
    )
    smoke.write_text(
        "mihomo-evidence-assets.json archive_sha256 binary_sha256 curl gzip "
        "mihomo-evidence-guard.py cargo nextest run --test real_process_smoke "
        "--run-ignored only log-level: debug MIHOMO_REAL_CONTROLLER_URL "
        "MIHOMO_REAL_CONTROLLER_SECRET MIHOMO_REAL_VERSION "
        "MIHOMO_REAL_BINARY MIHOMO_REAL_CONFIGS_ROOT MIHOMO_REAL_PROFILE_PATH -p infiltrator-core "
        "--test real_process_profile_smoke --run-ignored only "
        "-p infiltrator-composition --test mihomo_tun_configuration_real_process_smoke "
        "TUN stack typed apply/readback/restore kept the TUN disabled "
        "--test mihomo_profile_feature_real_process_smoke "
        "Profile.ApplyFeature IPv6 write/readback/restore kept the host TUN disabled "
        "--test mihomo_rule_trace_real_process_smoke "
        "read-only Rule.Trace resolved the live profile rule against Mihomo outbound evidence "
        "--test mihomo_subscription_real_process_smoke "
        "loopback subscription Import/UpdateProfile/UpdateAllSubscriptions and native profile checks "
        "--test mihomo_geo_update_real_process_smoke "
        "MIHOMO_REAL_GEO_FIXTURE_ORIGIN "
        "Geo update trigger was accepted with loopback geox URLs and a REJECT-default Mihomo profile "
        "geo-auto-update: false geosite: \"http://127.0.0.1:$probe_server_port/geosite.dat\" "
        "IP-CIDR,127.0.0.0/8,DIRECT,no-resolve MATCH,REJECT "
        "ipv6: false "
        "tun:\n  enable: false\n  stack: system "
        "rules:\n  - DOMAIN,core-smoke-active.test,CORE-SMOKE-HTTP\n  - RULE-SET,CORE-SMOKE-RULE,DIRECT "
        "MIHOMO_REAL_DNS_ALT_SERVER "
        "Profile, DNS and Rules transaction apply/readback/restore",
        encoding="utf-8",
    )
    core_apply.write_text("pub trait ConfigReloader { async fn reload(&self); }", encoding="utf-8")
    core_profile_transaction.write_text("MihomoProfileConfigurationTransactionAdapter", encoding="utf-8")
    mihomo_client.write_text("pub async fn reload_config(&self)", encoding="utf-8")
    mihomo_api.write_text("async fn reload_config(&self)", encoding="utf-8")
    integration.write_text(real_process_test, encoding="utf-8")
    profile_integration.write_text(
        "#[ignore = \"explicit profile smoke only\"] "
        "MIHOMO_REAL_CONTROLLER_URL MIHOMO_REAL_CONTROLLER_SECRET "
        "MIHOMO_REAL_VERSION MIHOMO_REAL_CONFIGS_ROOT MIHOMO_REAL_PROFILE_PATH "
        "MIHOMO_REAL_BINARY MIHOMO_REAL_DNS_ALT_SERVER EndpointConfigReloader "
        "ProfileEndpointSource MihomoProfileConfigurationTransactionAdapter "
        "profile_dns_rules_apply_readback_and_restore_use_the_live_mihomo_transaction "
        "ProfileConfigurationTransactionPort ProfileConfigurationTransactionRequest "
        "ProfileConfigurationTransactionOutcome::Committed "
        "ProfileConfigurationTransactionOutcome::RolledBack "
        "ProfileConfigurationTransactionOutcome::RollbackFailed "
        "ProfileStore::load_active_workspace get_config log_level let restored = "
        "restored_config ProfileConfigurationUpdate::DnsSettings "
        "dns_codec_readback DnsQueryPort::query core-smoke-dns-updated.test "
        "core-smoke-dns-restored.test 192.0.2.53 ProfileConfigurationUpdate::Rules "
        "rules_codec_readback get_rules() core-smoke-rule-apply.test",
        encoding="utf-8",
    )
    tun_integration.write_text(
        "#[ignore = \\\"explicit TUN real-process smoke only\\\"] "
        "RuntimeConfigurationApplication set_tun_stack(TunStack::Mixed) "
        "set_tun_stack(TunStack::System) ObservedValue::Reported(false) get_version()",
        encoding="utf-8",
    )
    rule_trace_integration.write_text(
        "#[ignore = \\\"explicit Rule.Trace real-process smoke only\\\"] "
        "RuleTracerApplication ProfileApplication MihomoOutboundAdapter "
        "RuleTraceOutboundFacts::Observed RuleTraceRuntimeMode::Live "
        "core-smoke-active.test CORE-SMOKE-HTTP "
        "the read-only rule-trace smoke does not compose rule override",
        encoding="utf-8",
    )
    profile_feature_integration.write_text(
        "#[ignore = \\\"explicit Profile.ApplyFeature real-process smoke only\\\"] "
        "RuntimeConfigurationApplication set_ipv6_routing(true) "
        "set_ipv6_routing(false) ObservedValue::Reported(false) get_version()",
        encoding="utf-8",
    )
    subscription_integration.write_text(
        "#[ignore] HttpSubscriptionSource::new .no_proxy() SubscriptionImportApplication "
        ".refresh_profile(&source, PROFILE_NAME) .refresh_all(&source, 1) "
        "SubscriptionUpdateOutcome::Updated MihomoProfileDocumentCodec "
        "native_check_candidate Matcher::Missing subscription-v2",
        encoding="utf-8",
    )
    geo_update_integration.write_text(
        "#[ignore] client.upgrade_geo() MIHOMO_REAL_GEO_FIXTURE_ORIGIN "
        "profile_source.contains(\"geo-auto-update: false\") every configured geox URL "
        "profile_source.contains(\"IP-CIDR,127.0.0.0/8,DIRECT,no-resolve\") "
        "profile_source.contains(\"MATCH,REJECT\")",
        encoding="utf-8",
    )
    return root


class MihomoEvidenceGuardTests(unittest.TestCase):
    def test_current_manifest_matches_product_asset_and_provider_wiring(self):
        self.assertEqual(GUARD.validate_documents(EVIDENCE, PRODUCT), [])
        self.assertEqual(GUARD.source_problems(ROOT), [])

    def test_gc_evidence_requires_the_startup_debug_precondition(self):
        with tempfile.TemporaryDirectory() as directory:
            root = source_fixture(
                Path(directory),
                "#[ignore] MihomoOutboundAdapter MihomoRoutingIntentAdapter "
                "OutboundPort RoutingIntentPort MIHOMO_REAL_CONTROLLER_URL "
                "MIHOMO_REAL_CONTROLLER_SECRET MIHOMO_REAL_VERSION "
                "runtime_stream_port() .logs(connection_scope.clone(), None) "
                "RuntimeStreamEvent::Connected "
                "MihomoRuntimeResourcePort .observe(connection_scope.clone()) "
                ".collect_garbage(connection_scope.clone(), before_bytes) "
                "GarbageCollectionOutcome::Collected after_bytes",
            )
            smoke = root / GUARD.SMOKE_SOURCE
            smoke.write_text(
                smoke.read_text(encoding="utf-8").replace("log-level: debug", "log-level: info"),
                encoding="utf-8",
            )
            self.assertTrue(
                any(
                    "must start Mihomo with log-level debug" in issue
                    for issue in GUARD.source_problems(root)
                )
            )

    def test_non_locked_version_and_prerelease_fail_closed(self):
        self.assertTrue(GUARD.is_canonical_release_version("v1.19.33"))
        self.assertNotIn("v1.19.33", GUARD.REQUIRED_VERSIONS)
        for version in ("v1.19.18-rc.1", "v1.19.32+build.1"):
            with self.subTest(version=version):
                self.assertFalse(GUARD.is_canonical_release_version(version))

        non_locked = copy.deepcopy(EVIDENCE)
        non_locked["assets"][1]["version"] = "v1.19.33"
        self.assertTrue(
            any(
                "exactly v1.19.18 and v1.19.32" in problem
                for problem in GUARD.validate_documents(non_locked, PRODUCT)
            )
        )

        prerelease = copy.deepcopy(EVIDENCE)
        prerelease["assets"][1]["version"] = "v1.19.18-rc.1"
        self.assertTrue(
            any(
                "non-canonical release version" in problem
                for problem in GUARD.validate_documents(prerelease, PRODUCT)
            )
        )

    def test_product_release_asset_must_match_evidence_asset_byte_lock(self):
        changed = copy.deepcopy(EVIDENCE)
        changed["assets"][0]["binary_sha256"] = "0" * 64
        problems = GUARD.validate_documents(changed, PRODUCT)
        self.assertTrue(
            any("binary_sha256 does not match packaging/mihomo-assets.json" in issue
                for issue in problems)
        )

    def test_unobserved_operation_cannot_be_added_to_the_evidence_lock(self):
        changed = copy.deepcopy(EVIDENCE)
        changed["assets"][0]["operations"].append("connection.list")
        self.assertTrue(
            any("operations do not match the twenty-four verified behaviors" in issue
                for issue in GUARD.validate_documents(changed, PRODUCT))
        )

        missing_tun_evidence = copy.deepcopy(EVIDENCE)
        missing_tun_evidence["assets"][0]["operations"].remove("tun.configure")
        self.assertTrue(
            any("operations do not match the twenty-four verified behaviors" in issue
                for issue in GUARD.validate_documents(missing_tun_evidence, PRODUCT))
        )

        missing_rule_trace = copy.deepcopy(EVIDENCE)
        missing_rule_trace["assets"][0]["operations"].remove("rules.trace")
        self.assertTrue(
            any("operations do not match the twenty-four verified behaviors" in issue
                for issue in GUARD.validate_documents(missing_rule_trace, PRODUCT))
        )

    def test_evidence_is_target_and_adapter_revision_scoped(self):
        wrong_target = copy.deepcopy(EVIDENCE)
        wrong_target["assets"][0]["binary_target"] = "aarch64-linux-android"
        self.assertTrue(
            any(
                "binary_target must match the locked x86_64-unknown-linux-gnu product target" in problem
                for problem in GUARD.validate_documents(wrong_target, PRODUCT)
            )
        )

        stale_revision = copy.deepcopy(EVIDENCE)
        stale_revision["assets"][0]["adapter_revision"] = 0
        self.assertTrue(
            any(
                "adapter_revision must be a positive integer" in problem
                for problem in GUARD.validate_documents(stale_revision, PRODUCT)
            )
        )

        duplicate_scope = copy.deepcopy(EVIDENCE)
        duplicate_scope["assets"][1] = copy.deepcopy(duplicate_scope["assets"][0])
        self.assertTrue(
            any(
                "duplicates a version/target/adapter revision scope" in problem
                for problem in GUARD.validate_documents(duplicate_scope, PRODUCT)
            )
        )

    def test_provider_must_consume_typed_declarations_and_target_evidence(self):
        with tempfile.TemporaryDirectory() as directory:
            root = source_fixture(
                Path(directory),
                "#[ignore] MihomoOutboundAdapter MihomoRoutingIntentAdapter "
                "OutboundPort RoutingIntentPort MIHOMO_REAL_CONTROLLER_URL "
                "MIHOMO_REAL_CONTROLLER_SECRET MIHOMO_REAL_VERSION "
                "runtime_stream_port() .logs(connection_scope.clone(), None) "
                "RuntimeStreamEvent::Connected "
                "MihomoRuntimeResourcePort .observe(connection_scope.clone()) "
                ".collect_garbage(connection_scope.clone(), before_bytes) "
                "GarbageCollectionOutcome::Collected after_bytes",
            )
            self.assertEqual(GUARD.source_problems(root), [])

            provider = root / GUARD.PROVIDER_SOURCE
            provider.write_text(
                provider.read_text(encoding="utf-8").replace(
                    "declarations.engine_decision_for_scope(", ""
                ),
                encoding="utf-8",
            )
            self.assertTrue(
                any("validated typed declaration's scoped resolver" in issue
                    for issue in GUARD.source_problems(root))
            )

            provider.write_text(
                provider.read_text(encoding="utf-8").replace(
                    "declarations.has_registered_target(binary_target, revision);", ""
                ),
                encoding="utf-8",
            )
            self.assertTrue(
                any("target/host mismatch" in issue
                    for issue in GUARD.source_problems(root))
            )

    def test_typed_declaration_resolver_must_keep_exact_scope_checks(self):
        with tempfile.TemporaryDirectory() as directory:
            root = source_fixture(Path(directory), "#[ignore]")
            declaration = root / GUARD.OPERATION_DECLARATION_SOURCE
            declaration.write_text(
                declaration.read_text(encoding="utf-8").replace(
                    "self.has_verified_support_for_scope(operation, version, target, revision);",
                    "",
                ),
                encoding="utf-8",
            )
            self.assertTrue(
                any("has_verified_support_for_scope" in issue
                    for issue in GUARD.source_problems(root))
            )

    def test_real_process_test_must_stay_ignored_and_use_typed_ports(self):
        typed_test = """
            #[ignore = \"explicit smoke only\"]
            MihomoOutboundAdapter MihomoRoutingIntentAdapter
            OutboundPort RoutingIntentPort
            runtime_stream_port() .logs(connection_scope.clone(), None)
            RuntimeStreamEvent::Connected
            MihomoRuntimeResourcePort .observe(connection_scope.clone())
            .collect_garbage(connection_scope.clone(), before_bytes)
            GarbageCollectionOutcome::Collected after_bytes
            DnsSettingsPatch
            MIHOMO_REAL_CONTROLLER_URL MIHOMO_REAL_CONTROLLER_SECRET MIHOMO_REAL_VERSION
        """
        with tempfile.TemporaryDirectory() as directory:
            root = source_fixture(Path(directory), typed_test)
            self.assertEqual(GUARD.source_problems(root), [])

            integration = root / GUARD.REAL_PROCESS_TEST
            integration.write_text(
                typed_test.replace('#[ignore = "explicit smoke only"]', ""),
                encoding="utf-8",
            )
            self.assertTrue(
                any("must be ignored in ordinary nextest runs" in issue
                    for issue in GUARD.source_problems(root))
            )

            profile_integration = root / GUARD.REAL_PROFILE_TEST
            profile_test = profile_integration.read_text(encoding="utf-8")
            profile_integration.write_text(
                profile_test.replace('#[ignore = "explicit profile smoke only"]', ""),
                encoding="utf-8",
            )
            self.assertTrue(
                any("real-process Profile test must be ignored" in issue
                    for issue in GUARD.source_problems(root))
            )

    def test_undocumented_live_preflight_and_untyped_force_switch_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = source_fixture(Path(directory), "#[ignore]")
            client = root / GUARD.MIHOMO_CLIENT_SOURCE
            client.write_text(
                client.read_text(encoding="utf-8")
                + "reload_config_with_"
                + "force(path, force) force="
                + "false",
                encoding="utf-8",
            )
            issues = GUARD.source_problems(root)
        self.assertTrue(any("unsupported Mihomo live preflight" in issue for issue in issues))
        self.assertTrue(any("untyped reload force switch" in issue for issue in issues))


if __name__ == "__main__":
    unittest.main()
