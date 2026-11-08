#!/usr/bin/env python3
"""Enforce one shared-copy catalog across every UI surface."""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys
import xml.etree.ElementTree as ET

ROOT = pathlib.Path(__file__).resolve().parents[2]
REGISTRY = ROOT / "crates/infiltrator-contract/src/shared_copy.rs"
ZH_TABLE = ROOT / "crates/infiltrator-shared/src/locales_table_shared_copy_zh.rs"
EN_TABLE = ROOT / "crates/infiltrator-shared/src/locales_table_shared_copy_en.rs"
LOCALE_DIR = ROOT / "crates/infiltrator-shared/src"
CONCISENESS_BASELINE = ROOT / "scripts/quality/copy_conciseness_baseline.json"
ANDROID_VALUES = ROOT / "android/app/src/main/res"
SLINT_SOURCES = sorted((ROOT / "crates/infiltrator-slint/ui").rglob("*.slint"))
KEY_PATTERN = re.compile(r'^\s*"([a-z0-9_]+)"\s*=>', re.MULTILINE)
ENTRY_PATTERN = re.compile(r'^\s*"([a-z0-9_]+)"\s*=>\s*"((?:[^"\\]|\\.)*)"', re.MULTILINE)
ANDROID_STRING_PATTERN = re.compile(r'<string name="([^"]+)">([^<]*)</string>')
GLOSS_PATTERN = re.compile(r"[（(]\s*[A-Za-z][A-Za-z0-9 .,+_&/-]{2,}\s*[)）]")
LONG_COPY_LIMIT = 45
SHARED_KEY_PATTERN = re.compile(r'SharedCopyKey::([A-Za-z0-9_]+)\.key\(\)|\.tr\(\s*SharedCopyKey::')
SLINT_COPY_PATTERN = re.compile(r'@tr\("([^"]+)"\)')
ICED_SOURCES = sorted((ROOT / "crates/infiltrator-iced/src").rglob("*.rs"))
VISIBLE_COPY_CALL = re.compile(
    r'(?:text|badge|form_field_label|labelled)\s*\(\s*"([^"\\]*(?:\\.[^"\\]*)*)"'
)
VISIBLE_COPY_ALLOWLIST = {
    "Ctrl+↵",
    "Ctrl+S",
    "Esc",
    "ESC",
    "TLS",
    "DIRECT",
    "fake_ip_range",
    "mtu",
    "dns_hijack (comma/newline separated)",
    "geoip.metadb / geosite.dat / Country.mmdb",
    "MetaCubeX",
}


def table_keys(path: pathlib.Path) -> set[str]:
    return set(KEY_PATTERN.findall(path.read_text(encoding="utf-8")))


def zh_tables() -> list[pathlib.Path]:
    """Every Chinese product locale table, newest catalog included."""
    tables = sorted(LOCALE_DIR.glob("locales_table*.rs"))
    return [path for path in tables if not re.search(r"_en(?:_|$)", path.stem)]


def verbose_glosses() -> set[str]:
    """Locale keys whose Chinese copy repeats a Latin gloss.

    Product copy states one fact once, in the active locale: the English
    spelling lives in the en-US table, not inside the Chinese string.
    """
    offenders: set[str] = set()
    for path in zh_tables():
        relative = path.relative_to(ROOT).as_posix()
        entries = ENTRY_PATTERN.findall(path.read_text(encoding="utf-8"))
        offenders.update(
            f"{relative}|{key}" for key, value in entries if GLOSS_PATTERN.search(value)
        )
    for locale_dir in ("values-zh-rCN", "values-zh-rTW"):
        path = ANDROID_VALUES / locale_dir / "strings.xml"
        relative = path.relative_to(ROOT).as_posix()
        for name, value in ANDROID_STRING_PATTERN.findall(path.read_text(encoding="utf-8")):
            if GLOSS_PATTERN.search(value):
                offenders.add(f"{relative}|{name}")
    return offenders


def long_copy() -> set[str]:
    """Iced-rendered labels longer than the concise-copy budget."""
    rendered: set[str] = set()
    for path in ICED_SOURCES:
        source = path.read_text(encoding="utf-8")
        rendered.update(re.findall(r'\.tr\("([a-z0-9_]+)"\)', source))
    offenders: set[str] = set()
    for path in zh_tables():
        for key, value in ENTRY_PATTERN.findall(path.read_text(encoding="utf-8")):
            if key in rendered and len(value) > LONG_COPY_LIMIT:
                offenders.add(f"{path.relative_to(ROOT).as_posix()}|{key}")
    return offenders


def conciseness_failures() -> tuple[list[str], list[str]]:
    """Return (new word-piling, baseline entries already repaid)."""
    if CONCISENESS_BASELINE.exists():
        payload = json.loads(CONCISENESS_BASELINE.read_text(encoding="utf-8"))
        recorded = set(payload["entries"]) | set(payload.get("long_copy", []))
    else:
        recorded = set()
    current = verbose_glosses() | long_copy()
    violations = [
        f"verbose Chinese copy repeats a Latin gloss: {entry}"
        for entry in sorted(verbose_glosses() - recorded)
    ] + [
        f"over-long Iced label states more than one fact: {entry}"
        for entry in sorted(long_copy() - recorded)
    ]
    repaid = sorted(recorded - current)
    return violations, repaid


def write_conciseness_baseline() -> int:
    entries = sorted(verbose_glosses())
    longs = sorted(long_copy())
    payload = json.dumps(
        {"version": 2, "entries": entries, "long_copy": longs},
        ensure_ascii=False,
        indent=2,
    )
    CONCISENESS_BASELINE.write_text(payload + "\n", encoding="utf-8")
    return len(entries) + len(longs)


