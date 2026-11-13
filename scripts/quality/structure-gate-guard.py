#!/usr/bin/env python3
"""GATERUN-001: the structure gate must execute every leg, and declare its red set.

`check-structure.sh` is the single executor of every mandatory standard in the
repository. It used to run as a flat list under `set -e`, so the first failing
leg aborted the batch and **every leg after it silently never ran** - locally
and in `main-gate.yml`. A red `i18n-guard --mode enforce` sat behind that
abort for 121 batches (`I18NNAV-001`) while the gate was described as having
"one known red leg", which was true of the exit code and false of the work.

The gate now collects: each leg runs, its status and first failure signature
are recorded, and the batch summarises once. This guard holds the contract
that makes the collector trustworthy rather than merely chatty:

1. the script must not abort on error (`set -e` is banned; `-uo pipefail` stays);
2. every command line must go through the `gate` runner - a bare leg would fail
   open by aborting again;
3. `structure_gate_ledger.json` must declare exactly the legs that are red now,
   each with an owner and a written reason, and every declared command must be
   a real leg (no stale declaration kept as a permanent excuse);
4. a leg that turns green must be removed from the ledger, so the ledger can
   never hide a regression by staying "already known red";
5. the summary must report the number of executed legs, and the failure list
   must be surfaced rather than swallowed.

Rules 3 and 4 compare against the observed failures handed over by the gate
through `--failures`; without that argument only the static rules run, which is
what CI's own leg check needs.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts" / "quality"))
from structure_gate_ownership import SELF_SKIPPING_LEGS, may_skip
GATE = ROOT / "scripts/quality/check-structure.sh"
ORIGINAL_GATE = GATE
LEDGER = ROOT / "scripts/quality/structure_gate_ledger.json"
OWNERSHIP = ROOT / "scripts/quality/structure_gate_ownership.py"

COMMAND_START = re.compile(r"^(?:python3|bash|cargo|grep|sed|awk)\s")
WRAPPED = re.compile(r"^gate\s")
# The reconciler is the collector's own machinery, not a standard: it must run
# after every leg and read the failure file, so it is deliberately unwrapped.
# The collector's own machinery, not standards: the reconciler reads the failure
# and skip lists after every leg, and the plan asks the ownership module which
# legs this tree cannot judge. Both must run unwrapped (they are the harness), so
# they are named here rather than being counted as collected legs.
RECONCILER = "python3 scripts/quality/structure-gate-guard.py --mode enforce --failures"
SKIP_CODE_PROBE = "python3 scripts/quality/structure_gate_ownership.py --skip-code"
MACHINERY_PREFIXES = (RECONCILER, SKIP_CODE_PROBE)
LEDGER_KEYS = {"version", "note", "declared_red_legs"}
LEG_KEYS = {"command", "owner", "reason"}


def display(path: pathlib.Path) -> str:
    """Repo-relative when possible; planted fixtures live outside the repo."""
    try:
        return str(path.relative_to(ROOT))
    except ValueError:
        return f"planted:{path.name}"


def gate_lines() -> list[str]:
    return GATE.read_text(encoding="utf-8").splitlines()


def command_lines() -> list[str]:
    """Every shell line that starts a command, wrapped or not."""
    result = []
    for line in gate_lines():
        stripped = line.lstrip()
        if COMMAND_START.match(stripped) or WRAPPED.match(stripped):
            result.append(stripped)
    return result


def unwrapped_commands() -> list[str]:
    return [line for line in command_lines() if not WRAPPED.match(line)]


def wrapped_commands() -> list[str]:
    return [line[len("gate "):].strip() for line in command_lines() if WRAPPED.match(line)]


def ledger_document() -> dict:
    return json.loads(LEDGER.read_text(encoding="utf-8"))


def read_failures(path: pathlib.Path) -> list[str]:
    commands: list[str] = []
    for line in path.read_text(encoding="utf-8").splitlines():
        parts = line.split("\t")
        if len(parts) >= 2 and parts[1].strip():
            commands.append(parts[1].strip())
    return commands


def read_skips(path: pathlib.Path) -> list[tuple[str, str]]:
    """Command and written reason from the collector's skip TSV.

    A row is returned even when its reason is missing. Dropping it would
    reclassify an unexplained hole in a standard as "that leg simply ran", which
    is the thing this gate exists to forbid; it is reported instead.
    """
    entries: list[tuple[str, str]] = []
    for line in path.read_text(encoding="utf-8").splitlines():
        parts = line.split("\t")
        if len(parts) >= 2 and parts[1].strip():
            reason = parts[2].strip() if len(parts) >= 3 else ""
            entries.append((parts[1].strip(), reason))
    return entries


def static_findings() -> list[str]:
    problems: list[str] = []
    lines = gate_lines()
    for line in lines:
        if re.match(r"^set\s", line) and "e" in re.findall(r"-\w+", line)[0][1:]:
            problems.append(
                f"{display(GATE)}: `set -e` aborts the batch at the first "
                "red leg, so later standards never execute (GATERUN-001)"
            )
    unwrapped = unwrapped_commands()
    strays = [line for line in unwrapped
              if not any(line.startswith(prefix) for prefix in MACHINERY_PREFIXES)]
    if strays:
        problems.append(
            f"{display(GATE)}: every leg must be collected; the only unwrapped "
            f"lines allowed are the collector's own machinery, found: "
            f"{strays[0][:90]}"
        )
    if not OWNERSHIP.is_file():
        problems.append("structure gate ownership module is missing")
    if "GATE_SKIPS" not in "\n".join(lines) or "SKIPPED_LEGS" not in "\n".join(lines):
        problems.append("structure gate collector has no typed skipped state")
    if not any(SKIP_CODE_PROBE in line for line in lines):
        problems.append("structure gate collector does not ask the ownership "
                        "module for the skip protocol")
    if not any("leg(s) executed, %d failed, %d skipped" in line for line in lines):
        problems.append("structure gate summary does not report skipped legs")
    if not any('cat "$GATE_SKIPS"' in line for line in lines):
        problems.append("structure gate swallows the skip list instead of surfacing it")
    if "--skips" not in "\n".join(lines):
        problems.append("structure gate reconciler never receives the skip list")
    if "GATE_INDEX" not in "\n".join(lines) or "FAILED_LEGS" not in "\n".join(lines):
        problems.append("the collector does not count executed and failed legs")
    if not any("leg(s) executed" in line for line in lines):
        problems.append("the gate never reports how many legs executed")
    if not any("cat \"$GATE_FAILURES\"" in line for line in lines):
        problems.append("the gate swallows the failure list instead of surfacing it")
    if not LEDGER.is_file():
        problems.append("structure gate ledger is missing")
        return problems
    document = ledger_document()
    unexpected = set(document) - LEDGER_KEYS
    if unexpected:
        problems.append(f"ledger has unexpected keys: {sorted(unexpected)}")
    if document.get("version") != 1:
        problems.append(f"ledger version must be 1, found {document.get('version')!r}")
    declared = document.get("declared_red_legs")
    if not isinstance(declared, list):
        problems.append("ledger declared_red_legs must be a list")
        return problems
    seen: set[str] = set()
    legs = wrapped_commands()
    for entry in declared:
        missing = LEG_KEYS - set(entry)
        if missing:
            problems.append(f"ledger entry lacks {sorted(missing)}")
            continue
        command = entry["command"].strip()
        if not command.startswith(("python3 ", "bash ", "cargo ")):
            problems.append(f"ledger command is not a gate leg: {command[:60]}")
        if command not in legs:
            problems.append(f"ledger declares a leg that does not exist: {command[:80]}")
        if command in seen:
            problems.append(f"ledger declares the same leg twice: {command[:60]}")
        seen.add(command)
        for key in ("owner", "reason"):
            if len(entry[key].strip()) < 12:
                problems.append(
                    f"ledger {key} for {command[:50]} must be a written sentence, not a token"
                )
    return problems


def declared_ledger() -> frozenset[str]:
    """Commands the ledger currently excuses for being red."""
    if not LEDGER.is_file():
        return frozenset()
    document = ledger_document()
    return frozenset(
        entry["command"].strip()
        for entry in document.get("declared_red_legs", [])
        if isinstance(entry, dict) and "command" in entry
    )


def findings(failures: list[str] | None) -> list[str]:
    problems = static_findings()
    if failures is None:
        return problems
    document = ledger_document()
    declared = {entry["command"].strip() for entry in document.get("declared_red_legs", [])}
    observed = set(failures)
    for command in sorted(observed - declared):
        problems.append(f"undeclared red leg: {command[:120]}")
    for command in sorted(declared - observed):
        problems.append(
            f"ledger declares a red leg that is now green: {command[:120]} "
            "(remove the declaration; keeping it would hide the next regression)"
        )
    return problems


def skip_findings(entries: list[tuple[str, str]] | None,
                  failures: list[str] | None,
                  declared: frozenset[str] = frozenset()) -> list[str]:
    """Reconcile the collector's third state against who may use it.

    A skip is neither a pass nor a failure, and it is the leg's own verdict: the
    collector never decides one on a leg's behalf, because only the leg knows
    which half of its standard still had facts to judge. That freedom needs
    three bounds. Only a listed leg may claim it, or any script could exit the
    magic number and make its standard vanish. It must say why, or an
    unexplained hole passes for a decision. And it cannot also be a failure or a
    leg the ledger is already excusing for being red - those are three different
    stories about the same run.
    """
    problems: list[str] = []
    if entries is None:
        return problems
    for command, reason in entries:
        if not may_skip(command):
            problems.append(
                f"leg may not declare itself skipped: {command[:120]} "
                "(only structure_gate_ownership.SELF_SKIPPING_LEGS may, so a "
                "standard cannot be switched off by its own exit code)"
            )
        if not reason:
            problems.append(f"skipped leg has no written reason: {command[:120]}")
    observed = {command for command, _reason in entries}
    for command in sorted(observed & set(failures or [])):
        problems.append(f"leg was both skipped and failed: {command[:120]}")
    for command in sorted(observed & set(declared)):
        problems.append(
            f"ledger declares a red leg that this run reported skipped: "
            f"{command[:120]}"
        )
    return problems


class GuardSelfTest(unittest.TestCase):
    """Every rule above is proved by planting the defect it forbids."""

    def setUp(self) -> None:
        self.globals = {"GATE": GATE, "LEDGER": LEDGER}
        self.original_leg = next(line[len("gate "):].strip() for line in ORIGINAL_GATE.read_text(encoding="utf-8").splitlines() if WRAPPED.match(line))

    def plant(self, **kwargs) -> None:
        """Point a module-level path at a planted file and remember the original."""
        for name, value in kwargs.items():
            globals()[name] = value
            self.addCleanup(self._restore, name)

    def _restore(self, name: str) -> None:
        globals()[name] = self.globals[name]

    def planted_ledger(self, entries: list[dict]) -> pathlib.Path:
        directory = tempfile.mkdtemp()
        path = pathlib.Path(directory) / "structure_gate_ledger.json"
        path.write_text(
            json.dumps({"version": 1, "note": "planted", "declared_red_legs": entries}),
            encoding="utf-8",
        )
        return path

    def one_leg(self) -> str:
        return self.original_leg

    def test_the_live_gate_passes(self) -> None:
        self.assertEqual(findings(None), [])

    def test_set_e_is_rejected(self) -> None:
        planted = pathlib.Path(tempfile.mkdtemp()) / "check-structure.sh"
        planted.write_text(
            GATE.read_text(encoding="utf-8").replace(
                "set -uo pipefail", "set -euo pipefail", 1
            ),
            encoding="utf-8",
        )
        self.plant(GATE=planted)
        self.assertTrue(any("set -e" in item for item in findings(None)))

    def test_a_bare_leg_is_rejected(self) -> None:
        text = GATE.read_text(encoding="utf-8").replace(
            "gate " + self.one_leg(), self.one_leg(), 1
        )
        planted = pathlib.Path(tempfile.mkdtemp()) / "check-structure.sh"
        planted.write_text(text, encoding="utf-8")
        self.plant(GATE=planted)
        self.assertTrue(any("every leg must be collected" in item for item in findings(None)))

    def test_the_collector_machinery_may_stay_unwrapped(self) -> None:
        # The two harness lines may run bare; nothing else may.
        self.assertEqual(
            sorted(unwrapped_commands()),
            sorted(line for line in unwrapped_commands()
                   if any(line.startswith(prefix) for prefix in MACHINERY_PREFIXES)),
        )

    def test_a_missing_ledger_is_rejected(self) -> None:
        self.plant(LEDGER=pathlib.Path(tempfile.mkdtemp()) / "missing.json")
        self.assertTrue(any("ledger is missing" in item for item in findings(None)))

    def test_a_red_leg_without_a_declaration_is_rejected(self) -> None:
        self.plant(LEDGER=self.planted_ledger([]))
        problems = findings([self.one_leg()])
        self.assertTrue(any("undeclared red leg" in item for item in problems), problems)

    def test_a_stale_declaration_is_rejected(self) -> None:
        entry = {
            "command": self.one_leg(),
            "owner": "infiltrator-contract",
            "reason": "planted reason long enough to satisfy the rule",
        }
        self.plant(LEDGER=self.planted_ledger([entry]))
        problems = findings([])
        self.assertTrue(any("now green" in item for item in problems), problems)

    def test_an_exact_red_set_is_accepted(self) -> None:
        entry = {
            "command": self.one_leg(),
            "owner": "infiltrator-contract",
            "reason": "planted reason long enough to satisfy the rule",
        }
        self.plant(LEDGER=self.planted_ledger([entry]))
        self.assertEqual(findings([entry["command"]]), [])

    def test_a_short_reason_is_rejected(self) -> None:
        entry = {
            "command": self.one_leg(),
            "owner": "infiltrator-contract",
            "reason": "red",
        }
        self.plant(LEDGER=self.planted_ledger([entry]))
        self.assertTrue(
            any("written sentence" in item for item in findings(None)),
        )

    def test_a_declaration_for_a_missing_leg_is_rejected(self) -> None:
        entry = {
            "command": "python3 scripts/quality/this-leg-does-not-exist.py --mode enforce",
            "owner": "infiltrator-contract",
            "reason": "planted reason long enough to satisfy the rule",
        }
        self.plant(LEDGER=self.planted_ledger([entry]))
        self.assertTrue(
            any("does not exist" in item for item in findings(None)),
        )

    # --- the third outcome: a leg may call itself unjudgeable, on three bounds --

    def an_allowed_leg(self) -> str:
        return next(iter(SELF_SKIPPING_LEGS))

    def test_a_skip_row_is_read_even_without_a_reason(self) -> None:
        # Dropping such a row would silently reclassify an unexplained hole in a
        # standard as "that leg simply ran", which is what the gate forbids.
        path = pathlib.Path(tempfile.mkdtemp()) / "skips.tsv"
        path.write_text(f"c7\t{self.an_allowed_leg()}\t\n", encoding="utf-8")
        self.assertEqual(read_skips(path), [(self.an_allowed_leg(), "")])

    def test_a_skip_must_state_why(self) -> None:
        problems = skip_findings([(self.an_allowed_leg(), "")], [])
        self.assertTrue(any("no written reason" in item for item in problems), problems)

    def test_an_unlisted_leg_may_not_declare_itself_skipped(self) -> None:
        # The bound that makes the third state safe: a magic exit code cannot buy
        # a standard out of existence.
        problems = skip_findings([(self.one_leg(), "planted reason")], [])
        self.assertTrue(
            any("may not declare itself skipped" in item for item in problems),
            problems,
        )

    def test_a_listed_leg_may_skip_with_a_reason(self) -> None:
        self.assertEqual(
            skip_findings([(self.an_allowed_leg(), "planted reason")], []), [],
        )

    def test_a_skip_cannot_be_counted_as_a_failure(self) -> None:
        command = self.an_allowed_leg()
        problems = skip_findings([(command, "planted reason")], [command])
        self.assertTrue(any("both skipped and failed" in item for item in problems))

    def test_a_skipped_leg_cannot_also_be_a_declared_red_leg(self) -> None:
        # Two different stories about one run: "this tree had no facts" and "it
        # had facts and is red for a known reason" cannot both be true.
        command = self.an_allowed_leg()
        problems = skip_findings(
            [(command, "planted reason")], [], declared=frozenset({command})
        )
        self.assertTrue(any("reported skipped" in item for item in problems), problems)

    def test_the_collector_never_skips_on_a_legs_behalf(self) -> None:
        # The shell has no plan table to consult: it reads only the code a leg
        # returns, so a hybrid standard keeps judging the half it can.
        text = GATE.read_text(encoding="utf-8")
        self.assertNotIn("--allowed-skips", text)
        self.assertIn("structure_gate_ownership.py --skip-code", text)

    def test_a_failure_line_is_parsed_from_the_collector_tsv(self) -> None:
        path = pathlib.Path(tempfile.mkdtemp()) / "failures.tsv"
        path.write_text(f"c7\t{self.one_leg()}\tsome signature\n", encoding="utf-8")
        self.assertEqual(read_failures(path), [self.one_leg()])


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("report", "enforce"), default="enforce")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument(
        "--failures",
        help="TSV of legs that just failed, written by the gate collector",
    )
    parser.add_argument(
        "--skips",
        help="TSV of legs the gate collector explicitly skipped, with reasons",
    )
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(GuardSelfTest)
        result = unittest.TextTestRunner(verbosity=1).run(suite)
        return 0 if result.wasSuccessful() else 1
    failures: list[str] | None = None
    if args.failures:
        path = pathlib.Path(args.failures)
        failures = read_failures(path) if path.is_file() else []
    skipped: list[tuple[str, str]] | None = None
    if args.skips:
        path = pathlib.Path(args.skips)
        skipped = read_skips(path) if path.is_file() else []
    problems = findings(failures)
    problems.extend(skip_findings(skipped, failures, declared_ledger()))
    for problem in problems:
        print(f"structure gate guard: {problem}", file=sys.stderr)
    if problems and args.mode == "enforce":
        return 1
    scope = (
        "reconciled with this run"
        if failures is not None and skipped is not None
        else "static shape only"
    )
    print(
        f"structure gate guard: {len(wrapped_commands())} collector leg(s), "
        f"{scope}, findings={len(problems)}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
