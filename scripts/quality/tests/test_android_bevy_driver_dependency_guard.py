from __future__ import annotations

import importlib.util
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[3]
SCRIPT = ROOT / "scripts/quality/android-bevy-driver-dependency-guard.py"
SPEC = importlib.util.spec_from_file_location("android_bevy_driver_dependency_guard", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
guard = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(guard)


class AndroidBevyDriverDependencyGuardTests(unittest.TestCase):
    def test_runtime_packages_are_rejected(self) -> None:
        names = {
            "infiltrator-bevy-ui",
            "infiltrator-application",
            "infiltrator-composition",
            "mihomo-api",
        }
        self.assertEqual(
            guard.dependency_problems(names),
            [
                "Android Bevy driver normal closure contains forbidden package `infiltrator-application`",
                "Android Bevy driver normal closure contains forbidden package `infiltrator-composition`",
                "Android Bevy driver normal closure contains forbidden package `mihomo-api`",
            ],
        )

    def test_capture_and_desktop_features_are_rejected(self) -> None:
        problems = guard.feature_problems(
            {"capture-tools", "desktop-host", "ui-application-projections"}
        )
        self.assertIn(
            "Android Bevy driver enables forbidden UI feature `ui-application-projections`",
            problems,
        )
        self.assertIn(
            "Android Bevy driver enables forbidden UI feature `capture-tools`", problems
        )
        self.assertIn(
            "Android Bevy driver enables forbidden UI feature `desktop-host`", problems
        )

    def test_driver_manifest_disables_all_ui_features(self) -> None:
        self.assertEqual(guard.manifest_problems(ROOT), [])
        self.assertEqual(guard.ui_default_manifest_problems(ROOT), [])

    def test_capture_features_are_rejected_from_product_defaults(self) -> None:
        self.assertEqual(
            guard.default_feature_problems(
                {"default": ["script-engine-boa"]}
            ),
            [],
        )
        self.assertEqual(
            guard.default_feature_problems(
                {"default": ["capture-tools", "capture-controller"]}
            ),
            [
                "crates/infiltrator-bevy-ui/Cargo.toml: capture-only feature `capture-tools` must not be a product default",
                "crates/infiltrator-bevy-ui/Cargo.toml: capture-only feature `capture-controller` must not be a product default",
            ],
        )

    def test_retired_application_projection_feature_is_rejected(self) -> None:
        self.assertEqual(
            guard.default_feature_problems(
                {
                    "default": ["script-engine-boa"],
                    "ui-application-projections": ["dep:infiltrator-application"],
                }
            ),
            [
                "crates/infiltrator-bevy-ui/Cargo.toml: retired feature `ui-application-projections` must be removed"
            ],
        )

    def test_owner_surface_entry_is_allowed_and_demo_entry_is_rejected(self) -> None:
        product_source = """
        fn android_main() {
            infiltrator_bevy_ui::run_with_host_surface_and_command_sink(source, sink, insets);
        }
        """
        self.assertEqual(guard.driver_source_problems(product_source), [])
        demo_source = """
        fn android_main() {
            infiltrator_bevy_ui::run_demo();
        }
        """
        problems = guard.driver_source_problems(demo_source)
        self.assertTrue(any("run_demo" in problem for problem in problems))


if __name__ == "__main__":
    unittest.main()
