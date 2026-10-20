"""The vendored Bevy scene gate must stay the shared implementation and must bite.

Two properties are verified, because a gate that does neither is decoration:

1. The vendored gate file is byte-identical to the one the shared package
   ``zhuge-ui-kit`` ships, pinned by the digest recorded in the ratchet ledger.
2. The gate fails on a new px literal that is not in the ledger and passes on
   the recorded debt it was written against.
"""

import hashlib
import json
import pathlib
import subprocess
import sys
import tempfile
import unittest

REPOSITORY = pathlib.Path(__file__).resolve().parents[3]
GATE = REPOSITORY / "scripts/quality/bevy_scene_gate.py"
LEDGER = REPOSITORY / "scripts/quality/bevy_scene_ratchet.json"


def run_gate(*arguments: str) -> subprocess.CompletedProcess[str]:
    """Run the vendored gate and capture its verdict."""

    return subprocess.run(
        [sys.executable, str(GATE), *arguments],
        capture_output=True,
        text=True,
        check=False,
    )


class BevySceneGateTests(unittest.TestCase):
    def test_ledger_pins_the_digest_of_the_vendored_gate(self):
        payload = json.loads(LEDGER.read_text(encoding="utf-8"))
        recorded = payload.get("gate", {}).get("sha256")
        self.assertTrue(recorded, "ratchet ledger does not record the gate digest")
        actual = hashlib.sha256(GATE.read_bytes()).hexdigest()
        self.assertEqual(recorded, actual)

    def test_recorded_debt_passes_and_a_new_px_literal_fails(self):
        with tempfile.TemporaryDirectory(prefix="bevy-scene-gate-") as raw:
            workspace = pathlib.Path(raw)
            tree = workspace / "src"
            tree.mkdir()
            (tree / "page.rs").write_text(
                "fn scene() { bsn! { Node { width: px(4.0) } } }\n", encoding="utf-8"
            )
            ledger = workspace / "ledger.json"
            written = run_gate("--root", str(tree), "--write-baseline", str(ledger))
            self.assertEqual(written.returncode, 0, written.stderr)
            passing = run_gate("--root", str(tree), "--baseline", str(ledger))
            self.assertEqual(passing.returncode, 0, passing.stderr)
            (tree / "other.rs").write_text(
                "fn scene() { bsn! { Node { height: px(8.0) } } }\n", encoding="utf-8"
            )
            failing = run_gate("--root", str(tree), "--baseline", str(ledger))
            self.assertEqual(failing.returncode, 1)
            self.assertIn("UIK-REM-001", failing.stderr)


if __name__ == "__main__":
    unittest.main()
