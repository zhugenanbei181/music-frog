#!/usr/bin/env python3
"""SLINT-014: workbench rows keep one action column, and loops own their spacing.

Two defects the doctor receipt measured:

* a workbench list that mixes rows with and without an action let the trailing
  status drift by the width of the missing button (失败 ink x1027 vs 警告
  x1036 in the old frame). The action capsule is now one shared component and a
  mixed-action list asks it for the shared reserved column;
* the doctor page hosted a per-item repeater directly under the page root, so
  every non-matching item paid the root's 16u gap and the lifecycle card sat
  ~112u below the issue rows. A bare-layout repeater must own its own zero
  spacing instead of inheriting the page's gap.

Rules are structural, so a new workbench page cannot grow a second capsule or
re-introduce the phantom loop gap.
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
PAGES = UI / "pages"
THEME = UI / "components/theme.slint"
PARTS = UI / "components/workbench_parts.slint"
DOCTOR = PAGES / "page_doctor.slint"

TOKEN_DECL = "out property <length> workbench-row-action-width:"
TOKEN_READ = "LumenTheme.workbench-row-action-width"
RESERVE_OPTION = "in property <bool> reserve-action-column:"
RESERVE_USE = "reserve-action-column: true"
BUTTON_COMPONENT = "component ItemRowActionButton inherits Rectangle {"
BUTTON_USE = "ItemRowActionButton {"
INLINE_CAPSULE = re.compile(r'if\s+root\.item\.action-id\s*!=\s*""\s*:\s*Rectangle\s*\{')
ROOT_LAYOUT = re.compile(r"\b(VerticalLayout|HorizontalLayout)\s*\{")
PAGE_ROOT_REPEAT = re.compile(r"for\s+\w+\s+in\s+[^:]+:\s*(VerticalLayout|HorizontalLayout)\s*$")
SPACING_ZERO = re.compile(r"spacing\s*:\s*0(\s*\*\s*LumenUiScale\.unit)?\s*;")


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


def block_body(source: str, open_index: int) -> tuple[str, int]:
    depth = 0
    index = source.index("{", open_index)
    start = index + 1
    while index < len(source):
        char = source[index]
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return source[start:index], index
        index += 1
    return source[start:], len(source)


def own_statements(block: str) -> str:
    """The element's own statements, without the blocks it contains."""
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


def root_repeats_without_spacing(source: str, path: pathlib.Path) -> list[str]:
    """Bare-layout repeaters that are direct children of a page root."""
    found: list[str] = []
    for match in re.finditer(r"export component \w+ inherits (VerticalLayout|HorizontalLayout)\s*\{", source):
        root, _end = block_body(source, match.start())
        for child in direct_children(root):
            # Only the child's own declaration matters: a card that contains a
            # repeater is not itself a repeater.
            head = child[: child.find("{")] if "{" in child else child
            if not PAGE_ROOT_REPEAT.search(head):
                continue
            if SPACING_ZERO.search(own_statements(child)):
                continue
            line = source.count("\n", 0, match.start()) + 1
            found.append(
                f"{path.relative_to(ROOT)}:{line}: a page root hosts a bare-layout repeater that "
                "does not own its spacing; every non-matching item pays the root's gap"
            )
    return found


