#!/usr/bin/env python3
"""Which structure-gate legs may call themselves unjudgeable, and what that costs.

The gate runs in two trees. A developer checkout carries the git-ignored
``.evidence/`` capture runs and a compiled exporter; a CI checkout carries the
committed corpus and neither of those. The no-compilation workflow
``main-gate.yml`` declares ``MUSICFROG_NO_COMPILE_GATE`` so a leg that needs to
build Rust can say, in that tree only, that its build half has nothing to
judge. A leg whose verdict comes from one of those inputs has nothing to judge
in one of the trees.

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

# The one declaration that a gate tree is compiled-free. The no-compilation
# workflow sets it; every leg that owns a build-dependent half reads the name
# from here. A second literal in a workflow or a guard would let the two drift.
NO_COMPILE_GATE_ENV = "MUSICFROG_NO_COMPILE_GATE"

# Values that truthfully mean "this is a no-compilation gate tree". Anything
# else (unset, "0", "false", a typo) is a compile-capable tree, so a stray
# value cannot silently switch off a build standard.
NO_COMPILE_GATE_TRUTHY = frozenset({"1", "true"})

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
RUSTDOC_DOCS = "python3 scripts/quality/rustdoc-guard.py --mode docs"
SCREENSHOT_IDENTITY = (
    "python3 scripts/quality/screenshot-identity-guard.py --mode enforce"
)

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
    RUSTDOC_DOCS: "its cargo doc build half needs compilation, which the "
                  f"{NO_COMPILE_GATE_ENV} workflow (main-gate.yml) forbids; "
                  "the offline docs.rs metadata and doctest audit still judges",
    SCREENSHOT_IDENTITY: "the published pixels it compares live in the "
                          "git-ignored .evidence/corpora/ trees, so a checkout "
                          "without a capture run has nothing to compare",
}


def may_skip(command: str) -> bool:
    """Whether this exact leg is entitled to report itself unjudgeable."""
    return command.strip() in SELF_SKIPPING_LEGS


def skip_reason(command: str) -> str | None:
    """The standing reason a listed leg gives, or None when it may not skip."""
    return SELF_SKIPPING_LEGS.get(command.strip())


def no_compile_gate(environment=None) -> bool:
    """Whether the given process environment declares a compiled-free gate tree.

    The value is read through the single env-name constant above. A leg may
    call this only after judging every half that does not need a build.
    """
    import os

    values = os.environ if environment is None else environment
    return str(values.get(NO_COMPILE_GATE_ENV, "")).strip().lower() in NO_COMPILE_GATE_TRUTHY


class OwnershipSelfTest(unittest.TestCase):
    def test_the_protocol_code_is_a_skip_and_never_a_pass(self) -> None:
        self.assertEqual(SKIP_EXIT_CODE, 3)
        self.assertNotEqual(SKIP_EXIT_CODE, 0)

    def test_the_capture_and_build_legs_may_skip(self) -> None:
        self.assertEqual(len(SELF_SKIPPING_LEGS), 7)
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

    def test_the_rustdoc_docs_leg_names_the_no_compilation_workflow(self) -> None:
        self.assertTrue(may_skip(RUSTDOC_DOCS))
        reason = skip_reason(RUSTDOC_DOCS)
        assert reason is not None
        self.assertIn(NO_COMPILE_GATE_ENV, reason)
        self.assertIn("main-gate.yml", reason)

    def test_only_a_truthful_env_value_declares_a_compiled_free_tree(self) -> None:
        for value in ("1", "true", "TRUE", " true "):
            self.assertTrue(no_compile_gate({NO_COMPILE_GATE_ENV: value}), value)
        for value in ("", "0", "false", "no", "yes"):
            self.assertFalse(no_compile_gate({NO_COMPILE_GATE_ENV: value}), value)
        self.assertFalse(no_compile_gate({}))
        self.assertFalse(no_compile_gate({"SOME_OTHER_GATE": "1"}))

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
