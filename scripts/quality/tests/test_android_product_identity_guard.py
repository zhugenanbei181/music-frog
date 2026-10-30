"""Guard product release identity from drift across Android build surfaces."""

import importlib.util
from pathlib import Path
import shutil
import sys
import tempfile
import tomllib
import unittest
from zipfile import ZipFile
from unittest.mock import patch

QUALITY = Path(__file__).resolve().parents[1]
REPO = QUALITY.parents[1]
sys.path.insert(0, str(REPO / "scripts"))
sys.path.insert(0, str(QUALITY))
SPEC = importlib.util.spec_from_file_location(
    "android_product_identity_guard",
    QUALITY / "android-product-identity-guard.py",
)
GUARD = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = GUARD
SPEC.loader.exec_module(GUARD)


def fixture_repo(root: Path) -> Path:
    files = (
        "Cargo.toml",
        "product-identity.properties",
        "android/app/build.gradle.kts",
        "android/app/src/main/AndroidManifest.xml",
        "android/app/src/compose/AndroidManifest.xml",
        "android/app/src/bevy/AndroidManifest.xml",
        "android/app/src/slint/AndroidManifest.xml",
        "android/app/src/bevy/res/values/strings.xml",
        "android/app/src/bevy/res/values-zh-rCN/strings.xml",
        "android/app/src/bevy/res/values-zh-rTW/strings.xml",
        "android/app/src/main/res/values/strings.xml",
        "android/app/src/main/res/values-zh-rCN/strings.xml",
        "android/app/src/main/res/values-zh-rTW/strings.xml",
        "crates/infiltrator-iced/Cargo.toml",
        "crates/infiltrator-iced/src/main.rs",
        "crates/infiltrator-bevy-ui/Cargo.toml",
        "crates/infiltrator-bevy-desktop-host/Cargo.toml",
        "crates/infiltrator-bevy-desktop-host/src/main.rs",
        "crates/infiltrator-slint/Cargo.toml",
        "crates/infiltrator-slint/src/main.rs",
        "crates/infiltrator-cli/Cargo.toml",
        "crates/infiltrator-cli/src/main.rs",
        "crates/infiltrator-android/Cargo.toml",
        "crates/infiltrator-shared/assets/product/icon.png",
        "crates/infiltrator-shared/assets/product/icon.ico",
        "crates/infiltrator-shared/assets/asset-catalog.json",
        "packaging/linux/appimage/infiltrator.desktop",
        "packaging/linux/appimage/infiltrator-bevy.desktop",
        "packaging/linux/appimage/infiltrator-slint.desktop",
        "packaging/linux/appimage/build-appimage.sh",
        "packaging/linux/deb/build-deb.sh",
        "packaging/macos/Info.plist",
        "packaging/windows/nsis/infiltrator.nsi",
        "packaging/windows/nsis/infiltrator-bevy.nsi",
        "packaging/windows/nsis/infiltrator-slint.nsi",
        "packaging/windows/nsis/build-nsis.sh",
        "scripts/product_identity.py",
        "scripts/render-product-template.py",
        "scripts/build-bevy-apk.sh",
        "scripts/verify-bevy-apk.sh",
        "scripts/android-emulator-tests.sh",
        ".github/workflows/release.yml",
        ".github/workflows/android.yml",
    )
    for relative in files:
        source = REPO / relative
        destination = root / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, destination)
    for icon_variant in sorted((REPO / "crates/infiltrator-shared/assets/product").glob("icon-*.png")):
        destination = root / "crates/infiltrator-shared/assets/product" / icon_variant.name
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(icon_variant, destination)
    return root


