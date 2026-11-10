#!/usr/bin/env python3
"""Hold the Bevy rail to navigation, and its rows to the registry.

The desktop reference measures a sidebar that carries destinations and
nothing else (`docs/reference/FLCLASH_DESKTOP_REFERENCE.md` section 2: the
upstream `NavigationSidebar` receives `destinations + selectedIndex +
expanded` and no state/rate/version property), and the space-utilization
standard turns that into the `chrome_band` 0% clause
(`docs/SPACE_UTILIZATION_STANDARD.md`). The Bevy surface violated both: its
rail mounted the brand block, the version line, the routing-mode pills, the
two master switches, the active-profile card, a 2x2 shortcut matrix and a
rate footer above the rows, and it enumerated `Route::ALL` as its own
navigation list. The mode pills and master switches were a second *live*
mount of controls the dashboard already publishes, wired to the same
commands.

This guard is a bounded lexical reader over the shell's own sources. It
checks the four facts that a re-edit is most likely to break, and it does not
pretend to see colour or layout -- those are the pixel receipts' business:

  A  the sidebar scene mounts the navigation column and nothing else
  B  no surface-side navigation list: the rows come from the registry
  C  the rail declares the current-destination reading, not a chosen member
  D  the retired sidebar blocks stay retired
  E  the compact dock is registry-driven too

Usage:
    python3 scripts/quality/bevy-sidebar-nav-guard.py [--mode report|enforce]
    python3 scripts/quality/bevy-sidebar-nav-guard.py --self-test

Exit status: 0 clean (or report mode), 1 findings in enforce mode, 2 unreadable.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
SRC = pathlib.Path("crates/infiltrator-bevy-ui/src")
SHELL_SCENE = SRC / "shell_scene.rs"
SHELL_NAV = SRC / "shell_nav.rs"

# D: symbols the navigation-only rail retired. They reappear only when a
# second publisher of a dashboard fact is being remounted in the column.
RETIRED = (
    "SidebarSystemProxyCard",
    "SidebarSystemProxyToggle",
    "SidebarTunCard",
    "SidebarTunToggle",
    "SidebarActiveProfileCard",
    "SidebarShortcutMatrix",
    "SidebarShortcutTile",
    "SidebarSpeedFooter",
    "SidebarFoot",
    "SidebarIdentityText",
    "SidebarRoutingIntentSegment",
    "SidebarFooterRow",
    "shell_scene_with_toggles",
    "sync_sidebar_toggle_visuals",
    "DensityToggle",
    "ThemeToggle",
    # The routing selector had two markers once the sidebar copy was deleted:
    # the dashboard card's `OverviewRoutingIntentSegmentPill` and the retired
    # sidebar pill's `OverviewRoutingIntentPill`. Only the mounted one may
    # exist — a second marker with no mount is how the shell kept routing
    # state in two places and the tests argued about which pills were live.
    "OverviewRoutingIntentPill",
    # The rail's second copy of the active row, computed from route ids rather
    # than the registry's destination. Replaced by `sync_sidebar_nav_active`
    # (bits) plus the widget layer's paint set (fill/ink/weight).
    "sync_sidebar_nav_visuals",
)

# The one block the rail may mount, written as the call it is.
SIDEBAR_MOUNT = "nav::nav_column_scene(palette)"


def read(relative: pathlib.Path) -> str:
    path = ROOT / relative
    if not path.is_file():
        raise OSError(f"{relative} is missing")
    return path.read_text(encoding="utf-8")


def block(text: str, opener: str) -> str:
    """The brace-balanced body that starts at `opener`'s first `{`."""
    start = text.index(opener)
    brace = text.index("{", start)
    depth = 0
    for index in range(brace, len(text)):
        if text[index] == "{":
            depth += 1
        elif text[index] == "}":
            depth -= 1
            if depth == 0:
                return text[brace : index + 1]
    raise ValueError(f"unbalanced braces after {opener!r}")


def bracketed(body: str, opener: str) -> str:
    """The bracket-balanced body of `opener` (`Children [`) inside `body`."""
    start = body.index(opener)
    box = body.index("[", start)
    depth = 0
    for index in range(box, len(body)):
        if body[index] == "[":
            depth += 1
        elif body[index] == "]":
            depth -= 1
            if depth == 0:
                return body[box : index + 1]
    raise ValueError(f"unbalanced brackets after {opener!r}")


def code_of(text: str) -> str:
    """The source with its comments removed.

    A doc comment may legitimately *name* the vocabulary a module no longer
    uses — that is the point of an adjudication note — so the scan reads code,
    never prose.
    """
    lines = []
    for line in text.splitlines():
        stripped = line.lstrip()
        if stripped.startswith("//"):
            continue
        lines.append(re.sub(r"//(?!\!).*$", "", line))
    return "\n".join(lines)


def sidebar_mounts(scene_text: str) -> list[str]:
    """Every `@{ ... }` the sidebar scene mounts, whitespace-normalised."""
    body = block(scene_text, "pub fn sidebar_scene")
    children = bracketed(body, "Children [")
    return [re.sub(r"\s+", " ", item).strip() for item in re.findall(r"@\{\s*([^}]+?)\s*\}", children)]


