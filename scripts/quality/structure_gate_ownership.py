#!/usr/bin/env python3
"""Which structure-gate legs may call themselves unjudgeable, and what that costs.

The gate runs in two trees. A developer checkout carries the git-ignored
``.evidence/`` capture runs and a compiled exporter; a CI checkout carries the
committed corpus and neither of those. A leg whose verdict comes from one of
those inputs has nothing to judge in one of the trees.

Both earlier answers were wrong, and both were measured here. Absence treated as
a pass hides a standard: ``space_utilization.py --enforce`` printed a reason and
returned 0, so a pixel leg that judged zero frames reported green, while the
space standard says in as many words that an unjudged pixel leg must not be
written up as a pass. Absence treated as a failure makes the gate permanently
red: ``receipt-freshness-guard.py --mode enforce`` returned violations=1 in CI
over a corpus that was perfectly fine, while the same command was green on a
developer machine - so no ledger entry could ever be true of both trees.

The third outcome belongs to the leg, not to the harness. A leg that ran and
found no facts exits ``SKIP_EXIT_CODE``; the collector records it as skipped with
the leg's own sentence as the reason; this module owns the number and the short
list of legs entitled to use it. Two properties keep that safe: a leg that is not
on the list cannot buy its way out of a standard by returning the magic code, and
no second copy of "does this tree have the facts" exists here to drift out of
step with the leg that actually reads them.
"""

from __future__ import annotations

import unittest

# The exit code meaning "I ran, and this tree has no facts for me to judge". It
# is the whole protocol between a leg and the collector, so it lives only here:
# a leg that kept its own copy could silently redefine it as a failure.
SKIP_EXIT_CODE = 3

RECEIPT_FRESHNESS = (
    "python3 scripts/quality/receipt-freshness-guard.py --mode enforce"
)
SLINT_DOCK_MATERIAL = (
    "python3 scripts/quality/slint-dock-material-guard.py --mode enforce"
)
SPACE_UTILIZATION = "python3 scripts/parity/space_utilization.py --enforce"
SPACE_UTILIZATION_ICED = (
    "python3 scripts/parity/space_utilization.py --surface iced --enforce"
)
SPACE_CHECK_EXPORT = "python3 scripts/parity/space_utilization.py --check-export"

# command -> the fact it reads that does not exist in both trees. Every leg here
# still judges whatever its tree does have: the dock guard checks theme,
# contract and app_chrome *source* in every tree and skips only its pixel receipt
# half, so a CI checkout keeps that coverage instead of losing a whole standard.
SELF_SKIPPING_LEGS: dict[str, str] = {
    RECEIPT_FRESHNESS: "its newest publication is chosen from git-ignored "
                       ".evidence/ capture runs",
    SLINT_DOCK_MATERIAL: "its pixel receipt half reads the latest published "
                         "capture run",
    SPACE_UTILIZATION: "the full-matrix batch it measures lives in "
                       ".evidence/captures/",
    SPACE_UTILIZATION_ICED: "the full-matrix batch it measures lives in "
                            ".evidence/captures/",
    SPACE_CHECK_EXPORT: "the page-structure exporter is a build artifact, and a "
                        "no-compilation workflow never builds it",
}


def may_skip(command: str) -> bool:
    """Whether this exact leg is entitled to report itself unjudgeable."""
    return command.strip() in SELF_SKIPPING_LEGS


def skip_reason(command: str) -> str | None:
    """The standing reason a listed leg gives, or None when it may not skip."""
    return SELF_SKIPPING_LEGS.get(command.strip())


class OwnershipSelfTest(unittest.TestCase):
    def test_the_protocol_code_is_a_skip_and_never_a_pass(self) -> None:
        self.assertEqual(SKIP_EXIT_CODE, 3)
        self.assertNotEqual(SKIP_EXIT_CODE, 0)

    def test_the_capture_and_build_legs_may_skip(self) -> None:
        self.assertEqual(len(SELF_SKIPPING_LEGS), 5)
        for command in SELF_SKIPPING_LEGS:
            self.assertTrue(may_skip(command), command)

    def test_an_ordinary_leg_may_not_declare_itself_skipped(self) -> None:
        # Any script could exit 3 to make a standard disappear. Only the list
        # above is entitled to, which is what makes the third state safe.
        for command in (
            "python3 scripts/quality/import-guard.py --mode enforce",
            "python3 scripts/quality/capture-corpus-integrity-guard.py --mode enforce",
            "python3 scripts/quality/receipt-freshness-guard.py --self-test",
            "python3 scripts/parity/space_utilization.py --self-test",
            "python3 scripts/quality/slint-dock-material-guard.py --self-test",
        ):
            self.assertFalse(may_skip(command), command)
            self.assertIsNone(skip_reason(command))

    def test_every_skip_carries_a_written_reason(self) -> None:
        for command, reason in SELF_SKIPPING_LEGS.items():
            self.assertGreaterEqual(len(reason), 20, command)

    def test_the_dock_guard_still_owes_its_source_half(self) -> None:
        # Its reason names the pixel half only: planned-skipping the whole leg
        # would delete theme/contract coverage from every CI run.
        self.assertIn("pixel", skip_reason(SLINT_DOCK_MATERIAL))

    def test_the_code_is_asked_and_not_guessed(self) -> None:
        # The number comes from here, so a leg that hard-codes its own copy of it
        # is what this assertion exists to make unnecessary.
        import contextlib
        import io

        printed = io.StringIO()
        with contextlib.redirect_stdout(printed):
            self.assertEqual(main(["--skip-code"]), 0)
        self.assertEqual(printed.getvalue().strip(), str(SKIP_EXIT_CODE))


def main(argv: list[str] | None = None) -> int:
    import argparse

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--skip-code", action="store_true",
                        help="print the exit code a leg returns to report itself "
                             "skipped; the protocol lives only in this module")
    parser.add_argument("--allowed-skips", action="store_true",
                        help="print one command per line: the legs entitled to "
                             "report themselves skipped")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args(argv)
    if args.skip_code:
        print(SKIP_EXIT_CODE)
        return 0
    if args.allowed_skips:
        for command in sorted(SELF_SKIPPING_LEGS):
            print(command)
        return 0
    result = unittest.TextTestRunner(verbosity=1).run(
        unittest.defaultTestLoader.loadTestsFromTestCase(OwnershipSelfTest)
    )
    return 0 if result.wasSuccessful() else 1


if __name__ == "__main__":
    raise SystemExit(main())
