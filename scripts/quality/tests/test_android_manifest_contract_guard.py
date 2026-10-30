"""Regression tests for the active Android production manifest contract."""

from __future__ import annotations

import copy
import importlib.util
from pathlib import Path
import shutil
import sys
import tempfile
import unittest
import xml.etree.ElementTree as ET

QUALITY = Path(__file__).resolve().parents[1]
REPO = QUALITY.parents[1]
sys.path.insert(0, str(QUALITY))
SPEC = importlib.util.spec_from_file_location(
    "android_manifest_contract_guard",
    QUALITY / "android-manifest-contract-guard.py",
)
GUARD = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = GUARD
SPEC.loader.exec_module(GUARD)


def fixture_repo(root: Path) -> Path:
    for relative in (
        "Cargo.toml",
        "product-identity.properties",
        "android/app/src/main/AndroidManifest.xml",
        "android/app/src/compose/AndroidManifest.xml",
        "android/app/src/bevy/AndroidManifest.xml",
    ):
        source = REPO / relative
        destination = root / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, destination)
    return root


def mutate_manifest(root: Path, relative: str, mutation) -> None:
    path = root / relative
    tree = ET.parse(path)
    mutation(tree.getroot())
    tree.write(path, encoding="utf-8", xml_declaration=True)


def application(root: ET.Element) -> ET.Element:
    return root.find("application")


