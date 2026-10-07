"""Guard the Android manifest's VPN, tile and boot declarations from removal."""
import importlib.util
from pathlib import Path
import sys
import tempfile
import unittest

QUALITY = Path(__file__).resolve().parents[1]
REPO = QUALITY.parents[1]
MANIFEST = REPO / "android/app/src/main/AndroidManifest.xml"
sys.path.insert(0, str(QUALITY))
SPEC = importlib.util.spec_from_file_location(
    "android_manifest_guard", QUALITY / "android-manifest-guard.py"
)
GUARD = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = GUARD
SPEC.loader.exec_module(GUARD)


class AndroidManifestRules(unittest.TestCase):
    def scan_text(self, text):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "AndroidManifest.xml"
            path.write_text(text, encoding="utf-8")
            return GUARD.scan(path)

    def test_real_manifest_declares_every_required_item(self):
        missing = GUARD.scan(MANIFEST)
        self.assertEqual(missing, {"permission": [], "service": [], "receiver": []})

    def test_removed_permission_is_reported(self):
        text = MANIFEST.read_text(encoding="utf-8")
        text = text.replace(
            '    <uses-permission android:name="android.permission.INTERNET" />\n',
            "",
        )
        missing = self.scan_text(text)
        self.assertIn(
            "uses-permission android.permission.INTERNET", missing["permission"]
        )

    def test_vpn_service_without_foreground_service_type_is_reported(self):
        text = MANIFEST.read_text(encoding="utf-8")
        text = text.replace(
            '            android:foregroundServiceType="systemExempted"\n', ""
        )
        missing = self.scan_text(text)
        self.assertTrue(
            any(
                ".MihomoVpnService" in item and "foregroundServiceType" in item
                for item in missing["service"]
            )
        )

    def test_missing_vpn_bind_permission_is_reported(self):
        text = MANIFEST.read_text(encoding="utf-8")
        text = text.replace(
            '            android:permission="android.permission.BIND_VPN_SERVICE">',
            ">",
        )
        missing = self.scan_text(text)
        self.assertIn(
            "service .MihomoVpnService must declare "
            "android:permission=android.permission.BIND_VPN_SERVICE",
            missing["service"],
        )

    def test_missing_boot_receiver_is_reported(self):
        text = MANIFEST.read_text(encoding="utf-8")
        text = text.replace('android:name=".BootReceiver"', 'android:name=".Other"')
        missing = self.scan_text(text)
        self.assertIn("receiver .BootReceiver is not declared", missing["receiver"])


if __name__ == "__main__":
    unittest.main()
