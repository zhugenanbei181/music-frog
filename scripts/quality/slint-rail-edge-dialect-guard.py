#!/usr/bin/env python3
"""SLINT-018: the rail is navigation only, and declares each boundary once.

The rail already owns a separator dialect, written down in its own comments:
a group heading IS the separator, so a hairline directly above it is a double
signal. The bottom cluster broke exactly that rule. Measured on every desktop
skin of the published receipt, a full-width `ListRule` sat at y610 and the
status card's own border at y617, both spanning x18..241 - one box with two
top edges 6px apart. Two owners claiming the same boundary is what made the
heading look stray in b139, and it reads here as a box with a doubled rim.

The cluster itself is gone now, and it is gone on purpose. The FlClash
desktop reference's sidebar carries navigation rows and nothing else: the
status, throughput, brand and version facts all live in the title bar and the
dashboard cards, and publishing them in the rail made a SECOND truth source
of them (the space-utilization judge now measures that as `chrome_band` and
holds it at 0%). So the rail's dialect is now three rules:

* the rail draws no `ListRule` at all: its separators are headings, and its
  boxes draw their own edges;
* the free air of the rail belongs below the navigation stack, so the stretch
  spacer must stay the last thing in the rail;
* the rail declares NOTHING below that spacer. A bottom cluster - a bordered
  card, a status row, a version line - is the second truth source coming
  back, and the guard must refuse it at the source level, not only at the
  pixel level.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys
import unittest
import unittest.mock

ROOT = pathlib.Path(__file__).resolve().parents[2]
SIDEBAR = ROOT / "crates/infiltrator-slint/ui/components/sidebar.slint"

LIST_RULE = re.compile(r"\bListRule\s*\{")
STRETCH_SPACER = re.compile(r"Rectangle\s*\{\s*vertical-stretch:\s*1;")
CARD_BORDER = re.compile(r"border-width:\s*1 \* LumenUiScale\.unit;")
# Anything that paints a fact below the navigation stack. The rail's only
# content is the typed registry's rows; a cluster is a second owner.
RAIL_CONTENT = re.compile(r"\b(Text|Image|SurfaceCard|IconButton|SectionCard)\s*\{")


def mask_comments(source: str) -> str:
    out: list[str] = []
    in_block = False
    index = 0
    while index < len(source):
        if not in_block and source[index : index + 2] == "//":
            end = source.find("\n", index)
            index = len(source) if end == -1 else end
            continue
        if source[index : index + 2] == "/*":
            in_block = True
            index += 2
            continue
        if in_block:
            if source[index : index + 2] == "*/":
                in_block = False
                index += 2
            else:
                index += 1
            continue
        out.append(source[index])
        index += 1
    return "".join(out)


def violations(source: str) -> list[str]:
    code = mask_comments(source)
    found = []
    rules = LIST_RULE.findall(code)
    if rules:
        found.append(
            f"the rail draws {len(rules)} ListRule separator(s): its separators are headings "
            "and its boxes draw their own edges, so a hairline duplicates a boundary"
        )
    spacer = STRETCH_SPACER.search(code)
    if spacer is None:
        found.append("the rail no longer parks its free air below the navigation stack")
    else:
        tail = code[spacer.end() :]
        if LIST_RULE.search(tail):
            found.append("a separator follows the stretch spacer: the bottom cluster declares its edge twice")
        if CARD_BORDER.search(tail):
            found.append(
                "the rail draws a bordered cluster below the navigation stack: the sidebar is "
                "navigation only, so status, throughput, brand and version facts keep their "
                "single owner in the title bar and the dashboard"
            )
        if RAIL_CONTENT.search(tail):
            found.append(
                "the rail paints content below the navigation stack: the sidebar is navigation "
                "only, so no second source of a status, throughput, brand or version fact may "
                "live below the nav stack"
            )
    return found


def live_violations() -> list[str]:
    return violations(SIDEBAR.read_text(encoding="utf-8"))


class GuardSelfTest(unittest.TestCase):
    def setUp(self) -> None:
        self.source = SIDEBAR.read_text(encoding="utf-8")

    def test_live_tree_is_clean(self) -> None:
        self.assertEqual(live_violations(), [])

    def test_a_duplicated_rim_is_caught(self) -> None:
        anchor = "        Rectangle { vertical-stretch: 1; background: transparent; }"
        self.assertIn(anchor, self.source)
        planted = self.source.replace(
            anchor,
            anchor + "\n        ListRule {}",
            1,
        )
        found = violations(planted)
        self.assertTrue(any("declares its edge twice" in item for item in found), found)

    def test_a_comment_mentioning_the_rule_does_not_count(self) -> None:
        anchor = "        Rectangle { vertical-stretch: 1; background: transparent; }"
        planted = self.source.replace(
            anchor,
            "        // ListRule {} was here and is now gone\n" + anchor,
            1,
        )
        self.assertEqual(violations(planted), [])

    def test_a_bottom_cluster_is_caught(self) -> None:
        """A rail that paints facts again is the second truth source."""
        anchor = "        Rectangle { vertical-stretch: 1; background: transparent; }"
        self.assertIn(anchor, self.source)
        planted = self.source.replace(
            anchor,
            anchor
            + "\n        SurfaceCard { Rectangle { border-width: 1 * LumenUiScale.unit; } }",
            1,
        )
        found = violations(planted)
        self.assertTrue(any("navigation only" in item for item in found), found)

    def test_the_free_air_stays_owned_by_the_spacer(self) -> None:
        planted = self.source.replace(
            "Rectangle { vertical-stretch: 1; background: transparent; }",
            "Rectangle { background: transparent; }",
            1,
        )
        found = violations(planted)
        self.assertTrue(any("free air" in item for item in found), found)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("enforce", "report"), default="enforce")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(GuardSelfTest)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1
    found = live_violations()
    for item in found:
        print(f"SLINT-018 [{args.mode}]: {item}")
    print(f"slint rail edge dialect guard: findings={len(found)}")
    return 1 if found and args.mode == "enforce" else 0


if __name__ == "__main__":
    sys.exit(main())
