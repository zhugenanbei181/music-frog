#!/usr/bin/env python3
"""One page, one name: the shell may not spell a page two ways.

The desktop rail, the command palette, the contract's search alias and the
Slint app bar all name the same pages. They used to carry two vocabularies
for three pages: the rail said 代理节点 / 配置管理 / 规则管理 while the app
bar, the palette and the pages' own summaries said 代理策略 / 配置订阅 /
分流规则. One screen then named one page two ways.

This guard pins the chain that keeps them together:

* the contract's canonical page vocabulary (`ShellPage::title_zh`, the bare
  Chinese the palette matches against) is the reference spelling;
* the shared catalog's navigation key (`ShellPage::sidebar_key`) and the
  palette's key (`ShellPage::title_key`) must spell that word in both
  locales;
* the product PO catalog must translate that word into the English word the
  rail paints;
* the Slint app bar's desktop dialect may only use those canonical words - a
  stale variant such as 代理节点 is a second vocabulary returning.

Usage:
    python3 scripts/quality/page-name-guard.py [--mode report|enforce]
    python3 scripts/quality/page-name-guard.py --self-test
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
CATALOGUE = ROOT / "crates/infiltrator-contract/src/command_catalogue.rs"
NAVIGATION = ROOT / "crates/infiltrator-contract/src/shell_navigation.rs"
LOCALE_DIR = ROOT / "crates/infiltrator-shared/src"
PO_DIR = ROOT / "crates/infiltrator-shared/locales/product/translations"
ZH_LIVE = ROOT / "crates/infiltrator-shared/src/locales_table.rs"
EN_LIVE = ROOT / "crates/infiltrator-shared/src/locales_table_en.rs"
SLINT_TITLES = ROOT / "crates/infiltrator-slint/ui/components/page_titles.slint"

ZH_PREFIX = "前往 "
EN_PREFIX = "Go to "
LOCALE_ENTRY = re.compile(r'"(?P<key>[a-z0-9_]+)"\s*=>\s*"(?P<value>(?:[^"\\]|\\.)*)"')
SHELL_PAGE_ARM = re.compile(r'Self::(?P<page>[A-Za-z]+)\s*=>\s*"(?P<value>(?:[^"\\]|\\.)*)"')
FUNCTION_BODY = re.compile(
    r"pub const fn (?P<name>[a-z_]+)\(self\)\s*->\s*&'static str\s*\{(?P<body>[^}]*\})"
)
DOCK_ARM = re.compile(r"Self::shared\(\s*ShellPage::(?P<page>[A-Za-z]+)")
DESKTOP_ARM = re.compile(r"Self::shared\(\s*ShellPage::(?P<page>[A-Za-z]+)")
PO_ID = re.compile(r'^msgid "(?P<id>[^"]*)"$')
PO_STR = re.compile(r'^msgstr "(?P<value>[^"]*)"$')
TOP_TITLE_ROUTE = re.compile(
    r'if \(page == "(?P<route>[A-Za-z]+)"\) \{ return @tr\("(?P<word>[^"]+)"\); \}'
)
TOP_TITLE_FALLBACK = re.compile(r'return @tr\("(?P<word>[^"]+)"\);')


def read(path: pathlib.Path) -> str:
    return path.read_text(encoding="utf-8")


def locale_table(source: str) -> dict[str, str]:
    return {
        match.group("key"): match.group("value")
        for match in LOCALE_ENTRY.finditer(source)
    }


def locale_tables(locale: str) -> dict[str, str]:
    """Every locale table of one language, merged the way the localizer reads them."""
    merged: dict[str, str] = {}
    for path in sorted(LOCALE_DIR.glob("locales_table*.rs")):
        english = bool(re.search(r"_en(?:_|$)", path.stem))
        if english != (locale == "en"):
            continue
        merged.update(locale_table(read(path)))
    return merged


def function_arms(source: str, name: str) -> dict[str, str]:
    """The `Self::Page => "value"` arms of one const fn in a contract file."""
    for match in FUNCTION_BODY.finditer(source):
        if match.group("name") != name:
            continue
        return {
            arm.group("page"): arm.group("value")
            for arm in SHELL_PAGE_ARM.finditer(match.group("body"))
        }
    return {}


def po_catalog(source: str) -> dict[str, str]:
    """The single-line msgid -> msgstr pairs of a product catalog."""
    entries: dict[str, str] = {}
    current: str | None = None
    for line in source.splitlines():
        identifier = PO_ID.match(line)
        if identifier:
            current = identifier.group("id")
            continue
        value = PO_STR.match(line)
        if value and current is not None:
            entries[current] = value.group("value")
            current = None
    return entries


def dock_pages(navigation: str) -> list[str]:
    """The shared pages the compact dock names."""
    start = navigation.index("pub const DOCK: [Self; 4]")
    end = navigation.index("];", start)
    return DOCK_ARM.findall(navigation[start:end])


def desktop_pages(navigation: str) -> list[str]:
    """The shared pages the desktop rail shows."""
    start = navigation.index("pub const DESKTOP: [Self; 8]")
    end = navigation.index("];", start)
    return DESKTOP_ARM.findall(navigation[start:end])


def top_title_words(titles: str) -> list[str]:
    """The words the Slint desktop app bar paints for its primary pages."""
    start = titles.index("public pure function top-title(")
    end = titles.index("\n    }", start)
    body = titles[start:end]
    words = [match.group("word") for match in TOP_TITLE_ROUTE.finditer(body)]
    branches = body.rfind("}")
    fallback = TOP_TITLE_FALLBACK.search(body, branches)
    if fallback:
        words.append(fallback.group("word"))
    return words


def page_title_words(titles: str) -> list[str]:
    """Every literal word the Slint surface paints as a page title."""
    start = titles.index("public pure function title(")
    end = titles.index("\n    }", start)
    body = titles[start:end]
    words = [match.group("word") for match in TOP_TITLE_ROUTE.finditer(body)]
    words.extend(match.group("word") for match in TOP_TITLE_FALLBACK.finditer(body))
    return words


def strip_prefix(value: str, prefix: str) -> str:
    return value[len(prefix) :] if value.startswith(prefix) else value


def findings(catalogue: str | None = None,
             navigation: str | None = None,
             zh_table: str | None = None,
             en_table: str | None = None,
             zh_catalog: str | None = None,
             en_catalog: str | None = None,
             titles: str | None = None) -> list[str]:
    """Every way one page could end up with two names."""
    catalogue = read(CATALOGUE) if catalogue is None else catalogue
    navigation = read(NAVIGATION) if navigation is None else navigation
    zh = locale_tables("zh-CN") if zh_table is None else locale_table(zh_table)
    en = locale_tables("en") if en_table is None else locale_table(en_table)
    if zh_catalog is None:
        zh_catalog = read(PO_DIR / "zh-CN" / "LC_MESSAGES" / "musicfrog.po")
    if en_catalog is None:
        en_catalog = read(PO_DIR / "en" / "LC_MESSAGES" / "musicfrog.po")
    titles = read(SLINT_TITLES) if titles is None else titles

    problems: list[str] = []
    sidebar_keys = function_arms(catalogue, "sidebar_key")
    palette_keys = function_arms(catalogue, "title_key")
    aliases = function_arms(catalogue, "title_zh")
    zh_po = po_catalog(zh_catalog)
    en_po = po_catalog(en_catalog)

    pages = sorted(set(sidebar_keys) & set(palette_keys) & set(aliases))
    if not pages:
        problems.append("the contract no longer resolves the page vocabulary")
        return problems

    canonical: dict[str, str] = {}
    for page in pages:
        word = strip_prefix(aliases[page], ZH_PREFIX)
        canonical[page] = word
        rail = zh.get(sidebar_keys[page], "")
        if rail != word:
            problems.append(
                f"the {page} page is named '{rail}' in the navigation catalog and "
                f"'{word}' in the contract vocabulary"
            )
        palette = strip_prefix(zh.get(palette_keys[page], ""), ZH_PREFIX)
        if palette != word:
            problems.append(
                f"the {page} page is named '{palette}' in the command palette and "
                f"'{word}' in the contract vocabulary"
            )

    for page in pages:
        word = canonical[page]
        english_palette = strip_prefix(en.get(palette_keys[page], ""), EN_PREFIX)
        english_rail = en.get(sidebar_keys[page], "")
        if english_palette != english_rail:
            problems.append(
                f"the {page} page is named '{english_rail}' in the English navigation "
                f"catalog and '{english_palette}' in the English command palette"
            )

    painted_pages = [page for page in pages if canonical[page] in page_title_words(titles)]
    for page in painted_pages:
        word = canonical[page]
        english_rail = en.get(sidebar_keys[page], "")
        if word not in zh_po:
            problems.append(f"the product catalog has no '{word}' message for the {page} page")
        elif en_po.get(word, "") != english_rail:
            problems.append(
                f"the {page} page is named '{english_rail}' in the English navigation "
                f"catalog and '{en_po.get(word, '')}' in the product catalog"
            )

    vocabulary = set(canonical.values())
    dock = dock_pages(navigation)
    dock_words = {canonical[page] for page in dock if page in canonical}
    painted = top_title_words(titles)
    if dock_words and set(painted) != dock_words:
        problems.append(
            f"the Slint app bar names {sorted(painted)} while the dock names "
            f"{sorted(dock_words)}"
        )
    stale = sorted(word for word in painted if word not in vocabulary)
    if stale:
        problems.append(f"the Slint app bar spells a page name outside the vocabulary: {stale}")
    return problems


class GuardSelfTest(unittest.TestCase):
    def test_live_tree_is_clean(self) -> None:
        self.assertEqual(findings(), [])

    def test_a_rail_that_renames_a_page_is_caught(self) -> None:
        planted = read(ZH_LIVE).replace(
            '"nav_proxies" => "代理策略".into(),',
            '"nav_proxies" => "代理节点".into(),',
            1,
        )
        found = findings(zh_table=planted)
        self.assertTrue(any("navigation catalog" in item for item in found), found)

    def test_a_palette_that_renames_a_page_is_caught(self) -> None:
        planted = read(ZH_LIVE).replace(
            '"cmd_nav_profiles" => "前往 配置订阅".into(),',
            '"cmd_nav_profiles" => "前往 配置管理".into(),',
            1,
        )
        found = findings(zh_table=planted)
        self.assertTrue(any("command palette" in item for item in found), found)

    def test_an_english_catalog_that_disagrees_is_caught(self) -> None:
        planted = read(EN_LIVE).replace(
            '"nav_proxies" => "Proxies".into(),',
            '"nav_proxies" => "Proxy Nodes".into(),',
            1,
        )
        found = findings(en_table=planted)
        self.assertTrue(any("English" in item for item in found), found)

    def test_a_stale_app_bar_word_is_caught(self) -> None:
        planted = read(SLINT_TITLES).replace(
            'if (page == "Proxy") { return @tr("代理策略"); }',
            'if (page == "Proxy") { return @tr("代理节点"); }',
            1,
        )
        found = findings(titles=planted)
        self.assertTrue(any("outside the vocabulary" in item for item in found), found)

    def test_an_app_bar_that_drops_a_page_is_caught(self) -> None:
        planted = read(SLINT_TITLES).replace(
            'if (page == "Config") { return @tr("配置订阅"); }',
            'if (page == "Config") { return @tr("配置管理"); }',
            1,
        )
        found = findings(titles=planted)
        self.assertTrue(any("outside the vocabulary" in item for item in found), found)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("report", "enforce"), default="report")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(GuardSelfTest)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1
    problems = findings()
    if problems:
        for problem in problems:
            print(f"VIOLATION [enforce]: {problem}")
        return 1 if args.mode == "enforce" else 0
    print("page name guard: findings=0")
    return 0


if __name__ == "__main__":
    sys.exit(main())
