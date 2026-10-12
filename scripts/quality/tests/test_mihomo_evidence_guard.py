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
    smoke = root / GUARD.SMOKE_SOURCE
    integration = root / GUARD.REAL_PROCESS_TEST
    profile_integration = root / GUARD.REAL_PROFILE_TEST
    core_apply = root / GUARD.CORE_APPLY_SOURCE
    core_profile_transaction = root / GUARD.CORE_PROFILE_TRANSACTION_SOURCE
    mihomo_client = root / GUARD.MIHOMO_CLIENT_SOURCE
    mihomo_api = root / GUARD.MIHOMO_API_SOURCE
    for path in (
        provider,
        smoke,
        integration,
        profile_integration,
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
        "declarations.support(operation);\n"
        "declarations.has_verified_support_for_scope(operation, version, binary_target, revision);\n"
        "declarations.has_registered_target(binary_target, revision);\n"
        "binary_target.host_kind();\n",
        encoding="utf-8",
    )
    smoke.write_text(
        "mihomo-evidence-assets.json archive_sha256 binary_sha256 curl gzip "
        "mihomo-evidence-guard.py cargo nextest run --test real_process_smoke "
        "--run-ignored only MIHOMO_REAL_CONTROLLER_URL "
        "MIHOMO_REAL_CONTROLLER_SECRET MIHOMO_REAL_VERSION "
        "MIHOMO_REAL_CONFIGS_ROOT MIHOMO_REAL_PROFILE_PATH -p infiltrator-core "
        "--test real_process_profile_smoke --run-ignored only",
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
        "MIHOMO_REAL_VERSION MIHOMO_REAL_CONFIGS_ROOT MIHOMO_REAL_PROFILE_PATH EndpointConfigReloader "
        "ProfileEndpointSource MihomoProfileConfigurationTransactionAdapter "
        "ProfileConfigurationTransactionPort ProfileConfigurationTransactionRequest "
        "ProfileConfigurationTransactionOutcome::Committed "
        "ProfileConfigurationTransactionOutcome::RolledBack "
        "ProfileConfigurationTransactionOutcome::RollbackFailed "
        "ProfileStore::load_active_workspace get_config log_level let restored = "
        "restored_config",
        encoding="utf-8",
    )
    return root


class MihomoEvidenceGuardTests(unittest.TestCase):
    def test_current_manifest_matches_product_asset_and_provider_wiring(self):
        self.assertEqual(GUARD.validate_documents(EVIDENCE, PRODUCT), [])
        self.assertEqual(GUARD.source_problems(ROOT), [])

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
            any("operations do not match the nine verified behaviors" in issue
                for issue in GUARD.validate_documents(changed, PRODUCT))
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
                "stream_logs_events StreamEvent::Connected",
            )
            self.assertEqual(GUARD.source_problems(root), [])

            provider = root / GUARD.PROVIDER_SOURCE
            provider.write_text(
                provider.read_text(encoding="utf-8").replace(
                    "declarations.support(operation);", ""
                ),
                encoding="utf-8",
            )
            self.assertTrue(
                any("validated typed declaration" in issue
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

    def test_real_process_test_must_stay_ignored_and_use_typed_ports(self):
        typed_test = """
            #[ignore = \"explicit smoke only\"]
            MihomoOutboundAdapter MihomoRoutingIntentAdapter
            OutboundPort RoutingIntentPort
            stream_logs_events StreamEvent::Connected
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
