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
import importlib.util
import sys
import unittest
import unittest.mock

ROOT = pathlib.Path(__file__).resolve().parents[2]
UI = ROOT / "crates/infiltrator-slint/ui"
THEME = UI / "components/theme.slint"
TITLE_FILE = UI / "components/page_titles.slint"
TIER_FILE = UI / "components/viewport_tier.slint"
LADDER_FILE = UI / "components/viewport-ladder.slint"
GENERATOR = ROOT / "scripts/generate_viewport_ladder.py"
PAGE_STRUCTURE = ROOT / "crates/infiltrator-contract/assets/page-structure.json"

# A raw viewport comparison: the WINDOW's own width or height compared
# against a length literal. Only the theme may write one, and only inside
# the ladder. A scroll view's `visible-height` or a sheet's `height` is not
# the viewport, so the subject must be the window itself.
RAW_VIEWPORT = re.compile(
    r"(?<![\w.-])(?:root|self|parent)\.(?:width|height)\s*(?:>=|<=|>|<)\s*\d+(?:\.\d+)?\s*\*\s*LumenUiScale\.unit"
)
TIER_VARIANT = re.compile(r"^\s{4}([\w-]+),?\s*$", re.M)
THEME_TOKEN = re.compile(r"out property <[a-z]+> ([a-z0-9-]+):")
# Each call by which the sidebar publishes where its navigation stack ends.
PUBLISH_STACK_BOTTOM = re.compile(
    r"root\.publish-stack-bottom\(([^;]*)\);")
# Each assignment by which the sidebar publishes its own leading, and the
# property the rows are padded with. The two are one fact: a pane that states a
# leading it does not paint with would describe geometry it never drew.
PUBLISH_NAV_LEADING = re.compile(
    r"root\.nav-content-offset = ([^;]*);")
PADS_FROM_NAV_LEADING = "padding-left: root.nav-content-offset"
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


def nav_form_variants() -> list[str]:
    """The surface's spelling of the navigation forms the ladder exports."""
    return enum_variants("NavigationForm")


def exported_nav_forms() -> list[str]:
    """The exported `nav_form` ids in Slint spelling, in tier order.

    The ladder has one entry per tier, so a form shared by two tiers appears
    once here. The order is the order the tiers introduce each form, which is
    the order the enum declares them.
    """
    seen: list[str] = []
    for entry in exported_tiers():
        variant = str(entry["nav_form"]).replace("_", "-")
        if variant not in seen:
            seen.append(variant)
    return seen


def exported_tiers() -> list[dict]:
    """The contract's exported ladder, the only source of these boundaries."""
    if not PAGE_STRUCTURE.is_file():
        return []
    return list(json.loads(PAGE_STRUCTURE.read_text(encoding="utf-8")).get("tiers", []))


def generator_module():
    """The ladder generator, loaded in place so the adapter is compared here."""
    spec = importlib.util.spec_from_file_location("generate_viewport_ladder", GENERATOR)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


