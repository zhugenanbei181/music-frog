"""Capture retries retain failed-attempt diagnostics without rewriting them."""
import subprocess
import tempfile
import unittest
from pathlib import Path

HELPER = Path(__file__).with_name("capture_attempt.sh")


class CaptureAttemptTests(unittest.TestCase):
    def archive(self, scenario, attempt, status):
        return subprocess.run(
            ["bash", "-c", 'source "$1"; archive_capture_attempt "$2" "$3" "$4" 42 7',
             "capture-attempt-test", str(HELPER), str(scenario), str(attempt), status],
            capture_output=True, text=True, check=False,
        )

    def test_retry_preserves_each_real_log_and_receipt(self):
        with tempfile.TemporaryDirectory() as directory:
            scenario = Path(directory)
            (scenario / "app.log").write_text("renderer failed before window creation\n")
            (scenario / "windows.err").write_text("IPC was unavailable\n")
            (scenario / "diagnostic.png").write_bytes(b"unaccepted compositor diagnostic")
            (scenario / "rendered-frame.png").write_bytes(b"native renderer evidence")
            self.assertEqual(self.archive(scenario, 1, "failed-window").returncode, 0)
            (scenario / "app.log").write_text("second renderer produced a frame\n")
            (scenario / "image.png").write_bytes(b"captured frame fixture")
            self.assertEqual(self.archive(scenario, 2, "ok").returncode, 0)
            self.assertEqual((scenario / "attempt-1/app.log").read_text(),
                             "renderer failed before window creation\n")
            self.assertEqual((scenario / "attempt-1/windows.err").read_text(), "IPC was unavailable\n")
            self.assertFalse((scenario / "attempt-1/image.png").exists())
            self.assertEqual((scenario / "attempt-1/diagnostic.png").read_bytes(), b"unaccepted compositor diagnostic")
            self.assertEqual((scenario / "attempt-1/rendered-frame.png").read_bytes(), b"native renderer evidence")
            self.assertEqual((scenario / "attempt-2/app.log").read_text(),
                             "second renderer produced a frame\n")
            self.assertEqual((scenario / "attempt-2/image.png").read_bytes(), b"captured frame fixture")
            self.assertIn("1\tfailed-window\t42\t7", (scenario / "attempt-1/receipt.tsv").read_text())
            self.assertIn("2\tok\t42\t7", (scenario / "attempt-2/receipt.tsv").read_text())

    def test_repeated_archive_refuses_to_replace_existing_diagnostics(self):
        with tempfile.TemporaryDirectory() as directory:
            scenario = Path(directory)
            (scenario / "app.log").write_text("original failure")
            self.assertEqual(self.archive(scenario, 1, "failed-marker").returncode, 0)
            (scenario / "app.log").write_text("unrelated subsequent output")
            self.assertNotEqual(self.archive(scenario, 1, "ok").returncode, 0)
            self.assertEqual((scenario / "attempt-1/app.log").read_text(), "original failure")
