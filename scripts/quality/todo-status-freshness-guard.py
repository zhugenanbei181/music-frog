#!/usr/bin/env python3
"""Keep the task boards from falling behind the archive they write into.

`TODO.md` is the ledger of record: a batch that closes work deletes the item
from the board and registers an archived row with its points. The narrative
detail next to those tables is edited by hand, and nothing checked it — so a
closed item could keep reading as open work.

Two measured instances found the gap. `FLCLASH-001` was archived by the 151st
unified verification batch, yet its §0.2 bullet still said "待独立 Core 批次自底
向上收口" and prescribed adding `ConnectionOperation::ResetStatistics` plus an
adapter-matrix rollout. That exact route was reviewed and **rejected**: the L3
desktop reference records that Mihomo's HTTP API has no traffic-reset endpoint,
so inventing the operation would have written a non-existent engine capability
as a second source of truth. An agent following the board would have undone a
deliberate architectural ruling. `SPACEUTIL-001` was archived too, and its
bullet still set acceptance as "the `pill_tail` leg passes" — a metric b146
abolished for good.

The rule is therefore twofold: an id that has an archived row must not occupy a
live board row (one item, one status), and a detail bullet that leads with an
archived id must say so. Batch-history lines are exempt: they narrate what
happened and legitimately mention ids that closed later.
"""
from __future__ import annotations

import argparse
import pathlib
import re
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
LEDGER = pathlib.Path("TODO.md")
DETAIL = pathlib.Path("todos/CODE_TODO.md")
BOARDS = (
    pathlib.Path("todos/CODE_TODO.md"),
    pathlib.Path("todos/CODE_TODO_PAST.md"),
    pathlib.Path("todos/CODE_TODO_BLOCKED.md"),
    pathlib.Path("todos/CODE_TODO_FUTURE.md"),
)
ARCHIVE_HEADING = "### 已归档交付原子条目"
LEDGER_BLOCK = re.compile(r"<!-- POINT_LEDGER:BEGIN -->.*?<!-- POINT_LEDGER:END -->", re.S)
# A registry row puts the id first and then its title in the same cell; a board
# row's first cell holds nothing but the id, so the two shapes are told apart by
# what follows the closing backtick.
ARCHIVE_ROW_RE = re.compile(r"^\|\s*`([A-Z0-9._\-]+)`[^|]*\|", re.M)
BOARD_ROW_RE = re.compile(r"^\|\s*`([A-Z0-9._\-]+)`\s*\|", re.M)
BULLET_RE = re.compile(r"^- `([A-Z0-9._\-]+)`", re.M)
ARCHIVED_STATUSES = ("`Verified`", "`Closed`")
# Words that make a bullet retrospective rather than a to-do. Kept short on
# purpose: any of them is what a human writes when they mean "this is history".
CLOSING_MARKERS = re.compile(
    r"已交付|已办结|已归档|已退出|已收口|收口退出|办结归档|直接办结|撤回|迁入过去佛"
    r"|交付批次|已由第[一二三四五六七八九十百零]+批|b\d{1,3}\s*交付|第[一二三四五六七八九十百零]+批.*已")
STRUCTURE_GATE = pathlib.Path("scripts/quality/check-structure.sh")


def archived_ids(root: pathlib.Path) -> set[str]:
    """Ids with a row in the ledger's archive registry."""
    text = (root / LEDGER).read_text(encoding="utf-8")
    heading = text.index(ARCHIVE_HEADING)
    rest = text[heading + len(ARCHIVE_HEADING):]
    stop = rest.find("\n## ")
    section = rest if stop < 0 else rest[:stop]
    return set(ARCHIVE_ROW_RE.findall(section))


def live_rows(root: pathlib.Path, board_text: str) -> list[tuple[str, str]]:
    """(board, id) for every status-table row on the four boards."""
    out = []
    for board in BOARDS:
        text = (root / board).read_text(encoding="utf-8") if board != DETAIL else board_text
        for match in BOARD_ROW_RE.finditer(text):
            line = text[match.start():text.find("\n", match.start())]
            if any(status in line for status in ARCHIVED_STATUSES):
                continue
            out.append((board.name, match.group(1)))
    return out


def detail_bullets(board_text: str) -> list[tuple[int, str, str]]:
    """(line, id, whole bullet) for top-level id bullets outside the batch ledger."""
    text = LEDGER_BLOCK.sub("", board_text)
    lines = text.splitlines()
    starts = [(i, m.group(1)) for i, line in enumerate(lines)
              if (m := BULLET_RE.match(line))]
    out = []
    for index, (line, ident) in enumerate(starts):
        end = starts[index + 1][0] if index + 1 < len(starts) else len(lines)
        out.append((line + 1, ident, "\n".join(lines[line:end])))
    return out


def violations(root: pathlib.Path) -> list[str]:
    archived = archived_ids(root)
    board_text = (root / DETAIL).read_text(encoding="utf-8")
    problems: list[str] = []

    for board, ident in live_rows(root, board_text):
        if ident in archived:
            problems.append(
                f"{board}: `{ident}` has an archived row in {LEDGER} and still occupies "
                f"a live board row")
    for line, ident, body in detail_bullets(board_text):
        if ident in archived and not CLOSING_MARKERS.search(body):
            first = body.splitlines()[0]
            problems.append(
                f"{DETAIL}:{line}: bullet for `{ident}` still reads as open work although "
                f"the id is archived; say what closed it and supersede any rejected plan "
                f"({first[:80]})")
    return problems


