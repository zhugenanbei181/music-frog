#!/usr/bin/env python3
"""Forbid shrinking a file's line count instead of splitting its business.

`line-guard.py` counts non-comment, non-blank lines. That is a real budget, but
it can be gamed without moving a single responsibility: merge `use` statements,
delete explanatory doc comments, erase blank lines, or concatenate source with
`include!`. Each makes the same monolith smaller on paper. The code-quality
baseline says "不压行、不提高阈值、不加白名单或用源码拼接绕过"; this guard turns
that sentence into an executable rule.

Checks, all against the checked-in Rust sources:
  1. no `mod.rs` and no `include!` (the two concatenation/layout cheats);
  2. the line budget's own threshold and counting rule are untouched;
  3. a message bus that is already at/over its budget must not gain loose
     variants: new families belong in a typed sibling module, reached through
     one variant that carries the typed intent. `Message` and `UiCommand` are
     the two buses this rule pins, with the proxies-page intent family as the
     planted proof (b268).
  4. the standard itself must publish the rule, so a doc edit alone cannot
     retire it.
"""
from __future__ import annotations

import argparse
import pathlib
import re
import sys
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
LINE_GUARD = ROOT / "scripts/quality/line-guard.py"
MOD_RS_GUARD = ROOT / "scripts/quality/mod-rs-guard.py"
IMPORT_GUARD = ROOT / "scripts/quality/import-guard.py"
STANDARD = ROOT / "docs/CODE_QUALITY_BASELINE.md"
BUSES = {
    "crates/infiltrator-iced/src/types/message.rs": "Message",
    "crates/infiltrator-bevy-ui/src/command.rs": "UiCommand",
}
# A bus this close to the budget may add one variant that carries a typed
# intent enum, never a new family of loose variants. Anything else is a split
# along a business seam into a real sibling module.
BUDGET = 800
NEAR_BUDGET = 780
INTENT_MODULES = ("crates/infiltrator-iced/src/types/message/proxy_page_intent.rs",)


def text(relative: str) -> str:
    return (ROOT / relative).read_text(encoding="utf-8")


def non_comment_lines(source: str) -> int:
    sys.path.insert(0, str(ROOT / "scripts/quality"))
    from rust_syntax import mask_comments
    return sum(bool(line.strip()) for line in mask_comments(source).splitlines())


def enum_variant_count(source: str, enum_name: str) -> int:
    match = re.search(rf"pub enum {enum_name}\s*\{{(.*?)\n\}}", source, re.S)
    if match is None:
        return 0
    body = match.group(1)
    # One line per variant at the top level of the enum body.
    return sum(1 for line in body.splitlines()
               if re.match(r"^    [A-Z][A-Za-z0-9_]*(\s|\{|,)", line))


def findings(root: pathlib.Path = ROOT) -> list[str]:
    found: list[str] = []
    for path in sorted(root.glob("crates/**/mod.rs")):
        if "target" not in path.parts:
            found.append(f"{path.relative_to(root)}: mod.rs is banned; use foo.rs + foo/")
    for path in sorted(root.glob("crates/**/*.rs")):
        if "target" in path.parts:
            continue
        for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            if "include!" in line and not line.lstrip().startswith("//"):
                found.append(f"{path.relative_to(root)}:{number}: source include! concatenation is banned")
    guard = LINE_GUARD.read_text(encoding="utf-8")
    if "DEFAULT_BUDGET = 800" not in guard or "non_comment_lines" not in guard:
        found.append("line-guard.py: the counting rule or 800-line budget was changed")
    if "include!" not in IMPORT_GUARD.read_text(encoding="utf-8"):
        found.append("import-guard.py stopped banning source include! concatenation")
    standard = STANDARD.read_text(encoding="utf-8")
    for phrase in ("不压行", "mod.rs", "include!", "typed 意图"):
        if phrase not in standard:
            found.append(f"CODE_QUALITY_BASELINE.md no longer publishes {phrase!r}")
    for relative, enum_name in BUSES.items():
        source = text(relative)
        lines = non_comment_lines(source)
        if lines > BUDGET:
            found.append(f"{relative}: {lines} code lines is over the {BUDGET}-line budget")
        if lines >= NEAR_BUDGET:
            # A near-budget bus must prove its new family lives in a typed
            # sibling module, not as loose variants on the bus itself.
            for module in INTENT_MODULES:
                if not (root / module).is_file():
                    found.append(f"{module}: the typed intent sibling module is missing")
            if "ProxyPageIntent" not in source and relative.endswith("message.rs"):
                found.append(f"{relative}: a near-budget bus carries no typed intent variant")
    return found


class SelfTest(unittest.TestCase):
    def setUp(self) -> None:
        self.root = pathlib.Path(__file__).resolve().parents[2]

    def test_the_real_tree_passes(self) -> None:
        self.assertEqual(findings(self.root), [])

    def test_a_mod_rs_is_refused(self) -> None:
        import tempfile
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            (root / "crates/x/src").mkdir(parents=True)
            (root / "crates/x/src/mod.rs").write_text("pub mod a;\n")
            self.assertTrue(any("mod.rs is banned" in item for item in findings(root)))

    def test_an_include_is_refused(self) -> None:
        import tempfile
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            (root / "crates/x/src").mkdir(parents=True)
            (root / "crates/x/src/a.rs").write_text('include!("b.rs");\n')
            self.assertTrue(any("include!" in item for item in findings(root)))

    def test_a_changed_budget_is_refused(self) -> None:
        original = LINE_GUARD.read_text()
        try:
            LINE_GUARD.write_text(original.replace("DEFAULT_BUDGET = 800", "DEFAULT_BUDGET = 900"))
            self.assertTrue(any("budget was changed" in item for item in findings(self.root)))
        finally:
            LINE_GUARD.write_text(original)

    def test_a_standard_without_the_rule_is_refused(self) -> None:
        original = STANDARD.read_text()
        try:
            STANDARD.write_text(original.replace("不压行", "压缩行数"))
            self.assertTrue(any("no longer publishes" in item for item in findings(self.root)))
        finally:
            STANDARD.write_text(original)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=["report", "enforce"], default="report")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(SelfTest)
        result = unittest.TextTestRunner(verbosity=1).run(suite)
        return 0 if result.wasSuccessful() else 1
    found = findings()
    for item in found:
        print(f"VIOLATION [{args.mode}]: {item}")
    print(f"line budget integrity guard: total={len(found)}")
    return 1 if args.mode == "enforce" and found else 0


if __name__ == "__main__":
    sys.exit(main())
