#!/usr/bin/env python3
"""SLINT-013: a list page's card is the page's frame and its list scrolls inside.

Logs, rules and connections are list pages. Their one card used to be as tall
as the rows it happened to hold, so three logs left more than half the page as
empty ground under the card, and three hundred rows pushed the card past the
viewport instead of scrolling inside it.

The system is one published fact plus one shared owner:

* the shell publishes the page body's height once (`LumenTheme.page-body-height`),
  after the page header and the floating dock band are taken off the window, so
  no page measures the window for itself;
* a list page asks the shared card owner for the `page-body` height mode, and
  the card hands the leftover height to its list;
* the list rows live in the shared `ListBody`, the one owner of the scroll
  behaviour: the rail follows the shell's viewport fact and the compact layout
  never paints a rail at all.

The rules are structural, not per-file, so a fourth list page cannot grow its
own copy of the pattern.
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
LIST_PAGE_FILE = PAGES / "page_observe.slint"
PUBLISHER = UI / "components/app_body.slint"
LIST_PAGES = ("PageConnections", "PageLogs", "PageRules")

GLOBAL_ASSIGN = re.compile(r"LumenTheme\.page-body-height\s*=")
HEIGHT_MODE = re.compile(r"height-mode\s*:\s*CardHeight\.page-body")
CONTENT_MODE = re.compile(r"height-mode\s*:\s*CardHeight\.content")
RAW_SCROLLVIEW = re.compile(r"(?<!\w)ScrollView\s*\{")
LIST_BODY = re.compile(r"(?<!\w)ListBody\s*\{")
VISIBLE_HEIGHT = re.compile(r"visible-height")
STRETCH_ONE = re.compile(r"vertical-stretch\s*:\s*1\s*;")


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


def component_block(source: str, name: str) -> str | None:
    """The full body of one component declaration, or None."""
    for match in re.finditer(rf"component\s+{name}\b", source):
        body, _end = block_body(source, match.start())
        return body
    return None


def nested_blocks(body: str, opener: re.Pattern[str]) -> list[tuple[int, str]]:
    """Every block whose declaration matches `opener`, with its body."""
    found: list[tuple[int, str]] = []
    for match in opener.finditer(body):
        block, _end = block_body(body, match.start())
        found.append((match.start(), block))
    return found


def list_page_findings(source: str) -> list[str]:
    """The list-page rules over one `page_observe.slint` text."""
    found: list[str] = []
    if RAW_SCROLLVIEW.search(source):
        found.append(
            f"{LIST_PAGE_FILE.relative_to(ROOT)}: a list page instantiates a raw ScrollView; "
            "the shared ListBody owns the list's scroll behaviour"
        )
    for name in LIST_PAGES:
        block = component_block(source, name)
        if block is None:
            found.append(f"{LIST_PAGE_FILE.relative_to(ROOT)}: the list page {name} is missing")
            continue
        cards = nested_blocks(block, re.compile(r"(?<!\w)SectionCard\s*\{"))
        if not cards:
            found.append(
                f"{LIST_PAGE_FILE.relative_to(ROOT)}: the list page {name} has no shared SectionCard frame"
            )
            continue
        _offset, card = cards[0]
        if not HEIGHT_MODE.search(card):
            mode = "content" if CONTENT_MODE.search(card) else "none"
            found.append(
                f"{LIST_PAGE_FILE.relative_to(ROOT)}: the list page {name} keeps a card in the "
                f"{mode} height mode instead of filling the page body"
            )
            continue
        bodies = nested_blocks(card, LIST_BODY)
        if not bodies:
            found.append(
                f"{LIST_PAGE_FILE.relative_to(ROOT)}: the list page {name} fills the page body "
                "without a shared ListBody, so its rows cannot scroll inside the frame"
            )
            continue
        _offset, body = bodies[0]
        if not STRETCH_ONE.search(body):
            found.append(
                f"{LIST_PAGE_FILE.relative_to(ROOT)}: the list page {name}'s ListBody does not ask "
                "to grow, so the card's leftover height is left empty inside the frame"
            )
    return found


def violations(ui: pathlib.Path = UI) -> list[str]:
    found: list[str] = []
    for path in sorted(ui.rglob("*.slint")):
        text = mask_slint_comments(path.read_text(encoding="utf-8"))
        if GLOBAL_ASSIGN.search(text) and path != PUBLISHER:
            found.append(
                f"{path.relative_to(ROOT)}: a second writer of LumenTheme.page-body-height; the "
                "shell publishes the page body's height once"
            )
        if path == LIST_PAGE_FILE:
            found.extend(list_page_findings(text))
        if PAGES in path.parents and VISIBLE_HEIGHT.search(text):
            found.append(
                f"{path.relative_to(ROOT)}: a page reads the scroll viewport's height; the page "
                "body's height is the shell's published LumenTheme.page-body-height"
            )
    return found


def violations_with(planted: str, target: pathlib.Path) -> list[str]:
    """Run the guard over one planted file, as the live tree's rules see it."""
    real = pathlib.Path.read_text

    def fake(self, *args, **kwargs):
        if self == target:
            return planted
        return real(self, *args, **kwargs)

    with unittest.mock.patch.object(pathlib.Path, "read_text", autospec=True) as read_text:
        read_text.side_effect = fake
        return violations()


