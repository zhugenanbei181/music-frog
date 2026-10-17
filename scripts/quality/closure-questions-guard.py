#!/usr/bin/env python3
"""Enforce the eight closure questions on every batch.

`docs/CLOSURE_QUESTIONS.md` is the single source for the eight questions every
batch must answer. This guard fails when
  * the eight questions are not exactly 1..8,
  * the "current status" table has a missing row, an empty verdict or an
    evidence level outside {L, C, D, N/A},
  * the "latest batch answers" block is missing a numbered answer or leaves one
    empty,
  * the document is not registered in `docs/README.md`.

`--self-test` runs the same parsing against synthetic good and bad documents.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DOCUMENT = Path("docs/CLOSURE_QUESTIONS.md")
REGISTRY = Path("docs/README.md")
LEVEL_PATTERN = re.compile(r"\b(?:L|C|D)\b|N/A")
ANSWER_PATTERN = re.compile(r"^(\d)\.\s+(.+)$")
STATUS_PATTERN = re.compile(r"^\|\s*(\d)\s*\|\s*([^|]*)\|", re.MULTILINE)


def section(text: str, title: str) -> str:
    """Return the body of the first `## title…` section (date suffix allowed)."""
    lines = text.splitlines()
    start = None
    for index, line in enumerate(lines):
        if line.strip().startswith(title):
            start = index + 1
            break
    if start is None:
        return ""
    body: list[str] = []
    for line in lines[start:]:
        if line.startswith("## "):
            break
        body.append(line)
    return "\n".join(body)


def problems_for(text: str) -> list[str]:
    problems: list[str] = []

    questions = set(int(m.group(1)) for m in STATUS_PATTERN.finditer(section(text, "## 八问与合格证据")) if m.group(1) in "12345678")
    if questions != set(range(1, 9)):
        problems.append(f"the eight questions must be numbered 1..8, found {sorted(questions)}")

    status_rows = {}
    for match in STATUS_PATTERN.finditer(section(text, "## 当前状态")):
        number = int(match.group(1))
        verdict = match.group(2).strip()
        rest = match.string[match.end():]
        level_cell = rest.split("|", 1)[0].strip()
        status_rows[number] = (verdict, level_cell)
    missing = sorted(set(range(1, 9)) - set(status_rows))
    if missing:
        problems.append(f"current status is missing rows {missing}")
    for number, (verdict, level_cell) in sorted(status_rows.items()):
        if not verdict:
            problems.append(f"current status row {number} has an empty verdict")
        if not LEVEL_PATTERN.search(level_cell):
            problems.append(
                f"current status row {number} has no evidence level (L/C/D/N/A): {level_cell!r}"
            )

    answers = {}
    for line in section(text, "## 最近一批问答").splitlines():
        match = ANSWER_PATTERN.match(line.strip())
        if match:
            answers[int(match.group(1))] = match.group(2).strip()
    missing_answers = sorted(set(range(1, 9)) - set(answers))
    if missing_answers:
        problems.append(f"latest batch answers are missing {missing_answers}")
    for number, answer in sorted(answers.items()):
        if len(answer) < 8:
            problems.append(f"answer {number} is too short to be an answer: {answer!r}")
        elif not LEVEL_PATTERN.search(answer):
            problems.append(f"answer {number} carries no evidence level (L/C/D/N/A)")

    return problems


def check(root: Path = ROOT) -> list[str]:
    document = root / DOCUMENT
    registry = root / REGISTRY
    if not document.is_file():
        return [f"{document}: closure-question document is missing"]
    if not registry.is_file() or DOCUMENT.name not in registry.read_text(encoding="utf-8"):
        return [f"{registry}: does not register {DOCUMENT.name}"]
    problems = problems_for(document.read_text(encoding="utf-8"))
    return [f"{document}: {problem}" for problem in problems]


def self_test() -> int:
    good = """# Q

## 八问与合格证据

| # | 问题 |
| --- | --- |
| 1 | a |
| 2 | a |
| 3 | a |
| 4 | a |
| 5 | a |
| 6 | a |
| 7 | a |
| 8 | a |

## 当前状态（2026-10-17）

| # | 结论 | 等级 | 缺口 |
| --- | --- | --- | --- |
| 1 | ok | L | - |
| 2 | ok | C | - |
| 3 | ok | D | - |
| 4 | ok | N/A | - |
| 5 | ok | L | - |
| 6 | ok | L | - |
| 7 | ok | L | - |
| 8 | ok | L | - |

## 最近一批问答（2026-10-17）

1. answer one with L
2. answer two with L
3. answer three with L
4. answer four with L
5. answer five with L
6. answer six with L
7. answer seven with L
8. answer eight with L
"""
    assert problems_for(good) == [], problems_for(good)

    missing_question = good.replace("| 8 | a |\n", "", 1)
    assert any("numbered 1..8" in p for p in problems_for(missing_question))

    empty_answer = good.replace("8. answer eight with L", "")
    assert any("missing [8]" in p for p in problems_for(empty_answer))

    bad_level = good.replace("| 1 | ok | L | - |", "| 1 | ok | unknown | - |", 1)
    assert any("no evidence level" in p for p in problems_for(bad_level))

    empty_verdict = good.replace("| 7 | ok | L | - |", "| 7 |  | L | - |", 1)
    assert any("empty verdict" in p for p in problems_for(empty_verdict))

    print("closure-questions guard self-test: PASS")
    return 0


def main() -> int:
    if "--self-test" in sys.argv:
        return self_test()
    problems = check()
    if problems:
        print("\n".join(problems), file=sys.stderr)
        return 1
    print("closure-questions guard: OK (8/8 answered)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
