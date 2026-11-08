#!/usr/bin/env python3
"""SLINT-015: a selector's segments are equal, and a button never stretches.

The app-routing receipt measured one defect the surface kept re-growing: the
three policy-mode buttons of the routing panel ended at 295 / 238 / 295 px.
`horizontal-stretch` cannot divide a row equally - it hands every child its own
preferred width (which follows the label) and only then splits whatever slack is
left, so the shortest label landed 57 px narrower than its neighbours and the
control stopped reading as one split.

Three rules keep the shape:

* a `WorkbenchButton` never stretches: a button that stretches takes its width
  from its own label, which is exactly the drift the overview grid (b133) and
  the workbench rows (b135) already had to remove elsewhere;
* a selector row divides one width into equal segments, so the gap lives in
  `LumenTheme.selector-segment-gap` and the segment width is one computation;
* the routing panel keeps that single computation instead of a width per label.
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
BUTTON_COMPONENT_FILE = UI / "components/workbench_button.slint"
APP_ROUTING = PAGES / "page_app_routing.slint"

GAP_DECL = "out property <length> selector-segment-gap:"
GAP_READ = "LumenTheme.selector-segment-gap"
SEGMENT_COUNT_DECL = "property <int> selector-segments:"
SEGMENT_WIDTH_DECL = "property <length> selector-segment-width:"
SEGMENT_WIDTH_READ = "width: root.selector-segment-width;"
SEGMENT_DIVISION = re.compile(r"/\s*root\.selector-segments\s*\)")
BUTTON_INSTANCE = re.compile(r"\bWorkbenchButton\s*\{")
BUTTON_DEFINITION = re.compile(r"\bcomponent\s+WorkbenchButton\s+inherits\b")
STRETCH = re.compile(r"horizontal-stretch\s*:")


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


def block_end(source: str, open_index: int) -> int:
    depth = 0
    index = source.index("{", open_index)
    while index < len(source):
        char = source[index]
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return index + 1
        index += 1
    return len(source)


def stretching_buttons(source: str, path: pathlib.Path) -> list[str]:
    """Buttons whose width follows their own label instead of their row."""
    found: list[str] = []
    if BUTTON_DEFINITION.search(source):
        return found
    for match in BUTTON_INSTANCE.finditer(source):
        block = source[match.start() : block_end(source, match.start())]
        if not STRETCH.search(own_statements(block)):
            continue
        line = source.count("\n", 0, match.start()) + 1
        found.append(
            f"{path.relative_to(ROOT)}:{line}: a WorkbenchButton stretches, so its width "
            "follows its own label; a selector row must divide one width into equal segments"
        )
    return found


def violations(ui: pathlib.Path = UI) -> list[str]:
    found: list[str] = []
    theme = mask_slint_comments((ui / "components/theme.slint").read_text(encoding="utf-8"))
    if theme.count(GAP_DECL) != 1:
        found.append(
            f"{THEME.relative_to(ROOT)}: the selector gap is declared "
            f"{theme.count(GAP_DECL)} times; a selector row needs exactly one gap owner"
        )

    for path in sorted(ui.rglob("*.slint")):
        text = mask_slint_comments(path.read_text(encoding="utf-8"))
        found.extend(stretching_buttons(text, path))

    routing = mask_slint_comments((ui / "pages/page_app_routing.slint").read_text(encoding="utf-8"))
    if GAP_READ not in routing:
        found.append(
            f"{APP_ROUTING.relative_to(ROOT)}: the selector row no longer reads "
            "LumenTheme.selector-segment-gap"
        )
    if SEGMENT_COUNT_DECL not in routing:
        found.append(
            f"{APP_ROUTING.relative_to(ROOT)}: the routing selector lost its segment count"
        )
    if SEGMENT_WIDTH_DECL not in routing or not SEGMENT_DIVISION.search(routing):
        found.append(
            f"{APP_ROUTING.relative_to(ROOT)}: the routing selector must size its segments as "
            "(row - segments * gap) / segments, not one width per label"
        )
    if SEGMENT_WIDTH_READ not in routing:
        found.append(
            f"{APP_ROUTING.relative_to(ROOT)}: the routing mode buttons no longer take the "
            "shared segment width"
        )
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

    def test_a_button_that_stretches_is_caught(self) -> None:
        routing = APP_ROUTING.read_text(encoding="utf-8")
        planted = routing.replace(
            "width: root.selector-segment-width;", "horizontal-stretch: 1;", 1
        )
        found = violations_with(planted, APP_ROUTING)
        self.assertTrue(any("WorkbenchButton stretches" in item for item in found), found)

    def test_a_second_gap_owner_is_caught(self) -> None:
        theme = THEME.read_text(encoding="utf-8")
        planted = theme + "\n    out property <length> selector-segment-gap: 10 * LumenUiScale.unit;\n"
        found = violations_with(planted, THEME)
        self.assertTrue(any("selector gap is declared" in item for item in found), found)

    def test_a_selector_that_stops_reading_the_shared_gap_is_caught(self) -> None:
        routing = APP_ROUTING.read_text(encoding="utf-8")
        planted = routing.replace(GAP_READ, "10 * LumenUiScale.unit")
        found = violations_with(planted, APP_ROUTING)
        self.assertTrue(any("selector-segment-gap" in item for item in found), found)

    def test_a_per_label_width_is_caught(self) -> None:
        routing = APP_ROUTING.read_text(encoding="utf-8")
        planted = routing.replace(SEGMENT_WIDTH_READ, "width: 200 * LumenUiScale.unit;", 1)
        found = violations_with(planted, APP_ROUTING)
        self.assertTrue(any("shared segment width" in item for item in found), found)

    def test_a_lost_segment_division_is_caught(self) -> None:
        routing = APP_ROUTING.read_text(encoding="utf-8")
        planted = routing.replace(
            "/ root.selector-segments)", ") / root.selector-segments / root.selector-segments)"
        )
        planted = planted.replace("property <length> selector-segment-width:", "property <length> other-width:")
        found = violations_with(planted, APP_ROUTING)
        self.assertTrue(any("one width per label" in item for item in found), found)


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
        print(f"SLINT-015 [{args.mode}]: {item}")
    print(f"slint selector segment guard: findings={len(found)}")
    return 1 if found and args.mode == "enforce" else 0


if __name__ == "__main__":
    sys.exit(main())