def ladder_findings(ui: pathlib.Path) -> list[str]:
    """One ladder, exported once, generated once, read everywhere.

    b150 measured three owners of "which viewport tier is this?" carrying three
    boundary sets: the contract said 600/840/1200, this surface's theme said
    600/800/1120, and the judge said 700/1000. The same 1180x780 proxies frame
    then got four node columns on Slint and three on Iced, and the judge
    measured an 800..999 frame as if it had no navigation column at all. The
    contract exports the ladder, the generator publishes it into Slint, and
    these checks are what keep the two ends from drifting apart again.
    """
    found: list[str] = []
    tiers = exported_tiers()
    if not tiers:
        return ["page-structure.json carries no viewport ladder; "
                "rerun export_page_structure"]
    if not LADDER_FILE.is_file():
        return ["components/viewport-ladder.slint is missing; the exported "
                "ladder has no adapter"]

    adapter = LADDER_FILE.read_text(encoding="utf-8")
    adapter_code = mask_comments(adapter)

    # The adapter must be the generator's own output, not a hand edit.
    expected = generator_module().slint_ladder({
        "version": 7,
        "tiers": tiers,
        "dashboard_columns": {"four_column_min_px": 860},
    })
    if adapter != expected:
        found.append(
            "components/viewport-ladder.slint is not the generated adapter; "
            "run scripts/generate_viewport_ladder.py instead of editing it"
        )

    # One boundary per tier exit, and the widest tier owns no upper bound.
    for entry in tiers[:-1]:
        if f"out property <length> {entry['id']}-max:" not in adapter_code:
            found.append(
                f"components/viewport-ladder.slint does not publish {entry['id']}-max"
            )
    if f"out property <length> {tiers[-1]['id']}-max:" in adapter_code:
        found.append(
            f"components/viewport-ladder.slint gives the widest tier an upper bound"
        )
    for entry in tiers[:-1]:
        if f"ViewportTier.{entry['id']}" not in adapter_code:
            found.append(
                f"components/viewport-ladder.slint does not cover tier {entry['id']}"
            )

    # The theme chain reads the adapter and never restates a number.
    theme_code = mask_comments((ui / "components/theme.slint").read_text(encoding="utf-8"))
    binding = re.search(
        r"out property <ViewportTier> viewport-tier:([^;]*);", theme_code, re.S)
    if not binding:
        found.append("components/theme.slint: the viewport tier ladder is gone")
        return found
    chain = binding.group(1)
    if re.search(r"\d+(?:\.\d+)?\s*\*\s*LumenUiScale\.unit", chain):
        found.append(
            "components/theme.slint: the tier ladder writes its own boundary number; "
            "read the generated ViewportLadder bounds"
        )
    for entry in tiers[:-1]:
        if f"ViewportLadder.{entry['id']}-max" not in chain:
            found.append(
                f"components/theme.slint: the tier ladder does not read ViewportLadder.{entry['id']}-max"
            )
    if not chain.rstrip().endswith(f"ViewportTier.{tiers[-1]['id']}"):
        found.append(
            f"components/theme.slint: the tier ladder does not end in {tiers[-1]['id']}"
        )

    # The exported column and grid facts are read, not re-typed per page.
    surfaces = "\n".join(
        mask_comments(path.read_text(encoding="utf-8"))
        for path in sorted(ui.rglob("*.slint"))
        if path != LADDER_FILE
    )
    for function in re.findall(r"pure function (\w+)\(", adapter_code):
        if f"ViewportLadder.{function}(" not in surfaces:
            found.append(
                f"components/viewport-ladder.slint: {function}() is generated but never read"
            )
    if "ViewportLadder.nav_column(" not in theme_code:
        found.append(
            "components/theme.slint: the navigation column is no longer the exported width"
        )
    return found


def tier_vocabulary_findings() -> list[str]:
    """One tier, one name, across the contract and the surface.

    An id that only exists in one language is a fork waiting to be measured
    differently, so the Slint enum must spell the exported ladder's tiers.
    """
    exported = [entry["id"] for entry in exported_tiers()]
    declared = tier_variants()
    if exported != declared:
        return [
            "components/viewport_tier.slint: the tier names are "
            f"{declared} but the exported ladder says {exported}"
        ]
    forms = exported_nav_forms()
    declared_forms = nav_form_variants()
    if sorted(forms) != sorted(declared_forms):
        return [
            "components/viewport_tier.slint: the navigation forms are "
            f"{declared_forms} but the exported ladder says {forms}"
        ]
    return []


def navigation_mount_findings(ui: pathlib.Path) -> list[str]:
    """Navigation is mounted by the exported form, never by page density.

    SPACEUTIL-006 measured what the conflation cost: `compact-layout` answered
    "does this window have a labelled column", so the medium tier - a narrow
    page that owns an icon rail - mounted a phone's bottom bar instead and paid
    a 72u band across a 480-row window. The form is one typed fact out of the
    ladder, the sidebar mounts on it, and the dock band follows the bar rather
    than the density boolean.
    """
    found: list[str] = []
    app_code = mask_comments((ui / "lumen-app.slint").read_text(encoding="utf-8"))
    if ("out property <NavigationForm> navigation-form: "
            "ViewportLadder.shell_navigation(") not in app_code:
        found.append(
            "lumen-app.slint: the shell no longer publishes the exported "
            "navigation form"
        )
    body_code = mask_comments(
        (ui / "components/app_body.slint").read_text(encoding="utf-8"))
    mount = re.search(r"if ([^\n:]*): DesktopSideBar \{", body_code)
    if not mount:
        found.append("components/app_body.slint: the desktop sidebar lost its mount")
    elif "root.navigation-form != NavigationForm.bottom-nav" not in mount.group(1):
        found.append(
            "components/app_body.slint: the sidebar mounts on "
            f"{mount.group(1).strip()!r}; the exported navigation form owns "
            "that decision"
        )
    driver = ROOT / "crates/infiltrator-slint/src/mobile_dock_driver.rs"
    if driver.is_file():
        text = driver.read_text(encoding="utf-8")
        if "navigation: to_contract_form(window.get_navigation_form())" not in text:
            found.append(
                "mobile_dock_driver.rs: the dock band no longer reads the "
                "typed navigation form"
            )
        if "compact_layout:" in text:
            found.append(
                "mobile_dock_driver.rs: the dock band still reads compact-layout; "
                "a band belongs to a bar, not to a page density"
            )
    return found


