#!/usr/bin/env python3
"""Reject Slint text that assembles one typeface from several TextRoles.

The shipped font inventory has only 400/500/600 physical faces. A Text that
takes `micro-size` with `mono-family`, or `caption-size` with
`labelStrong-weight`, asks the renderer for an unregistered triple and either
synthesises emboldening or silently substitutes a face. This guard pairs every
`Text { ... }` block and requires its explicit font properties to name one
role, or to resolve a whole typed role through `DesignType.size/weight/family`.
It is deliberately a bounded lexical reader; dynamic whole-role expressions
and theme aliases are allowed, partial role mixes are not.
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import pathlib
import re
import sys
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
AUTHORITY = ROOT / "crates/infiltrator-contract/assets/design-authority.json"
SLINT_UI = ROOT / "crates/infiltrator-slint/ui"
GENERATOR = ROOT / "scripts/generate_design_resources.py"
TEXT_START = re.compile(r"\bText\s*\{")
COMPONENT_START = re.compile(r"^(?:export )?component (\w+)", re.M)
COMPONENT_FONT_INPUT = re.compile(
    r"^\s*in\s+(?:out\s+)?property\s*<length>\s+font-size\b", re.M
)
ROLE_TOKEN = re.compile(r"DesignType\.([A-Za-z0-9]+)-(?:size|weight|family)")
ROLE_FUNCTION = re.compile(r"DesignType\.(?:size|weight|family)\(")
ROLE_FUNCTION_ARG = re.compile(r"TextRole\.([A-Za-z0-9]+)")
WHOLE_ROLE_PROPERTY = re.compile(
    r"\bproperty\s*<TextRole>\s*([A-Za-z0-9_-]+)\s*:"
)
FONT_PROPERTY = re.compile(r"\b(font-size|font-weight|font-family)\s*:\s*([^;]+);")
PARENT_FONT_SIZE = re.compile(r"\b(?:root|self|parent)\.font-size\b")
WHOLE_ROLE_SIZE = re.compile(r"DesignType\.size\(")


def block_at(source: str, opening: int) -> str:
    """Return the body of the brace block opened at `opening`."""
    index = opening
    depth = 0
    while index < len(source):
        if source[index] == "{":
            depth += 1
        elif source[index] == "}":
            depth -= 1
            if depth == 0:
                return source[opening + 1 : index]
        index += 1
    return source[opening + 1 :]


def component_ranges(source: str) -> list[tuple[str, int, int, bool]]:
    """Top-level component name/range plus whether its Text may see a font-size input."""
    ranges = []
    for match in COMPONENT_START.finditer(source):
        end = source.find("{", match.start())
        if end < 0:
            continue
        body = block_at(source, end)
        ranges.append(
            (
                match.group(1),
                match.start(),
                end + len(body) + 1,
                bool(COMPONENT_FONT_INPUT.search(body)),
            )
        )
    return ranges


def blocks(source: str) -> list[tuple[int, str, bool]]:
    result: list[tuple[int, str, bool]] = []
    components = component_ranges(source)
    for match in TEXT_START.finditer(source):
        line = source.count("\n", 0, match.start()) + 1
        body = block_at(source, match.end() - 1)
        has_font_input = any(
            start <= match.start() < end and font_input
            for _name, start, end, font_input in components
        )
        result.append((line, body, has_font_input))
    return result


def findings_for(source: str, path: pathlib.Path) -> list[str]:
    findings: list[str] = []
    for line, block, has_font_input in blocks(source):
        prefixes = sorted(set(ROLE_TOKEN.findall(block)))
        has_whole_role = bool(ROLE_FUNCTION.search(block))
        properties = {name: value.strip() for name, value in FONT_PROPERTY.findall(block)}
        role_arguments = {
            frozenset(ROLE_FUNCTION_ARG.findall(value))
            for value in properties.values()
            if ROLE_FUNCTION.search(value)
        }
        nonempty_role_arguments = [roles for roles in role_arguments if roles]
        local_role_properties = set(WHOLE_ROLE_PROPERTY.findall(block))
        local_role_property_used = any(
            re.search(rf"\b{re.escape(name)}\b", value)
            for value in properties.values()
            for name in local_role_properties
        )
        uses_local_role_property = any(
            ROLE_FUNCTION.search(value) and local_role_property_used
            for value in properties.values()
        )
        local_role_expressions = {
            frozenset(ROLE_FUNCTION_ARG.findall(match.group(0)))
            for match in WHOLE_ROLE_PROPERTY.finditer(block)
        }
        local_role_expressions.discard(frozenset())
        if len(prefixes) > 1 and not has_whole_role:
            findings.append(
                f"{path}:{line}: Text mixes typography roles {', '.join(prefixes)}; "
                "use one registered TextRole triple or DesignType.size/weight/family(TextRole.…)"
            )
        whole_role_mix = (
            not uses_local_role_property
            and any(ROLE_TOKEN.search(value) for value in properties.values())
        )
        if has_whole_role and whole_role_mix:
            findings.append(
                f"{path}:{line}: Text mixes whole TextRole resolutions from different roles"
            )
        elif (
            has_whole_role
            and uses_local_role_property
            and len(set(nonempty_role_arguments) | local_role_expressions) > 1
        ):
            findings.append(
                f"{path}:{line}: Text mixes a local TextRole owner with raw role literals"
            )
        font_size = properties.get("font-size", "")
        if PARENT_FONT_SIZE.search(font_size) and WHOLE_ROLE_SIZE.search(font_size):
            findings.append(
                f"{path}:{line}: Text derives font-size from the parent input and a whole TextRole; "
                "choose the parent font-size once, then use DesignType.weight/family for that role"
            )
        if has_font_input and WHOLE_ROLE_SIZE.search(font_size) and not PARENT_FONT_SIZE.search(font_size):
            findings.append(
                f"{path}:{line}: Text bypasses the component's parent font-size input with DesignType.size"
            )
    return findings


def load_generator():
    spec = importlib.util.spec_from_file_location("generate_design_resources", GENERATOR)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


def authority_findings(root: pathlib.Path = ROOT) -> list[str]:
    import json

    generator = load_generator()
    authority_path = (
        generator.AUTHORITY if root == ROOT else root / "crates/infiltrator-contract/assets/design-authority.json"
    )
    authority = json.loads(authority_path.read_text(encoding="utf-8"))
    findings = [
        f"design-authority.json: {problem}"
        for problem in generator.authority_problems(authority)
    ]
    authority_roles = {
        role["role"][:1].lower() + role["role"][1:]
        for role in authority.get("type_roles", [])
    }
    ui = root / "crates/infiltrator-slint/ui"
    for path in sorted(ui.rglob("*.slint")):
        if path.name == "design-type.slint":
            continue
        text = path.read_text(encoding="utf-8")
        for match in ROLE_TOKEN.finditer(text):
            if match.group(1) not in authority_roles:
                line = text.count("\n", 0, match.start()) + 1
                findings.append(
                    f"{path.relative_to(root)}:{line}: dead TextRole prefix {match.group(1)}"
                )
        for match in ROLE_FUNCTION_ARG.finditer(text):
            if match.group(1) not in authority_roles:
                line = text.count("\n", 0, match.start()) + 1
                findings.append(
                    f"{path.relative_to(root)}:{line}: unregistered TextRole.{match.group(1)}"
                )
        for match in re.finditer(r'font-family:\s*"([^"]+)"', text):
            line = text.count("\n", 0, match.start()) + 1
            findings.append(
                f"{path.relative_to(root)}:{line}: raw font family {match.group(1)!r}; use DesignType.family(role)"
            )
        for match in re.finditer(r"font-weight:\s*([^;]+);", text):
            values = re.findall(r"\b\d{3}\b", match.group(1))
            if any(int(value) not in {400, 500, 600} for value in values):
                line = text.count("\n", 0, match.start()) + 1
                findings.append(
                    f"{path.relative_to(root)}:{line}: unshipped physical weight {match.group(1).strip()}"
                )
        for match in re.finditer(r"\banimate\s+[^;{]*\bfont-weight\b", text):
            line = text.count("\n", 0, match.start()) + 1
            findings.append(
                f"{path.relative_to(root)}:{line}: font-weight is animated; switch typed roles instead"
            )
    return findings


def check(root: pathlib.Path = ROOT) -> list[str]:
    findings: list[str] = []
    for path in sorted((root / "crates/infiltrator-slint/ui").rglob("*.slint")):
        if path.name == "design-type.slint":
            continue
        findings += findings_for(path.read_text(encoding="utf-8"), path.relative_to(root))
    findings += authority_findings(root)
    return findings


class GuardSelfTest(unittest.TestCase):
    def test_single_role_is_allowed(self) -> None:
        source = "Text { font-size: DesignType.micro-size; font-weight: DesignType.micro-weight; font-family: DesignType.micro-family; }"
        self.assertEqual(findings_for(source, pathlib.Path("unit.slint")), [])

    def test_cross_role_mix_is_caught(self) -> None:
        source = "Text { font-size: DesignType.micro-size; font-family: DesignType.mono-family; }"
        findings = findings_for(source, pathlib.Path("unit.slint"))
        self.assertTrue(any("mixes typography roles" in finding for finding in findings))

    def test_typed_role_resolution_is_allowed(self) -> None:
        source = "Text { font-size: DesignType.size(root.active ? TextRole.microStrong : TextRole.micro); font-weight: DesignType.weight(root.active ? TextRole.microStrong : TextRole.micro); }"
        self.assertEqual(findings_for(source, pathlib.Path("unit.slint")), [])

    def test_closed_state_switch_with_fixed_parent_size_is_allowed(self) -> None:
        source = """Text {
            font-size: DesignType.size(TextRole.label);
            font-weight: DesignType.weight(index == root.current-index ? TextRole.labelStrong : TextRole.label);
        }"""
        self.assertEqual(findings_for(source, pathlib.Path("unit.slint")), [])

    def test_whole_role_and_raw_prefix_are_caught(self) -> None:
        source = "Text { font-size: DesignType.size(TextRole.microStrong); font-weight: DesignType.mono-weight; }"
        findings = findings_for(source, pathlib.Path("unit.slint"))
        self.assertTrue(any("whole TextRole resolution" in finding for finding in findings))

    def test_whole_role_arguments_must_agree(self) -> None:
        source = """Text {
            font-family: DesignType.family(TextRole.numericLarge);
            font-size: DesignType.size(root.dense ? TextRole.numericDense : TextRole.numericLarge);
            font-weight: DesignType.weight(TextRole.numericLarge);
        }"""
        findings = findings_for(source, pathlib.Path("unit.slint"))
        self.assertEqual(findings, [])

    def test_whole_role_arguments_may_switch_as_one_closed_set(self) -> None:
        source = """Text {
            font-size: DesignType.size(root.dense ? TextRole.numericDense : TextRole.numericLarge);
            font-weight: DesignType.weight(root.dense ? TextRole.numericDense : TextRole.numericLarge);
            font-family: DesignType.family(root.dense ? TextRole.numericDense : TextRole.numericLarge);
        }"""
        self.assertEqual(findings_for(source, pathlib.Path("unit.slint")), [])

    def test_split_size_and_family_role_mix_is_caught(self) -> None:
        source = """Text {
            font-family: DesignType.family(TextRole.numericLarge);
            font-size: DesignType.numericDense-size;
            font-weight: DesignType.weight(TextRole.numericLarge);
        }"""
        findings = findings_for(source, pathlib.Path("unit.slint"))
        self.assertTrue(any("whole TextRole" in finding for finding in findings), findings)

    def test_dynamic_whole_role_expression_is_allowed(self) -> None:
        source = """Text {
            property <TextRole> numeric-role: root.dense ? TextRole.numericDense : TextRole.numericLarge;
            font-family: DesignType.family(root.numeric-role);
            font-size: DesignType.size(root.numeric-role);
            font-weight: DesignType.weight(root.numeric-role);
        }"""
        self.assertEqual(findings_for(source, pathlib.Path("unit.slint")), [])

    def test_raw_family_is_caught_by_authority_scan(self) -> None:
        import tempfile

        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            ui = root / "crates/infiltrator-slint/ui"
            ui.mkdir(parents=True)
            authority = root / "crates/infiltrator-contract/assets"
            authority.mkdir(parents=True)
            (authority / "design-authority.json").write_text(
                AUTHORITY.read_text(encoding="utf-8"), encoding="utf-8"
            )
            (ui / "unit.slint").write_text(
                'Text { font-family: "HarmonyOS Sans SC"; }', encoding="utf-8"
            )
            findings = check(root)
        self.assertTrue(any("raw font family" in finding for finding in findings))

    def test_animated_weight_is_caught_by_authority_scan(self) -> None:
        import tempfile

        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            ui = root / "crates/infiltrator-slint/ui"
            ui.mkdir(parents=True)
            authority = root / "crates/infiltrator-contract/assets"
            authority.mkdir(parents=True)
            (authority / "design-authority.json").write_text(
                AUTHORITY.read_text(encoding="utf-8"), encoding="utf-8"
            )
            (ui / "unit.slint").write_text(
                "Text { animate color, font-weight { duration: 120ms; } }",
                encoding="utf-8",
            )
            findings = check(root)
        self.assertTrue(any("font-weight is animated" in finding for finding in findings))

    def test_parent_font_input_and_whole_role_size_are_caught(self) -> None:
        source = """component Pill {
    in property <length> font-size;
    Text { font-size: root.font-size + DesignType.size(TextRole.caption); }
}"""
        findings = findings_for(source, pathlib.Path("unit.slint"))
        self.assertTrue(any("parent input and a whole TextRole" in finding for finding in findings), findings)

    def test_parent_font_input_bypass_is_caught(self) -> None:
        source = """component Pill {
    in property <length> font-size;
    Text { font-size: DesignType.size(TextRole.caption); }
}"""
        findings = findings_for(source, pathlib.Path("unit.slint"))
        self.assertTrue(any("bypasses the component" in finding for finding in findings), findings)

    def test_parent_font_input_with_role_weight_is_allowed(self) -> None:
        source = """component Pill {
    in property <length> font-size;
    Text { font-size: root.font-size; font-weight: DesignType.weight(TextRole.captionStrong); font-family: DesignType.family(TextRole.captionStrong); }
}"""
        self.assertEqual(findings_for(source, pathlib.Path("unit.slint")), [])

    def test_authority_duplicate_triple_is_caught(self) -> None:
        import json

        generator = load_generator()
        authority = json.loads(generator.AUTHORITY.read_text(encoding="utf-8"))
        duplicate = dict(authority["type_roles"][-1])
        duplicate["role"] = "DuplicateUnitRole"
        authority["type_roles"].append(duplicate)
        self.assertTrue(
            any("duplicate typeface triple" in problem for problem in generator.authority_problems(authority))
        )

    def test_authority_inventory_is_closed(self) -> None:
        import json

        generator = load_generator()
        authority = json.loads(generator.AUTHORITY.read_text(encoding="utf-8"))
        self.assertEqual(len(authority["font_faces"]), 7)
        self.assertEqual(len(authority["type_roles"]), 26)
        self.assertEqual(authority["font_weights"], [400, 500, 600])
        self.assertEqual(authority["connection_column_weights"], [8, 16, 3, 5, 6, 6, 4])
        self.assertEqual(authority["hero_fact_column_weights"], [5, 7])
        cjk = {face["family_name"] for face in authority["font_faces"] if face["family"] == "Cjk"}
        self.assertEqual(cjk, {"HarmonyOS Sans SC"})

    def test_live_tree_is_clean(self) -> None:
        self.assertEqual(check(), [])


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--mode", choices=["report", "enforce"], default="enforce")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(GuardSelfTest)
        return 0 if unittest.TextTestRunner(verbosity=0).run(suite).wasSuccessful() else 1
    findings = check()
    for finding in findings:
        print(f"SLINT-TYPOGRAPHY: {finding}", file=sys.stderr)
    print(f"slint typography role guard: findings={len(findings)}")
    return 1 if findings and args.mode == "enforce" else 0


if __name__ == "__main__":
    raise SystemExit(main())