def gate_legs(root: pathlib.Path) -> list[str]:
    """This guard must run in the structure gate, both legs."""
    if not (root / STRUCTURE_GATE).is_file():
        return [f"{STRUCTURE_GATE}: missing, so no guard can be enforced"]
    text = (root / STRUCTURE_GATE).read_text(encoding="utf-8")
    name = pathlib.Path(__file__).name
    problems = []
    for flag in ("--self-test", "--mode enforce"):
        if f"quality/{name} {flag}" not in text:
            problems.append(f"{STRUCTURE_GATE}: no `{name} {flag}` leg")
    return problems


class Fixture(unittest.TestCase):
    """Planted regressions: each rule must bite, and history must not."""

    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.root = pathlib.Path(self._tmp.name)
        (self.root / "todos").mkdir()
        (self.root / "scripts" / "quality").mkdir(parents=True)
        self.ledger = self.root / LEDGER
        self.ledger.write_text(
            f"# Ledger\n\n{ARCHIVE_HEADING}\n\n| id | pt | 证据 |\n| --- | --- | --- |\n"
            "| `DEMO-001` 已交付的事 | 2 | 统一验证批次 |\n\n## 其它小节\n\n"
            "| `NOT-ARCHIVED-001` 未归档 | 1 | x |\n", encoding="utf-8")
        (self.root / STRUCTURE_GATE).write_text(
            "gate python3 scripts/quality/todo-status-freshness-guard.py --self-test\n"
            "gate python3 scripts/quality/todo-status-freshness-guard.py --mode enforce\n",
            encoding="utf-8")
        for board in BOARDS:
            path = self.root / board
            if not path.is_file():
                path.write_text("# board\n", encoding="utf-8")

    def tearDown(self) -> None:
        self._tmp.cleanup()

    def board(self, body: str) -> None:
        (self.root / DETAIL).write_text(
            "# 现在佛\n\n## 0.2 小节\n\n" + body +
            "\n<!-- POINT_LEDGER:BEGIN -->\n- 历史批次办结了 `DEMO-001`（3 pt）\n"
            "<!-- POINT_LEDGER:END -->\n", encoding="utf-8")

    def test_archived_set_reads_only_the_registry_section(self) -> None:
        self.assertEqual(archived_ids(self.root), {"DEMO-001"})

    def test_a_clean_board_passes(self) -> None:
        self.board("- `OPEN-001` 还没做的事：细节。\n"
                   "- `DEMO-001` **b9 已交付、第一百五十一批次已办结归档**：结论。\n")
        self.assertEqual(violations(self.root), [])
        self.assertEqual(gate_legs(self.root), [])

    def test_an_archived_id_may_not_hold_a_live_row(self) -> None:
        self.board("| ID | 状态 | points |\n| --- | --- | --- |\n"
                   "| `DEMO-001` | `Planned` / P1 | 2 |\n")
        self.assertTrue(any("still occupies a live board row" in problem
                            for problem in violations(self.root)), violations(self.root))

    def test_a_bullet_that_still_reads_as_work_is_rejected(self) -> None:
        self.board("- `DEMO-001` 审计发现两个缺陷，待独立批次自底向上收口：先加一个 typed 操作。\n")
        problems = violations(self.root)
        self.assertEqual(len(problems), 1, problems)
        self.assertIn("reads as open work", problems[0])

    def test_batch_history_lines_are_exempt(self) -> None:
        # The ledger narrates what happened; it legitimately names ids that
        # closed in a later batch, and must not be flagged for it.
        self.board("- `OPEN-001` 待办。\n")
        self.assertEqual(violations(self.root), [])

    def test_removing_a_gate_leg_is_rejected(self) -> None:
        (self.root / STRUCTURE_GATE).write_text(
            "gate python3 scripts/quality/todo-status-freshness-guard.py --self-test\n",
            encoding="utf-8")
        self.assertEqual(gate_legs(self.root),
                         [f"{STRUCTURE_GATE}: no `todo-status-freshness-guard.py --mode enforce` leg"])


def self_test() -> int:
    suite = unittest.TestLoader().loadTestsFromTestCase(Fixture)
    result = unittest.TextTestRunner(verbosity=0).run(suite)
    ok = result.testsRun - len(result.failures) - len(result.errors)
    print(f"todo-status-freshness: self-test {ok}/{result.testsRun} passed")
    return 0 if result.wasSuccessful() else 1


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--mode", choices=("enforce", "report"), default="enforce")
    parser.add_argument("--root", type=pathlib.Path, default=ROOT)
    args = parser.parse_args(argv)

    if args.self_test:
        return self_test()
    problems = violations(args.root) + gate_legs(args.root)
    for problem in problems:
        print(f"todo-status-freshness: {problem}", file=sys.stderr)
    if problems and args.mode == "enforce":
        return 1
    print(f"todo-status-freshness: findings={len(problems)} over "
          f"{len(archived_ids(args.root))} archived ids")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
