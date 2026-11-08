#!/usr/bin/env python3
"""SLINT-016: the dashboard never pays for columns its tier does not have.

The overview grid declares two columns in the dense tier and four in the wide
tier. Rows 1 and 2 used to be plain `HorizontalLayout`s that placed a span card
(two shares) next to two single cards: four shares in a two-column tier. The
overflow was silent and total - at 720 wide the upload and download cards and
the system-proxy and TUN cards landed past the right edge and never rendered at
all, so a whole feature band disappeared from the receipt while the page still
looked internally tidy.

The rules are structural:

* a non-wrapping row may not combine a span card with single cards; that shape
  needs a `GridLayout` so the dense tier can move the extra cards down;
* every card inside such a `GridLayout` states its own `col:` and `row:`, so a
  tier change re-plans the placement instead of dropping cards;
* the dense share count and the declared dense column count are the same fact.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys
import unittest
import unittest.mock

ROOT = pathlib.Path(__file__).resolve().parents[2]
UI = ROOT / "crates/infiltrator-slint/ui"
GRID_FILE = UI / "pages/home_desktop_grid.slint"
THEME = UI / "components/theme.slint"

COLUMN_BASIS = re.compile(
    r"property <int> card-columns:\s*root\.dense\s*\?\s*(\d+)\s*:\s*LumenTheme\.overview-card-columns"
)
SPAN_WIDTH = "width: root.card-span-width;"
COLUMN_WIDTH = "width: root.card-column-width;"
COL_DECL = re.compile(r"^\s*col\s*:", re.MULTILINE)
ROW_DECL = re.compile(r"^\s*row\s*:", re.MULTILINE)


def mask_slint_comments(source: str) -> str:
    result: list[str] = []
    index = 0
    in_block = False
    while index < len(source):
        if not in_block and source[index : index + 2] == "//":
            end = source.find("\n", index)
            if end == -1:
                break
            result.append("\n")
            index = end + 1
        elif not in_block and source[index : index + 2] == "/*":
            in_block = True
            index += 2
        elif in_block and source[index : index + 2] == "*/":
            in_block = False
            index += 2
        else:
            result.append("" if in_block else source[index])
            index += 1
    return "".join(result)


def block_span(source: str, open_index: int) -> tuple[int, int]:
    """The (start, end) offsets of the block whose declaration starts at
    `open_index`, where the block's own statements stop at its first child."""
    depth = 0
    index = source.index("{", open_index)
    body_start = index + 1
    while index < len(source):
        char = source[index]
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return body_start, index
        index += 1
    return body_start, len(source)


def direct_children(body: str) -> list[str]:
    children: list[str] = []
    depth = 0
    statement_start = 0
    index = 0
    while index < len(body):
        char = body[index]
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                children.append(body[statement_start : index + 1])
                statement_start = index + 1
        elif char == ";" and depth == 0:
            statement_start = index + 1
        index += 1
    return children


def own_statements(block: str) -> str:
    inner = block[block.find("{") + 1 : block.rfind("}")]
    result: list[str] = []
    depth = 0
    start = 0
    for index, char in enumerate(inner):
        if char == "{":
            depth += 1
            if depth == 1:
                result.append(inner[start:index])
                start = index
        elif char == "}":
            depth -= 1
            if depth == 0:
                start = index + 1
    result.append(inner[start:])
    return "".join(result)


def dense_columns(source: str) -> int | None:
    match = COLUMN_BASIS.search(source)
    return int(match.group(1)) if match else None