def violations(ui: pathlib.Path = UI) -> list[str]:
    found: list[str] = []
    theme = mask_slint_comments((ui / "components/theme.slint").read_text(encoding="utf-8"))
    parts = mask_slint_comments((ui / "components/workbench_parts.slint").read_text(encoding="utf-8"))
    if theme.count(TOKEN_DECL) != 1:
        found.append(
            f"{THEME.relative_to(ROOT)}: the reserved action-column width is declared "
            f"{theme.count(TOKEN_DECL)} times; the row action column needs exactly one owner"
        )
    if TOKEN_READ not in parts:
        found.append(
            f"{PARTS.relative_to(ROOT)}: the reserved action column no longer reads "
            "LumenTheme.workbench-row-action-width"
        )
    if RESERVE_OPTION not in parts:
        found.append(f"{PARTS.relative_to(ROOT)}: ItemRow lost its reserve-action-column option")
    if parts.count(BUTTON_COMPONENT) != 1 or parts.count(BUTTON_USE) < 1:
        found.append(
            f"{PARTS.relative_to(ROOT)}: the action capsule must be one shared "
            "ItemRowActionButton component that ItemRow instantiates"
        )
    if INLINE_CAPSULE.search(parts):
        found.append(f"{PARTS.relative_to(ROOT)}: a second inline action capsule returned")
    doctor = mask_slint_comments((ui / "pages/page_doctor.slint").read_text(encoding="utf-8"))
    if RESERVE_USE not in doctor:
        found.append(
            f"{DOCTOR.relative_to(ROOT)}: the mixed-action doctor list no longer asks for the "
            "reserved action column, so mixed rows drift apart again"
        )
    for path in sorted(PAGES.rglob("*.slint")):
        text = mask_slint_comments(path.read_text(encoding="utf-8"))
        found.extend(root_repeats_without_spacing(text, path))
    return found


def violations_with(planted: str, target: pathlib.Path) -> list[str]:
    real = pathlib.Path.read_text

    def fake(self, *args, **kwargs):
        if self == target:
            return planted
        return real(self, *args, **kwargs)

    with unittest.mock.patch.object(pathlib.Path, "read_text", autospec=True) as read_text:
        read_text.side_effect = fake
        return violations()


class GuardSelfTest(unittest.TestCase):
    def test_live_tree_is_clean(self) -> None:
        self.assertEqual(violations(), [])

    def test_a_second_action_width_owner_is_caught(self) -> None:
        theme = THEME.read_text(encoding="utf-8")
        planted = theme.replace(TOKEN_DECL, TOKEN_DECL, 1)
        planted = planted.replace(
            "out property <color> accent-foreground",
            TOKEN_DECL + " 96 * LumenUiScale.unit;\n    out property <color> accent-foreground",
            1,
        )
        found = violations_with(planted, THEME)
        self.assertTrue(any("exactly one owner" in item for item in found))

    def test_an_inline_capsule_is_caught(self) -> None:
        planted = PARTS.read_text(encoding="utf-8").replace(
            BUTTON_USE,
            'if root.item.action-id != "": Rectangle {',
            1,
        )
        found = violations_with(planted, PARTS)
        self.assertTrue(any("inline action capsule" in item for item in found))

    def test_a_doctor_row_without_the_reserved_column_is_caught(self) -> None:
        planted = DOCTOR.read_text(encoding="utf-8").replace(RESERVE_USE, "", 1)
        found = violations_with(planted, DOCTOR)
        self.assertTrue(any("reserved action column" in item for item in found))

    def test_a_bare_root_repeater_without_spacing_is_caught(self) -> None:
        planted = """export component PageSample inherits VerticalLayout {
    spacing: LumenTheme.space-4;
    for item in root.items: VerticalLayout {
        if item.id == "x": Rectangle { }
    }
}
"""
        found = violations_with(planted, PAGES / "page_sync.slint")
        self.assertTrue(any("does not own its spacing" in item for item in found))

    def test_a_wrapped_repeater_is_allowed(self) -> None:
        planted = """export component PageSample inherits VerticalLayout {
    spacing: LumenTheme.space-4;
    VerticalLayout {
        spacing: 0;
        for item in root.items: VerticalLayout {
            if item.id == "x": Rectangle { }
        }
    }
}
"""
        found = violations_with(planted, PAGES / "page_sync.slint")
        self.assertFalse(any("does not own its spacing" in item for item in found))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=["report", "enforce"], default="report")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--ui-root", type=pathlib.Path, default=UI)
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(GuardSelfTest)
        return 0 if unittest.TextTestRunner(verbosity=0).run(suite).wasSuccessful() else 1
    found = violations(args.ui_root)
    for item in found:
        print(f"SLINT-014 [{args.mode}]: {item}")
    print(f"slint workbench row guard: findings={len(found)}")
    return 1 if found and args.mode == "enforce" else 0


if __name__ == "__main__":
    sys.exit(main())
