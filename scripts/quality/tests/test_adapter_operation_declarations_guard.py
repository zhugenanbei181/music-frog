"""Positive and negative fixtures for engine operation declarations."""

import copy
import importlib.util
import json
from pathlib import Path
import shutil
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location(
    "adapter_operation_declarations_guard",
    ROOT / "scripts/quality/adapter-operation-declarations-guard.py",
)
GUARD = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(GUARD)


class AdapterOperationDeclarationGuardTests(unittest.TestCase):
    def setUp(self):
        self.catalog = [
            {"category": "profile", "operation": "validate"},
            {"category": "outbound", "operation": "list"},
        ]
        self.evidence = {
            "schema_version": 2,
            "engine": "fixture",
            "assets": [
                {
                    "version": "v1.0.0",
                    "binary_target": "x86_64-unknown-linux-gnu",
                    "adapter_revision": 1,
                    "operations": ["profile.validate"],
                },
                {
                    "version": "v1.0.1",
                    "binary_target": "x86_64-unknown-linux-gnu",
                    "adapter_revision": 1,
                    "operations": ["profile.validate"],
                },
            ],
        }
        self.declaration = {
            "schema_version": 1,
            "engine": "fixture",
            "operations": [
                {
                    "operation": self.catalog[0],
                    "support": {
                        "status": "supported",
                        "evidence_id": "profile.validate",
                    },
                },
                {
                    "operation": self.catalog[1],
                    "support": {
                        "status": "evidence_open",
                        "reason": "real operation evidence is open",
                    },
                },
            ],
        }

    def test_current_engine_declarations_cover_all_core_operations(self):
        self.assertEqual(GUARD.source_problems(ROOT), [])

    def test_scope_bound_mihomo_logs_stream_has_target_bound_real_evidence(self):
        spec = GUARD.ENGINE_ADAPTERS["mihomo"]
        declaration = GUARD.load_json(ROOT / spec["declaration"])
        evidence = GUARD.load_json(ROOT / spec["evidence"])
        stream = next(
            item
            for item in declaration["operations"]
            if item["operation"] == {"category": "logs", "operation": "stream"}
        )
        self.assertEqual(
            stream["support"],
            {"status": "supported", "evidence_id": "logs.stream"},
        )
        self.assertTrue(
            all("logs.stream" in asset["operations"] for asset in evidence["assets"])
        )

    def test_registered_evidence_and_explicit_open_status_are_valid(self):
        self.assertEqual(
            GUARD.validate_declaration(
                "fixture", self.declaration, self.catalog, self.evidence
            ),
            [],
        )

    def test_known_engine_evidence_open_requires_audited_production_composition(self):
        cases = [
            ("mihomo", "dns", "query", True),
            ("mihomo", "dns", "read_configuration", False),
            ("sing-box", "outbound", "list", False),
        ]
        for engine, category, name, production_composed in cases:
            operation = {"category": category, "operation": name}
            declaration = {
                "schema_version": 1,
                "engine": engine,
                "operations": [
                    {
                        "operation": operation,
                        "support": {
                            "status": "evidence_open",
                            "reason": "real binary evidence has not been recorded",
                        },
                    }
                ],
            }
            evidence = {
                "schema_version": 2,
                "engine": engine,
                "assets": [
                    {
                        "version": "v1.0.0",
                        "binary_target": "x86_64-unknown-linux-gnu",
                        "adapter_revision": 1,
                        "operations": [],
                    }
                ],
            }
            problems = GUARD.validate_declaration(
                engine, declaration, [operation], evidence
            )
            with self.subTest(engine=engine, operation=f"{category}.{name}"):
                if production_composed:
                    self.assertEqual(problems, [])
                else:
                    self.assertTrue(
                        any("no audited production composition" in problem for problem in problems)
                    )

            if not production_composed:
                declaration["operations"][0]["support"] = {
                    "status": "unsupported",
                    "reason": "no production engine owner is registered",
                }
                self.assertEqual(
                    GUARD.validate_declaration(engine, declaration, [operation], evidence),
                    [],
                )

    def test_evidence_ids_do_not_make_uncomposed_operations_supported(self):
        cases = [
            ("sing-box", {"category": "profile", "operation": "validate"}),
            ("mihomo", {"category": "dns", "operation": "read_configuration"}),
            ("mihomo", {"category": "profile", "operation": "validate"}),
            ("mihomo", {"category": "profile", "operation": "apply"}),
        ]
        for engine, operation in cases:
            evidence_id = GUARD.evidence_operation_id(
                (operation["category"], operation["operation"])
            )
            declaration = {
                "schema_version": 1,
                "engine": engine,
                "operations": [
                    {
                        "operation": operation,
                        "support": {
                            "status": "supported",
                            "evidence_id": evidence_id,
                        },
                    }
                ],
            }
            evidence = {
                "schema_version": 2,
                "engine": engine,
                "assets": [
                    {
                        "version": "v1.0.0",
                        "binary_target": "x86_64-unknown-linux-gnu",
                        "adapter_revision": 1,
                        "operations": [evidence_id],
                    }
                ],
            }
            problems = GUARD.validate_declaration(
                engine, declaration, [operation], evidence
            )
            with self.subTest(engine=engine, operation=operation):
                self.assertTrue(
                    any("production" in problem for problem in problems)
                )

    def test_historical_profile_binary_ids_can_remain_locked_while_core_ops_are_unsupported(self):
        for operation, evidence_id in [
            ({"category": "profile", "operation": "validate"}, "profile.validate"),
            ({"category": "profile", "operation": "apply"}, "profile.apply"),
        ]:
            declaration = {
                "schema_version": 1,
                "engine": "mihomo",
                "operations": [
                    {
                        "operation": operation,
                        "support": {
                            "status": "unsupported",
                            "reason": "CORE-013.4 native transaction port is not composed",
                        },
                    }
                ],
            }
            evidence = {
                "schema_version": 2,
                "engine": "mihomo",
                "assets": [
                    {
                        "version": "v1.19.32",
                        "binary_target": "x86_64-unknown-linux-gnu",
                        "adapter_revision": 1,
                        "operations": [evidence_id],
                    }
                ],
            }
            with self.subTest(operation=operation):
                self.assertEqual(
                    GUARD.validate_declaration("mihomo", declaration, [operation], evidence),
                    [],
                )

    def test_owner_managed_status_requires_the_registered_typed_owner(self):
        catalog = [
            {"category": "lifecycle", "operation": "start"},
            {"category": "version", "operation": "rollback"},
        ]
        declaration = {
            "schema_version": 1,
            "engine": "fixture",
            "operations": [
                {
                    "operation": catalog[0],
                    "support": {"status": "owner_managed", "owner": "core_lifecycle"},
                },
                {
                    "operation": catalog[1],
                    "support": {
                        "status": "owner_managed",
                        "owner": "version_management",
                    },
                },
            ],
        }
        self.assertEqual(
            GUARD.validate_declaration("fixture", declaration, catalog, self.evidence),
            [],
        )

        wrong_owner = copy.deepcopy(declaration)
        wrong_owner["operations"][0]["support"]["owner"] = "version_management"
        self.assertTrue(
            any(
                "does not match `core_lifecycle`" in problem
                for problem in GUARD.validate_declaration(
                    "fixture", wrong_owner, catalog, self.evidence
                )
            )
        )

        engine_operation = copy.deepcopy(declaration)
        engine_operation["operations"][0]["operation"] = self.catalog[1]
        self.assertTrue(
            any(
                "no owner-managed authority" in problem
                for problem in GUARD.validate_declaration(
                    "fixture", engine_operation, self.catalog, self.evidence
                )
            )
        )

    def test_missing_and_duplicate_operations_are_rejected(self):
        missing = copy.deepcopy(self.declaration)
        missing["operations"].pop()
        self.assertTrue(
            any(
                "omits CoreOperation entries" in problem
                for problem in GUARD.validate_declaration(
                    "fixture", missing, self.catalog, self.evidence
                )
            )
        )

        duplicate = copy.deepcopy(self.declaration)
        duplicate["operations"].append(copy.deepcopy(duplicate["operations"][0]))
        self.assertTrue(
            any(
                "duplicates profile.validate" in problem
                for problem in GUARD.validate_declaration(
                    "fixture", duplicate, self.catalog, self.evidence
                )
            )
        )

    def test_string_actions_and_boolean_support_states_are_rejected(self):
        string_action = copy.deepcopy(self.declaration)
        string_action["operations"][0]["operation"] = "profile.validate"
        self.assertTrue(
            any(
                "CoreOperation tagged enum shape" in problem
                for problem in GUARD.validate_declaration(
                    "fixture", string_action, self.catalog, self.evidence
                )
            )
        )

        boolean_state = copy.deepcopy(self.declaration)
        boolean_state["operations"][0]["support"]["status"] = True
        self.assertTrue(
            any(
                "status must be an enum string" in problem
                for problem in GUARD.validate_declaration(
                    "fixture", boolean_state, self.catalog, self.evidence
                )
            )
        )

        action_field = copy.deepcopy(self.declaration)
        action_field["operations"][0]["action"] = "validate"
        self.assertTrue(
            any(
                "typed operation and support variant" in problem
                for problem in GUARD.validate_declaration(
                    "fixture", action_field, self.catalog, self.evidence
                )
            )
        )

    def test_supported_operation_must_match_registered_engine_evidence(self):
        wrong_operation = copy.deepcopy(self.declaration)
        wrong_operation["operations"][0]["support"]["evidence_id"] = "outbound.list"
        self.assertTrue(
            any(
                "does not match its typed operation" in problem
                for problem in GUARD.validate_declaration(
                    "fixture", wrong_operation, self.catalog, self.evidence
                )
            )
        )

        unregistered = copy.deepcopy(self.declaration)
        unregistered["operations"][0]["support"]["evidence_id"] = "profile.apply"
        self.assertTrue(
            any(
                "not registered in the engine lock" in problem
                for problem in GUARD.validate_declaration(
                    "fixture", unregistered, self.catalog, self.evidence
                )
            )
        )

    def test_target_scoped_evidence_assets_require_registered_target_and_revision(self):
        partial_lock = copy.deepcopy(self.evidence)
        partial_lock["assets"][1]["operations"] = []
        self.assertEqual(
            GUARD.validate_declaration("fixture", self.declaration, self.catalog, partial_lock),
            [],
        )

        missing_target = copy.deepcopy(self.evidence)
        missing_target["assets"][0]["binary_target"] = "unsupported-platform"
        self.assertTrue(
            any(
                "unregistered binary_target" in problem
                for problem in GUARD.validate_declaration(
                    "fixture", self.declaration, self.catalog, missing_target
                )
            )
        )

        stale_adapter = copy.deepcopy(self.evidence)
        stale_adapter["assets"][0]["adapter_revision"] = 0
        self.assertTrue(
            any(
                "positive adapter_revision" in problem
                for problem in GUARD.validate_declaration(
                    "fixture", self.declaration, self.catalog, stale_adapter
                )
            )
        )

        duplicate_scope = copy.deepcopy(self.evidence)
        duplicate_scope["assets"][1] = copy.deepcopy(duplicate_scope["assets"][0])
        self.assertTrue(
            any(
                "duplicates a version/target/revision scope" in problem
                for problem in GUARD.validate_declaration(
                    "fixture", self.declaration, self.catalog, duplicate_scope
                )
            )
        )

    def test_unsupported_and_evidence_open_require_reasons(self):
        for status in ("unsupported", "evidence_open"):
            with self.subTest(status=status):
                changed = copy.deepcopy(self.declaration)
                changed["operations"][1]["support"] = {"status": status, "reason": " "}
                self.assertTrue(
                    any(
                        f"{status} reason must be non-empty" in problem
                        for problem in GUARD.validate_declaration(
                            "fixture", changed, self.catalog, self.evidence
                        )
                    )
                )

    def test_new_adapter_package_without_engine_mapping_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            contract = root / GUARD.CORE_OPERATIONS_SOURCE
            contract.parent.mkdir(parents=True)
            shutil.copyfile(ROOT / GUARD.CORE_OPERATIONS_SOURCE, contract)
            for engine, spec in GUARD.ENGINE_ADAPTERS.items():
                package = spec["package"]
                package_root = root / "crates" / package
                package_root.mkdir(parents=True)
                (package_root / "Cargo.toml").write_text(
                    f"[package]\nname = \"{package}\"\nversion = \"0.1.0\"\n",
                    encoding="utf-8",
                )
                for key in ("declaration", "evidence"):
                    source = ROOT / spec[key]
                    destination = root / spec[key]
                    destination.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copyfile(source, destination)
            new_adapter = root / "crates/new-engine-adapter"
            new_adapter.mkdir(parents=True)
            (new_adapter / "Cargo.toml").write_text(
                '[package]\nname = "new-engine-adapter"\nversion = "0.1.0"\n',
                encoding="utf-8",
            )

            problems = GUARD.source_problems(root)
            self.assertTrue(
                any("new-engine-adapter" in problem and "no registered declaration mapping" in problem
                    for problem in problems)
            )
            (root / GUARD.ENGINE_ADAPTERS["mihomo"]["declaration"]).unlink()
            missing_declaration = GUARD.source_problems(root)
            self.assertTrue(
                any("adapter declaration is missing" in problem for problem in missing_declaration)
            )


if __name__ == "__main__":
    unittest.main()