def row_findings(source: str, path: pathlib.Path) -> list[str]:
    """Every top-level row container of the dashboard grid."""
    found: list[str] = []
    component = re.search(r"export component HomeDesktopGrid inherits VerticalLayout\s*\{", source)
    if component is None:
        return [f"{path.relative_to(ROOT)}: HomeDesktopGrid is missing"]
    body_start, body_end = block_span(source, component.start())
    body = source[body_start:body_end]
    dense = dense_columns(source)
    if dense is None:
        return [f"{path.relative_to(ROOT)}: the column basis lost its single owner"]
    for match in re.finditer(r"\b(HorizontalLayout|GridLayout)\s*\{", body):
        if match.start() == 0:
            continue
        child_start, child_end = block_span(body, match.start())
        # Only top-level rows: skip nested containers inside a card.
        children = direct_children(body[child_start:child_end])
        cards = [c for c in children if "SurfaceCard {" in c[: c.find("{") + 12] or c.lstrip().startswith("SurfaceCard")]
        if not cards:
            continue
        spans = sum(1 for c in cards if SPAN_WIDTH in own_statements(c))
        singles = sum(1 for c in cards if COLUMN_WIDTH in own_statements(c))
        line = source.count("\n", 0, body_start + match.start()) + 1
        is_wrapping = body[match.start() : match.start() + 12].startswith("GridLayout")
        if spans and singles and not is_wrapping:
            found.append(
                f"{path.relative_to(ROOT)}:{line}: a non-wrapping row places {spans} span "
                f"card(s) with {singles} single card(s); that pays {2 * spans + singles} shares "
                f"while the dense tier declares {dense}, so the overflow never renders"
            )
        if is_wrapping and (spans or singles):
            for card in cards:
                own = own_statements(card)
                if SPAN_WIDTH not in own and COLUMN_WIDTH not in own:
                    continue
                if not COL_DECL.search(own) or not ROW_DECL.search(own):
                    found.append(
                        f"{path.relative_to(ROOT)}:{line}: a dashboard card sits in a GridLayout "
                        "without stating its own col: and row:, so a tier change drops it instead "
                        "of re-planning the row"
                    )
        if spans or singles:
            demand = 2 * spans + singles
            if not is_wrapping and demand > dense:
                found.append(
                    f"{path.relative_to(ROOT)}:{line}: this row pays {demand} column shares "
                    f"but the dense tier declares {dense}"
                )
    return found


def violations(grid: pathlib.Path = GRID_FILE) -> list[str]:
    source = mask_slint_comments(grid.read_text(encoding="utf-8"))
    return row_findings(source, grid)


def violations_with(planted: str) -> list[str]:
    real = pathlib.Path.read_text

    def fake(self, *args, **kwargs):
        if self == GRID_FILE:
            return planted
        return real(self, *args, **kwargs)

    with unittest.mock.patch.object(pathlib.Path, "read_text", autospec=True) as read_text:
        read_text.side_effect = fake
        return violations()


class GuardSelfTest(unittest.TestCase):
    def setUp(self) -> None:
        self.grid = GRID_FILE.read_text(encoding="utf-8")

    def test_live_tree_is_clean(self) -> None:
        self.assertEqual(violations(), [])

    def test_an_unwrapped_span_plus_singles_row_is_caught(self) -> None:
        planted = self.grid.replace(
            "    // Row 1: Hero Card (50%) + Upload Card (25%) + Download Card (25%)",
            "",
            1,
        )
        # Put row 1 back into a plain HorizontalLayout: the shape that clipped.
        planted = planted.replace(
            """    GridLayout {
        width: parent.width;
        spacing: root.dense ? LumenTheme.space-3 : LumenTheme.space-4;

        // Hero Card (FlClash Style)""",
            """    HorizontalLayout {
        width: parent.width;
        spacing: root.dense ? LumenTheme.space-3 : LumenTheme.space-4;

        // Hero Card (FlClash Style)""",
            1,
        )
        found = violations_with(planted)
        self.assertTrue(any("never renders" in item for item in found), found)

    def test_a_card_without_its_own_placement_is_caught(self) -> None:
        planted = self.grid.replace(
            """        SurfaceCard {
            col: 0;
            row: 0;
            colspan: 2;
            width: root.card-span-width;""",
            """        SurfaceCard {
            width: root.card-span-width;""",
            1,
        )
        found = violations_with(planted)
        self.assertTrue(any("without stating its own col:" in item for item in found), found)

    def test_a_row_that_outgrows_the_dense_basis_is_caught(self) -> None:
        planted = self.grid.replace(
            "property <int> card-columns: root.dense ? 2 : LumenTheme.overview-card-columns;",
            "property <int> card-columns: root.dense ? 1 : LumenTheme.overview-card-columns;",
            1,
        )
        planted = planted.replace(
            """    // Row 4: Realtime Connections Mini Table (~62%) + Policy Groups Grid (~38%)""",
            """    HorizontalLayout {
        width: parent.width;
        SurfaceCard {
            width: root.card-column-width;
        }
        SurfaceCard {
            width: root.card-column-width;
        }
    }

    // Row 4: Realtime Connections Mini Table (~62%) + Policy Groups Grid (~38%)""",
            1,
        )
        found = violations_with(planted)
        self.assertTrue(any("column shares" in item for item in found), found)

    def test_the_column_basis_stays_one_owner(self) -> None:
        self.assertEqual(len(COLUMN_BASIS.findall(mask_slint_comments(self.grid))), 1)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("enforce", "report"), default="enforce")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(GuardSelfTest)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1
    found = violations()
    for item in found:
        print(f"SLINT-016 [{args.mode}]: {item}")
    print(f"slint dashboard grid guard: findings={len(found)}")
    return 1 if found and args.mode == "enforce" else 0


if __name__ == "__main__":
    sys.exit(main())
