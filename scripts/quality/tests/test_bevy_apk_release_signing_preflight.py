import os
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[3]
SCRIPT = ROOT / "scripts" / "build-bevy-apk.sh"


class BevyApkReleaseSigningPreflightTests(unittest.TestCase):
    def run_release_builder(self, signing_environment):
        environment = os.environ.copy()
        environment.pop("CARGO_APK_RELEASE_KEYSTORE", None)
        environment.pop("CARGO_APK_RELEASE_KEYSTORE_PASSWORD", None)
        environment.pop("RELEASE", None)
        environment.update(signing_environment)
        result = subprocess.run(
            ["bash", str(SCRIPT), "--release"],
            cwd=ROOT,
            env=environment,
            capture_output=True,
            text=True,
            check=False,
        )
        return result

    def test_release_without_keystore_fails_before_toolchain_work(self):
        result = self.run_release_builder({})

        self.assertEqual(result.returncode, 1)
        self.assertIn("CARGO_APK_RELEASE_KEYSTORE is required", result.stderr)
        self.assertNotIn("installing Android", result.stdout)
        self.assertNotIn("cargo apk build", result.stdout)

    def test_release_with_missing_keystore_fails_before_toolchain_work(self):
        result = self.run_release_builder(
            {
                "CARGO_APK_RELEASE_KEYSTORE": str(ROOT / "target" / "missing.keystore"),
                "CARGO_APK_RELEASE_KEYSTORE_PASSWORD": "test-only",
            }
        )

        self.assertEqual(result.returncode, 1)
        self.assertIn("release keystore does not exist", result.stderr)
        self.assertNotIn("installing Android", result.stdout)

    def test_release_without_keystore_password_fails_before_toolchain_work(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            keystore = Path(temporary_directory) / "release.keystore"
            keystore.write_bytes(b"test fixture only")
            result = self.run_release_builder(
                {"CARGO_APK_RELEASE_KEYSTORE": str(keystore)}
            )

        self.assertEqual(result.returncode, 1)
        self.assertIn("CARGO_APK_RELEASE_KEYSTORE_PASSWORD is required", result.stderr)
        self.assertNotIn("installing Android", result.stdout)


if __name__ == "__main__":
    unittest.main()
