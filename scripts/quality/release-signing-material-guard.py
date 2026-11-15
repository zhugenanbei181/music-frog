#!/usr/bin/env python3
"""Keep the release signing material alive for the build that signs with it.

The Android release leg resolves the signing material into a keystore file, and
the release variant refuses to build without it - that is the intended
fail-closed behaviour, not something to work around. The runner also has to be
scrubbed of that material afterwards.

Both requirements are easy to satisfy in the wrong order. A cleanup step that
runs *before* the build deletes the very file the build requires, and the leg
then fails with "Release artifacts require the persistent release keystore"
while the material was present a moment earlier. The failure looks like a
missing secret, so it is easy to misattribute to the user's secrets instead of
to the step order.

This guard therefore pins the order of the three steps - resolve, build,
remove - and requires the removal to still happen when the build fails.
"""

from __future__ import annotations

import argparse
import re
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = Path(".github/workflows/release.yml")

RESOLVE_STEP = "Resolve the release signing material"
BUILD_STEP = "Build all Android release flavors"
REMOVE_STEP = "Remove the release keystore"

STEP_NAME = re.compile(r"^\s*-\s*name:\s*(?P<name>[^#]+?)\s*$")
STEP_IF = re.compile(r"^\s*if:\s*(?P<condition>.+?)\s*$")


def step_order(text: str) -> list[tuple[str, str]]:
    """Every step in the workflow as `(name, if-condition)` in file order."""
    steps: list[tuple[str, str]] = []
    current: str | None = None
    for line in text.splitlines():
        match = STEP_NAME.match(line)
        if match:
            current = match.group("name").strip().strip("'\"")
            steps.append((current, ""))
            continue
        condition = STEP_IF.match(line)
        if condition and steps:
            name, _ = steps[-1]
            steps[-1] = (name, condition.group("condition").strip())
    return steps


def workflow_problems(text: str) -> list[str]:
    steps = step_order(text)
    names = [name for name, _ in steps]
    problems: list[str] = []

    for wanted in (RESOLVE_STEP, BUILD_STEP, REMOVE_STEP):
        if wanted not in names:
            problems.append(f"the release workflow has no '{wanted}' step")

    if problems:
        return problems

    resolve = names.index(RESOLVE_STEP)
    build = names.index(BUILD_STEP)
    remove = names.index(REMOVE_STEP)

    if resolve > build:
        problems.append(
            f"'{RESOLVE_STEP}' must run before '{BUILD_STEP}': the build signs with "
            "the material the resolve step writes"
        )
    if remove < build:
        problems.append(
            f"'{REMOVE_STEP}' must run after '{BUILD_STEP}': scrubbing the keystore "
            "before the build deletes the file the release variant requires"
        )
    condition = dict(steps)[REMOVE_STEP]
    if "always" not in condition:
        problems.append(
            f"'{REMOVE_STEP}' must keep its `if: always()` condition so a failed "
            "build still leaves the runner without the signing material"
        )
    return problems


def _workflow_with(order: list[str]) -> str:
    body = "\n".join(f"      - name: {name}" for name in order)
    return f"jobs:\n  android:\n    steps:\n{body}\n"


class Fixture(unittest.TestCase):
    """Planted regressions: the order must bite, and the good order must pass."""

    def test_the_real_workflow_passes(self) -> None:
        text = (ROOT / WORKFLOW).read_text(encoding="utf-8")
        self.assertEqual(workflow_problems(text), [])

    def test_removing_before_the_build_is_rejected(self) -> None:
        problems = workflow_problems(
            _workflow_with([RESOLVE_STEP, REMOVE_STEP, BUILD_STEP])
        )
        self.assertTrue(any("must run after" in problem for problem in problems))

    def test_resolving_after_the_build_is_rejected(self) -> None:
        problems = workflow_problems(
            _workflow_with([BUILD_STEP, RESOLVE_STEP, REMOVE_STEP])
        )
        self.assertTrue(any("must run before" in problem for problem in problems))

    def test_a_missing_removal_step_is_rejected(self) -> None:
        problems = workflow_problems(_workflow_with([RESOLVE_STEP, BUILD_STEP]))
        self.assertTrue(any(REMOVE_STEP in problem for problem in problems))

    def test_a_removal_without_always_is_rejected(self) -> None:
        text = (
            "jobs:\n  android:\n    steps:\n"
            f"      - name: {RESOLVE_STEP}\n"
            f"      - name: {BUILD_STEP}\n"
            f"      - name: {REMOVE_STEP}\n"
            "        if: success()\n"
        )
        problems = workflow_problems(text)
        self.assertTrue(any("always" in problem for problem in problems))


def self_test() -> int:
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(Fixture)
    result = unittest.TextTestRunner(verbosity=1).run(suite)
    if not result.wasSuccessful():
        return 1
    print("release signing material guard self-test: PASS")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--mode", default="enforce", choices=["enforce", "report"])
    arguments = parser.parse_args()
    if arguments.self_test:
        return self_test()

    text = (ROOT / WORKFLOW).read_text(encoding="utf-8")
    problems = workflow_problems(text)
    if problems:
        print("release signing material guard: FAILED", file=sys.stderr)
        for problem in problems:
            print(f"- {problem}", file=sys.stderr)
        return 1
    print(
        "release signing material guard: OK (the keystore outlives the build that "
        "signs with it and the runner is scrubbed afterwards)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