def receipt_findings(ui: pathlib.Path) -> list[str]:
    """The chrome a frame painted travels with the frame, and nowhere else.

    SPACEUTIL-007 retired the last geometry the space-utilization judge kept
    for itself (page margin 30/16 and shell air 8/72, mirrored from the surface
    and pinned on both sides). Now the shell publishes the same numbers it
    bound its layout with, the host writes them into the frame's marker file,
    and the judge measures the box the frame describes. Two ways to fail this:
    the surface stops saying what it painted, or the judge starts remembering
    paddings again.
    """
    found: list[str] = []
    app_code = mask_comments((ui / "lumen-app.slint").read_text(encoding="utf-8"))
    rail_code = mask_comments((ui / "components/sidebar.slint").read_text(encoding="utf-8"))
    body_code = mask_comments((ui / "components/app_body.slint").read_text(encoding="utf-8"))
    for needle, message in [
        ("out property <ReceiptChrome> receipt-chrome:",
         "lumen-app.slint: the shell no longer publishes the chrome it paints"),
        ("nav-stack-bottom: app-body.nav-stack-bottom",
         "lumen-app.slint: the receipt no longer reports the measured nav-stack bottom"),
        ("nav-content-offset: app-body.nav-content-offset",
         "lumen-app.slint: the receipt no longer reports the leading the pane gives away"),
        ("page-margin: app-body.page-margin",
         "lumen-app.slint: the receipt no longer reports the page margin the body binds"),
        ("shell-bottom-air: app-body.page-content-padding-bottom",
         "lumen-app.slint: the receipt no longer reports the shell's bottom air"),
        ("dock-band: app-body.dock-band-height",
         "lumen-app.slint: the receipt no longer reports the reserved dock band"),
    ]:
        if needle not in app_code:
            found.append(message)
    if "nav-stack-bottom <=> root.nav-stack-bottom" not in body_code:
        found.append(
            "components/app_body.slint: the pane's measured stack end no longer "
            "reaches the shell's receipt"
        )
    if "nav-content-offset <=> root.nav-content-offset" not in body_code:
        found.append(
            "components/app_body.slint: the pane's own leading no longer reaches "
            "the shell's receipt"
        )
    # Every publish call must state the row's bottom edge in window pixels. A
    # call that publishes a pane-relative height, or a spacer's offset, is the
    # number b154 had to withdraw - so the rule reads all of them, not just one.
    publishes = PUBLISH_STACK_BOTTOM.findall(rail_code)
    if not publishes:
        found.append(
            "components/sidebar.slint: the sidebar no longer measures where its "
            "nav stack ends, so the judge would guess a fraction of the height again"
        )
    else:
        for argument in publishes:
            if "absolute-position.y + self.height" not in argument:
                found.append(
                    "components/sidebar.slint: a nav-stack publish does not state the "
                    f"last row's bottom edge in window pixels ({argument.strip()})"
                )
    judge = ROOT / "scripts/parity/space_utilization.py"
    if judge.is_file():
        text = judge.read_text(encoding="utf-8")
        if "CAPTURE_CHROME" not in text or "chrome_of(" not in text:
            found.append(
                "space_utilization.py: the judge no longer reads the chrome line "
                "the frame carries"
            )
        remembered = re.findall(
            r"^[A-Z][A-Z_]*(?:MARGIN|AIR|BAND|LEADING|OFFSET)_PX = \d+", text, re.M)
        if remembered:
            found.append(
                f"space_utilization.py: the judge keeps chrome geometry again "
                f"({remembered}); it belongs to the frame's receipt"
            )
        if re.search(r"height \* 0\.7", text):
            found.append(
                "space_utilization.py: the judge guesses the stack end from a "
                "fraction of the window height again; the frame's receipt states it"
            )
    # The pane's leading is published and painted from one property. A pane that
    # pads from the token directly would leave the receipt free to drift away
    # from the pixels it describes.
    leading_publishes = PUBLISH_NAV_LEADING.findall(rail_code)
    if not leading_publishes:
        found.append(
            "components/sidebar.slint: the sidebar no longer publishes the leading "
            "it gives away, so that axis is unmeasured again (SPACEUTIL-010)"
        )
    else:
        for argument in leading_publishes:
            if "labelled" not in argument or "rail-content-leading" not in argument:
                found.append(
                    "components/sidebar.slint: a nav leading publish neither reads "
                    f"the token nor asks whether the column labels its rows ({argument.strip()})"
                )
    if rail_code.count(PADS_FROM_NAV_LEADING) < 2:
        found.append(
            "components/sidebar.slint: a nav row no longer pads itself with the "
            "published leading, so the receipt states a number the pixels never used"
        )
    return found

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


