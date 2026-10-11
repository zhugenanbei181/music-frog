#!/usr/bin/env python3
"""Check shared Rust catalogs and compiler-driven product PO localization."""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys
import tomllib
from rust_syntax import mask_comments, mask_test_items


ROOT = pathlib.Path(__file__).resolve().parents[2]
LOCALE_DIR = ROOT / "crates/infiltrator-shared/src"
LOCALE_TABLES = sorted(
    path.relative_to(ROOT)
    for path in LOCALE_DIR.glob("locales_table*.rs")
)
EN_TABLES = [path for path in LOCALE_TABLES if re.search(r"_en(?:_|$)", path.stem)]
ZH_TABLES = [path for path in LOCALE_TABLES if path not in EN_TABLES]
PRODUCT_CATALOGS = {
    "en-US": ROOT / "crates/infiltrator-shared/locales/product/translations/en/LC_MESSAGES/musicfrog.po",
    "zh-CN": ROOT / "crates/infiltrator-shared/locales/product/translations/zh-CN/LC_MESSAGES/musicfrog.po",
}


def ui_source_roots() -> list[pathlib.Path]:
    roots = []
    for manifest_path in sorted((ROOT / "crates").glob("*/Cargo.toml")):
        manifest = tomllib.loads(manifest_path.read_text(encoding="utf-8"))
        dependency_names = set()
        for section in ("dependencies", "dev-dependencies", "build-dependencies"):
            dependency_names.update(manifest.get(section, {}))
        for target in manifest.get("target", {}).values():
            for section in ("dependencies", "dev-dependencies", "build-dependencies"):
                dependency_names.update(target.get(section, {}))
        if dependency_names.intersection({"bevy", "iced", "slint"}):
            source = manifest_path.parent / "src"
            if source.is_dir():
                roots.append(source)
    return roots


VIEW_SCAN_DIRS = ui_source_roots()

TEMPLATE_PATTERN = re.compile(
    r'"([a-z0-9_]+)"\s*=>\s*(?:\{\s*)?"((?:[^"\\]|\\.)*)"'
)
PLACEHOLDER_PATTERN = re.compile(r"\{([^{}]+)\}")
KEY_PATTERN = re.compile(r'^\s*"([a-z0-9_]+)"\s*=>', re.MULTILINE)
TRANSLATION_REF_PATTERN = re.compile(
    r'(?:\.tr|Localized(?:Text|Label)::(?:plain|new))\(\s*"((?:[^"\\]|\\.)*)"'
)
INTERPOLATION_REF_PATTERN = re.compile(r'\blocalize\(\s*[^,\n]+,\s*"([a-z0-9_]+)"')
CJK_PATTERN = re.compile(r"[\u4e00-\u9fff]")
STRING_LITERAL_PATTERN = re.compile(r'"([^"\\]*(?:\\.[^"\\]*)*)"')
PO_FIELD_PATTERN = re.compile(r'^(msgid|msgstr)\s+("(?:[^"\\]|\\.)*")$')


def read(path: pathlib.Path) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def extract_keys(path: pathlib.Path) -> set[str]:
    return set(KEY_PATTERN.findall(read(path)))


def extract_all_keys(paths: list[pathlib.Path]) -> set[str]:
    keys: set[str] = set()
    for path in paths:
        keys.update(extract_keys(path))
    return keys


