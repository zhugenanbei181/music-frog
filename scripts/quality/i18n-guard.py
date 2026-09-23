#!/usr/bin/env python3
"""Fail-closed internationalization checks for the Iced surface."""

from __future__ import annotations

import argparse
import pathlib
import re
import sys


ROOT = pathlib.Path(__file__).resolve().parents[2]
ZH_TABLES = [
    pathlib.Path("crates/infiltrator-shared/src/locales_table.rs"),
    pathlib.Path("crates/infiltrator-shared/src/locales_table_legacy.rs"),
    pathlib.Path("crates/infiltrator-shared/src/locales_table_ext.rs"),
]
EN_TABLES = [
    pathlib.Path("crates/infiltrator-shared/src/locales_table_en.rs"),
    pathlib.Path("crates/infiltrator-shared/src/locales_table_en_legacy.rs"),
    pathlib.Path("crates/infiltrator-shared/src/locales_table_en_ext.rs"),
]
VIEW_SCAN_DIRS = [
    pathlib.Path("crates/infiltrator-iced/src/view"),
    pathlib.Path("crates/infiltrator-iced/src/view_root"),
]
EXEMPT_FILES = {
    "crates/infiltrator-iced/src/demo/fixtures.rs",
    "crates/infiltrator-iced/src/demo/proxy_fixtures.rs",
    "crates/infiltrator-iced/src/demo/state.rs",
    "crates/infiltrator-iced/src/view/svg_icons.rs",
}

KEY_PATTERN = re.compile(r'^\s*"([a-z0-9_]+)"\s*=>', re.MULTILINE)
TRANSLATION_REF_PATTERN = re.compile(r'\.tr\(\s*"([a-z0-9_]+)"')
CJK_PATTERN = re.compile(r"[\u4e00-\u9fff]")
STRING_LITERAL_PATTERN = re.compile(r'"([^"\\]*(?:\\.[^"\\]*)*)"')


def read(path: pathlib.Path) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def extract_keys(path: pathlib.Path) -> set[str]:
    return set(KEY_PATTERN.findall(read(path)))


def extract_all_keys(paths: list[pathlib.Path]) -> set[str]:
    keys: set[str] = set()
    for path in paths:
        keys.update(extract_keys(path))
    return keys


def code_lines(path: pathlib.Path):
    in_test_module = False
    for line_no, line in enumerate(read(path).splitlines(), start=1):
        stripped = line.strip()
        if stripped.startswith("#[cfg(test)]") or stripped.startswith("mod tests"):
            in_test_module = True
        if in_test_module or stripped.startswith("//") or stripped.startswith("/*"):
            continue
        yield line_no, line.split("//", 1)[0]


def check_key_parity() -> list[str]:
    zh_keys = extract_all_keys(ZH_TABLES)
    en_keys = extract_all_keys(EN_TABLES)
    violations: list[str] = []
    for key in sorted(zh_keys - en_keys):
        violations.append(f"en-US table missing key '{key}'")
    for key in sorted(en_keys - zh_keys):
        violations.append(f"zh-CN table missing key '{key}'")
    return violations


def check_key_names() -> list[str]:
    violations: list[str] = []
    for path in ZH_TABLES + EN_TABLES:
        for line_no, line in enumerate(read(path).splitlines(), start=1):
            match = re.match(r'^\s*"([^"]+)"\s*=>', line)
            if match and re.fullmatch(r"[a-z0-9_]+", match.group(1)) is None:
                violations.append(f"{path}:{line_no}: invalid locale key '{match.group(1)}'")
    return violations


def check_english_copy() -> list[str]:
    violations: list[str] = []
    for path in EN_TABLES:
        for line_no, line in enumerate(read(path).splitlines(), start=1):
            if CJK_PATTERN.search(line):
                violations.append(f"{path}:{line_no}: en-US copy contains CJK text")
    return violations


def check_untranslated_literals() -> list[str]:
    violations: list[str] = []
    for scan_dir in VIEW_SCAN_DIRS:
        absolute_dir = ROOT / scan_dir
        if not absolute_dir.is_dir():
            continue
        for path in sorted(absolute_dir.rglob("*.rs")):
            relative = str(path.relative_to(ROOT)).replace("\\", "/")
            if relative in EXEMPT_FILES or "test" in relative.lower():
                continue
            relative_path = scan_dir / path.relative_to(absolute_dir)
            for line_no, code in code_lines(relative_path):
                for match in STRING_LITERAL_PATTERN.finditer(code):
                    if CJK_PATTERN.search(match.group(1)):
                        violations.append(
                            f"{relative}:{line_no}: hardcoded CJK string '{match.group(1)}'"
                        )
    return violations


def check_view_references(all_keys: set[str]) -> list[str]:
    references: dict[str, list[str]] = {}
    for scan_dir in VIEW_SCAN_DIRS:
        absolute_dir = ROOT / scan_dir
        if not absolute_dir.is_dir():
            continue
        for path in sorted(absolute_dir.rglob("*.rs")):
            relative = str(path.relative_to(ROOT)).replace("\\", "/")
            relative_path = scan_dir / path.relative_to(absolute_dir)
            for line_no, code in code_lines(relative_path):
                for key in TRANSLATION_REF_PATTERN.findall(code):
                    references.setdefault(key, []).append(f"{relative}:{line_no}")

    violations: list[str] = []
    for key in sorted(set(references) - all_keys):
        locations = ", ".join(references[key][:3])
        violations.append(f"{locations}: view references missing locale key '{key}'")
    return violations


def run_self_test() -> None:
    zh_keys = extract_all_keys(ZH_TABLES)
    en_keys = extract_all_keys(EN_TABLES)
    violations = (
        check_key_parity()
        + check_key_names()
        + check_english_copy()
        + check_untranslated_literals()
        + check_view_references(zh_keys | en_keys)
    )
    if violations:
        raise AssertionError("\n".join(violations))
    print(f"i18n guard self-test passed; locale keys={len(zh_keys)}")


def main() -> int:
    parser = argparse.ArgumentParser(description="Iced i18n quality guard")
    parser.add_argument("--mode", choices=["report", "enforce"], default="report")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        run_self_test()
        return 0

    zh_keys = extract_all_keys(ZH_TABLES)
    en_keys = extract_all_keys(EN_TABLES)
    parity = check_key_parity()
    names = check_key_names()
    english = check_english_copy()
    literals = check_untranslated_literals()
    references = check_view_references(zh_keys | en_keys)
    violations = parity + names + english + literals + references

    for violation in violations:
        print(f"VIOLATION [{args.mode}]: {violation}")
    print(
        "i18n quality guard: "
        f"parity_errors={len(parity)} key_name_errors={len(names)} "
        f"english_errors={len(english)} literal_errors={len(literals)} "
        f"reference_errors={len(references)} total={len(violations)}"
    )
    return 1 if args.mode == "enforce" and violations else 0


if __name__ == "__main__":
    sys.exit(main())
