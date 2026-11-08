#!/usr/bin/env python3
"""SLINT-017: a workbench button carries an action, never a status sentence.

The editor receipt measured one defect at both viewports: the export-review and
snapshot-restoration buttons took their labels from `script_export_review_instructions`
and `snapshot_restore_ready`, which are whole sentences the card header already
publishes as its status. A 96-character English label made one button wider than
the compact card, the non-wrapping action bar pushed the last two buttons past the
window edge, and confirming a snapshot restoration became unreachable at 720 wide.

Two rules keep the shape:

* a button label resolves to copy without sentence punctuation - a sentence is a
  status the header owns, not an action a button owns;
* a button label stays inside the action budget, so one label can never outgrow
  the row that hosts it.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys
import unittest
import unittest.mock

ROOT = pathlib.Path(__file__).resolve().parents[2]
PROJECTION = ROOT / "crates/infiltrator-slint/src/lumen_projection"
LOCALES = ROOT / "crates/infiltrator-shared/src"

BUTTON_CALL = re.compile(
    r'\bbutton\(\s*"([a-z0-9\-]+)",\s*&?catalog\.text\("([a-z0-9_]+)"\)',
    re.MULTILINE,
)
ENTRY = re.compile(r'"([a-z0-9_]+)"\s*=>\s*"((?:[^"\\]|\\.)*)"')
SENTENCE = re.compile(r"[。；]|\. ")
ZH_BUDGET = 16
EN_BUDGET = 44


def locale_tables() -> tuple[dict[str, str], dict[str, str]]:
    chinese: dict[str, str] = {}
    english: dict[str, str] = {}
    for path in sorted(LOCALES.glob("locales_table*.rs")):
        target = english if re.search(r"_en(?:_|$)", path.stem) else chinese
        target.update(ENTRY.findall(path.read_text(encoding="utf-8")))
    return chinese, english


def button_labels() -> list[tuple[str, str, str]]:
    found = []
    for path in sorted(PROJECTION.rglob("*.rs")):
        for item, key in BUTTON_CALL.findall(path.read_text(encoding="utf-8")):
            found.append((path.relative_to(ROOT).as_posix(), item, key))
    return found


def violations() -> list[str]:
    chinese, english = locale_tables()
    found = []
    for location, item, key in button_labels():
        for locale, table, budget in (("zh-CN", chinese, ZH_BUDGET), ("en-US", english, EN_BUDGET)):
            copy = table.get(key)
            if copy is None:
                found.append(f"{location}: button '{item}' references missing {locale} key '{key}'")
                continue
            if SENTENCE.search(copy):
                found.append(
                    f"{location}: button '{item}' label '{key}' is a {locale} sentence, not an action"
                )
            if len(copy) > budget:
                found.append(
                    f"{location}: button '{item}' label '{key}' is {len(copy)} {locale} chars (budget {budget})"
                )
    return found


class GuardSelfTest(unittest.TestCase):
    def test_live_tree_is_clean(self) -> None:
        self.assertEqual(violations(), [])

    def test_a_status_sentence_label_is_caught(self) -> None:
        with unittest.mock.patch(
            f"{__name__}.button_labels",
            return_value=[("editor.rs", "snapshot-restore-prepare", "snapshot_restore_ready")],
        ):
            found = violations()
        self.assertTrue(any("sentence" in item for item in found), found)

    def test_an_over_budget_label_is_caught(self) -> None:
        with unittest.mock.patch(
            f"{__name__}.button_labels",
            return_value=[("editor.rs", "script-export-prepare", "script_export_review_instructions")],
        ):
            found = violations()
        self.assertTrue(any("budget" in item for item in found), found)

    def test_a_missing_key_is_caught(self) -> None:
        with unittest.mock.patch(
            f"{__name__}.button_labels",
            return_value=[("editor.rs", "ghost", "no_such_key_at_all")],
        ):
            found = violations()
        self.assertTrue(any("missing" in item for item in found), found)

    def test_the_short_action_labels_pass(self) -> None:
        with unittest.mock.patch(
            f"{__name__}.button_labels",
            return_value=[
                ("editor.rs", "script-export-prepare", "script_export_review_action"),
                ("editor.rs", "snapshot-restore-prepare", "snapshot_restore_review_action"),
            ],
        ):
            self.assertEqual(violations(), [])


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
        print(f"SLINT-017 [{args.mode}]: {item}")
    print(f"slint action label guard: findings={len(found)}")
    return 1 if found and args.mode == "enforce" else 0


if __name__ == "__main__":
    sys.exit(main())