def extract_po_catalog(path: pathlib.Path) -> dict[str, str]:
    """Read the simple product PO entries shared with Rust and Slint."""
    entries: dict[str, str] = {}
    message_id: str | None = None
    translation: str | None = None
    active_field: str | None = None

    def flush() -> None:
        nonlocal message_id, translation, active_field
        if message_id and translation:
            entries[message_id] = translation
        message_id = None
        translation = None
        active_field = None

    for raw_line in path.read_text(encoding="utf-8").splitlines():
        line = raw_line.strip()
        if not line:
            flush()
            continue
        if line.startswith(("#", "msgctxt ", "msgid_plural ", "msgstr[")):
            if line.startswith(("msgid_plural ", "msgstr[")):
                active_field = None
            continue
        match = PO_FIELD_PATTERN.match(line)
        if match:
            if match.group(1) == "msgid" and (
                message_id is not None or translation is not None
            ):
                flush()
            value = json.loads(match.group(2))
            active_field = match.group(1)
            if active_field == "msgid":
                message_id = value
            else:
                translation = value
            continue
        if line.startswith('"') and active_field in ("msgid", "msgstr"):
            value = json.loads(line)
            if active_field == "msgid" and message_id is not None:
                message_id += value
            elif active_field == "msgstr" and translation is not None:
                translation += value
    flush()
    return entries


def product_catalog_keys() -> set[str]:
    return set().union(*(set(extract_po_catalog(path)) for path in PRODUCT_CATALOGS.values()))


def check_product_catalogs() -> list[str]:
    catalogs = {locale: extract_po_catalog(path) for locale, path in PRODUCT_CATALOGS.items()}
    english = catalogs["en-US"]
    chinese = catalogs["zh-CN"]
    violations: list[str] = []
    for key in sorted(set(english) - set(chinese)):
        violations.append(f"product zh-CN catalog missing message '{key}'")
    for key in sorted(set(chinese) - set(english)):
        violations.append(f"product en-US catalog missing message '{key}'")
    for locale, catalog in catalogs.items():
        for key, copy in catalog.items():
            if not copy.strip():
                violations.append(f"product {locale} catalog has empty translation for '{key}'")
            if locale == "en-US" and CJK_PATTERN.search(copy):
                violations.append(f"product en-US catalog has CJK text for '{key}'")
    for key in sorted(set(english) & set(chinese)):
        zh_parameters = set(PLACEHOLDER_PATTERN.findall(chinese[key]))
        en_parameters = set(PLACEHOLDER_PATTERN.findall(english[key]))
        if zh_parameters != en_parameters:
            violations.append(
                f"product message '{key}' has different interpolation parameters: "
                f"zh-CN={sorted(zh_parameters)} en-US={sorted(en_parameters)}"
            )
    return violations


def production_code(text: str) -> str:
    """Mask only test-gated items, preserving production following imports or test modules."""
    return mask_test_items(mask_comments(text))


def is_test_path(path: pathlib.Path) -> bool:
    return any(part == "tests" or part.endswith("_test_cases") for part in path.parts) or (
        path.stem.endswith(("_test", "_tests")) or path.stem.startswith("test_")
    )


def untranslated_cjk_literals(code: str) -> list[tuple[int, str]]:
    translated_spans = [
        match.span(1) for match in TRANSLATION_REF_PATTERN.finditer(code)
    ]
    result = []
    for match in STRING_LITERAL_PATTERN.finditer(code):
        inside_translation = any(
            start <= match.start(1) and match.end(1) <= end
            for start, end in translated_spans
        )
        if CJK_PATTERN.search(match.group(1)) and not inside_translation:
            result.append((code[:match.start()].count("\n") + 1, match.group(1)))
    return result


def code_lines(path: pathlib.Path):
    yield from enumerate(production_code(read(path)).splitlines(), start=1)


def check_key_parity() -> list[str]:
    zh_keys = extract_all_keys(ZH_TABLES)
    en_keys = extract_all_keys(EN_TABLES)
    violations: list[str] = []
    for key in sorted(zh_keys - en_keys):
        violations.append(f"en-US table missing key '{key}'")
    for key in sorted(en_keys - zh_keys):
        violations.append(f"zh-CN table missing key '{key}'")
    return violations


def extract_templates(paths: list[pathlib.Path]) -> dict[str, str]:
    templates: dict[str, str] = {}
    for path in paths:
        templates.update(TEMPLATE_PATTERN.findall(read(path)))
    return templates