LIST_PAGE_SNIPPET = """export component PageLogs inherits VerticalLayout {{
    SectionCard {{
{0}
    }}
}}
"""


class GuardSelfTest(unittest.TestCase):
    def test_live_tree_is_clean(self) -> None:
        self.assertEqual(violations(), [])

    def test_a_content_card_on_a_list_page_is_caught(self) -> None:
        planted = LIST_PAGE_SNIPPET.format("        heading: @tr(\"x\");")
        found = violations_with(planted, LIST_PAGE_FILE)
        found = [item for item in found if "PageLogs" in item]
        self.assertTrue(any("instead of filling the page body" in item for item in found))

    def test_a_page_body_card_without_a_list_body_is_caught(self) -> None:
        planted = LIST_PAGE_SNIPPET.format(
            "        height-mode: CardHeight.page-body;\n        heading: @tr(\"x\");"
        )
        found = violations_with(planted, LIST_PAGE_FILE)
        found = [item for item in found if "PageLogs" in item]
        self.assertTrue(any("cannot scroll inside the frame" in item for item in found))

    def test_a_list_body_that_keeps_the_leftover_empty_is_caught(self) -> None:
        planted = LIST_PAGE_SNIPPET.format(
            "        height-mode: CardHeight.page-body;\n"
            "        heading: @tr(\"x\");\n"
            "        ListBody { VerticalLayout { } }\n"
        )
        found = violations_with(planted, LIST_PAGE_FILE)
        found = [item for item in found if "PageLogs" in item]
        self.assertTrue(any("leftover height is left empty" in item for item in found))

    def test_a_raw_scroll_view_on_a_list_page_is_caught(self) -> None:
        planted = LIST_PAGE_SNIPPET.format(
            "        height-mode: CardHeight.page-body;\n"
            "        heading: @tr(\"x\");\n"
            "        ListBody { vertical-stretch: 1; VerticalLayout { } }\n"
            "        ScrollView { }\n"
        )
        found = violations_with(planted, LIST_PAGE_FILE)
        self.assertTrue(any("raw ScrollView" in item for item in found))

    def test_a_page_that_measures_the_viewport_is_caught(self) -> None:
        planted = 'export component PageSample inherits VerticalLayout {\n    Rectangle { height: other.visible-height; }\n}\n'
        found = violations_with(planted, PAGES / "page_settings.slint")
        self.assertTrue(any("reads the scroll viewport" in item for item in found))

    def test_a_second_publisher_of_the_page_body_height_is_caught(self) -> None:
        planted = "export global Sample {\n    LumenTheme.page-body-height = 0 * LumenUiScale.unit;\n}\n"
        found = violations_with(planted, UI / "components/theme.slint")
        self.assertTrue(any("second writer" in item for item in found))


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
        print(f"SLINT-013 [{args.mode}]: {item}")
    print(f"slint list page guard: findings={len(found)}")
    return 1 if found and args.mode == "enforce" else 0


if __name__ == "__main__":
    sys.exit(main())
