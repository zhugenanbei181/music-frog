#!/usr/bin/env python3
"""SLINT-018: the rail declares each boundary once.

The rail already owns a separator dialect, written down in its own comments:
a group heading IS the separator, so a hairline directly above it is a double
signal. The bottom cluster broke exactly that rule. Measured on every desktop
skin of the published receipt, a full-width `ListRule` sat at y610 and the
status card's own border at y617, both spanning x18..241 - one box with two
top edges 6px apart. Two owners claiming the same boundary is what made the
heading look stray in b139, and it reads here as a box with a doubled rim.

Three rules keep the dialect:

* the rail draws no `ListRule` at all: its separators are headings, and its
  boxes draw their own edges;
* the free air of the rail belongs above the bottom cluster, so the stretch
  spacer must stay the last thing before the status card;
* the status card keeps exactly one edge, so removing the rule cannot leave
  the cluster boundary-less.
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
        found.append("the rail no longer parks its free air above the bottom cluster")
    else:
        tail = code[spacer.end() :]
        if LIST_RULE.search(tail):
            found.append("a separator follows the stretch spacer: the bottom cluster declares its edge twice")
    card = code[spacer.end():] if spacer else code
    if len(CARD_BORDER.findall(card)) < 1:
        found.append("the status cluster lost its single edge")
    return found


def live_violations() -> list[str]:
    return violations(SIDEBAR.read_text(encoding="utf-8"))


class GuardSelfTest(unittest.TestCase):
    def setUp(self) -> None:
        self.source = SIDEBAR.read_text(encoding="utf-8")

    def test_live_tree_is_clean(self) -> None:
        self.assertEqual(live_violations(), [])

    def test_a_duplicated_rim_is_caught(self) -> None:
        planted = self.source.replace(
            "        // FlClash-standard unified status card",
            "        ListRule {}\n        // FlClash-standard unified status card",
            1,
        )
        found = violations(planted)
        self.assertTrue(any("declares its edge twice" in item for item in found), found)

    def test_a_comment_mentioning_the_rule_does_not_count(self) -> None:
        planted = self.source.replace(
            "        // FlClash-standard unified status card",
            "        // ListRule {} was here and is now gone\n        // FlClash-standard unified status card",
            1,
        )
        self.assertEqual(violations(planted), [])

    def test_a_borderless_status_cluster_is_caught(self) -> None:
        start = self.source.index("FlClash-standard unified status card")
        head, tail = self.source[:start], self.source[start:]
        planted = head + tail.replace("border-width: 1 * LumenUiScale.unit;", "", 1)
        found = violations(planted)
        self.assertTrue(any("lost its single edge" in item for item in found), found)

    def test_the_free_air_stays_owned_by_the_spacer(self) -> None:
        planted = self.source.replace("vertical-stretch: 1; background: transparent; }", "background: transparent; }", 1)
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