def check_placeholder_parity() -> list[str]:
    chinese = extract_templates(ZH_TABLES)
    english = extract_templates(EN_TABLES)
    violations: list[str] = []
    for key in sorted(chinese.keys() & english.keys()):
        zh_parameters = set(PLACEHOLDER_PATTERN.findall(chinese[key]))
        en_parameters = set(PLACEHOLDER_PATTERN.findall(english[key]))
        if zh_parameters != en_parameters:
            violations.append(
                f"locale key '{key}' has different interpolation parameters: "
                f"zh-CN={sorted(zh_parameters)} en-US={sorted(en_parameters)}"
            )
    return violations


def check_duplicate_keys() -> list[str]:
    violations: list[str] = []
    for locale, paths in (("zh-CN", ZH_TABLES), ("en-US", EN_TABLES)):
        owners: dict[str, str] = {}
        for path in paths:
            source = read(path)
            for match in KEY_PATTERN.finditer(source):
                key = match.group(1)
                location = f"{path}:{source[:match.start()].count(chr(10)) + 1}"
                if key in owners:
                    violations.append(f"{locale} duplicate locale key '{key}': {owners[key]}, {location}")
                else:
                    owners[key] = location
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
            if "demo" in path.relative_to(ROOT).parts or is_test_path(path):
                continue
            code = production_code(read(path.relative_to(ROOT)))
            for line_no, literal in untranslated_cjk_literals(code):
                violations.append(
                    f"{relative}:{line_no}: hardcoded CJK string '{literal}'"
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
                for key in TRANSLATION_REF_PATTERN.findall(code) + INTERPOLATION_REF_PATTERN.findall(code):
                    references.setdefault(key, []).append(f"{relative}:{line_no}")

    violations: list[str] = []
    available_keys = all_keys | product_catalog_keys()
    for key in sorted(set(references) - available_keys):
        locations = ", ".join(references[key][:3])
        violations.append(f"{locations}: view references missing locale key '{key}'")
    return violations


def run_self_test() -> None:
    zh_keys = extract_all_keys(ZH_TABLES)
    en_keys = extract_all_keys(EN_TABLES)
    violations = (
        check_key_parity()
        + check_placeholder_parity()
        + check_duplicate_keys()
        + check_key_names()
        + check_english_copy()
        + check_product_catalogs()
        + check_untranslated_literals()
        + check_view_references(zh_keys | en_keys | product_catalog_keys())
    )
    if violations:
        raise AssertionError("\n".join(violations))
    print(f"i18n guard self-test passed; locale keys={len(zh_keys)}")


def main() -> int:
    parser = argparse.ArgumentParser(description="UI localization quality guard")
    parser.add_argument("--mode", choices=["report", "enforce"], default="report")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        run_self_test()
        return 0

    zh_keys = extract_all_keys(ZH_TABLES)
    en_keys = extract_all_keys(EN_TABLES)
    parity = check_key_parity()
    parameters = check_placeholder_parity()
    duplicates = check_duplicate_keys()
    names = check_key_names()
    english = check_english_copy()
    products = check_product_catalogs()
    literals = check_untranslated_literals()
    references = check_view_references(zh_keys | en_keys | product_catalog_keys())
    violations = parity + parameters + duplicates + names + english + products + literals + references

    for violation in violations:
        print(f"VIOLATION [{args.mode}]: {violation}")
    print(
        "i18n quality guard: "
        f"parity_errors={len(parity)} parameter_errors={len(parameters)} duplicate_errors={len(duplicates)} key_name_errors={len(names)} "
        f"english_errors={len(english)} product_catalog_errors={len(products)} "
        f"literal_errors={len(literals)} reference_errors={len(references)} total={len(violations)}"
    )
    return 1 if args.mode == "enforce" and violations else 0


if __name__ == "__main__":
    sys.exit(main())
