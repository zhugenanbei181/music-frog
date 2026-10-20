#!/usr/bin/env python3
"""Prove that every environment-blocked board entry has an executable procedure.

The environment-blocked board (`todos/CODE_TODO_BLOCKED.md`) lists every task
whose remaining closure action needs an environment this machine does not
have: an ARM64 physical device, a controlled `v*` tag CI run, user-held
release keystore secrets, a crates.io upload, or a foreign target machine.

This checker is the tripwire that keeps that board executable:

  * every entry id of the board must appear in the mapping table of
    `scripts/blocked/README.md`;
  * every mapped entry must name a script that exists on disk;
  * every mapped entry must name the exact command to run;
  * every procedure script in `scripts/blocked/` must be mapped by at least
    one entry (no orphan script).

A new blocked entry that arrives without a procedure fails this checker, and
because every script in `scripts/blocked/` runs it as its first step, the
failure surfaces the moment anybody tries to close an entry.

Usage:
    python3 scripts/blocked/check-blocked-coverage.py
    python3 scripts/blocked/check-blocked-coverage.py --json
    python3 scripts/blocked/check-blocked-coverage.py --self-test
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BLOCKED_DIR = ROOT / "scripts" / "blocked"
BOARD = ROOT / "todos" / "CODE_TODO_BLOCKED.md"
REPORT = BLOCKED_DIR / "README.md"

# `- [ ] <ID> [kind: ...]` — the shape every board entry uses.
BOARD_ENTRY = re.compile(r"^-\s+\[(?P<mark>[ xX])\]\s+(?P<id>[A-Z][A-Z0-9]*-[0-9][^\s]*)\s+\[kind:")

# Scripts that are infrastructure rather than a board procedure.
INFRASTRUCTURE = {"lib.sh", "check-blocked-coverage.py", "README.md", "run-all.sh"}


def strip_cell(cell: str) -> str:
    """Remove the Markdown decoration of a table cell."""
    text = cell.strip()
    text = re.sub(r"^`", "", text)
    text = re.sub(r"`$", "", text)
    return text.strip()


def board_entries(text: str, only_open: bool = True) -> list[str]:
    """Every entry id of the blocked board, in board order, without duplicates.

    `only_open` keeps the entries that are still `- [ ]`; a closed entry
    (`- [x]`) no longer needs a procedure.
    """
    found: list[str] = []
    for line in text.splitlines():
        match = BOARD_ENTRY.match(line)
        if not match:
            continue
        if only_open and match.group("mark") != " ":
            continue
        entry = match.group("id")
        if entry not in found:
            found.append(entry)
    return found


def mapping_rows(text: str) -> list[dict]:
    """The mapping table of the report: one record per table row."""
    rows: list[dict] = []
    for line in text.splitlines():
        stripped = line.strip()
        if not stripped.startswith("|"):
            continue
        cells = [strip_cell(cell) for cell in stripped.strip("|").split("|")]
        if len(cells) < 5:
            continue
        entry, script, command = cells[0], cells[1], cells[2]
        if not entry or entry in {"Board entry", "---"} or set(entry) == {"-"}:
            continue
        if entry.lower() in {"entry", "条目"}:
            continue
        rows.append(
            {
                "entry": entry,
                "script": script,
                "command": command,
                "evidence": cells[3],
                "environment": cells[4],
            }
        )
    return rows


def procedure_scripts(directory: Path | None = None) -> list[str]:
    """Every procedure script of scripts/blocked/, sorted."""
    base = directory if directory is not None else BLOCKED_DIR
    if not base.is_dir():
        return []
    return sorted(
        path.name
        for path in base.glob("*.sh")
        if path.name not in INFRASTRUCTURE
    )


def check(
    root: Path = ROOT,
    board: Path | None = None,
    report: Path | None = None,
    scripts_dir: Path | None = None,
) -> dict:
    board_path = board if board is not None else BOARD
    report_path = report if report is not None else REPORT
    problems: list[str] = []
    if not board_path.is_file():
        return {
            "ok": False,
            "problems": [f"blocked board is missing: {board_path}"],
            "entries": [],
            "mapped": [],
        }
    if not report_path.is_file():
        return {
            "ok": False,
            "problems": [f"blocked-board report is missing: {report_path}"],
            "entries": [],
            "mapped": [],
        }

    entries = board_entries(board_path.read_text(encoding="utf-8"))
    rows = mapping_rows(report_path.read_text(encoding="utf-8"))
    mapped = {row["entry"] for row in rows}

    for entry in entries:
        if entry not in mapped:
            problems.append(
                f"blocked entry `{entry}` has no procedure in {report_path.name}"
            )

    known_scripts = set(procedure_scripts(scripts_dir))
    for row in rows:
        script = row["script"]
        if not script:
            problems.append(f"entry `{row['entry']}` does not name a script")
            continue
        name = Path(script).name
        if not script.endswith(".sh") or name not in known_scripts:
            problems.append(
                f"entry `{row['entry']}` names an unknown script `{script}`"
            )
        if not row["command"]:
            problems.append(f"entry `{row['entry']}` does not name the exact command")
        if not row["evidence"]:
            problems.append(f"entry `{row['entry']}` does not name the evidence it produces")

    referenced = {Path(row["script"]).name for row in rows if row["script"]}
    for script in sorted(known_scripts - referenced):
        problems.append(
            f"procedure script `{script}` is not mapped by any blocked entry"
        )

    return {
        "ok": not problems,
        "problems": problems,
        "entries": entries,
        "mapped": sorted(mapped & set(entries)),
        "extra": sorted(mapped - set(entries)),
        "scripts": sorted(known_scripts),
    }


def self_test() -> int:
    board = """# board

