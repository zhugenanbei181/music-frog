"""Executable package provenance checks; no external network or real kernel process."""
import gzip
import importlib.util
import io
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import zipfile

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("kernel_fetch", ROOT / "scripts/fetch-mihomo.py")
kernel = importlib.util.module_from_spec(spec)
spec.loader.exec_module(kernel)


class KernelAssetTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.binary = b"test kernel bytes, never executed"
        self.archive = gzip.compress(self.binary)
        self.asset = {
            "archive": "mihomo-test-v1.2.3.gz", "binary": "mihomo-test",
            "target": "test-target", "archive_sha256": kernel.sha256(self.archive),
            "binary_sha256": kernel.sha256(self.binary),
        }
        self.lock = {"version": "v1.2.3", "assets": [self.asset]}

    def fetch(self, response):
        with patch.object(kernel.urllib.request, "urlopen", return_value=io.BytesIO(response)) as network:
            kernel.fetch_asset(self.asset, "v1.2.3", "https://example.test", self.root)
        return network

    def test_missing_and_corrupted_cache_are_replaced_only_by_verified_bytes(self):
        network = self.fetch(self.archive)
        network.assert_called_once()
        output = self.root / self.asset["binary"]
        self.assertEqual(output.read_bytes(), self.binary)
        output.write_bytes(b"corrupted cached kernel")
        self.fetch(self.archive)
        self.assertEqual(output.read_bytes(), self.binary)
        with patch.object(kernel.urllib.request, "urlopen") as network:
            kernel.fetch_asset(self.asset, "v1.2.3", "https://example.test", self.root)
        network.assert_not_called()
        self.assertEqual(list(self.root.iterdir()), [output])

    def test_wrong_archive_or_extracted_digest_preserves_existing_cache(self):
        output = self.root / self.asset["binary"]
        output.write_bytes(b"old local kernel")
        with self.assertRaisesRegex(ValueError, "archive checksum mismatch"):
            self.fetch(b"incorrect downloaded archive")
        self.assertEqual(output.read_bytes(), b"old local kernel")
        self.asset["binary_sha256"] = "0" * 64
        with self.assertRaisesRegex(ValueError, "extracted kernel checksum mismatch"):
            self.fetch(self.archive)
        self.assertEqual(output.read_bytes(), b"old local kernel")
        self.assertEqual(list(self.root.iterdir()), [output])

    def test_zip_requires_one_executable_and_keeps_its_exact_bytes(self):
        def archive(entries):
            data = io.BytesIO()
            with zipfile.ZipFile(data, "w") as file:
                for name, binary in entries:
                    file.writestr(name, binary)
            return data.getvalue()
        self.assertEqual(kernel.decode_archive("kernel.zip", archive([("dir/mihomo.exe", self.binary)])), self.binary)
        for entries in [[], [("first.exe", b"a"), ("second.exe", b"b")]]:
            with self.assertRaises(ValueError):
                kernel.decode_archive("kernel.zip", archive(entries))

    def test_lock_refuses_missing_digest_duplicate_identity_and_unsafe_paths(self):
        kernel.validate_lock(self.lock)
        for key, value in [("archive_sha256", ""), ("binary_sha256", "wrong"), ("binary", "../escape")]:
            invalid = json.loads(json.dumps(self.lock))
            invalid["assets"][0][key] = value
            with self.assertRaises(ValueError):
                kernel.validate_lock(invalid)
        self.lock["assets"].append(dict(self.asset))
        with self.assertRaises(ValueError):
            kernel.validate_lock(self.lock)

    def test_package_staging_requires_one_target_and_includes_license_and_provenance(self):
        lock_path = self.root / "lock.json"
        lock_path.write_text(json.dumps(self.lock))
        (self.root / "packaging").mkdir()
        (self.root / "packaging/Mihomo-LICENSE").write_bytes(b"fixture license")
        (self.root / "vendor").mkdir()
        (self.root / "vendor" / self.asset["binary"]).write_bytes(self.binary)
        output = self.root / "package with spaces/mihomo"
        with patch.object(kernel, "ROOT", self.root), patch.object(kernel, "LOCK", lock_path), \
             patch.dict(os.environ, {"MIHOMO_VERSION": "v1.2.3"}), \
             patch("sys.argv", ["fetch", "--target", "test-target", "--stage", str(output)]):
            kernel.main()
        self.assertEqual(output.read_bytes(), self.binary)
        self.assertEqual((output.parent / "Mihomo-LICENSE.txt").read_bytes(), b"fixture license")
        self.assertEqual(json.loads((output.parent / "mihomo-assets.json").read_text()), self.lock)

    def test_unlocked_version_or_target_never_calls_the_network(self):
        lock_path = self.root / "lock.json"
        lock_path.write_text(json.dumps(self.lock))
        for version, target in [("v9.9.9", "test-target"), ("v1.2.3", "unlocked-target")]:
            with patch.object(kernel, "LOCK", lock_path), patch.dict(os.environ, {"MIHOMO_VERSION": version}), \
                 patch("sys.argv", ["fetch", "--target", target]), patch.object(kernel.urllib.request, "urlopen") as network:
                with self.assertRaises(ValueError):
                    kernel.main()
                network.assert_not_called()


if __name__ == "__main__":
    unittest.main()