def violations(ui: pathlib.Path, dock_checks: bool = True,
               ladder_checks: bool = True) -> list[str]:
    found: list[str] = []
    found += generated_column_findings(ui)
    if ladder_checks:
        found += ladder_findings(ui)
        found += tier_vocabulary_findings()
        found += navigation_mount_findings(ui)
        found += receipt_findings(ui)
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
        if "LumenTheme.overview-card-gap" not in page_code:
            found.append(f"pages/{name}: the card grid no longer reads the shared gap")
        if name == "home_desktop_grid.slint":
            # SPACEUTIL-008: the span grid reads the contract's exported share
            # lookup by page width. A private `compact-layout ? 2 : 4`, or the
            # wrapping flow's tier column count, would be a second answer to
            # the same question.
            if not re.search(
                r"ViewportLadder\.dashboard_grid_shares\(root\.width,\s*"
                r"LumenTheme\.viewport-tier\)",
                page_code,
            ):
                found.append(
                    "pages/home_desktop_grid.slint: the span grid no longer reads "
                    "the exported dashboard_grid_shares lookup"
                )
            if "LumenTheme.overview-card-columns" in page_code:
                found.append(
                    "pages/home_desktop_grid.slint: the span grid reads the phone "
                    "quick-info literal instead of its own typed contract fact"
                )
        elif "LumenTheme.overview-card-columns" not in page_code:
            found.append(
                "pages/home_compact_flow.slint: the phone quick-info grid lost "
                "its two-up width basis"
            )
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
        # The shell's page air is a CHROME fact, not a dock fact.
        #
        # The old rule made the bottom air follow the dock ("no band
        # reserved, so pay 72u"). That was right for the desktop chrome,
        # which keeps a page breath, and wrong for the compact chrome: its
        # bottom room belongs to the floating dock, which reserves its own
        # band in the column above, so a dockless compact page paid 72u of
        # dead ground under its last block - 72 of 480 rows on the 720x480
        # tier, 12.9% of the body, and the whole reason the framed list
        # pages' frame floated above a band of nothing.
        chrome_air = re.findall(
            r"property <length> page-content-padding-bottom:\s*root\.compact-layout\s*\?"
            r"\s*0\s*\*\s*LumenUiScale\.unit\s*:\s*72\s*\*\s*LumenUiScale\.unit",
            body_code,
        )
        if len(paddings) != 1:
            found.append(
                f"components/app_body.slint: the dock padding facts are {len(paddings)}, expected one column band"
            )
        if len(chrome_air) != 1:
            found.append(
                "components/app_body.slint: the shell's bottom air is no longer a chrome fact; "
                "the compact chrome pays no air and the desktop chrome keeps its 72u page breath"
            )
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
    """Planted defects, so a rule that stopped biting stops the build."""

    def _findings_with(self, overrides: dict[str, str]) -> list[str]:
        """Serve planted file bodies to the guard without touching the tree.

        The real reader is captured before the patch, or the passthrough
        would call the mock and recurse.
        """
        real = pathlib.Path.read_text

        def fake(self: pathlib.Path, *args, **kwargs):
            for name, body in overrides.items():
                if self.name == name:
                    return body
            return real(self, *args, **kwargs)

        with unittest.mock.patch.object(pathlib.Path, "read_text", autospec=True) as read:
            read.side_effect = fake
            return violations(UI, dock_checks=False)

    def test_a_private_boundary_in_the_chain_is_caught(self) -> None:
        """A theme that types its own width number is a second ladder."""
        theme = (UI / "components/theme.slint").read_text(encoding="utf-8")
        anchor = "root.viewport-width < ViewportLadder.medium-max ? ViewportTier.medium"
        assert anchor in theme, anchor
        planted = theme.replace(
            anchor, "root.viewport-width < 800 * LumenUiScale.unit ? ViewportTier.medium", 1
        )
        findings = self._findings_with({"theme.slint": planted})
        self.assertTrue(
            any("writes its own boundary number" in item for item in findings), findings)

    def test_a_hand_edited_adapter_is_caught(self) -> None:
        """The adapter is generated; editing it splits the ladder in two."""
        adapter = LADDER_FILE.read_text(encoding="utf-8")
        planted = adapter.replace("return 280;", "return 300;", 1)
        self.assertNotEqual(adapter, planted)
        findings = self._findings_with({"viewport-ladder.slint": planted})
        self.assertTrue(
            any("is not the generated adapter" in item for item in findings), findings)

    def test_a_generated_lookup_nobody_reads_is_caught(self) -> None:
        """A generated table with no consumer is a dead knob wearing the ladder."""
        proxies = (UI / "pages/page_proxies.slint").read_text(encoding="utf-8")
        anchor = "ViewportLadder.proxy_node_columns(LumenTheme.viewport-tier)"
        assert anchor in proxies, anchor
        planted = proxies.replace(anchor, "2", 1)
        findings = self._findings_with({"page_proxies.slint": planted})
        self.assertTrue(
            any("proxy_node_columns() is generated but never read" in item
                for item in findings), findings)

    def test_a_renamed_tier_in_the_surface_is_caught(self) -> None:
        """One tier, one name: the surface cannot keep a private vocabulary."""
        tier_file = TIER_FILE.read_text(encoding="utf-8")
        planted = tier_file.replace("    ultra,", "    wide,", 1)
        self.assertNotEqual(tier_file, planted)
        findings = self._findings_with({"viewport_tier.slint": planted})
        self.assertTrue(
            any("the exported ladder says" in item for item in findings), findings)

    def test_a_renamed_navigation_form_in_the_surface_is_caught(self) -> None:
        """The navigation vocabulary is exported too, not spelled per surface."""
        tier_file = TIER_FILE.read_text(encoding="utf-8")
        planted = tier_file.replace("    rail,", "    icon-rail,", 1)
        self.assertNotEqual(tier_file, planted)
        findings = self._findings_with({"viewport_tier.slint": planted})
        self.assertTrue(
            any("the navigation forms are" in item for item in findings), findings)

    def test_a_sidebar_mounted_by_page_density_is_caught(self) -> None:
        """Density says how tight the page is, not what chrome the shell mounts.

        This is the exact line that cost the medium tier a bottom band: it read
        `compact-layout`, which is true for a narrow page that owns a rail.
        """
        body = (UI / "components/app_body.slint").read_text(encoding="utf-8")
        anchor = "if root.navigation-form != NavigationForm.bottom-nav: DesktopSideBar {"
        assert anchor in body, anchor
        planted = body.replace(
            anchor, "if !root.compact-layout: DesktopSideBar {", 1)
        findings = self._findings_with({"app_body.slint": planted})
        self.assertTrue(
            any("the sidebar mounts on" in item for item in findings), findings)

    def test_a_dock_band_that_reads_density_is_caught(self) -> None:
        """A band belongs to a bar. The projection reads the form or it is wrong."""
        driver = ROOT / "crates/infiltrator-slint/src/mobile_dock_driver.rs"
        text = driver.read_text(encoding="utf-8")
        planted = text.replace(
            "navigation: to_contract_form(window.get_navigation_form()),",
            "compact_layout: window.get_compact_layout(),",
            1,
        )
        self.assertNotEqual(text, planted)
        findings = self._findings_with({"mobile_dock_driver.rs": planted})
        self.assertTrue(
            any("a band belongs to a bar" in item for item in findings), findings)

    def test_a_judge_that_remembers_chrome_paddings_is_caught(self) -> None:
        """The chrome a frame painted travels with the frame, or it is a fork.

        The judge used to keep 30/16 and 8/72 beside the surface's own
        bindings and pinned the two against each other. A number typed back
        into the judge is that second source, in the file that must not have one.
        """
        judge = ROOT / "scripts/parity/space_utilization.py"
        text = judge.read_text(encoding="utf-8")
        planted = text.replace(
            "CHROME_FILE = \"marker.log\"",
            "CHROME_FILE = \"marker.log\"\nLABELLED_SIDEBAR_MARGIN_PX = 30",
            1,
        )
        self.assertNotEqual(text, planted)
        findings = self._findings_with({"space_utilization.py": planted})
        self.assertTrue(
            any("keeps chrome geometry again" in item for item in findings), findings)

    def test_a_judge_that_remembered_the_stack_fraction_is_caught(self) -> None:
        """The stack end is the receipt's measurement, not a remembered ratio.

        The judge used to cut the rail at 70% of the window height; on a short
        window that line crossed real nav rows. A fraction typed back into the
        judge is the same fork the paddings were, so the guard bites on it.
        """
        judge = ROOT / "scripts/parity/space_utilization.py"
        text = judge.read_text(encoding="utf-8")
        planted = text.replace(
            "def rail_of(image: np.ndarray, chrome: dict) -> dict | None:",
            "def rail_of(image: np.ndarray, chrome: dict) -> dict | None:\n"
            "    stack_line = window_height * 0.70",
            1,
        )
        self.assertNotEqual(text, planted)
        findings = self._findings_with({"space_utilization.py": planted})
        self.assertTrue(
            any("fraction of the window height" in item for item in findings), findings)

    def test_a_sidebar_that_stops_measuring_its_stack_is_caught(self) -> None:
        """The pane owns the measurement; a receipt without it goes blind."""
        sidebar = (UI / "components/sidebar.slint").read_text(encoding="utf-8")
        planted = sidebar.replace(
            "root.publish-stack-bottom(self.absolute-position.y + self.height, index);",
            "root.publish-stack-bottom(self.height, index);",
            1,
        )
        self.assertNotEqual(sidebar, planted)
        findings = self._findings_with({"sidebar.slint": planted})
        self.assertTrue(
            any("does not state the last row's bottom edge" in item for item in findings),
            findings)

    def test_a_sidebar_that_publishes_no_stack_end_is_caught(self) -> None:
        # Dropping every publish leaves the receipt stating 0 for a frame that
        # owns a column, which the contract's own invariant refuses; the guard
        # must see that on the surface side too, not only in the pixels.
        sidebar = (UI / "components/sidebar.slint").read_text(encoding="utf-8")
        planted = sidebar.replace(
            "root.publish-stack-bottom(self.absolute-position.y + self.height, index);",
            "",
        )
        self.assertNotIn("publish-stack-bottom(self.absolute-position", planted)
        findings = self._findings_with({"sidebar.slint": planted})
        self.assertTrue(
            any("no longer measures where its" in item for item in findings), findings)

    def test_live_tree_is_clean(self) -> None:
        self.assertEqual(violations(UI), [])

    def test_a_sidebar_that_stops_publishing_its_leading_is_caught(self) -> None:
        """SPACEUTIL-010: the leading is a published fact, not a token the judge
        is expected to remember."""
        sidebar = (UI / "components/sidebar.slint").read_text(encoding="utf-8")
        planted = sidebar.replace(
            "        root.nav-content-offset = root.labelled\n"
            "            ? LumenTheme.rail-content-leading\n"
            "            : 0 * LumenUiScale.unit;",
            "",
            1,
        )
        self.assertNotEqual(sidebar, planted)
        findings = self._findings_with({"sidebar.slint": planted})
        self.assertTrue(
            any("no longer publishes the leading" in item for item in findings), findings)

    def test_a_sidebar_that_pads_from_the_token_is_caught(self) -> None:
        """The receipt and the pixels must be one property, not two readings."""
        sidebar = (UI / "components/sidebar.slint").read_text(encoding="utf-8")
        planted = sidebar.replace(
            "padding-left: root.nav-content-offset;",
            "padding-left: LumenTheme.rail-content-leading;",
        )
        self.assertNotEqual(sidebar, planted)
        findings = self._findings_with({"sidebar.slint": planted})
        self.assertTrue(
            any("no longer pads itself with the published leading" in item
                for item in findings), findings)

    def test_a_judge_that_remembers_the_leading_is_caught(self) -> None:
        # The leading budget travels with the ladder; a constant here would be a
        # second copy of the standard, exactly like the paddings b154 retired.
        judge = ROOT / "scripts/parity/space_utilization.py"
        text = judge.read_text(encoding="utf-8")
        planted = text.replace(
            "ROW_INK_FRACTION = 0.04",
            "NAV_LEADING_PX = 12\nROW_INK_FRACTION = 0.04",
            1,
        )
        self.assertNotEqual(text, planted)
        findings = self._findings_with({"space_utilization.py": planted})
        self.assertTrue(
            any("keeps chrome geometry again" in item for item in findings), findings)

    def test_a_span_grid_that_forgets_the_exported_lookup_is_caught(self) -> None:
        """SPACEUTIL-008: the 2/4 share grid reads a typed contract fact, not a
        private tier boolean."""
        grid = (UI / "pages" / "home_desktop_grid.slint").read_text(encoding="utf-8")
        planted = grid.replace(
            "ViewportLadder.dashboard_grid_shares(root.width, LumenTheme.viewport-tier)",
            "root.dense ? 2 : 4",
            1,
        )
        self.assertNotEqual(grid, planted)
        findings = self._findings_with({"home_desktop_grid.slint": planted})
        self.assertTrue(
            any("no longer reads the exported dashboard_grid_shares lookup" in item
                for item in findings), findings)

    def test_a_span_grid_that_reads_the_wrapping_column_count_is_caught(self) -> None:
        # The two typed facts must stay separate: a grid that reads the
        # phone quick-info literal is answering the wrong question.
        grid = (UI / "pages" / "home_desktop_grid.slint").read_text(encoding="utf-8")
        planted = grid.replace(
            "property <int> card-columns:\n"
            "        ViewportLadder.dashboard_grid_shares(root.width, LumenTheme.viewport-tier);",
            "property <int> card-columns: LumenTheme.overview-card-columns;",
            1,
        )
        self.assertNotEqual(grid, planted)
        findings = self._findings_with({"home_desktop_grid.slint": planted})
        self.assertTrue(
            any("reads the phone quick-info literal" in item for item in findings),
            findings)

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

    def test_dock_following_bottom_air_is_caught(self) -> None:
        """A bottom air that follows the dock is a second owner of the bottom.

        The shell's page air is a chrome fact: the compact chrome pays
        nothing and the desktop chrome keeps its 72u page breath. The old
        rule made the air follow the dock instead, and every dockless
        compact page then reserved 72u of ground that nothing owned - 72 of
        480 rows on the 720x480 tier, and the whole reason the framed list
        pages' frame floated above a band of nothing.
        """
        body = UI / "components/app_body.slint"
        original = body.read_text(encoding="utf-8")
        # The air is published (`out`) because the frame's receipt reports it;
        # the rule being planted on is unchanged.
        live = """        out property <length> page-content-padding-bottom: root.compact-layout
            ? 0 * LumenUiScale.unit
            : 72 * LumenUiScale.unit;"""
        assert live in original
        planted = original.replace(
            live,
            """        out property <length> page-content-padding-bottom: MobileDockBandProjection.reserves_band(root.mobile-dock-band-state)
            ? 0 * LumenUiScale.unit
            : 72 * LumenUiScale.unit;""",
            1,
        )
        real = pathlib.Path.read_text

        def fake(self, *args, **kwargs):
            if self == body:
                return planted
            return real(self, *args, **kwargs)

        with unittest.mock.patch.object(pathlib.Path, "read_text", autospec=True) as read_text:
            read_text.side_effect = fake
            findings = violations(UI)
        self.assertTrue(
            any("bottom air is no longer a chrome fact" in item for item in findings),
            findings,
        )

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
                        "page-content-padding-bottom: root.compact-layout\n            ? 0 * LumenUiScale.unit",
                        "page-content-padding-bottom: root.compact-layout\n            ? LumenTheme.space-mobile-card-gap",
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
                "the page pays the gap twice": "bottom air is no longer a chrome fact",
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