class AndroidProductIdentityRules(unittest.TestCase):
    def test_real_android_products_share_the_declared_identity(self):
        self.assertEqual(GUARD.source_problems(), [])

    def test_compose_and_bevy_version_codes_derive_from_the_shared_version(self):
        identity = GUARD.read_properties()
        major, minor, patch = GUARD.version_parts(identity)
        self.assertEqual(
            GUARD.get_value(identity, "compose.version_code"),
            str(major * 1_000_000 + minor * 1_000 + patch),
        )
        self.assertEqual(
            GUARD.get_value(identity, "bevy.version_code"),
            str((1 << 24) | (major << 16) | (minor << 8) | patch),
        )
        registry = (REPO / "product-identity.properties").read_text(encoding="utf-8")
        self.assertNotIn("compose.version_code=", registry)
        self.assertNotIn("bevy.version_code=", registry)

    def test_product_crate_cannot_override_the_workspace_release_version(self):
        with tempfile.TemporaryDirectory() as directory:
            root = fixture_repo(Path(directory))
            path = root / "crates/infiltrator-iced/Cargo.toml"
            contents = path.read_text(encoding="utf-8")
            path.write_text(contents.replace("version.workspace = true", 'version = "9.9.9"'))
            problems = GUARD.source_problems(root)
        self.assertTrue(any("must inherit [workspace.package].version" in item for item in problems))

    def test_product_version_is_read_from_the_workspace_not_the_android_registry(self):
        identity = GUARD.read_properties()
        workspace = tomllib.loads((REPO / "Cargo.toml").read_text(encoding="utf-8"))
        self.assertEqual(identity["version.name"], workspace["workspace"]["package"]["version"])
        registry = (REPO / "product-identity.properties").read_text(encoding="utf-8")
        self.assertNotIn("version.name=", registry)

    def test_hard_coded_launcher_name_cannot_shadow_the_identity_registry(self):
        with tempfile.TemporaryDirectory() as directory:
            root = fixture_repo(Path(directory))
            path = root / "android/app/src/main/res/values-zh-rCN/strings.xml"
            contents = path.read_text(encoding="utf-8")
            path.write_text(
                contents.replace(
                    "<resources>",
                    "<resources>\n    <string name=\"app_name\">Wrong name</string>",
                    1,
                )
            )
            problems = GUARD.source_problems(root)
        self.assertTrue(any("must not duplicate generated identity resource 'app_name'" in item for item in problems))

    def test_bevy_launcher_name_cannot_be_duplicated_in_a_flavor_resource(self):
        with tempfile.TemporaryDirectory() as directory:
            root = fixture_repo(Path(directory))
            path = root / "android/app/src/bevy/res/values-zh-rCN/strings.xml"
            contents = path.read_text(encoding="utf-8")
            path.write_text(
                contents.replace(
                    "<resources>",
                    "<resources>\n    <string name=\"app_name\">Wrong Bevy name</string>",
                    1,
                )
            )
            problems = GUARD.source_problems(root)
        self.assertTrue(
            any(
                "android/app/src/bevy/res/values-zh-rCN/strings.xml must not duplicate" in item
                for item in problems
            )
        )

    def test_gradle_launcher_name_must_come_from_the_registered_product(self):
        with tempfile.TemporaryDirectory() as directory:
            root = fixture_repo(Path(directory))
            path = root / "android/app/build.gradle.kts"
            contents = path.read_text(encoding="utf-8")
            path.write_text(
                contents.replace(
                    'resValue("string", "app_name", bevyDisplayName)',
                    'resValue("string", "app_name", composeDisplayName)',
                )
            )
            problems = GUARD.source_problems(root)
        self.assertTrue(any("must generate bevy display name" in item for item in problems))

    def test_application_id_must_be_bound_inside_the_matching_flavor(self):
        with tempfile.TemporaryDirectory() as directory:
            root = fixture_repo(Path(directory))
            path = root / "android/app/build.gradle.kts"
            contents = path.read_text(encoding="utf-8")
            path.write_text(contents.replace(
                "applicationId = bevyApplicationId",
                "applicationId = composeApplicationId",
            ))
            problems = GUARD.source_problems(root)
        self.assertTrue(any("must bind applicationId to bevy.application_id" in item for item in problems))

    def test_version_code_must_be_bound_inside_the_matching_flavor(self):
        with tempfile.TemporaryDirectory() as directory:
            root = fixture_repo(Path(directory))
            path = root / "android/app/build.gradle.kts"
            contents = path.read_text(encoding="utf-8")
            path.write_text(contents.replace(
                "versionCode = bevyVersionCode",
                "versionCode = composeVersionCode",
            ))
            problems = GUARD.source_problems(root)
        self.assertTrue(any("must derive versionCode from bevy.version_code" in item for item in problems))

    def test_registered_product_launcher_activity_must_match_its_flavor(self):
        with tempfile.TemporaryDirectory() as directory:
            root = fixture_repo(Path(directory))
            path = root / "android/app/src/bevy/AndroidManifest.xml"
            contents = path.read_text(encoding="utf-8")
            path.write_text(contents.replace(".BevyHostActivity", ".MainActivity"))
            problems = GUARD.source_problems(root)
        self.assertTrue(
            any("bevy launcher manifest must expose exactly one MAIN/LAUNCHER" in item for item in problems)
        )

    def test_registered_product_must_have_its_launcher_manifest(self):
        with tempfile.TemporaryDirectory() as directory:
            root = fixture_repo(Path(directory))
            (root / "android/app/src/bevy/AndroidManifest.xml").unlink()
            problems = GUARD.source_problems(root)
        self.assertTrue(any("bevy launcher manifest is missing" in item for item in problems))

    def test_desktop_launcher_must_render_the_registered_product_name(self):
        with tempfile.TemporaryDirectory() as directory:
            root = fixture_repo(Path(directory))
            path = root / "packaging/linux/appimage/infiltrator.desktop"
            contents = path.read_text(encoding="utf-8")
            path.write_text(
                contents.replace(
                    "@desktop.iced.display_name@",
                    "@desktop.bevy.display_name@",
                    1,
                ),
                encoding="utf-8",
            )
            problems = GUARD.source_problems(root)
        self.assertTrue(any("Linux desktop Name" in item for item in problems))

    def test_unregistered_desktop_release_artifact_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = fixture_repo(Path(directory))
            path = root / ".github/workflows/release.yml"
            with path.open("a", encoding="utf-8") as workflow:
                workflow.write("\n            artifact_name: Future-Desktop-macOS\n")
            problems = GUARD.source_problems(root)
        self.assertTrue(any("unregistered artifact 'Future-Desktop-macOS'" in item for item in problems))

    def test_release_tag_must_match_the_registry_version(self):
        identity = GUARD.read_properties()
        major, minor, patch = GUARD.version_parts(identity)
        self.assertEqual(GUARD.release_tag_problems(identity, f"v{major}.{minor}.{patch}"), [])
        self.assertTrue(GUARD.release_tag_problems(identity, f"v{major}.{minor}.{patch + 1}"))

    def test_unregistered_product_reference_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = fixture_repo(Path(directory))
            path = root / "android/app/build.gradle.kts"
            with path.open("a", encoding="utf-8") as build_file:
                build_file.write('\nval futureApplicationId = identityValue("future.application_id")\n')
            problems = GUARD.source_problems(root)
        self.assertTrue(any("unregistered product 'future'" in item for item in problems))

    def test_registry_entry_requires_a_build_and_release_adapter(self):
        with tempfile.TemporaryDirectory() as directory:
            root = fixture_repo(Path(directory))
            path = root / "product-identity.properties"
            contents = path.read_text(encoding="utf-8")
            path.write_text(
                contents.replace(
                    "android.products=compose,bevy",
                    "android.products=compose,bevy,future",
                )
                + "\nfuture.application_id=app.musicfrog.future\n"
                + "future.display_name=MusicFrog Future\n"
                + "future.icon_resource=ic_app_icon\n"
                + "future.icon_asset=crates/infiltrator-iced/icons/icon.png\n"
                + "future.name_resource=app_name\n"
                + "future.name_resource_origin=gradle-resValue\n"
                + "future.launch_manifest=android/app/src/compose/AndroidManifest.xml\n"
                + "future.launcher_activity=.MainActivity\n"
                + "future.release_artifact_prefix=MusicFrog-Future\n"
                + "future.version_code_scheme=android-composite\n"
                + "future.builder=gradle\n",
                encoding="utf-8",
            )
            problems = GUARD.source_problems(root)
        self.assertTrue(any("registered product 'future' has no Android build adapter" in item for item in problems))
        self.assertTrue(any("does not derive future artifact names" in item for item in problems))
        self.assertTrue(any("does not aapt-check the future APK" in item for item in problems))
        self.assertTrue(any("must export the registered future artifact prefix" in item for item in problems))
        self.assertTrue(any("Android upload artifact name for future" in item for item in problems))

    def test_new_android_application_module_cannot_skip_product_registration(self):
        with tempfile.TemporaryDirectory() as directory:
            root = fixture_repo(Path(directory))
            module = root / "android/future-app/build.gradle.kts"
            module.parent.mkdir(parents=True)
            module.write_text(
                'plugins { id("com.android.application") }\n'
                'android { namespace = "app.musicfrog.future" }\n',
                encoding="utf-8",
            )
            problems = GUARD.source_problems(root)
        self.assertTrue(
            any("android/future-app/build.gradle.kts has no registered product identity" in item for item in problems)
        )

    def test_registered_android_application_product_cannot_be_implemented_twice(self):
        with tempfile.TemporaryDirectory() as directory:
            root = fixture_repo(Path(directory))
            module = root / "android/duplicate-app/build.gradle.kts"
            module.parent.mkdir(parents=True)
            module.write_text(
                'plugins { id("com.android.application") }\n'
                'val composeApplicationId = identityValue("compose.application_id")\n'
                'android { defaultConfig { applicationId = composeApplicationId } }\n',
                encoding="utf-8",
            )
            problems = GUARD.source_problems(root)
        self.assertTrue(any("claimed by multiple application modules" in item for item in problems))

    def test_new_rust_executable_cannot_skip_product_identity(self):
        with tempfile.TemporaryDirectory() as directory:
            root = fixture_repo(Path(directory))
            manifest = root / "crates/infiltrator-preview/Cargo.toml"
            manifest.parent.mkdir(parents=True)
            manifest.write_text(
                "[package]\nname = \"infiltrator-preview\"\n"
                "version.workspace = true\nedition = \"2024\"\n",
                encoding="utf-8",
            )
            source = manifest.parent / "src/main.rs"
            source.parent.mkdir(parents=True)
            source.write_text("fn main() {}\n", encoding="utf-8")
            problems = GUARD.source_problems(root)
        self.assertTrue(
            any("Rust executable 'infiltrator-preview' has no product identity" in item for item in problems)
        )

    def test_android_icon_asset_must_exist_and_be_a_png_source(self):
        with tempfile.TemporaryDirectory() as directory:
            root = fixture_repo(Path(directory))
            path = root / "product-identity.properties"
            contents = path.read_text(encoding="utf-8")
            path.write_text(
                contents.replace(
                    "compose.icon_asset=crates/infiltrator-shared/assets/product/icon.png",
                    "compose.icon_asset=crates/infiltrator-shared/assets/product/future_icon.png",
                ),
                encoding="utf-8",
            )
            problems = GUARD.source_problems(root)
        self.assertTrue(any("launcher icon asset is missing" in item for item in problems))
        with tempfile.TemporaryDirectory() as directory:
            root = fixture_repo(Path(directory))
            icon = root / "crates/infiltrator-shared/assets/product/future_icon.png"
            icon.write_bytes(b"fixture icon")
            path = root / "product-identity.properties"
            contents = path.read_text(encoding="utf-8")
            path.write_text(
                contents.replace(
                    "compose.icon_asset=crates/infiltrator-shared/assets/product/icon.png",
                    "compose.icon_asset=crates/infiltrator-shared/assets/product/future_icon.png",
                ),
                encoding="utf-8",
            )
            problems = GUARD.source_problems(root)
        self.assertTrue(any("must be a valid PNG source file" in item for item in problems))

    def test_bevy_diagnostic_apk_must_keep_registered_version_name_and_icon(self):
        with tempfile.TemporaryDirectory() as directory:
            root = fixture_repo(Path(directory))
            path = root / "scripts/build-bevy-apk.sh"
            contents = path.read_text(encoding="utf-8")
            path.write_text(
                contents.replace(
                    'BEVY_ICON_ASSET="$(identity_value bevy.icon_asset)"',
                    'BEVY_ICON_ASSET="crates/infiltrator-iced/icons/icon.png"',
                ),
                encoding="utf-8",
            )
            problems = GUARD.source_problems(root)
        self.assertTrue(
            any("must consume product identity via identity_value bevy.icon_asset" in item for item in problems)
        )

    def test_bevy_diagnostic_verifier_must_use_registered_apk_name(self):
        with tempfile.TemporaryDirectory() as directory:
            root = fixture_repo(Path(directory))
            path = root / "scripts/verify-bevy-apk.sh"
            contents = path.read_text(encoding="utf-8")
            path.write_text(
                contents.replace(
                    'APK_NAME="$(identity_value bevy.smoke.apk_name)"',
                    'APK_NAME="infiltrator-bevy-ui-smoke"',
                ),
                encoding="utf-8",
            )
            problems = GUARD.source_problems(root)
        self.assertTrue(
            any("must consume product identity via identity_value bevy.smoke.apk_name" in item for item in problems)
        )

    def test_unregistered_android_release_apk_check_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = fixture_repo(Path(directory))
            path = root / ".github/workflows/release.yml"
            with path.open("a", encoding="utf-8") as workflow:
                workflow.write('\n          --apk "future=$APK"\n')
            problems = GUARD.source_problems(root)
        self.assertTrue(any("checks unregistered product 'future'" in item for item in problems))

    def test_release_artifact_filename_must_use_the_registered_product_prefix(self):
        with tempfile.TemporaryDirectory() as directory:
            root = fixture_repo(Path(directory))
            path = root / ".github/workflows/release.yml"
            contents = path.read_text(encoding="utf-8")
            path.write_text(contents.replace(
                '"dist-android/${COMPOSE_ARTIFACT_PREFIX}-${PRODUCT_VERSION}-universal-release.apk"',
                '"dist-android/Hardcoded-${PRODUCT_VERSION}-universal-release.apk"',
            ))
            problems = GUARD.source_problems(root)
        self.assertTrue(
            any("must name the compose APK from its registered artifact prefix" in item for item in problems)
        )

    def test_android_upload_group_names_must_use_registered_product_prefixes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = fixture_repo(Path(directory))
            path = root / ".github/workflows/release.yml"
            contents = path.read_text(encoding="utf-8")
            path.write_text(
                contents.replace(
                    "name: ${{ env.COMPOSE_ARTIFACT_PREFIX }}-APKs",
                    "name: Infiltrator-Android-APKs",
                ),
                encoding="utf-8",
            )
            problems = GUARD.source_problems(root)
        self.assertTrue(
            any(
                "Android upload artifact name for compose must derive from its registered prefix" in item
                for item in problems
            )
        )

    def test_android_ci_builds_tests_and_checks_each_flavor(self):
        with tempfile.TemporaryDirectory() as directory:
            root = fixture_repo(Path(directory))
            path = root / "scripts/android-emulator-tests.sh"
            contents = path.read_text(encoding="utf-8")
            path.write_text(
                contents.replace(
                    ":app:testBevyDebugUnitTest",
                    ":app:testComposeDebugUnitTest",
                )
            )
            problems = GUARD.source_problems(root)
        self.assertTrue(any("missing :app:testBevyDebugUnitTest" in item for item in problems))

    def test_final_apk_badging_checks_package_version_name_and_icon(self):
        identity = GUARD.read_properties()
        version_name = identity["version.name"]
        version_code = GUARD.get_value(identity, "compose.version_code")
        with tempfile.TemporaryDirectory() as directory:
            apk = Path(directory) / "compose.apk"
            icon_source = REPO / "crates/infiltrator-shared/assets/product/icon-android-xhdpi.png"
            with ZipFile(apk, "w") as archive:
                archive.write(icon_source, "res/b7.png")
            badging = "\n".join(
                (
                    f"package: name='com.musicfrog.infiltrator' versionCode='{version_code}' versionName='{version_name}'",
                    "application-label:'MusicFrog Infiltrator'",
                    "application-icon-320:'res/b7.png'",
                )
            )
            with patch.object(GUARD.subprocess, "run", return_value=type("Result", (), {"stdout": badging})()):
                problems = GUARD.apk_problems("compose", apk, Path("aapt"), identity)
        self.assertEqual(problems, [])

    def test_apk_icon_must_match_its_declared_density_bucket(self):
        identity = GUARD.read_properties()
        version_code = GUARD.get_value(identity, "compose.version_code")
        badging = (
            f"package: name='com.musicfrog.infiltrator' versionCode='{version_code}'"
            " versionName='ignored'\n"
            "application-label:'MusicFrog Infiltrator'\n"
            "application-icon-320:'res/b7.png'\n"
        )
        with tempfile.TemporaryDirectory() as directory:
            apk = Path(directory) / "compose.apk"
            with ZipFile(apk, "w") as archive:
                archive.write(
                    REPO / "crates/infiltrator-shared/assets/product/icon-android-hdpi.png",
                    "res/b7.png",
                )
            with patch.object(GUARD.subprocess, "run", return_value=type("Result", (), {"stdout": badging})()):
                problems = GUARD.apk_problems("compose", apk, Path("aapt"), identity)
        self.assertTrue(any("registered density icon dimensions" in item for item in problems))

    def test_icon_variant_assets_must_exist_with_declared_dimensions(self):
        with tempfile.TemporaryDirectory() as directory:
            root = fixture_repo(Path(directory))
            icon = root / "crates/infiltrator-shared/assets/product/icon-16.png"
            icon.write_bytes(b"truncated")
            problems = GUARD.source_problems(root)
        self.assertTrue(any("icon-16.png must be an exact 16px PNG" in item for item in problems))
        with tempfile.TemporaryDirectory() as directory:
            root = fixture_repo(Path(directory))
            path = root / "product-identity.properties"
            contents = path.read_text(encoding="utf-8")
            path.write_text(
                contents.replace(
                    "product.icon_variant_px=16,24,32,48,256",
                    "product.icon_variant_px=16,24,32,48,999",
                ),
                encoding="utf-8",
            )
            problems = GUARD.source_problems(root)
        self.assertTrue(any("icon-999.png must be an exact 999px PNG" in item for item in problems))

    def test_product_ico_must_mirror_the_shared_px_ladder(self):
        with tempfile.TemporaryDirectory() as directory:
            root = fixture_repo(Path(directory))
            ico = root / "crates/infiltrator-shared/assets/product/icon.ico"
            ico.write_bytes(
                b"\x00\x00\x01\x00\x01\x00"
                + bytes(14)
                + b"\x00\x00\x00\x00\x00\x00\x00\x00"
            )
            problems = GUARD.source_problems(root)
        self.assertTrue(any("must mirror the shared px ladder" in item for item in problems))

    def test_gradle_module_must_generate_icons_from_registered_density_variants(self):
        with tempfile.TemporaryDirectory() as directory:
            root = fixture_repo(Path(directory))
            path = root / "android/app/build.gradle.kts"
            source = path.read_text(encoding="utf-8")
            path.write_text(source.replace("product.android_icon_density", "product.drift"), encoding="utf-8")
            problems = GUARD.source_problems(root)
        self.assertTrue(any("icon resources from the registered density variants" in item for item in problems))

    def test_final_apk_icon_path_must_exist_in_the_apk(self):
        identity = GUARD.read_properties()
        version_name = identity["version.name"]
        version_code = GUARD.get_value(identity, "compose.version_code")
        with tempfile.TemporaryDirectory() as directory:
            apk = Path(directory) / "compose.apk"
            with ZipFile(apk, "w") as archive:
                archive.writestr("res/other.png", b"not the launcher icon")
            badging = "\n".join(
                (
                    f"package: name='com.musicfrog.infiltrator' versionCode='{version_code}' versionName='{version_name}'",
                    "application-label:'MusicFrog Infiltrator'",
                    "application-icon-320:'res/b7.png'",
                )
            )
            with patch.object(GUARD.subprocess, "run", return_value=type("Result", (), {"stdout": badging})()):
                problems = GUARD.apk_problems("compose", apk, Path("aapt"), identity)
        self.assertTrue(any("references missing icon resource 'res/b7.png'" in item for item in problems))


if __name__ == "__main__":
    unittest.main()
