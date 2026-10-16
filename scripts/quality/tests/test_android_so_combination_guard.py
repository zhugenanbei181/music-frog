"""Tests for the Android .so packaging and symbol combination guard."""

import importlib.util
from pathlib import Path
import sys
import unittest

QUALITY = Path(__file__).resolve().parents[1]
REPO = QUALITY.parents[1]
sys.path.insert(0, str(REPO / "scripts"))
sys.path.insert(0, str(QUALITY))

SPEC = importlib.util.spec_from_file_location(
    "android_so_combination_guard",
    QUALITY / "android-so-combination-guard.py",
)
assert SPEC is not None and SPEC.loader is not None
GUARD = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = GUARD
SPEC.loader.exec_module(GUARD)


class AndroidSoCombinationGuardTests(unittest.TestCase):
    def test_repo_gradle_and_identity_pass_guard(self) -> None:
        gradle_text = (REPO / "android/app/build.gradle.kts").read_text(encoding="utf-8")
        self.assertEqual(GUARD.gradle_problems(gradle_text), [])

        props_text = (REPO / "product-identity.properties").read_text(encoding="utf-8")
        self.assertEqual(GUARD.product_identity_problems(props_text), [])

    def test_bevy_host_leaked_to_compose_is_rejected(self) -> None:
        bad_gradle = """
        dependencies {
            implementation(project(":bevy-host"))
        }
        """
        problems = GUARD.gradle_problems(bad_gradle)
        self.assertTrue(any("must not leak :bevy-host" in p for p in problems))

    def test_missing_bevy_implementation_is_rejected(self) -> None:
        bad_gradle = """
        dependencies {
            implementation("androidx.compose.ui:ui")
        }
        """
        problems = GUARD.gradle_problems(bad_gradle)
        self.assertTrue(any("must add :bevy-host only via bevyImplementation" in p for p in problems))

    def test_missing_so_in_flavor_is_rejected(self) -> None:
        problems = GUARD.so_boundary_problems("bevy", {"libinfiltrator_android.so", "libmihomo.so"})
        self.assertTrue(any("missing required native libraries" in p for p in problems))
        self.assertTrue(any("libinfiltrator_bevy_android.so" in p for p in problems))

    def test_unexpected_so_in_flavor_is_rejected(self) -> None:
        problems = GUARD.so_boundary_problems(
            "compose",
            {"libinfiltrator_android.so", "libinfiltrator_bevy_android.so", "libmihomo.so"},
        )
        self.assertTrue(any("unexpected native libraries" in p for p in problems))
        self.assertTrue(any("libinfiltrator_bevy_android.so" in p for p in problems))

    def test_native_activity_symbol_rules(self) -> None:
        # Bevy driver must have ANativeActivity_onCreate
        self.assertEqual(
            GUARD.native_symbol_boundary_problems(
                "libinfiltrator_bevy_android.so",
                {"ANativeActivity_onCreate"},
            ),
            [],
        )
        self.assertTrue(
            any(
                "must export ANativeActivity_onCreate" in p
                for p in GUARD.native_symbol_boundary_problems(
                    "libinfiltrator_bevy_android.so",
                    set(),
                )
            )
        )

        # Compose host must NOT have ANativeActivity_onCreate
        self.assertEqual(
            GUARD.native_symbol_boundary_problems(
                "libinfiltrator_android.so",
                {"Java_uniffi_..."},
                is_slint_flavor=False,
            ),
            [],
        )
        self.assertTrue(
            any(
                "must not export ANativeActivity_onCreate" in p
                for p in GUARD.native_symbol_boundary_problems(
                    "libinfiltrator_android.so",
                    {"ANativeActivity_onCreate"},
                    is_slint_flavor=False,
                )
            )
        )

        # Slint host MUST have ANativeActivity_onCreate
        self.assertEqual(
            GUARD.native_symbol_boundary_problems(
                "libinfiltrator_android.so",
                {"ANativeActivity_onCreate"},
                is_slint_flavor=True,
            ),
            [],
        )
        self.assertTrue(
            any(
                "must export ANativeActivity_onCreate" in p
                for p in GUARD.native_symbol_boundary_problems(
                    "libinfiltrator_android.so",
                    {"Java_uniffi_..."},
                    is_slint_flavor=True,
                )
            )
        )


if __name__ == "__main__":
    unittest.main()
