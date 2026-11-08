"""The easing-owner tests must fail if their oracle stops calling the owner.

The six behavior tests in ``easing_owner_tests.rs`` are real composition proofs
only while their oracle calls the shared owner. This mutation check replaces
the owner import and its two calls with a local second implementation, then
runs an owner-binding census on the mutant: a local ramp expression is a
second authority and must be reported.
"""

from __future__ import annotations

import re
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPOSITORY = Path(__file__).resolve().parents[3]
TEST = REPOSITORY / "crates/infiltrator-bevy-widgets/tests/headless/easing_owner_tests.rs"
GATE = REPOSITORY / "scripts/quality/easing_owner_gate.py"
OWNER_IMPORT = "use zhuge_ui_core::easing::{CubicBezier, smoothstep};"
LOCAL_IMPORT = """use zhuge_ui_core::easing::{CubicBezier, smoothstep};

fn hand_written_ramp(t: f32) -> f32 { t * t * (3.0 - 2.0 * t) }
"""


def mutant(source: str) -> str:
    text = source.replace(OWNER_IMPORT, LOCAL_IMPORT, 1)
    text = text.replace("smoothstep(parameter)", "hand_written_ramp(parameter)")
    text = text.replace("smoothstep(0.5)", "hand_written_ramp(0.5)")
    return text


def second_ramp(source: str) -> str | None:
    """Return the hand-written smoothstep polynomial, if one is present."""

    match = re.search(r"\b\w+\s*\*\s*\w+\s*\*\s*\(\s*3\.0\s*-\s*2\.0\s*\*\s*\w+\s*\)", source)
    return match.group(0) if match else None


class EasingOwnerMutationTests(unittest.TestCase):
    def test_mutation_removes_every_smoothstep_call_from_the_oracle(self):
        source = TEST.read_text(encoding="utf-8")
        self.assertIn(OWNER_IMPORT, source)
        self.assertEqual(source.count("smoothstep("), 2)
        changed = mutant(source)
        self.assertNotIn("smoothstep(", changed)
        self.assertEqual(changed.count("hand_written_ramp("), 3)

    def test_owner_calls_are_replaced_and_the_mutant_holds_a_second_ramp(self):
        source = TEST.read_text(encoding="utf-8")
        changed = mutant(source)
        self.assertIsNone(second_ramp(source))
        self.assertEqual(second_ramp(changed), "t * t * (3.0 - 2.0 * t)")
        self.assertNotIn("smoothstep(", changed)
        self.assertIn("hand_written_ramp(parameter)", changed)
        self.assertIn("hand_written_ramp(0.5)", changed)

        with tempfile.TemporaryDirectory(
            prefix="uik-easing-mutation-",
            dir=TEST.parent,
        ) as raw:
            mutant_path = Path(raw) / "mutant_easing_owner_tests.rs"
            mutant_path.write_text(changed, encoding="utf-8")
            result = subprocess.run(
                [sys.executable, str(GATE), "--root", str(mutant_path)],
                capture_output=True,
                text=True,
                check=False,
            )
        self.assertEqual(result.returncode, 1)
        self.assertIn("UIK-EASE-001", result.stderr)
        self.assertIn("hand_written_ramp", result.stderr)


if __name__ == "__main__":
    unittest.main()
