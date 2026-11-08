#!/usr/bin/env python3
"""SLINT-009: one viewport ladder, one page name, no dead theme token.

Three findings from the published receipts, all of the same kind: the tree
held a second, private answer to a question that already had an owner.

* **Viewport.** Six width comparisons (480, 600, 640, 760, 800, 1120)
  answered four different questions - "is this a phone", "is this window
  tight", "how many node columns fit" and "does the title bar still have
  room for the traffic capsule". Two of them sat inside the desktop title
  bar, which only exists above the compact threshold, so they could never be
  false. The boundary now lives once, in `LumenTheme.viewport-tier`, and a
  page that measures the window itself is a second idea of the viewport.
* **Page identity.** The desktop app bar printed the PRODUCT name ("Lumen")
  over the dashboard while the rail called the same page "核心概览", so one
  screen showed the brand three times and its heading was the only label on
  screen that did not name a page. `PageTitles.page-name` owns the name now.
* **Dead tokens.** A theme token nobody reads is a second, silent decision:
  it looks like a live knob and drifts away from the values that are live.
* **Fact typography.** A dashboard fact value is one role decision. The exit
  IP used to be 18px mono on the desktop grid and 14px mono on the phone flow,
  the memory total 22px versus 15px, and the profile name sans 15/400 versus
  *mono* 13 - so sibling cards in one row put their value baselines 6-8px
  apart and each page had its own idea of how loud a number is. `LumenTheme
  .metric-value-*`, `.metric-fact-*` and `.profile-name-*` own the choice and
  both tiers read them.
* **Dock band.** The dock is a floating pill, but it never draws over the
  scrolling page: a 720x480 window once cut the 配置 card through its name
  line with the pill sitting on top of it, and a phone frame let card rows
  pass behind the pill. The band the dock owns is one derived theme token
  (`bottom-bar-band-height`), the scroll viewport subtracts exactly that, and
  "does the dock exist" is one property handed down by the window
  (`MobileDockBandState::Closed / Reserved`) that the reserved region, the
  page ground, the pill and toast avoidance all read. A placement enum, a
  pair of present/visible bools, a height comparison or a second copy of the
  dock geometry would each be a second answer to a question that already has
  one owner.
  The band also owns the air above the pill (`dock-top-air`, exactly one card
  gap): with the clip line on the pill's own top edge a half card was sliced
  0px from the border, which reads as a covering band and not as a floating
  control, and the page may not pay that gap a second time with its own
  bottom padding.

The ladder reserve (the pressed end of the published 8-step surface ladder)
is the one documented exemption: `slint-surface-ladder-guard.py` pins its
shape, so those two tiers are the scale's reserved end rather than a stale
fact.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys
import unittest
import unittest.mock

ROOT = pathlib.Path(__file__).resolve().parents[2]
UI = ROOT / "crates/infiltrator-slint/ui"
THEME = UI / "components/theme.slint"
TITLE_FILE = UI / "components/page_titles.slint"
TIER_FILE = UI / "components/viewport_tier.slint"

# A raw viewport comparison: the WINDOW's own width or height compared
# against a length literal. Only the theme may write one, and only inside
# the ladder. A scroll view's `visible-height` or a sheet's `height` is not
# the viewport, so the subject must be the window itself.
RAW_VIEWPORT = re.compile(
    r"(?<![\w.-])(?:root|self|parent)\.(?:width|height)\s*(?:>=|<=|>|<)\s*\d+(?:\.\d+)?\s*\*\s*LumenUiScale\.unit"
)
TIER_VARIANT = re.compile(r"^\s{4}(\w+),?\s*$", re.M)
THEME_TOKEN = re.compile(r"out property <[a-z]+> ([a-z0-9-]+):")
LADDER_RESERVE = {"zinc-6", "zinc-7"}

# Every dashboard fact value asks one shared role, in both tiers. The pairs are
# the measured divergence: the exit IP was 18px mono on desktop and 14px mono on
# the phone, the memory total 22px versus 15px, the profile name sans 15/400
# versus mono 13, so one row of cards drew its value baselines 6-8px apart.
FACT_SITES = {
    "root.dashboard-facts.lan-ip": "LumenTheme.metric-value-size",
    "root.dashboard-facts.memory-total": "LumenTheme.metric-value-size",
    "root.active-profile": "LumenTheme.profile-name-size",
}
HERO_FACT_COLUMNS = ("LayoutTokens.hero-fact-column-1", "LayoutTokens.hero-fact-column-2")
CONNECTION_COLUMNS = tuple(
    f"LayoutTokens.connection-column-{index}" for index in range(1, 8)
)
CONNECTION_WEIGHTS = (8, 16, 3, 5, 6, 6, 4)
HERO_FACT_WEIGHTS = (5, 7)
RAW_COLUMN_RE = re.compile(r"\bcolumn-unit\b|column-content-width\s*:\s*root\.width\s*/")
CONNECTION_TOKEN_RE = re.compile(
    r"out property <float> connection-column-(\d): (\d+) / 48;"
)
HERO_TOKEN_RE = re.compile(
    r"out property <float> hero-fact-column-(\d): (\d+) / 12;"
)


def generated_column_findings(ui: pathlib.Path) -> list[str]:
    """Column fractions must be the authority's generated tokens, not literals."""
    found: list[str] = []
    authority_path = (
        ui / "../../../crates/infiltrator-contract/assets/design-authority.json"
    )
    if not authority_path.is_file():
        authority_path = ROOT / "crates/infiltrator-contract/assets/design-authority.json"
    authority = json.loads(authority_path.read_text(encoding="utf-8"))
    if authority.get("connection_column_weights") != list(CONNECTION_WEIGHTS):
        found.append("design-authority.json changed the seven connection column weights")
    if authority.get("hero_fact_column_weights") != list(HERO_FACT_WEIGHTS):
        found.append("design-authority.json changed the two hero fact column weights")
    layout_path = ui / "components/layout-tokens.slint"
    if not layout_path.is_file():
        layout_path = UI / "components/layout-tokens.slint"
    layout = layout_path.read_text(encoding="utf-8")
    connection = {
        int(index): int(weight)
        for index, weight in CONNECTION_TOKEN_RE.findall(layout)
    }
    hero = {int(index): int(weight) for index, weight in HERO_TOKEN_RE.findall(layout)}
    if connection != dict(enumerate(CONNECTION_WEIGHTS, 1)):
        found.append("layout-tokens.slint does not export the 7 authority connection fractions")
    if hero != dict(enumerate(HERO_FACT_WEIGHTS, 1)):
        found.append("layout-tokens.slint does not export the 2 authority hero fact fractions")
    home_widgets_path = ui / "pages/home_widgets.slint"
    if not home_widgets_path.is_file():
        home_widgets_path = UI / "pages/home_widgets.slint"
    home_widgets = mask_comments(home_widgets_path.read_text(encoding="utf-8"))
    for column in CONNECTION_COLUMNS:
        if column not in home_widgets:
            found.append(f"pages/home_widgets.slint does not read {column}")
    if RAW_COLUMN_RE.search(home_widgets):
        found.append("pages/home_widgets.slint re-derives a private column unit")
    # The hero fact row has one owner component; the pages must not keep
    # their own copy of the column fractions (SLINT-010 owns the row itself).
    fact_row_path = ui / "pages/home_fact_row.slint"
    if fact_row_path.is_file():
        fact_row = mask_comments(fact_row_path.read_text(encoding="utf-8"))
        for index, weight in enumerate(HERO_FACT_WEIGHTS, 1):
            if f"horizontal-stretch: LayoutTokens.hero-fact-column-{index};" not in fact_row:
                found.append(f"pages/home_fact_row.slint does not use hero-fact-column-{index}")
            if f"{weight} / 12" in fact_row:
                found.append(f"pages/home_fact_row.slint repeats the hero fact literal {weight} / 12")
    else:
        found.append("pages/home_fact_row.slint is missing; the hero fact row lost its owner")
    return found


