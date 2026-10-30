"""Static contract tests for the Android Mihomo lock-to-APK pipeline."""

import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

QUALITY = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "android_mihomo_assets_guard",
    QUALITY / "android-mihomo-assets-guard.py",
)
GUARD = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(GUARD)

VALID_FETCHER = '''
LOCK = ROOT / "packaging/mihomo-assets.json"
parser.add_argument("--stage", type=Path)
if sha256(archive) != asset["archive_sha256"]:
if sha256(binary) != asset["binary_sha256"]:
if requested != lock["version"]:
'''

VALID_GRADLE = '''
val fetchMihomoScript = rootProject.file("../scripts/fetch-mihomo.py")
val mihomoStagingDirectory = layout.buildDirectory.dir("generated/mihomo-assets")
fun fetchMihomoAsset(taskName: String, target: String, abi: String) =
    tasks.register<Exec>(taskName) {
        commandLine(
            "python3",
            fetchMihomoScript.absolutePath,
            "--target",
            target,
            "--stage",
            mihomoStagingDirectory.get().file("$abi/libmihomo.so").asFile.absolutePath,
        )
    }
fetchMihomoAsset("fetchMihomoArm64Asset", "aarch64-linux-android", "arm64-v8a")
fetchMihomoAsset("fetchMihomoX86_64Asset", "x86_64-linux-android", "x86_64")
tasks.register<Copy>("prepareMihomoAsset") {
    dependsOn(fetchMihomoArm64Asset, fetchMihomoX86_64Asset)
    from(mihomoStagingDirectory.get().dir("arm64-v8a")) {
        include("libmihomo.so")
        into("arm64-v8a")
    }
    from(mihomoStagingDirectory.get().dir("x86_64")) {
        include("libmihomo.so")
        into("x86_64")
    }
    into(layout.projectDirectory.dir("src/main/jniLibs"))
}
tasks.register<Exec>("verifyAndroidMihomoAssets") {}
tasks.named("preBuild") {
    dependsOn("prepareMihomoAsset")
    dependsOn("verifyAndroidMihomoAssets")
}
'''


def fixture_repo(root: Path) -> Path:
    for relative in ("android/app", "scripts", "packaging"):
        (root / relative).mkdir(parents=True, exist_ok=True)
    (root / "android/app/build.gradle.kts").write_text(VALID_GRADLE, encoding="utf-8")
    (root / "scripts/fetch-mihomo.py").write_text(VALID_FETCHER, encoding="utf-8")
    lock = {
        "version": "v1.19.33",
        "assets": [
            {
                "target": target,
                "binary_sha256": "a" * 64,
            }
            for target in GUARD.ANDROID_ASSETS
        ],
    }
    (root / "packaging/mihomo-assets.json").write_text(json.dumps(lock), encoding="utf-8")
    return root


class AndroidMihomoAssetsGuardTests(unittest.TestCase):
    def test_locked_fetch_and_abi_staging_contract_is_accepted(self):
        with tempfile.TemporaryDirectory() as directory:
            root = fixture_repo(Path(directory))
            self.assertEqual(GUARD.source_problems(root), [])

    def test_gradle_cannot_read_legacy_vendor_binaries(self):
        with tempfile.TemporaryDirectory() as directory:
            root = fixture_repo(Path(directory))
            gradle = root / "android/app/build.gradle.kts"
            gradle.write_text(
                gradle.read_text(encoding="utf-8")
                + '\nval oldKernel = rootProject.file("../vendor/mihomo-android-amd64")\n',
                encoding="utf-8",
            )
            self.assertTrue(
                any("must not read legacy vendor Mihomo binaries" in issue for issue in GUARD.source_problems(root))
            )

    def test_stale_vendor_files_are_irrelevant_to_the_android_asset_source(self):
        with tempfile.TemporaryDirectory() as directory:
            root = fixture_repo(Path(directory))
            vendor = root / "vendor"
            vendor.mkdir()
            (vendor / "mihomo-android-arm64-v8").write_bytes(b"old untrusted asset")
            (vendor / "mihomo-android-amd64").write_bytes(b"old untrusted asset")
            self.assertEqual(GUARD.source_problems(root), [])

    def test_both_abis_must_use_the_locked_fetcher_targets(self):
        with tempfile.TemporaryDirectory() as directory:
            root = fixture_repo(Path(directory))
            gradle = root / "android/app/build.gradle.kts"
            gradle.write_text(
                gradle.read_text(encoding="utf-8").replace(
                    '"aarch64-linux-android", "arm64-v8a"',
                    '"x86_64-linux-android", "arm64-v8a"',
                ),
                encoding="utf-8",
            )
            self.assertTrue(
                any("must stage aarch64-linux-android" in issue for issue in GUARD.source_problems(root))
            )

    def test_prebuild_must_run_the_asset_provenance_guard(self):
        with tempfile.TemporaryDirectory() as directory:
            root = fixture_repo(Path(directory))
            gradle = root / "android/app/build.gradle.kts"
            gradle.write_text(
                gradle.read_text(encoding="utf-8").replace(
                    '    dependsOn("verifyAndroidMihomoAssets")\n', ""
                ),
                encoding="utf-8",
            )
            self.assertTrue(
                any("dependsOn(\"verifyAndroidMihomoAssets\")" in issue for issue in GUARD.source_problems(root))
            )

    def test_fetcher_must_verify_archive_and_extracted_binary_hashes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = fixture_repo(Path(directory))
            fetcher = root / "scripts/fetch-mihomo.py"
            fetcher.write_text(
                fetcher.read_text(encoding="utf-8").replace(
                    'if sha256(binary) != asset["binary_sha256"]:', ""
                ),
                encoding="utf-8",
            )
            self.assertTrue(
                any("binary_sha256" in issue for issue in GUARD.source_problems(root))
            )


if __name__ == "__main__":
    unittest.main()
