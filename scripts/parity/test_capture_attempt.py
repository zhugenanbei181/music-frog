"""Capture retries retain failed-attempt diagnostics without rewriting them."""
import subprocess
import tempfile
import unittest
from pathlib import Path

HELPER = Path(__file__).with_name("capture_attempt.sh")

MATRICES = ("capture-slint-matrix.sh", "capture-iced-matrix.sh", "capture-bevy-matrix.sh")


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

    def settle(self, image, minimum="5000"):
        return subprocess.run(
            ["bash", "-c", 'source "$1"; capture_settled_image "$2" "$3"',
             "capture-attempt-test", str(HELPER), str(image), minimum],
            capture_output=True, text=True, check=False,
        )

    def test_settled_read_is_the_digest_of_the_bytes_that_remain(self):
        """A receipt must describe the file that gets published.

        The measured defect: niri returns from `screenshot-window` while the
        PNG is still being flushed, so the old loop broke on the first size
        above its floor and hashed a 65536-byte in-flight write of a 75920-byte
        frame. The manifest then indexed bytes that no longer existed.
        """
        import hashlib
        with tempfile.TemporaryDirectory() as directory:
            image = Path(directory) / "image.png"
            image.write_bytes(b"settled frame content" * 400)
            result = self.settle(image)
            self.assertEqual(result.returncode, 0, result.stderr)
            size, digest = result.stdout.split()
            payload = image.read_bytes()
            self.assertEqual(int(size), len(payload))
            self.assertEqual(digest, hashlib.sha256(payload).hexdigest())

    def test_a_file_still_being_written_is_not_receipted(self):
        """The size must stop changing before it is read; a growing file is not done."""
        import hashlib
        with tempfile.TemporaryDirectory() as directory:
            image = Path(directory) / "image.png"
            grower = subprocess.Popen(
                ["bash", "-c", 'f="$1"; : >"$f"; for i in $(seq 1 60); do '
                 'printf "chunk-%03d\\n" "$i" >>"$f"; sleep 0.05; done',
                 "grower", str(image)],
            )
            try:
                result = self.settle(image, minimum="10")
                self.assertEqual(result.returncode, 0, result.stderr)
                size, digest = result.stdout.split()
            finally:
                grower.terminate()
                grower.wait(timeout=10)
            # The writer keeps appending long after settle's first poll, so a
            # receipt taken early would index bytes that no longer exist.
            payload = image.read_bytes()
            self.assertEqual(int(size), len(payload))
            self.assertEqual(digest, hashlib.sha256(payload).hexdigest())

    def test_an_absent_or_tiny_file_is_refused(self):
        """No receipt may be invented for a frame that never arrived."""
        with tempfile.TemporaryDirectory() as directory:
            missing = Path(directory) / "missing.png"
            self.assertNotEqual(self.settle(missing).returncode, 0)
            tiny = Path(directory) / "tiny.png"
            tiny.write_bytes(b"partial")
            self.assertNotEqual(self.settle(tiny).returncode, 0)

    def test_every_matrix_reads_its_receipt_from_the_settled_file(self):
        """The race is fixed once, in the shared harness, for all three surfaces."""
        root = Path(__file__).resolve().parents[2]
        for name in MATRICES:
            text = (root / "scripts" / name).read_text(encoding="utf-8")
            self.assertIn("capture_settled_image", text, name)
            self.assertNotIn('bytes="$(stat -c%s "$image")"', text,
                             f"{name} still sizes the image inside the settle loop")
            self.assertNotIn('hash="$(sha256sum "$image" | cut', text,
                             f"{name} still hashes a possibly in-flight write")
