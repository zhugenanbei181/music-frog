"""Positive and negative fixtures for the Android architecture red-line guard."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import sys
import unittest


ROOT = Path(__file__).resolve().parents[3]
SCRIPT = ROOT / "scripts/quality/android-architecture-redline-guard.py"
SPEC = importlib.util.spec_from_file_location("android_architecture_redline_guard", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
guard = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = guard
SPEC.loader.exec_module(guard)


class AndroidArchitectureRedlineGuardTests(unittest.TestCase):
    def codes(self, source: str, symbols: set[str] | None = None) -> set[str]:
        return {violation.code for violation in guard.find_violations("fixture/Fixture.kt", source, symbols)}

    def test_the_current_android_production_tree_has_no_red_line(self) -> None:
        self.assertEqual(guard.scan_repository(ROOT), [])

    def test_the_guard_scans_the_hand_written_production_tree(self) -> None:
        files = guard.production_files(ROOT)
        self.assertGreater(len(files), 100)
        self.assertTrue(
            all("infiltrator_android" not in path.parts for path in files),
            "the generated UniFFI binding must stay outside the guard scope",
        )

    def test_a_production_sleep_is_rejected(self) -> None:
        self.assertIn("ARCH001", self.codes("class Owner {\n    fun wait() {\n        Thread.sleep(10)\n    }\n}"))

    def test_a_main_thread_blocking_wait_is_rejected(self) -> None:
        source = (
            "class Owner {\n"
            "    fun onCreate() {\n"
            "        runBlocking { work() }\n"
            "    }\n"
            "}\n"
        )
        self.assertIn("ARCH002", self.codes(source))

    def test_a_blocking_wait_off_the_main_thread_is_accepted(self) -> None:
        source = (
            "class Owner {\n"
            "    fun onCreate() {\n"
            "        scope.launch {\n"
            "            runBlocking { work() }\n"
            "        }\n"
            "    }\n"
            "}\n"
        )
        self.assertNotIn("ARCH002", self.codes(source))

    def test_an_unbounded_global_scope_is_rejected(self) -> None:
        source = "class Owner {\n    fun start() {\n        GlobalScope.launch { work() }\n    }\n}"
        self.assertIn("ARCH003", self.codes(source))

    def test_an_empty_catch_block_is_rejected(self) -> None:
        source = (
            "class Owner {\n"
            "    fun read() {\n"
            "        try {\n"
            "            work()\n"
            "        } catch (error: Exception) {\n"
            "        }\n"
            "    }\n"
            "}\n"
        )
        self.assertIn("ARCH004", self.codes(source))

    def test_a_swallowed_cancellation_is_rejected(self) -> None:
        source = (
            "class Owner {\n"
            "    fun read() {\n"
            "        try {\n"
            "            work()\n"
            "        } catch (error: CancellationException) {\n"
            "            report()\n"
            "        }\n"
            "    }\n"
            "}\n"
        )
        self.assertIn("ARCH005", self.codes(source))

    def test_a_rethrown_cancellation_is_accepted(self) -> None:
        source = (
            "class Owner {\n"
            "    fun read() {\n"
            "        try {\n"
            "            work()\n"
            "        } catch (error: CancellationException) {\n"
            "            throw error\n"
            "        }\n"
            "    }\n"
            "}\n"
        )
        self.assertNotIn("ARCH005", self.codes(source))

    def test_a_not_null_assertion_is_rejected(self) -> None:
        source = "class Owner {\n    fun read(value: String?) {\n        val text = value!!\n    }\n}"
        self.assertIn("ARCH006", self.codes(source))

    def test_a_not_null_assertion_inside_a_string_template_is_rejected(self) -> None:
        source = "class Owner {\n    fun read(value: String?) {\n        val text = \"${value!!}\"\n    }\n}"
        self.assertIn("ARCH006", self.codes(source))

    def test_a_multi_state_boolean_wire_is_rejected(self) -> None:
        source = "data class Snapshot(\n    val isLoading: Boolean,\n    val isReady: Boolean,\n)"
        self.assertIn("ARCH007", self.codes(source))

    def test_independent_boolean_facts_are_accepted(self) -> None:
        source = "data class Snapshot(\n    val foreground: Boolean,\n    val ipv6: Boolean,\n)"
        self.assertNotIn("ARCH007", self.codes(source))

    def test_text_that_looks_like_a_red_line_is_accepted(self) -> None:
        source = (
            "class Owner {\n"
            "    val text = \"Thread.sleep(10) and !! and GlobalScope.launch\"\n"
            "    // Thread.sleep(10) belongs in a comment only\n"
            "    fun work() = Unit\n"
            "}\n"
        )
        self.assertEqual(self.codes(source), set())

    def test_a_narrow_exception_needs_a_local_explanation(self) -> None:
        source = (
            "class Owner {\n"
            "    fun wait() {\n"
            "        // ARCH-REDLINE: short\n"
            "        Thread.sleep(10)\n"
            "    }\n"
            "}\n"
        )
        self.assertIn("ARCH001", self.codes(source))

    def test_a_narrow_exception_needs_a_behaviour_regression(self) -> None:
        source = (
            "class Owner {\n"
            "    fun wait() {\n"
            "        // ARCH-REDLINE: the typed owner wait directive is bounded and runs off the main thread\n"
            "        Thread.sleep(10)\n"
            "    }\n"
            "}\n"
        )
        violations = guard.find_violations("fixture/Fixture.kt", source, set())
        self.assertTrue(any("behaviour regression" in violation.detail for violation in violations))

    def test_a_documented_narrow_exception_with_a_regression_is_accepted(self) -> None:
        source = (
            "class Owner {\n"
            "    fun wait() {\n"
            "        // ARCH-REDLINE: the typed owner wait directive is bounded and runs off the main thread\n"
            "        Thread.sleep(10)\n"
            "    }\n"
            "}\n"
        )
        self.assertEqual(guard.find_violations("fixture/Fixture.kt", source, {"Owner"}), [])

    def test_the_guard_self_test_passes(self) -> None:
        self.assertEqual(guard.self_test(), 0)

    def test_the_gate_configuration_cannot_be_weakened(self) -> None:
        self.assertEqual(guard.configuration_problems(ROOT), [])

    def test_a_lint_baseline_is_rejected(self) -> None:
        self.assertIn("lint-baseline.xml", guard.FORBIDDEN_BASELINES)
        self.assertIn("detekt-baseline.xml", guard.FORBIDDEN_BASELINES)
        self.assertFalse(
            [path for path in (ROOT / "android").rglob("*") if path.name in guard.FORBIDDEN_BASELINES],
            "the Android tree must not carry a permanent baseline",
        )

    def test_the_approved_lint_suppressions_are_documented(self) -> None:
        text = (ROOT / "android/app/lint.xml").read_text(encoding="utf-8")
        for issue in guard.APPROVED_LINT_SUPPRESSIONS:
            self.assertIn(issue, text)
            self.assertTrue(
                guard.APPROVED_LINT_SUPPRESSIONS[issue].strip(),
                f"{issue} must carry a reason",
            )

    def test_the_detekt_profile_matches_the_documented_scope(self) -> None:
        text = (ROOT / "android/config/detekt/detekt.yml").read_text(encoding="utf-8")
        for rule in guard.APPROVED_DETEKT_EXCLUSIONS:
            self.assertIn(rule.split(">")[1], text)
        self.assertIsNone(
            __import__("re").search(r"maxIssues\s*:", text),
            "the Detekt profile must not raise the maxIssues threshold",
        )

    def test_the_manifest_still_declares_the_vpn_service(self) -> None:
        self.assertTrue(guard._declares_vpn_service(ROOT))


if __name__ == "__main__":
    unittest.main()
