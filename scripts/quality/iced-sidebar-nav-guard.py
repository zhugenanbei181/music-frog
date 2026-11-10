#!/usr/bin/env python3
"""Hold the Iced rail to navigation, and its rows to the shared registry.

The desktop reference measures a sidebar that carries destinations and nothing
else (`docs/reference/FLCLASH_DESKTOP_REFERENCE.md` section 2), and the
space-utilization standard turns that into the `chrome_band` 0% clause
(`docs/SPACE_UTILIZATION_STANDARD.md`). The Iced pane broke both halves at
once: it declared its own primary/secondary route tables, it matched routes
onto glyphs twice (once per morphology), and it mounted the brand tile with the
package version, the routing-intent segmented control, the system-proxy/TUN
switch cards, the active-profile card with its usage bar, a shortcut matrix
with count badges and a live speed footer with a sparkline above the rows.
Every one of those facts already had an owner (the overview cards, the window
chrome strip, the quota card, the About page), so the rail was a second truth
source for five facts.

This is a bounded lexical reader over this surface's own sources. It checks the
facts a re-edit is most likely to break and does not pretend to see layout -
the pixel receipts own that, and the space-utilization judge will own it for
Iced once the surface publishes its chrome receipt:

  A  the rows come from the shared typed registry, not a local table
  B  every form of the pane reads that one registry (labelled, rail, dock)
  C  no route-to-glyph table anywhere in the crate
  D  the retired non-navigation blocks stay retired
  E  the shell's own facts are not remounted in the column

Usage:
    python3 scripts/quality/iced-sidebar-nav-guard.py [--mode report|enforce]
    python3 scripts/quality/iced-sidebar-nav-guard.py --self-test

Exit status: 0 clean (or report mode), 1 findings in enforce mode, 2 unreadable.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
SRC = pathlib.Path("crates/infiltrator-iced/src")
SIDEBAR = SRC / "view/sidebar.rs"
COMPONENTS = SRC / "view/components.rs"

# D: symbols the navigation-only column retired. They reappear only when a
# second publisher of a dashboard fact is being remounted in the column.
RETIRED = (
    "common_routing_intents",
    "routing_intent_control",
    "toggle_card",
    "short_label",
    "profile_card",
    "shortcut_matrix",
    "shortcut_tile",
    "speed_footer",
    "speed_leg",
    "clickable_card",
    "NAV_PRIMARY",
)

# E: the shell facts whose single owner is elsewhere. A semantic node mounted
# in the column again means the pane published a second affordance for a fact
# the chrome strip or a page already owns.
FOREIGN_NODES = (
    "ShellA11yNode::GlobalStatusDot",
    "ShellA11yNode::TrafficReadout",
    "ShellA11yNode::SystemProxySwitch",
    "ShellA11yNode::TunSwitch",
    "ShellA11yNode::RoutingIntentSegment",
)

ROUTE_GLYPH_TABLE = re.compile(r"Route::\w+\s*=>\s*(?:svg_icons::)?Icon::\w+")
ROUTE_LITERAL_TABLE = re.compile(r"\[\s*\(\s*Route::\w+\s*,")


def masked(text: str) -> str:
    """Drop line comments, so a rule is never satisfied by prose."""
    return "\n".join(line.split("//", 1)[0] for line in text.splitlines())


def read(path: pathlib.Path) -> str:
    return masked(path.read_text(encoding="utf-8"))


def sidebar_findings(sidebar: str) -> list[str]:
    found: list[str] = []
    if "desktop_items(SurfaceKind::IcedDesktop)" not in sidebar:
        found.append(
            f"{SIDEBAR}: the column no longer reads `desktop_items(SurfaceKind::IcedDesktop)`; "
            "a local row list is a second navigation registry"
        )
    if "NavigationSurface::Dock" not in sidebar:
        found.append(
            f"{SIDEBAR}: the compact bar no longer reads `NavigationSurface::Dock`; "
            "the four-item dock is the registry's row set too"
        )
    if ROUTE_LITERAL_TABLE.search(sidebar):
        found.append(
            f"{SIDEBAR}: a literal route table is back in the column; the registry is the "
            "one answer to which destinations exist, in which order"
        )
    for symbol in RETIRED:
        if symbol in sidebar:
            found.append(
                f"{SIDEBAR}: the retired sidebar block `{symbol}` is mounted again; that fact's "
                "owner is the overview page, the chrome strip or the settings index"
            )
    for node in FOREIGN_NODES:
        if node in sidebar:
            found.append(
                f"{SIDEBAR}: `{node}` is mounted in the column again; the shell's own facts "
                "belong to the title bar and the dashboard, not to the rail"
            )
    return found


def crate_findings(sources: dict[pathlib.Path, str]) -> list[str]:
    found: list[str] = []
    for path, text in sorted(sources.items()):
        if ROUTE_GLYPH_TABLE.search(text):
            found.append(
                f"{path}: a route-to-glyph table is back; a row's semantic icon arrives from "
                "`shell_navigation` and the glyph from `icon_for_semantic`"
            )
    return found


def violations(sidebar: str, sources: dict[pathlib.Path, str]) -> list[str]:
    return sidebar_findings(sidebar) + crate_findings(sources)


def live_sources() -> dict[pathlib.Path, str]:
    sources: dict[pathlib.Path, str] = {}
    for path in sorted((ROOT / SRC).rglob("*.rs")):
        sources[path.relative_to(ROOT)] = read(path)
    return sources


def live_violations() -> list[str]:
    return violations(read(ROOT / SIDEBAR), live_sources())


class GuardSelfTest(unittest.TestCase):
    def setUp(self) -> None:
        self.sidebar = read(ROOT / SIDEBAR)
        self.sources = {COMPONENTS: read(ROOT / COMPONENTS)}

    def findings(self, sidebar: str | None = None, sources: dict[pathlib.Path, str] | None = None) -> list[str]:
        return violations(
            self.sidebar if sidebar is None else sidebar,
            self.sources if sources is None else sources,
        )

    def test_live_tree_is_clean(self) -> None:
        self.assertEqual(live_violations(), [])

    def test_a_local_row_list_is_caught(self) -> None:
        planted = self.sidebar.replace(
            "desktop_items(SurfaceKind::IcedDesktop)",
            "NavigationItem::DESKTOP.to_vec()",
            1,
        )
        self.assertNotEqual(planted, self.sidebar)
        self.assertTrue(any("second navigation registry" in item for item in self.findings(sidebar=planted)))

    def test_a_literal_route_table_is_caught(self) -> None:
        planted = self.sidebar + "\npub const NAV_PRIMARY: [(Route, &str); 1] = [(Route::Overview, \"nav_overview\")];\n"
        found = self.findings(sidebar=planted)
        self.assertTrue(
            any("literal route table" in item or "retired sidebar block" in item for item in found),
            found,
        )

    def test_a_dock_that_stops_reading_the_registry_is_caught(self) -> None:
        planted = self.sidebar.replace("NavigationSurface::Dock", "DOCK", 1)
        self.assertNotEqual(planted, self.sidebar)
        self.assertTrue(any("four-item dock" in item for item in self.findings(sidebar=planted)))

    def test_a_remounted_speed_footer_is_caught(self) -> None:
        planted = self.sidebar.replace(
            "fn sidebar_inner",
            "fn speed_footer<'a>(state: &AppState) -> Element<'a, Message> { Space::new().into() }\n\nfn sidebar_inner",
            1,
        )
        self.assertNotEqual(planted, self.sidebar)
        self.assertTrue(any("speed_footer" in item for item in self.findings(sidebar=planted)))

    def test_a_remounted_status_dot_is_caught(self) -> None:
        planted = self.sidebar.replace(
            "fn sidebar_inner",
            "fn header_dot() -> ShellA11yNode { ShellA11yNode::GlobalStatusDot }\n\nfn sidebar_inner",
            1,
        )
        found = self.findings(sidebar=planted)
        self.assertTrue(any("GlobalStatusDot" in item for item in found), found)

    def test_a_route_to_glyph_table_is_caught(self) -> None:
        planted = {
            COMPONENTS: self.sources[COMPONENTS]
            + "\nfn glyph(route: Route) -> Icon {\n    match route {\n        Route::Overview => Icon::LayoutGrid,\n        _ => Icon::Globe,\n    }\n}\n"
        }
        found = self.findings(sources=planted)
        self.assertTrue(any("route-to-glyph table" in item for item in found), found)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("enforce", "report"), default="enforce")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(GuardSelfTest)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1
    found = live_violations()
    for item in found:
        print(f"ICED-RAIL [{args.mode}]: {item}")
    print(f"iced sidebar nav guard: findings={len(found)}")
    return 1 if found and args.mode == "enforce" else 0


if __name__ == "__main__":
    sys.exit(main())
