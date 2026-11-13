#!/usr/bin/env python3
"""The rule-target picker consumes the Core readback, not a view-local list.

`RuleDocumentSnapshot.targets` is the set of outbounds the active profile can
actually commit. The Iced rules page used to sort live proxy groups and append
literal `DIRECT` / `REJECT` inside the view, so it could offer a target the
commit path would reject and hide one it would accept. The ClashParty parity
recon found the same pattern as a business-fact-in-the-view defect; this guard
keeps the owner/consumer seam honest without running cargo.
"""
from __future__ import annotations

import argparse
import pathlib
import sys
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
OWNER = ROOT / "crates/infiltrator-presentation/src/rule_form_binding.rs"
CONSUMER = ROOT / "crates/infiltrator-iced/src/view/rules.rs"


def findings(owner: pathlib.Path = OWNER, consumer: pathlib.Path = CONSUMER) -> list[str]:
    problems: list[str] = []
    if not owner.is_file():
        return [f"rule form owner is missing: {owner}"]
    if not consumer.is_file():
        return [f"rules page is missing: {consumer}"]
    owner_text = owner.read_text(encoding="utf-8")
    consumer_text = consumer.read_text(encoding="utf-8")
    if "pub fn form_targets(" not in owner_text:
        problems.append("the presentation owner does not publish form_targets()")
    if "base.targets" not in owner_text:
        problems.append("form_targets() no longer reads RuleDocumentSnapshot.targets")
    if "form_targets(&state.editor.rule_list)" not in consumer_text:
        problems.append("the Iced rules page does not consume form_targets()")
    for literal in ('"DIRECT".to_string()', '"REJECT".to_string()'):
        if literal in consumer_text:
            problems.append(
                f"the Iced rules page rebuilds a target catalogue with {literal}; "
                "the Core readback is the only owner"
            )
    if "available_targets.sort()" in consumer_text or "proxy_groups" in consumer_text:
        problems.append(
            "the Iced rules page derives targets from live proxy groups again; "
            "the active rule document owns the selectable outbounds"
        )
    return problems


class GuardSelfTest(unittest.TestCase):
    def test_the_live_tree_is_clean(self) -> None:
        self.assertEqual(findings(), [])

    def plant(self, *, owner: str | None = None, consumer: str | None = None):
        import tempfile

        directory = pathlib.Path(tempfile.mkdtemp())
        owner_path = directory / "owner.rs"
        consumer_path = directory / "consumer.rs"
        owner_path.write_text(owner if owner is not None else OWNER.read_text())
        consumer_path.write_text(consumer if consumer is not None else CONSUMER.read_text())
        return owner_path, consumer_path

    def test_a_missing_owner_is_caught(self) -> None:
        owner, consumer = self.plant(owner="pub fn other() {}\n")
        problems = findings(owner, consumer)
        self.assertTrue(any("does not publish form_targets" in item for item in problems))

    def test_a_view_local_direct_literal_is_caught(self) -> None:
        consumer = CONSUMER.read_text() + '\nlet x = "DIRECT".to_string();\n'
        owner, planted = self.plant(consumer=consumer)
        self.assertTrue(any("rebuilds a target catalogue" in item for item in findings(owner, planted)))

    def test_a_proxy_group_derivation_is_caught(self) -> None:
        consumer = CONSUMER.read_text() + "\n// state.runtime.proxy_groups\nlet mut available_targets = vec![];\navailable_targets.sort();\n"
        owner, planted = self.plant(consumer=consumer)
        self.assertTrue(any("derives targets from live proxy groups" in item for item in findings(owner, planted)))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--mode", choices=("report", "enforce"), default="enforce")
    args = parser.parse_args()
    if args.self_test:
        result = unittest.TextTestRunner(verbosity=1).run(
            unittest.defaultTestLoader.loadTestsFromTestCase(GuardSelfTest)
        )
        return 0 if result.wasSuccessful() else 1
    problems = findings()
    for problem in problems:
        print(f"rule-target-owner guard: {problem}", file=sys.stderr)
    print(f"rule-target-owner guard: findings={len(problems)}")
    return 1 if problems and args.mode == "enforce" else 0


if __name__ == "__main__":
    raise SystemExit(main())