def tier_variants() -> list[str]:
    return enum_variants("ViewportTier")


def enum_variants(name: str) -> list[str]:
    text = TIER_FILE.read_text(encoding="utf-8")
    body = text.split(f"export enum {name} {{", 1)[-1].split("}", 1)[0]
    return [name for name in TIER_VARIANT.findall(body) if name]


def mask_comments(source: str) -> str:
    """Blank out Slint comments so prose cannot satisfy or trip a rule."""
    out = []
    index = 0
    length = len(source)
    while index < length:
        if source.startswith("//", index):
            end = source.find("\n", index)
            end = length if end < 0 else end
            out.append(" " * (end - index))
            index = end
        elif source.startswith("/*", index):
            end = source.find("*/", index + 2)
            end = length if end < 0 else end + 2
            out.append("".join("\n" if char == "\n" else " " for char in source[index:end]))
            index = end
        else:
            out.append(source[index])
            index += 1
    return "".join(out)


def violations(ui: pathlib.Path, dock_checks: bool = True) -> list[str]:
    found: list[str] = []
    found += generated_column_findings(ui)
    app_code = mask_comments((ui / "lumen-app.slint").read_text(encoding="utf-8"))
    typed_dock_owner = (
        "in property <MobileDockBandState> mobile-dock-band-state:" in app_code
        and "MobileDockBandProjection.reserves_band(root.mobile-dock-band-state)" in app_code
    )
    theme_text = (ui / "components/theme.slint").read_text(encoding="utf-8")

    # 1. The ladder is written once.
    if "viewport-tier:" not in theme_text or "viewport-width" not in theme_text:
        found.append("components/theme.slint: the viewport tier ladder is gone")
    for path in sorted(ui.rglob("*.slint")):
        if path.name == "theme.slint":
            continue
        text = path.read_text(encoding="utf-8")
        relative = path.relative_to(ROOT)
        for match in RAW_VIEWPORT.finditer(text):
            line = text[: match.start()].count("\n") + 1
            found.append(
                f"{relative}:{line}: raw viewport comparison; read LumenTheme.viewport-tier"
            )

    # 2. Every tier variant is consumed; a tier nobody reads is a dead stage.
    variants = tier_variants()
    if not variants:
        found.append("components/viewport_tier.slint: the tier ladder declares no variant")
    consumers = "\n".join(
        path.read_text(encoding="utf-8")
        for path in sorted(ui.rglob("*.slint"))
        if path.name != "viewport_tier.slint"
    )
    for variant in variants:
        if f"ViewportTier.{variant}" not in consumers:
            found.append(f"components/viewport_tier.slint: tier {variant} is declared but never read")
    # The dock band has one derived owner, and neither the pill nor any
    # consumer of it may keep a second copy of the dock geometry.
    if "bottom-bar-band-height" not in theme_text:
        found.append("components/theme.slint: the dock band is no longer derived from the dock tokens")
    if not typed_dock_owner:
        found.append("lumen-app.slint: the window has no typed mobile dock band owner")
    for retired in ("bottom-bar-present", "bottom-bar-visible", "hide-bottom-bar-on-scroll"):
        if retired in app_code:
            found.append(f"lumen-app.slint: retired dock flag {retired} came back")
    if dock_checks:
        for retired in ("dock-placement", "floating-dock-min-height", "viewport-height"):
            if retired in mask_comments(theme_text):
                found.append(f"components/theme.slint: the retired {retired} owner came back")
        if "DockPlacement" in mask_comments(TIER_FILE.read_text(encoding="utf-8")):
            found.append("components/viewport_tier.slint: a dock placement enum came back; the dock owns one band")
        if "DockPlacement" in mask_comments(consumers):
            found.append("a surface reads a dock placement; the dock band is one derived token")

    # 2b. One role decision per dashboard fact, read by both tiers.
    theme_code = mask_comments(theme_text)
    for token, role in (
        ("metric-value", "numericDense"),
        ("metric-fact", "numericSmall"),
    ):
        for part in ("size", "weight", "family"):
            if f"{token}-{part}: DesignType.{role}-{part};" not in theme_code:
                found.append(
                    f"components/theme.slint: {token}-{part} no longer resolves to the registered {role} role"
                )
    for part in ("size", "weight"):
        if f"profile-name-{part}: DesignType.body-{part};" not in theme_code:
            found.append(f"components/theme.slint: profile-name-{part} left the body role")
    # 2c. The overview card grid is one shared width basis. `horizontal-stretch`
    # only hands out the slack left over after each card has taken its preferred
    # width, so sibling cards in one row measured 187u/169u (phone) and
    # 199u/201u/216u/216u (desktop) and never lined up on their inner edges.
    for name in ("home_desktop_grid.slint", "home_compact_flow.slint"):
        page = (ui / "pages" / name).read_text(encoding="utf-8")
        page_code = mask_comments(page)
        lines = page_code.splitlines()
        for token in ("LumenTheme.overview-card-columns", "LumenTheme.overview-card-gap"):
            if token not in page_code:
                found.append(f"pages/{name}: the card grid no longer reads {token}")
        for index, line in enumerate(lines):
            if not line.rstrip().endswith("SurfaceCard {"):
                continue
            indent = len(line) - len(line.lstrip())
            depth, cursor = 1, index + 1
            while cursor < len(lines) and depth:
                depth += lines[cursor].count("{") - lines[cursor].count("}")
                if depth == 0:
                    break
                cursor += 1
            body = "\n".join(lines[index + 1:cursor])
            # Only the card's OWN statement counts: the layouts inside a card
            # legitimately use stretch to hand out the card's content width.
            own = re.compile(
                r"^ {" + str(indent + 4) + r"}horizontal-stretch: [12];\s*$", re.M
            )
            if own.search(body):
                found.append(
                    f"pages/{name}:{index + 1}: a card is sized by horizontal-stretch; "
                    "take a share of overview-card-columns instead"
                )
        for fact, token in FACT_SITES.items():
            hits = [index for index, line in enumerate(lines) if fact in line]
            if not hits:
                found.append(f"pages/{name}: the dashboard fact {fact} left the shared flow")
                continue
            for index in hits:
                window = "\n".join(lines[index:index + 7])
                if token not in window:
                    found.append(
                        f"pages/{name}: {fact} re-invented its own type role instead of {token}"
                    )
                if "LumenTheme.fact-value-line-height" not in window:
                    found.append(
                        f"pages/{name}: {fact} left the shared value line box"
                    )
                if "vertical-alignment: bottom" not in window:
                    found.append(
                        f"pages/{name}: {fact} no longer hugs the bottom of the shared line box"
                    )
        for label in ("运行时长", "组织"):
            if f'@tr("{label}")' in page_code:
                found.append(
                    f"pages/{name}: hero fact label {label} left the shared row owner"
                )
        if "hero-organization-max-width" in page_code:
            found.append(f"pages/{name}: organization field re-invented a private width budget")
    fact_row_path = ui / "pages/home_fact_row.slint"
    if fact_row_path.is_file():
        fact_row_code = mask_comments(fact_row_path.read_text(encoding="utf-8"))
        for column in HERO_FACT_COLUMNS:
            if column not in fact_row_code:
                found.append(f"pages/home_fact_row.slint: hero fact grid does not read {column}")
        for label in ("运行时长", "组织"):
            if f'@tr("{label}")' not in fact_row_code:
                found.append(f"pages/home_fact_row.slint: hero fact label {label} left the row owner")
    else:
        found.append("pages/home_fact_row.slint is missing; the hero fact row lost its owner")

    if dock_checks:
        # 3. The page name has one owner, and it is a page.
        titles = mask_comments((ui / "components/page_titles.slint").read_text(encoding="utf-8"))
        if "page-name" not in titles:
            found.append("components/page_titles.slint: the page-name owner is gone")
        if '"Lumen"' in titles:
            found.append("components/page_titles.slint: a page title is the product name")
    body = (ui / "components/app_body.slint").read_text(encoding="utf-8")
    body_code = mask_comments(body)
    chrome_code = mask_comments((ui / "components/app_chrome.slint").read_text(encoding="utf-8"))

    if dock_checks:
        if "PageTitles.page-name(" not in body:
            found.append("components/app_body.slint: the app bar does not read the page-name owner")
        if "MobileDockBandProjection.reserves_band(root.mobile-dock-band-state)" not in body_code or "LumenTheme.bottom-bar-band-height" not in body_code:
            found.append("components/app_body.slint: the scroll viewport no longer subtracts the dock band")
        # The band is subtracted once, on the column that holds the header and
        # the page viewport together. An absolute height on the scroll view is
        # the old second copy, and it silently pushed the viewport below the
        # header on the compact 工具 page until rows were cut behind the pill
        # again.
        if "root.dock-band-height" not in body_code:
            found.append("components/app_body.slint: the page column no longer reserves the dock band")
        if "parent.height - root.dock-band-height" in body_code:
            found.append("components/app_body.slint: the scroll view keeps its own copy of the dock band instead of stretching")
        if "bottom-bar-height" in body_code or "bottom-bar-inset" in body_code:
            found.append("components/app_body.slint: a second copy of the dock geometry")
        if "LumenTheme.bottom-bar-band-height" not in chrome_code:
            found.append("components/app_chrome.slint: the dock strip no longer owns the shared band")
        if "LumenTheme.bottom-bar-band-height + LumenTheme.dock-toast-clearance" not in chrome_code:
            found.append("components/app_chrome.slint: the toast does not clear the dock band")
        # The band owns the air above the pill, and it is the only gap between a
        # clipped row and the floating control. Neither the pill nor the page
        # may answer that distance with a second number.
        if "out property <length> dock-top-air: root.space-mobile-card-gap;" not in theme_code:
            found.append("components/theme.slint: the dock no longer publishes the air above the pill as one card gap")
        if ("out property <length> bottom-bar-band-height: root.dock-top-air + "
            "root.bottom-bar-height + root.bottom-bar-inset;") not in theme_code:
            found.append("components/theme.slint: the dock band no longer includes the air above the pill")
        if "y: LumenTheme.dock-top-air;" not in chrome_code:
            found.append("components/app_chrome.slint: the pill sits at the top of its band again, so it cuts content at its own edge")
        paddings = re.findall(r"padding-bottom:\s*root\.dock-band-height\s*;", body_code)
        zero_pads = re.findall(
            r"page-content-padding-bottom:\s*MobileDockBandProjection\.reserves_band"
            r"\(root\.mobile-dock-band-state\)\s*\?\s*0\s*\*\s*LumenUiScale\.unit",
            body_code,
        )
        if len(paddings) != 1:
            found.append(
                f"components/app_body.slint: the dock padding facts are {len(paddings)}, expected one column band"
            )
        if len(zero_pads) != 1:
            found.append("components/app_body.slint: the list pad stopped reading the typed dock state")
        for path in sorted(ui.rglob("*.slint")):
            if path.name not in ("theme.slint", "app_chrome.slint"):
                if "dock-top-air" in mask_comments(path.read_text(encoding="utf-8")):
                    found.append(f"{path.relative_to(ROOT)}: re-derives the dock's top air instead of reading the band")
        if not typed_dock_owner:
            found.append("lumen-app.slint: the window no longer owns the typed dock-band state")
        if ("property <length> page-content-padding-top: root.compact-layout" not in body_code
                or "padding-top: root.page-content-padding-top;" not in body_code):
            found.append("components/app_body.slint: the compact padding-top fork returned")
        rail = (ui / "components/sidebar.slint").read_text(encoding="utf-8")
        if "PageTitles.page-name(" not in rail:
            found.append("components/sidebar.slint: the rail does not read the page-name owner")

    # 4. No dead theme token.
    everywhere = "\n".join(path.read_text(encoding="utf-8") for path in sorted(ui.rglob("*.slint")))
    for token in THEME_TOKEN.findall(theme_text):
        if token in LADDER_RESERVE:
            continue
        if f"LumenTheme.{token}" not in everywhere and f"root.{token}" not in everywhere:
            found.append(f"components/theme.slint: token {token} is declared but never read")
    return found