def findings(scene_text: str, nav_text: str, retired_sources: dict[str, str]) -> list[str]:
    problems: list[str] = []

    # A: the rail mounts navigation and nothing else.
    try:
        mounts = sidebar_mounts(scene_text)
    except (ValueError, OSError) as error:
        return [f"sidebar scene cannot be read: {error}"]
    if mounts != [SIDEBAR_MOUNT]:
        problems.append(
            f"sidebar_scene must mount exactly the navigation column "
            f"[{SIDEBAR_MOUNT!r}], found {mounts} (chrome_band 0%: the "
            "reference gives the sidebar destinations only)"
        )

    # B: no second navigation list on the surface.
    for name, text in (
        ("shell_scene.rs", code_of(scene_text)),
        ("shell_nav.rs", code_of(nav_text)),
    ):
        if "Route::ALL" in text:
            problems.append(
                f"{name} enumerates Route::ALL: the rows are the registry's "
                "`desktop_items`/`items(NavigationSurface::Dock)`, never a "
                "surface-side route list"
            )

    # C: the rail's row is the current destination, painted by the widget layer.
    if "NavRowKind::CurrentDestination" not in code_of(nav_text):
        problems.append(
            "shell_nav.rs no longer declares NavRowKind::CurrentDestination: "
            "a nav row is not a chosen member and must not wear the accent capsule"
        )
    if re.search(r"\bnav_fill_token\b", code_of(nav_text)):
        problems.append(
            "shell_nav.rs calls nav_fill_token: the widget layer's "
            "sync_nav_visuals is the only writer of a row's fill"
        )

    # D: the retired blocks stay retired across the crate's sources.
    for symbol, source in sorted(retired_sources.items()):
        problems.append(f"{source}: retired sidebar symbol {symbol} is back")

    # E: the dock reads the registry's dock surface.
    if "NavigationSurface::Dock" not in code_of(scene_text):
        problems.append(
            "shell_scene.rs no longer builds the bottom dock from "
            "NavigationItem::items(NavigationSurface::Dock)"
        )
    return problems


def retired_symbols_in(sources: dict[str, str]) -> list[tuple[str, str]]:
    hits: list[tuple[str, str]] = []
    for symbol in RETIRED:
        for relative, text in sorted(sources.items()):
            if re.search(rf"\b{re.escape(symbol)}\b", text):
                hits.append((symbol, str(relative)))
    return hits


def check() -> list[str]:
    sources = {
        path.relative_to(ROOT): path.read_text(encoding="utf-8")
        for path in (ROOT / SRC).rglob("*.rs")
    }
    return findings(
        sources[SHELL_SCENE],
        sources[SHELL_NAV],
        {
            str(symbol): str(source)
            for symbol, source in retired_symbols_in(
                {key: code_of(value) for key, value in sources.items()}
            )
        },
    )


class GuardSelfTest(unittest.TestCase):
    def setUp(self) -> None:
        self.scene = read(SHELL_SCENE)
        self.nav = read(SHELL_NAV)

    def test_live_sources_are_clean(self) -> None:
        self.assertEqual(check(), [])

    def test_a_second_block_in_the_rail_is_caught(self) -> None:
        planted = self.scene.replace(
            SIDEBAR_MOUNT,
            f"{SIDEBAR_MOUNT} }}\n                --\n                @{{ sidebar_speed_footer_scene(palette)",
        )
        found = findings(planted, self.nav, {})
        self.assertTrue(any("chrome_band" in item for item in found), found)

    def test_a_surface_side_route_list_is_caught(self) -> None:
        planted = self.nav.replace(
            "let items = desktop_items(SurfaceKind::BevyDesktop);",
            "let items = Route::ALL.iter().map(|_| unreachable!()).collect();",
        )
        found = findings(self.scene, planted, {})
        self.assertTrue(any("Route::ALL" in item for item in found), found)

    def test_the_chosen_member_reading_is_caught(self) -> None:
        planted = self.nav.replace("NavRowKind::CurrentDestination", "nav_fill_token(active)")
        found = findings(self.scene, planted, {})
        self.assertTrue(any("NavRowKind" in item for item in found), found)
        self.assertTrue(any("nav_fill_token" in item for item in found), found)

    def test_a_remounted_sidebar_card_is_caught(self) -> None:
        found = findings(self.scene, self.nav, {"crates/x.rs": "SidebarTunCard"})
        self.assertTrue(any("SidebarTunCard" in item for item in found), found)

    def test_a_literal_dock_is_caught(self) -> None:
        planted = self.scene.replace("NavigationSurface::Dock", "NavigationSurface::Desktop")
        found = findings(planted, self.nav, {})
        self.assertTrue(any("NavigationSurface::Dock" in item for item in found), found)

    def test_the_guard_is_part_of_the_structure_gate(self) -> None:
        # A guard nobody runs is decorative. Both legs must be inside the one
        # structure gate the CI invokes.
        gate = (ROOT / "scripts/quality/check-structure.sh").read_text(encoding="utf-8")
        self.assertIn("bevy-sidebar-nav-guard.py --self-test", gate)
        self.assertIn("bevy-sidebar-nav-guard.py --mode enforce", gate)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("report", "enforce"), default="enforce")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(GuardSelfTest)
        result = unittest.TextTestRunner(verbosity=1).run(suite)
        return 0 if result.wasSuccessful() else 1
    try:
        problems = check()
    except (OSError, ValueError) as error:
        print(f"bevy sidebar nav guard: {error}", file=sys.stderr)
        return 2
    for problem in problems:
        print(f"bevy sidebar nav guard: {problem}", file=sys.stderr)
    if problems and args.mode == "enforce":
        return 1
    print(f"bevy sidebar nav guard: rail mounts the nav column only, findings={len(problems)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
