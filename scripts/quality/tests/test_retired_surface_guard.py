"""Exact regression checks for the retired product-surface guard."""

import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest


QUALITY = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "retired_surface_guard",
    QUALITY / "retired-surface-guard.py",
)
GUARD = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = GUARD
SPEC.loader.exec_module(GUARD)


class RetiredSurfaceGuardTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        (self.root / "scripts/quality").mkdir(parents=True)
        (self.root / "android/app/src/test").mkdir(parents=True)
        (self.root / "android/app/src/test/VpnProcessStateTest.kt").write_text(
            "class VpnProcessStateTest\n",
            encoding="utf-8",
        )
        (self.root / "crates/infiltrator-cli/src").mkdir(parents=True)
        (self.root / "crates/infiltrator-cli/src/commands_test.rs").write_text(
            "fn outbound_commands_use_typed_ids() {}\n",
            encoding="utf-8",
        )
        (self.root / "crates/infiltrator-application/src").mkdir(parents=True)
        (self.root / "crates/infiltrator-application/src/command_outbound_tests.rs").write_text(
            "fn outbound_selection_has_readback() {}\n",
            encoding="utf-8",
        )
        self.registry = {
            "version": 1,
            "retired_surfaces": [
                {
                    "id": "old-vpn-manager",
                    "kind": "public_symbol",
                    "language": "kotlin",
                    "text": "VpnStateManager",
                    "rationale": "Replaced by typed VPN lifecycle state.",
                    "replacement_behavior_tests": [
                        "android/app/src/test/VpnProcessStateTest.kt",
                    ],
                }
            ],
        }
        self.write_registry()

    def tearDown(self):
        self.temporary.cleanup()

    def write_registry(self):
        (self.root / "scripts/quality/retired-surfaces.json").write_text(
            json.dumps(self.registry),
            encoding="utf-8",
        )

    def use_cli_command_surface(self, phrase):
        entry = self.registry["retired_surfaces"][0]
        entry["kind"] = "product_command"
        entry["language"] = "cli"
        entry["text"] = phrase
        entry["replacement_behavior_tests"] = [
            "crates/infiltrator-cli/src/commands_test.rs",
            "crates/infiltrator-application/src/command_outbound_tests.rs",
        ]
        self.write_registry()

    def problems(self):
        return GUARD.check(self.root)

    def test_repository_registry_and_replacement_tests_are_current(self):
        self.assertEqual(GUARD.check(GUARD.REPO_ROOT), [])

    def test_exact_symbol_resurrection_in_production_source_is_rejected(self):
        source = self.root / "android/app/src/main/java/VpnService.kt"
        source.parent.mkdir(parents=True)
        source.write_text("private val manager = VpnStateManager\n", encoding="utf-8")

        problems = self.problems()

        self.assertTrue(any("retired surface `VpnStateManager`" in item for item in problems))

    def test_exact_symbol_in_authoritative_product_document_is_rejected(self):
        (self.root / "README.md").write_text(
            "The VpnStateManager API starts the tunnel.\n",
            encoding="utf-8",
        )

        problems = self.problems()

        self.assertTrue(any("README.md:1" in item for item in problems))

    def test_exact_identifier_boundary_avoids_longer_names(self):
        source = self.root / "android/app/src/main/java/VpnService.kt"
        source.parent.mkdir(parents=True)
        source.write_text(
            "private val current = VpnStateManagerV2\n"
            "private val note = NotVpnStateManager\n",
            encoding="utf-8",
        )

        self.assertEqual(self.problems(), [])

    def test_history_fixtures_generated_trees_and_todo_are_excluded(self):
        excluded_files = {
            "android/app/src/test/OldNameTest.kt": "VpnStateManager",
            "android/app/src/main/java/target/generated.kt": "VpnStateManager",
            "android/app/src/main/java/vendor/copied.kt": "VpnStateManager",
            "docs/archive/old-guide.md": "VpnStateManager",
            "docs/TODO.md": "VpnStateManager",
            "TODO.md": "VpnStateManager",
        }
        for relative, content in excluded_files.items():
            path = self.root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content, encoding="utf-8")

        self.assertEqual(self.problems(), [])

    def test_unknown_registry_enum_value_fails_closed(self):
        self.registry["retired_surfaces"][0]["language"] = "swift"
        self.write_registry()

        problems = self.problems()

        self.assertTrue(any("swift" in item for item in problems))

    def test_missing_replacement_behavior_test_fails_closed(self):
        self.registry["retired_surfaces"][0]["replacement_behavior_tests"] = [
            "android/app/src/test/MissingTest.kt",
        ]
        self.write_registry()

        problems = self.problems()

        self.assertTrue(any("replacement behavior test is missing" in item for item in problems))

    def test_cli_command_phrase_is_checked_in_cli_source(self):
        self.use_cli_command_surface("proxy groups")
        source = self.root / "crates/infiltrator-cli/src/commands.rs"
        source.write_text("/// List selectable proxy groups.\n", encoding="utf-8")

        problems = self.problems()

        self.assertTrue(any("retired surface `proxy groups`" in item for item in problems))
        self.assertTrue(any("commands.rs:1" in item for item in problems))

    def test_cli_command_phrase_is_checked_in_shell_and_authoritative_docs(self):
        self.use_cli_command_surface("proxy switch")
        shell = self.root / "scripts/proxy-tool.sh"
        shell.parent.mkdir(parents=True, exist_ok=True)
        shell.write_text("infiltrator proxy switch\n", encoding="utf-8")
        readme = self.root / "README.md"
        readme.write_text("Use `infiltrator proxy switch` to choose a proxy.\n", encoding="utf-8")

        problems = self.problems()

        self.assertTrue(any("proxy-tool.sh:1" in item for item in problems))
        self.assertTrue(any("README.md:1" in item for item in problems))

    def test_cli_phrase_scope_and_token_boundaries_avoid_false_positives(self):
        self.use_cli_command_surface("proxy groups")
        cli_source = self.root / "crates/infiltrator-cli/src/commands.rs"
        cli_source.write_text("/// proxy groupster and proxy group.\n", encoding="utf-8")
        unrelated = self.root / "crates/infiltrator-application/src/notes.rs"
        unrelated.write_text("// proxy groups remain a domain concept.\n", encoding="utf-8")

        self.assertEqual(self.problems(), [])

    def test_source_root_scope_catches_resurrection_without_cross_product_name_collision(self):
        entry = self.registry["retired_surfaces"][0]
        entry["source_roots"] = ["android/app/src/main/java/infiltrator_android"]
        self.write_registry()
        scoped = self.root / "android/app/src/main/java/infiltrator_android/Bindings.kt"
        scoped.parent.mkdir(parents=True, exist_ok=True)
        scoped.write_text("fun VpnStateManager() {}\n", encoding="utf-8")
        unrelated = self.root / "android/app/src/main/java/com/example/Screen.kt"
        unrelated.parent.mkdir(parents=True, exist_ok=True)
        unrelated.write_text("val VpnStateManager = current\n", encoding="utf-8")

        problems = self.problems()

        self.assertTrue(any("infiltrator_android/Bindings.kt:1" in item for item in problems))
        self.assertFalse(any("com/example/Screen.kt" in item for item in problems))

    def test_source_root_scope_rejects_paths_outside_repository(self):
        self.registry["retired_surfaces"][0]["source_roots"] = ["../outside"]
        self.write_registry()

        problems = self.problems()

        self.assertTrue(any("source root must stay inside the repository" in item for item in problems))


if __name__ == "__main__":
    unittest.main()