def iced_visible_copy_failures() -> list[str]:
    """Reject user-visible literals that never reach the shared catalog."""
    failures: list[str] = []
    for path in ICED_SOURCES:
        relative = path.relative_to(ROOT).as_posix()
        for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            stripped = line.lstrip()
            if stripped.startswith("//") or stripped.startswith("#["):
                continue
            for literal in VISIBLE_COPY_CALL.findall(line):
                value = literal.strip()
                if (
                    value not in VISIBLE_COPY_ALLOWLIST
                    and len(value) > 2
                    and re.search(r"[A-Za-z]{3}", value)
                    and "{" not in value
                ):
                    failures.append(f"{relative}:{number} hard-codes visible copy {value!r}")
    return failures


def registry_keys() -> set[str]:
    source = REGISTRY.read_text(encoding="utf-8")
    return set(re.findall(r'Self::\w+ => "([a-z0-9_]+)"', source))


def android_keys(path: pathlib.Path) -> set[str]:
    root = ET.parse(path).getroot()
    return {
        element.attrib["name"]
        for element in root.findall("string")
        if "name" in element.attrib
    }


def slint_source() -> str:
    return "\n".join(
        path.read_text(encoding="utf-8") for path in SLINT_SOURCES
    )


def check() -> list[str]:
    registry = registry_keys()
    zh = table_keys(ZH_TABLE)
    en = table_keys(EN_TABLE)
    violations: list[str] = []
    for locale, keys in (("zh-CN", zh), ("en-US", en)):
        for key in sorted(registry - keys):
            violations.append(f"{locale} shared-copy table missing key '{key}'")
        for key in sorted(keys - registry):
            violations.append(f"{locale} shared-copy table has unregistered key '{key}'")

    for resource_dir in ("values", "values-zh-rCN", "values-zh-rTW"):
        keys = android_keys(ANDROID_VALUES / resource_dir / "strings.xml")
        for key in sorted(registry - keys):
            violations.append(f"{resource_dir} strings.xml missing shared key '{key}'")

    po_entries = ROOT / "crates/infiltrator-shared/locales/product/translations"
    for locale_dir in ("zh-CN", "en"):
        catalog = (po_entries / locale_dir / "LC_MESSAGES/musicfrog.po").read_text(encoding="utf-8")
        for message in (
            "Ice",
            "Lurker",
            "Source",
            "Lumen",
            "图标中的青蛙乐手是 MusicFrog 内部标识；产品名只保留本名。",
            "AMOLED",
            "English",
            "布局密度",
        ):
            if f'msgid "{message}"' not in catalog:
                violations.append(f"{locale_dir} product catalog missing message '{message}'")

    for crate in ("infiltrator-iced", "infiltrator-bevy-ui", "infiltrator-slint"):
        source = "\n".join(
            path.read_text(encoding="utf-8")
            for path in (ROOT / "crates" / crate / "src").rglob("*.rs")
        )
        if not SHARED_KEY_PATTERN.search(source) and crate != "infiltrator-slint":
            violations.append(f"{crate} does not resolve SharedCopyKey")

    if 'SharedCopyKey::AppDisplayName' not in slint_source():
        # Slint resolves compiler-driven @tr IDs; assert the routed product copy instead.
        for message in ("Lumen", "AMOLED", "English"):
            if f'@tr("{message}")' not in slint_source():
                violations.append(f"Slint surface does not localize '{message}'")

    slint_about = (ROOT / "crates/infiltrator-slint/ui/pages/page_about.slint").read_text(encoding="utf-8")
    if '@tr("图标中的青蛙乐手是 MusicFrog 内部标识；产品名只保留本名。")' not in slint_about:
        violations.append("Slint About does not explain the internal frog icon")

    for path in (ROOT / "crates/infiltrator-contract/src/a11y.rs", ROOT / "crates/infiltrator-bevy-ui/src/a11y.rs"):
        if "label_zh" in path.read_text(encoding="utf-8"):
            violations.append(f"{path.relative_to(ROOT)} still owns a bare-Chinese label")
    violations.extend(conciseness_failures()[0])
    violations.extend(iced_visible_copy_failures())
    return violations


def main() -> int:
    parser = argparse.ArgumentParser(description="Shared UI copy quality guard")
    parser.add_argument("--mode", choices=["report", "enforce"], default="report")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument(
        "--write-conciseness-baseline",
        action="store_true",
        help="record the current word-piling debt so later runs only allow it to shrink",
    )
    args = parser.parse_args()
    if args.write_conciseness_baseline:
        print(f"conciseness baseline: {write_conciseness_baseline()} keys recorded")
        return 0
    if args.self_test:
        assert registry_keys() == table_keys(ZH_TABLE) == table_keys(EN_TABLE)
        assert not check()
        print("shared-copy guard self-test: PASS")
        return 0
    violations = check()
    for violation in violations:
        print(f"VIOLATION [{args.mode}]: {violation}")
    _, repaid = conciseness_failures()
    print(
        f"shared-copy guard: keys={len(registry_keys())} "
        f"verbose={len(verbose_glosses())} long={len(long_copy())} "
        f"repaid={len(repaid)} total={len(violations)}"
    )
    if repaid:
        print("Repaid debt — drop these from the baseline: " + ", ".join(repaid[:5]), file=sys.stderr)
    return 1 if args.mode == "enforce" and violations else 0


if __name__ == "__main__":
    sys.exit(main())
