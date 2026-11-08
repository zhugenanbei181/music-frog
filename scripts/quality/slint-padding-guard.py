#!/usr/bin/env python3
"""SLINT-012: a padded layout owns its children's width.

A child that sizes itself to `parent.width` inside a container that declares
horizontal padding does not get the container's *content* width - it gets the
container's whole width, so it runs past the inner edge by exactly the padding
it was supposed to keep.

Two live defects had this shape: the desktop rail's status card drew its
throughput row as wide as the card itself, so the download rate ended on the
card's border (measured ink x205 against a card border at x209), and the
workbench draft field made its input 28u wider than the field it sits in.

The rule is structural, not per-file: no direct child of a padded layout may
size itself to `parent.width`.
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

LAYOUT_OPEN = re.compile(r"\b(HorizontalLayout|VerticalLayout)\s*\{")
PADDING = re.compile(r"padding-(left|right)\s*:")
PARENT_WIDTH = re.compile(r"\bwidth\s*:\s*parent\.width\s*;")


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
    """Return the body of the brace block that opens at `open_index`."""
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


def direct_children(body: str) -> list[str]:
    """The text of every element that is a direct child of a block body.

    Only brace-delimited elements are children. The statements before them
    are the layout's own properties, and a property is not a child: a layout
    that declares `width: parent.width` for itself is sizing its own box, not
    asking its container for more room.
    """
    children: list[str] = []
    depth = 0
    start = 0
    statement_start = 0
    index = 0
    while index < len(body):
        char = body[index]
        if char == "{":
            if depth == 0:
                start = index
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                children.append(body[statement_start:index + 1])
        elif char == ";" and depth == 0:
            statement_start = index + 1
        index += 1
    return children


def padded_layouts(source: str) -> list[tuple[int, str]]:
    """Every padded layout block with its offset in the file."""
    found: list[tuple[int, str]] = []
    for match in LAYOUT_OPEN.finditer(source):
        body, _end = block_body(source, match.start())
        if PADDING.search(body.split("\n")[0] if body else "") or PADDING.search(body[:400]):
            found.append((match.start(), body))
    return found


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


ABSOLUTE = re.compile(r"\b(x|y)\s*:")
HIT_SURFACE = re.compile(r"^\s*(\w+\s*:=\s*)?TouchArea\b")


def violations(ui: pathlib.Path = UI) -> list[str]:
    found: list[str] = []
    for path in sorted(ui.rglob("*.slint")):
        text = mask_slint_comments(path.read_text(encoding="utf-8"))
        for offset, body in padded_layouts(text):
            for child in direct_children(body):
                statements = own_statements(child)
                if not PARENT_WIDTH.search(statements):
                    continue
                # A hit surface and an absolutely placed overlay are allowed
                # to span the whole box: neither one paints ink, so neither
                # one can run past the padding.
                if HIT_SURFACE.match(child) or ABSOLUTE.search(statements):
                    continue
                line = text.count("\n", 0, offset) + 1
                found.append(
                    f"{path.relative_to(ROOT)}:{line}: a child of a padded layout sizes itself to "
                    "parent.width and ignores the container's inner padding"
                )
    return found


class GuardSelfTest(unittest.TestCase):
    def test_live_tree_is_clean(self) -> None:
        self.assertEqual(violations(), [])

    def test_padded_child_with_parent_width_is_caught(self) -> None:
        planted = """export component Sample inherits Rectangle {
    HorizontalLayout {
        padding-left: 16 * LumenUiScale.unit;
        padding-right: 16 * LumenUiScale.unit;
        row := Rectangle { width: parent.width; }
    }
}"""
        found = violations_with(planted)
        self.assertTrue(any("ignores the container's inner padding" in item for item in found))

    def test_an_unpadded_child_is_allowed(self) -> None:
        planted = """export component Sample inherits Rectangle {
    HorizontalLayout {
        row := Rectangle { width: parent.width; }
    }
}"""
        found = violations_with(planted)
        self.assertFalse(any("ignores the container's inner padding" in item for item in found))

    def test_a_hit_surface_and_an_overlay_are_allowed(self) -> None:
        planted = """export component Sample inherits Rectangle {
    HorizontalLayout {
        padding-left: 16 * LumenUiScale.unit;
        touch := TouchArea { width: parent.width; height: parent.height; }
        overlay := Rectangle { x: 0; y: 0; width: parent.width; }
    }
}"""
        found = violations_with(planted)
        self.assertFalse(any("ignores the container's inner padding" in item for item in found))


def violations_with(planted: str) -> list[str]:
    """Run the guard over one planted file, as the live tree's rules see it."""
    target = UI / "components/sidebar.slint"
    real = pathlib.Path.read_text

    def fake(self, *args, **kwargs):
        if self == target:
            return planted
        return real(self, *args, **kwargs)

    with unittest.mock.patch.object(pathlib.Path, "read_text", autospec=True) as read_text:
        read_text.side_effect = fake
        return violations()


ORIGINAL_READ_TEXT = pathlib.Path.read_text


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
        print(f"SLINT-012 [{args.mode}]: {item}")
    print(f"slint padding guard: findings={len(found)}")
    return 1 if found and args.mode == "enforce" else 0


if __name__ == "__main__":
    sys.exit(main())