class AndroidManifestContractRules(unittest.TestCase):
    def test_current_production_manifests_satisfy_the_contract(self):
        self.assertEqual(GUARD.source_problems(), [])

    def test_required_notification_permission_cannot_be_removed(self):
        with tempfile.TemporaryDirectory() as directory:
            root = fixture_repo(Path(directory))

            def remove_permission(manifest):
                for item in list(manifest.findall("uses-permission")):
                    if item.get(GUARD.ANDROID_NAME) == "android.permission.POST_NOTIFICATIONS":
                        manifest.remove(item)

            mutate_manifest(root, "android/app/src/main/AndroidManifest.xml", remove_permission)
            problems = GUARD.source_problems(root)
        self.assertTrue(any("required Android permission is missing: android.permission.POST_NOTIFICATIONS" in item for item in problems))

    def test_vpn_service_declaration_cannot_be_removed(self):
        with tempfile.TemporaryDirectory() as directory:
            root = fixture_repo(Path(directory))

            def remove_vpn_service(manifest):
                app = application(manifest)
                for service in list(app.findall("service")):
                    if service.get(GUARD.ANDROID_NAME) == ".MihomoVpnService":
                        app.remove(service)

            mutate_manifest(root, "android/app/src/main/AndroidManifest.xml", remove_vpn_service)
            problems = GUARD.source_problems(root)
        self.assertTrue(any("required Android service declaration is missing" in item and "MihomoVpnService" in item for item in problems))

    def test_vpn_foreground_service_type_must_match_android_14_contract(self):
        with tempfile.TemporaryDirectory() as directory:
            root = fixture_repo(Path(directory))

            def change_type(manifest):
                vpn = next(
                    item for item in application(manifest).findall("service")
                    if item.get(GUARD.ANDROID_NAME) == ".MihomoVpnService"
                )
                vpn.set(f"{GUARD.ANDROID}foregroundServiceType", "dataSync")

            mutate_manifest(root, "android/app/src/main/AndroidManifest.xml", change_type)
            problems = GUARD.source_problems(root)
        self.assertTrue(any("foregroundServiceType must be 'systemExempted'" in item for item in problems))

    def test_stale_service_declaration_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = fixture_repo(Path(directory))

            def add_stale_service(manifest):
                service = ET.Element("service")
                service.set(GUARD.ANDROID_NAME, ".LegacyVpnService")
                application(manifest).append(service)

            mutate_manifest(root, "android/app/src/main/AndroidManifest.xml", add_stale_service)
            problems = GUARD.source_problems(root)
        self.assertTrue(any("stale or unregistered Android service declaration" in item and "LegacyVpnService" in item for item in problems))

    def test_duplicate_service_declaration_across_flavors_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = fixture_repo(Path(directory))
            main = ET.parse(root / "android/app/src/main/AndroidManifest.xml").getroot()
            vpn = next(
                item for item in application(main).findall("service")
                if item.get(GUARD.ANDROID_NAME) == ".MihomoVpnService"
            )

            def add_duplicate(manifest):
                application(manifest).append(copy.deepcopy(vpn))

            mutate_manifest(root, "android/app/src/compose/AndroidManifest.xml", add_duplicate)
            problems = GUARD.source_problems(root)
        self.assertTrue(any("Android service com.musicfrog.infiltrator.MihomoVpnService is declared more than once" in item for item in problems))
        self.assertTrue(any("must be owned by the main manifest" in item and "MihomoVpnService" in item for item in problems))

    def test_duplicate_service_declaration_in_main_manifest_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = fixture_repo(Path(directory))

            def duplicate_vpn(manifest):
                app = application(manifest)
                vpn = next(
                    item for item in app.findall("service")
                    if item.get(GUARD.ANDROID_NAME) == ".MihomoVpnService"
                )
                app.append(copy.deepcopy(vpn))

            mutate_manifest(root, "android/app/src/main/AndroidManifest.xml", duplicate_vpn)
            problems = GUARD.source_problems(root)
        self.assertTrue(any("Android service com.musicfrog.infiltrator.MihomoVpnService is declared more than once" in item for item in problems))

    def test_boot_receiver_and_tile_bind_permissions_are_required(self):
        with tempfile.TemporaryDirectory() as directory:
            root = fixture_repo(Path(directory))

            def remove_boot_receiver(manifest):
                app = application(manifest)
                for receiver in list(app.findall("receiver")):
                    app.remove(receiver)

            mutate_manifest(root, "android/app/src/main/AndroidManifest.xml", remove_boot_receiver)

            def remove_tile_permission(manifest):
                tile = next(
                    item for item in application(manifest).findall("service")
                    if item.get(GUARD.ANDROID_NAME) == ".InfiltratorTileService"
                )
                tile.attrib.pop(f"{GUARD.ANDROID}permission")

            mutate_manifest(root, "android/app/src/main/AndroidManifest.xml", remove_tile_permission)
            problems = GUARD.source_problems(root)
        self.assertTrue(any("required Android boot receiver declaration is missing" in item for item in problems))
        self.assertTrue(any("InfiltratorTileService" in item and "BIND_QUICK_SETTINGS_TILE" in item for item in problems))

    def test_compose_activity_launch_mode_and_exported_state_are_required(self):
        with tempfile.TemporaryDirectory() as directory:
            root = fixture_repo(Path(directory))

            def break_activity(manifest):
                activity = application(manifest).find("activity")
                activity.set(f"{GUARD.ANDROID}launchMode", "standard")
                activity.set(f"{GUARD.ANDROID}exported", "false")

            mutate_manifest(root, "android/app/src/compose/AndroidManifest.xml", break_activity)
            problems = GUARD.source_problems(root)
        self.assertTrue(any("android:launchMode must be 'singleTask'" in item for item in problems))
        self.assertTrue(any("android:exported must be 'true'" in item for item in problems))

    def test_compose_owns_the_optional_leanback_launcher(self):
        with tempfile.TemporaryDirectory() as directory:
            root = fixture_repo(Path(directory))

            def remove_compose_leanback_category(manifest):
                activity = application(manifest).find("activity")
                for intent_filter in list(activity.findall("intent-filter")):
                    if any(
                        category.get(GUARD.ANDROID_NAME) == GUARD.LEANBACK_LAUNCHER
                        for category in intent_filter.findall("category")
                    ):
                        activity.remove(intent_filter)

            mutate_manifest(
                root,
                "android/app/src/compose/AndroidManifest.xml",
                remove_compose_leanback_category,
            )
            problems = GUARD.source_problems(root)
        self.assertTrue(
            any("Compose launcher must expose a MAIN/LEANBACK_LAUNCHER" in item for item in problems)
        )

    def test_leanback_capability_cannot_be_declared_by_a_shared_source_set(self):
        with tempfile.TemporaryDirectory() as directory:
            root = fixture_repo(Path(directory))

            def add_main_leanback_feature(manifest):
                feature = ET.Element("uses-feature")
                feature.set(GUARD.ANDROID_NAME, GUARD.LEANBACK_FEATURE)
                feature.set(f"{GUARD.ANDROID}required", "false")
                manifest.append(feature)

            mutate_manifest(
                root,
                "android/app/src/main/AndroidManifest.xml",
                add_main_leanback_feature,
            )
            problems = GUARD.source_problems(root)
        self.assertTrue(
            any("Leanback capability must be declared once by the Compose flavor" in item for item in problems)
        )

    def test_bevy_native_activity_library_binding_is_required(self):
        with tempfile.TemporaryDirectory() as directory:
            root = fixture_repo(Path(directory))

            def break_library_binding(manifest):
                activity = application(manifest).find("activity")
                metadata = activity.find("meta-data")
                metadata.set(GUARD.ANDROID_VALUE, "wrong_library")

            mutate_manifest(root, "android/app/src/bevy/AndroidManifest.xml", break_library_binding)
            problems = GUARD.source_problems(root)
        self.assertTrue(any("android.app.lib_name" in item and "infiltrator_bevy_android" in item for item in problems))

    def test_unregistered_production_manifest_cannot_silently_change_the_apk(self):
        with tempfile.TemporaryDirectory() as directory:
            root = fixture_repo(Path(directory))
            path = root / "android/app/src/debug/AndroidManifest.xml"
            path.parent.mkdir(parents=True)
            path.write_text("<manifest xmlns:android=\"http://schemas.android.com/apk/res/android\" />", encoding="utf-8")
            problems = GUARD.source_problems(root)
        self.assertTrue(any("unregistered production source-set manifest would alter the APK" in item for item in problems))


if __name__ == "__main__":
    unittest.main()