class GuardSelfTest(unittest.TestCase):
    def test_live_tree_is_clean(self) -> None:
        self.assertEqual(violations(UI), [])

    def test_parallel_dock_wip_does_not_hide_typography_and_columns(self) -> None:
        findings = violations(UI, dock_checks=False)
        self.assertEqual(
            [
                item
                for item in findings
                if "pages/" in item
                or "layout-tokens.slint" in item
                or "design-authority.json" in item
            ],
            [],
        )

    def test_column_authority_is_closed(self) -> None:
        self.assertEqual(generated_column_findings(UI), [])

    def test_raw_viewport_comparison_is_caught(self) -> None:
        class Single:
            """A one-file tree, so the planted comparison is the only input."""

            def __truediv__(self, other: str):
                return UI / other

            def rglob(self, _pattern: str):
                return [UI / "pages/page_home.slint"]

        original = (UI / "pages/page_home.slint").read_text(encoding="utf-8")
        anchor = "LumenTheme.viewport-width >= LumenTheme.dashboard-grid-min-width"
        assert anchor in original
        planted = original.replace(
            anchor,
            "root.width >= 640 * LumenUiScale.unit",
            1,
        )
        real = pathlib.Path.read_text
        with unittest.mock.patch.object(pathlib.Path, "read_text", autospec=True) as read_text:
            def fake_read_text(path: pathlib.Path, *args, **kwargs):
                if path.name == "page_home.slint":
                    return planted
                return real(path, *args, **kwargs)

            read_text.side_effect = fake_read_text
            findings = violations(Single(), dock_checks=False)
        self.assertTrue(any("page_home.slint" in item for item in findings))

    def test_split_fact_typography_is_caught(self) -> None:
        """A phone fact value that re-invents its own size is a second owner."""
        page = UI / "pages/home_compact_flow.slint"
        original = page.read_text(encoding="utf-8")
        live = """text: root.dashboard-facts.lan-ip;
                font-family: LumenTheme.metric-value-family;
                color: LumenTheme.text-primary;
                font-size: LumenTheme.metric-value-size;"""
        assert live in original
        planted = original.replace(
            live,
            live.replace(
                "font-size: LumenTheme.metric-value-size;",
                "font-size: DesignType.label-size;",
            ),
            1,
        )
        real = pathlib.Path.read_text

        def fake(self, *args, **kwargs):
            if self == page:
                return planted
            return real(self, *args, **kwargs)

        with unittest.mock.patch.object(pathlib.Path, "read_text", autospec=True) as read_text:
            read_text.side_effect = fake
            findings = violations(UI, dock_checks=False)
        self.assertTrue(
            any("re-invented its own type role" in item for item in findings)
            or any("left the shared flow" in item for item in findings),
            findings,
        )

    def test_stretched_card_is_caught(self) -> None:
        """A card that lets the layout hand out slack is the old ragged grid."""
        page = UI / "pages/home_compact_flow.slint"
        original = page.read_text(encoding="utf-8")
        live = """        // LAN IP
        SurfaceCard {
            width: root.card-column-width;"""
        assert live in original
        planted = original.replace(
            live,
            live.replace(
                "width: root.card-column-width;", "horizontal-stretch: 1;", 1
            ),
            1,
        )
        real = pathlib.Path.read_text

        def fake(self, *args, **kwargs):
            if self == page:
                return planted
            return real(self, *args, **kwargs)

        with unittest.mock.patch.object(pathlib.Path, "read_text", autospec=True) as read_text:
            read_text.side_effect = fake
            findings = violations(UI, dock_checks=False)
        self.assertTrue(
            any("a card is sized by horizontal-stretch" in item for item in findings),
            findings,
        )

    def test_value_outside_the_shared_line_box_is_caught(self) -> None:
        """A value that keeps its own line box puts sibling metas on two baselines."""
        page = UI / "pages/home_compact_flow.slint"
        original = page.read_text(encoding="utf-8")
        live = """                font-size: LumenTheme.metric-value-size;
                font-weight: LumenTheme.metric-value-weight;
                height: LumenTheme.fact-value-line-height;
                vertical-alignment: bottom;
                overflow: elide;"""
        assert live in original
        planted = original.replace(
            live,
            """                font-size: LumenTheme.metric-value-size;
                font-weight: LumenTheme.metric-value-weight;
                overflow: elide;""",
            1,
        )
        real = pathlib.Path.read_text

        def fake(self, *args, **kwargs):
            if self == page:
                return planted
            return real(self, *args, **kwargs)

        with unittest.mock.patch.object(pathlib.Path, "read_text", autospec=True) as read_text:
            read_text.side_effect = fake
            findings = violations(UI, dock_checks=False)
        self.assertTrue(
            any("left the shared value line box" in item for item in findings),
            findings,
        )

    def test_second_dock_geometry_copy_is_caught(self) -> None:
        body_file = UI / "components/app_body.slint"
        original = body_file.read_text(encoding="utf-8")
        live = "property <length> dock-band-height: MobileDockBandProjection.reserves_band(root.mobile-dock-band-state)"
        assert live in original
        planted = original.replace(
            live,
            "property <length> dock-band-height: true ? LumenTheme.bottom-bar-height"
            " + LumenTheme.bottom-bar-inset : 0 * LumenUiScale.unit;",
            1,
        )
        real = pathlib.Path.read_text

        def fake(self, *args, **kwargs):
            if self == body_file:
                return planted
            return real(self, *args, **kwargs)

        with unittest.mock.patch.object(pathlib.Path, "read_text", autospec=True) as read_text:
            read_text.side_effect = fake
            findings = violations(UI)
        self.assertTrue(
            any("second copy of the dock geometry" in item for item in findings),
            findings,
        )

    def test_private_hero_fact_width_is_caught(self) -> None:
        """A hero fact grid that drops shared absolute columns is a fork."""
        owner = UI / "pages/home_fact_row.slint"
        original = owner.read_text(encoding="utf-8")
        live = "horizontal-stretch: LayoutTokens.hero-fact-column-2;"
        assert live in original
        planted = original.replace(live, "horizontal-stretch: 7 / 12;", 1)
        real = pathlib.Path.read_text

        def fake(self, *args, **kwargs):
            if self == owner:
                return planted
            return real(self, *args, **kwargs)

        with unittest.mock.patch.object(pathlib.Path, "read_text", autospec=True) as read_text:
            read_text.side_effect = fake
            findings = violations(UI, dock_checks=False)
        self.assertTrue(
            any("hero fact grid does not read" in item for item in findings)
            or any("repeats the hero fact literal" in item for item in findings)
            or any("does not use hero-fact-column" in item for item in findings),
            findings,
        )

    def test_second_connection_column_unit_is_caught(self) -> None:
        page = UI / "pages/home_widgets.slint"
        original = page.read_text(encoding="utf-8")
        planted = original.replace(
            "property <length> column-1: root.column-content-width * LayoutTokens.connection-column-1;",
            "property <length> column-unit: root.width / 48;",
            1,
        )
        self.assertNotEqual(original, planted)
        real = pathlib.Path.read_text

        def fake(self, *args, **kwargs):
            if self == page:
                return planted
            return real(self, *args, **kwargs)

        with unittest.mock.patch.object(pathlib.Path, "read_text", autospec=True) as read_text:
            read_text.side_effect = fake
            findings = generated_column_findings(UI)
        self.assertTrue(any("private column unit" in item for item in findings), findings)

    def test_product_name_as_page_title_is_caught(self) -> None:
        titles = mask_comments(TITLE_FILE.read_text(encoding="utf-8"))
        self.assertNotIn('"Lumen"', titles)

    def test_pill_cutting_content_at_its_own_edge_is_caught(self) -> None:
        """A dock whose clip line is the pill's top edge is a covering band."""
        chrome_file = UI / "components/app_chrome.slint"
        body_file = UI / "components/app_body.slint"
        theme_file = UI / "components/theme.slint"
        real = pathlib.Path.read_text

        for label, planted in (
            (
                "the pill is back at the top of its band",
                {chrome_file: real(chrome_file).replace("y: LumenTheme.dock-top-air;", "y: 0;", 1)},
            ),
            (
                "the band forgot the air",
                {
                    theme_file: real(theme_file).replace(
                        "root.dock-top-air + root.bottom-bar-height + root.bottom-bar-inset",
                        "root.bottom-bar-height + root.bottom-bar-inset",
                        1,
                    )
                },
            ),
            (
                "the page pays the gap twice",
                {
                    body_file: real(body_file).replace(
                        "page-content-padding-bottom: MobileDockBandProjection.reserves_band(root.mobile-dock-band-state)\n            ? 0 * LumenUiScale.unit",
                        "page-content-padding-bottom: MobileDockBandProjection.reserves_band(root.mobile-dock-band-state)\n            ? LumenTheme.space-mobile-card-gap",
                        1,
                    )
                },
            ),
            (
                "the viewport takes a second copy of the band",
                {
                    body_file: real(body_file).replace(
                        "page-scroll := ScrollView {\n                    content-width: self.width;",
                        "page-scroll := ScrollView {\n                    content-width: self.width;\n"
                        "                    height: MobileDockBandProjection.reserves_band(root.mobile-dock-band-state)\n"
                        "                        ? max(0 * LumenUiScale.unit, parent.height - root.dock-band-height)\n"
                        "                        : parent.height;",
                        1,
                    )
                },
            ),
        ):
            expected = {
                "the pill is back at the top of its band": "the pill sits at the top of its band again",
                "the band forgot the air": "no longer includes the air above the pill",
                "the page pays the gap twice": "list pad stopped reading the typed dock state",
                "the viewport takes a second copy of the band": "keeps its own copy of the dock band",
            }[label]
            with self.subTest(label):

                def fake(self, *args, **kwargs):
                    override = planted.get(self)
                    if override is not None:
                        return override
                    return real(self, *args, **kwargs)

                with unittest.mock.patch.object(pathlib.Path, "read_text", autospec=True) as read_text:
                    read_text.side_effect = fake
                    findings = violations(UI)
                self.assertTrue(
                    any(expected in item for item in findings),
                    f"{label}: expected {expected!r} in {findings}",
                )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=["report", "enforce"], default="report")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(GuardSelfTest)
        return 0 if unittest.TextTestRunner(verbosity=0).run(suite).wasSuccessful() else 1
    found = violations(UI)
    for item in found:
        print(f"SLINT-009 [{args.mode}]: {item}")
    print(f"slint viewport tier guard: findings={len(found)}")
    return 1 if found and args.mode == "enforce" else 0


if __name__ == "__main__":
    sys.exit(main())
