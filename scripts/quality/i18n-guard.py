#!/usr/bin/env python3
"""i18n quality guard for MusicFrog Infiltrator.

Enforces two strict internationalization rules:
1. **100% Key Parity**: Every translation key in `locales_table.rs` (zh-CN)
   must have an exact 1:1 counterpart in `locales_table_en.rs` (en-US),
   and vice versa.
2. **Zero Bare Chinese Literals**: UI view layer files (e.g. `view/`, `view_root/`)
   must not contain raw, hardcoded Chinese string literals in UI code.
   All user-facing copy must be resolved through `Lang::tr(...)`, `Localizer`,
   or locale dictionaries.

Usage:
    python3 scripts/quality/i18n-guard.py [--mode report|enforce]
    python3 scripts/quality/i18n-guard.py --self-test
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys

ZH_TABLES = [
    pathlib.Path("crates/infiltrator-shared/src/locales_table.rs"),
    pathlib.Path("crates/infiltrator-shared/src/locales_table_ext.rs"),
]
EN_TABLES = [
    pathlib.Path("crates/infiltrator-shared/src/locales_table_en.rs"),
    pathlib.Path("crates/infiltrator-shared/src/locales_table_en_ext.rs"),
]

# Scanning roots for UI view code that must use i18n
VIEW_SCAN_DIRS = [
    pathlib.Path("crates/infiltrator-iced/src/view"),
    pathlib.Path("crates/infiltrator-iced/src/view_root"),
]

# Paths allowed to contain string literals with Chinese characters (e.g., demo fixture files)
EXEMPT_FILES = {
    "crates/infiltrator-iced/src/demo/fixtures.rs",
    "crates/infiltrator-iced/src/demo/proxy_fixtures.rs",
    "crates/infiltrator-iced/src/demo/state.rs",
    "crates/infiltrator-iced/src/view/svg_icons.rs",
}

KEY_PATTERN = re.compile(r'\"([a-zA-Z0-9_\-\.]+)\"\s*=>')
ZH_CHAR_PATTERN = re.compile(r'[\u4e00-\u9fff]')
STRING_LITERAL_PATTERN = re.compile(r'\"([^\"]*)\"')


def extract_keys_from_table(path: pathlib.Path) -> set[str]:
    if not path.is_file():
        raise FileNotFoundError(f"Locales table not found: {path}")
    content = path.read_text(encoding="utf-8")
    return set(KEY_PATTERN.findall(content))


def extract_all_keys(paths: list[pathlib.Path]) -> set[str]:
    keys = set()
    for p in paths:
        keys.update(extract_keys_from_table(p))
    return keys


def check_key_parity() -> list[str]:
    violations = []
    zh_keys = extract_all_keys(ZH_TABLES)
    en_keys = extract_all_keys(EN_TABLES)

    missing_in_en = zh_keys - en_keys
    missing_in_zh = en_keys - zh_keys

    for k in sorted(missing_in_en):
        violations.append(f"Key parity violation: '{k}' defined in zh-CN but missing in en-US table")
    for k in sorted(missing_in_zh):
        violations.append(f"Key parity violation: '{k}' defined in en-US but missing in zh-CN table")

    return violations


def check_untranslated_literals() -> list[str]:
    violations = []
    for scan_dir in VIEW_SCAN_DIRS:
        if not scan_dir.is_dir():
            continue
        for file_path in sorted(scan_dir.rglob("*.rs")):
            str_path = str(file_path).replace("\\", "/")
            if str_path in EXEMPT_FILES or "test" in str_path.lower():
                continue
            
            lines = file_path.read_text(encoding="utf-8").splitlines()
            in_test_module = False
            for line_no, line in enumerate(lines, start=1):
                trimmed = line.strip()
                if trimmed.startswith("#[cfg(test)]") or trimmed.startswith("mod tests"):
                    in_test_module = True
                if in_test_module:
                    continue

                # Strip out line comments
                code_part = line.split("//")[0]
                for match in STRING_LITERAL_PATTERN.finditer(code_part):
                    literal = match.group(1)
                    if ZH_CHAR_PATTERN.search(literal):
                        violations.append(
                            f"Hardcoded literal: {str_path}:{line_no}: \"{literal}\""
                        )
    return violations


def run_self_test() -> None:
    print("Running i18n-guard self tests...")
    zh_keys = extract_all_keys(ZH_TABLES)
    en_keys = extract_all_keys(EN_TABLES)
    assert len(zh_keys) > 0, "ZH keys table should not be empty"
    assert len(en_keys) > 0, "EN keys table should not be empty"
    assert zh_keys == en_keys, "ZH and EN keys must be in 100% parity"
    print(f"Self-test OK. Key count: {len(zh_keys)}")


def main() -> int:
    parser = argparse.ArgumentParser(description="i18n Quality Guard")
    parser.add_argument(
        "--mode",
        choices=["report", "enforce"],
        default="report",
        help="report: display findings without non-zero exit; enforce: exit 1 on violations",
    )
    parser.add_argument("--self-test", action="store_true", help="Run self tests")
    args = parser.parse_args()

    if args.self_test:
        run_self_test()
        return 0

    parity_violations = check_key_parity()
    literal_violations = check_untranslated_literals()
    all_violations = parity_violations + literal_violations

    for v in all_violations:
        print(f"VIOLATION [{args.mode}]: {v}")

    print(
        f"i18n quality guard: parity_errors={len(parity_violations)} "
        f"literal_violations={len(literal_violations)} total={len(all_violations)}"
    )

    if args.mode == "enforce" and all_violations:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