- [ ] ALPHA-001 [kind: evidence] [status: Blocked] first
- [ ] ALPHA-002.3 [kind: evidence] [status: Blocked] second
- [x] ALPHA-009 [kind: evidence] done
"""
    report = """| Board entry | Script | Command | Evidence | Environment |
| --- | --- | --- | --- | --- |
| `ALPHA-001` | `scripts/blocked/arm64-device-evidence.sh` | `bash scripts/blocked/arm64-device-evidence.sh --dry-run` | plan | device |
| `ALPHA-002.3` | `scripts/blocked/arm64-device-evidence.sh` | `bash scripts/blocked/arm64-device-evidence.sh --dry-run` | plan | device |
"""
    assert board_entries(board) == ["ALPHA-001", "ALPHA-002.3"], board_entries(board)
    assert board_entries(board, only_open=False) == [
        "ALPHA-001",
        "ALPHA-002.3",
        "ALPHA-009",
    ], board_entries(board, only_open=False)
    rows = mapping_rows(report)
    assert len(rows) == 2, rows
    assert rows[0]["entry"] == "ALPHA-001", rows
    assert rows[0]["script"] == "scripts/blocked/arm64-device-evidence.sh", rows
    assert rows[0]["command"].startswith("bash scripts/blocked/"), rows
    assert rows[0]["evidence"] == "plan", rows
    assert rows[0]["environment"] == "device", rows

    # The negative path: an entry without a procedure must be reported, and an
    # unmapped procedure script must be reported as well.
    with tempfile.TemporaryDirectory() as directory:
        fixture = Path(directory)
        board_path = fixture / "CODE_TODO_BLOCKED.md"
        report_path = fixture / "README.md"
        scripts_path = fixture / "blocked"
        scripts_path.mkdir()
        (scripts_path / "arm64-device-evidence.sh").write_text("#!/bin/sh\n", encoding="utf-8")
        board_path.write_text(board, encoding="utf-8")
        report_path.write_text(report, encoding="utf-8")

        result = check(
            root=fixture,
            board=board_path,
            report=report_path,
            scripts_dir=scripts_path,
        )
        assert result["ok"], result["problems"]
        assert result["entries"] == ["ALPHA-001", "ALPHA-002.3"], result

        # A third open entry without a procedure must fail the check.
        board_path.write_text(
            board.replace("- [x] ALPHA-009", "- [ ] ALPHA-009"), encoding="utf-8"
        )
        result = check(
            root=fixture,
            board=board_path,
            report=report_path,
            scripts_dir=scripts_path,
        )
        assert not result["ok"], result
        assert any("ALPHA-009" in problem for problem in result["problems"]), result

        # A procedure script without a board entry must fail the check too.
        board_path.write_text(board, encoding="utf-8")
        (scripts_path / "orphan-procedure.sh").write_text("#!/bin/sh\n", encoding="utf-8")
        result = check(
            root=fixture,
            board=board_path,
            report=report_path,
            scripts_dir=scripts_path,
        )
        assert not result["ok"], result
        assert any("orphan-procedure.sh" in problem for problem in result["problems"]), result

    print("blocked-board coverage self-test: PASS")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--json", action="store_true", help="emit the result as JSON")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        return self_test()

    result = check()
    if args.json:
        print(json.dumps(result, indent=2, ensure_ascii=False))
    else:
        print(
            f"blocked-board coverage: {len(result['mapped'])}/{len(result['entries'])} "
            f"entries mapped, {len(result.get('scripts', []))} procedure scripts"
        )
        for problem in result["problems"]:
            print(f"- {problem}", file=sys.stderr)
    if result["extra"]:
        for entry in result["extra"]:
            print(f"- mapped entry `{entry}` is not on the blocked board", file=sys.stderr)
    return 0 if result["ok"] and not result["extra"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
