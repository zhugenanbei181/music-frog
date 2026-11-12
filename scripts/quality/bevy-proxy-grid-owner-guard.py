#!/usr/bin/env python3
"""Keep Bevy's Proxies grid on the shared FlClash page-width owner.

The tracked desktop reference uses `getProxiesColumns(max(width - 32, 0))`:
the page's usable width and a 250dp card floor answer how many node cards fit.
A tier table of 1/2/3/4 columns, a compact-view boolean, or a second
`ProxyGridGeometry` would put the answer back into this surface.

This bounded lexical guard reads the production chain that must stay intact:
contract rule -> page geometry -> scene marker -> full-mount and virtual
consumers -> the structure gate's two legs.

Usage:
    python3 scripts/quality/bevy-proxy-grid-owner-guard.py [--mode report|enforce]
    python3 scripts/quality/bevy-proxy-grid-owner-guard.py --self-test
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
GEOMETRY = ROOT / "crates/infiltrator-bevy-ui/src/pages/proxy_grid_geometry.rs"
PROXIES = ROOT / "crates/infiltrator-bevy-ui/src/pages/proxies.rs"
SCENE = ROOT / "crates/infiltrator-bevy-ui/src/pages/proxies/scene.rs"
VIRTUAL = ROOT / "crates/infiltrator-bevy-ui/src/pages/proxies_virtual.rs"
MOUNT = ROOT / "crates/infiltrator-bevy-ui/src/route/mount.rs"
SURFACE = ROOT / "crates/infiltrator-bevy-ui/src/surface_projection.rs"
PREFERENCES = ROOT / "crates/infiltrator-bevy-ui/src/pages/proxies_preferences.rs"
GROUP_CARD = ROOT / "crates/infiltrator-bevy-ui/src/pages/proxies_card/group_card.rs"
NODE_CARD = ROOT / "crates/infiltrator-bevy-ui/src/pages/proxies_card/node_card.rs"
RECONCILE = ROOT / "crates/infiltrator-bevy-ui/src/pages/proxies_reconcile.rs"
GATE = ROOT / "scripts/quality/check-structure.sh"

RETIRED = (
    "compact_view",
    "proxies_virtual_columns",
    "proxy_grid_columns",
    "PROXY_GRID_INSET_PX",
)
TIER_TABLE = re.compile(
    r"Breakpoint::(?:Compact|Medium|Expanded|Ultra)\s*=>\s*[1-4]"
)
GEOMETRY_OWNER = "proxy_grid_geometry"
SHARED_RULE = "proxy_node_columns"


def read(path: pathlib.Path) -> str:
    return path.read_text(encoding="utf-8")


def code(text: str) -> str:
    lines = []
    for line in text.splitlines():
        stripped = line.lstrip()
        if stripped.startswith("//"):
            continue
        lines.append(re.sub(r"//(?!\!).*$", "", line))
    return "\n".join(lines)


def findings(geometry: str, proxies: str, scene: str, virtual: str, mount: str,
             surface: str, preferences: str, group_card: str, node_card: str,
             reconcile: str, gate: str) -> list[str]:
    problems: list[str] = []
    sources = {
        "proxies.rs": proxies,
        "proxies/scene.rs": scene,
        "proxies_virtual.rs": virtual,
        "route/mount.rs": mount,
        "surface_projection.rs": surface,
        "proxies_preferences.rs": preferences,
        "proxies_card/group_card.rs": group_card,
        "proxies_card/node_card.rs": node_card,
        "proxies_reconcile.rs": reconcile,
    }
    for symbol in RETIRED:
        for name, text in sources.items():
            if re.search(rf"\b{re.escape(symbol)}\b", code(text)):
                problems.append(f"{name}: retired proxy grid symbol {symbol} is back")
    for name, text in sources.items():
        if TIER_TABLE.search(code(text)):
            problems.append(f"{name}: proxy grid uses a private Breakpoint 1/2/3/4 table")

    geometry_code = code(geometry)
    for required in (SHARED_RULE, "ProxyGridForm", "page_body_width_px", "OutboundLayout"):
        if required not in geometry_code:
            problems.append(f"proxy_grid_geometry.rs must consume shared owner {required}")
    if "ViewportTier::from_width" not in geometry_code:
        problems.append("proxy_grid_geometry.rs must resolve the shared viewport tier")

    for name, text in (
        ("proxies.rs", proxies),
        ("proxies_virtual.rs", virtual),
        ("proxies/scene.rs", scene),
    ):
        if f"{GEOMETRY_OWNER}(" not in code(text):
            problems.append(f"{name} must call the shared proxy_grid_geometry owner")
    if "let grid_container = ProxyGridContainer(geometry);" not in code(scene):
        problems.append("the Proxies scene must publish its typed ProxyGridContainer")
    if "ProxyGridContainer" not in geometry_code:
        problems.append("proxy_grid_geometry.rs must declare ProxyGridContainer")
    if "layout: OutboundLayout" not in code(proxies):
        problems.append("ProxiesProjection must carry the owner's typed OutboundLayout")
    if "layout: preferences.map_or" not in code(surface):
        problems.append("surface_projection.rs must publish the owner's typed layout")
    if "proxies_page(" not in code(mount) or "viewport," not in code(mount):
        problems.append("route/mount.rs must pass the responsive viewport to proxies_page")
    if "ResponsiveViewportSnapshot" not in code(scene):
        problems.append("proxies_page must accept a ResponsiveViewportSnapshot")
    if "percent(31)" in code(node_card):
        problems.append("node_card.rs must not hard-code the old three-column width")
    if "width: Val" not in code(node_card) or "width," not in code(node_card):
        problems.append("node_card.rs must consume the caller's shared column width")
    if "width: Val" not in code(group_card) or "width," not in code(group_card):
        problems.append("group_card.rs must pass the shared column width to node cards")
    if f"{GEOMETRY_OWNER}(" not in code(reconcile):
        problems.append("proxies_reconcile.rs must re-mount cards at the shared column width")

    for leg in (
        "bevy-proxy-grid-owner-guard.py --self-test",
        "bevy-proxy-grid-owner-guard.py --mode enforce",
    ):
        if leg not in gate:
            problems.append(f"structure gate is missing leg: {leg}")
    return problems


def check() -> list[str]:
    return findings(
        read(GEOMETRY), read(PROXIES), read(SCENE), read(VIRTUAL), read(MOUNT),
        read(SURFACE), read(PREFERENCES), read(GROUP_CARD), read(NODE_CARD),
        read(RECONCILE), read(GATE),
    )


class GuardSelfTest(unittest.TestCase):
    def setUp(self) -> None:
        self.inputs = (
            read(GEOMETRY), read(PROXIES), read(SCENE), read(VIRTUAL), read(MOUNT),
            read(SURFACE), read(PREFERENCES), read(GROUP_CARD), read(NODE_CARD),
            read(RECONCILE), read(GATE),
        )

    def found(self, *replacements: tuple[int, str, str]) -> list[str]:
        parts = list(self.inputs)
        for index, old, new in replacements:
            parts[index] = parts[index].replace(old, new, 1)
        return findings(*parts)

    def test_live_sources_are_clean(self) -> None:
        self.assertEqual(check(), [])

    def test_a_tier_table_is_caught(self) -> None:
        found = self.found((1, "let fallback_width", "match ctx.breakpoint { Breakpoint::Compact => 1, Breakpoint::Medium => 2, Breakpoint::Expanded => 3, Breakpoint::Ultra => 4 };\n    let fallback_width"))
        self.assertTrue(any("Breakpoint" in item for item in found), found)

    def test_a_compact_view_boolean_is_caught(self) -> None:
        found = self.found((1, "pub layout: OutboundLayout", "pub compact_view: bool"))
        self.assertTrue(any("compact_view" in item for item in found), found)

    def test_a_missing_scene_marker_is_caught(self) -> None:
        found = self.found((2, "    let grid_container = ProxyGridContainer(geometry);", "    let grid_container = ();"))
        self.assertTrue(any("ProxyGridContainer" in item for item in found), found)

    def test_a_missing_shared_rule_is_caught(self) -> None:
        found = self.found((0, "proxy_node_columns(form, page_body_width_px)", "private_proxy_columns(form, page_body_width_px)"), (0, "    ProxyGridForm, ResponsiveViewportSnapshot, ViewportTier, proxy_node_columns,", "    ProxyGridForm, ResponsiveViewportSnapshot, ViewportTier,"), (0, "                proxy_node_columns(form, page_body_width_px),", "                4,"))
        self.assertTrue(any("shared owner" in item for item in found), found)

    def test_a_missing_virtual_consumer_is_caught(self) -> None:
        found = self.found((3, "proxy_grid_geometry(&viewport, projection.layout).columns", "3"))
        self.assertTrue(any("shared proxy_grid_geometry owner" in item for item in found), found)

    def test_a_missing_gate_leg_is_caught(self) -> None:
        found = self.found((10, "gate python3 scripts/quality/bevy-proxy-grid-owner-guard.py --self-test", "# removed"))
        self.assertTrue(any("structure gate" in item for item in found), found)

    def test_a_hard_coded_first_frame_width_is_caught(self) -> None:
        found = self.found((8, "proxy_node_scene_sized(context, node, width, palette)", "proxy_node_scene_sized(context, node, percent(31), palette)"))
        self.assertTrue(any("three-column width" in item or "shared column width" in item for item in found), found)

    def test_a_reconcile_without_shared_width_is_caught(self) -> None:
        found = self.found((9, "proxy_grid_geometry(&viewport, layout).columns", "3"))
        self.assertTrue(any("proxies_reconcile.rs" in item for item in found), found)


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
    except OSError as error:
        print(f"bevy proxy grid owner guard: {error}", file=sys.stderr)
        return 2
    for problem in problems:
        print(f"bevy proxy grid owner guard: {problem}", file=sys.stderr)
    if problems and args.mode == "enforce":
        return 1
    print(f"bevy proxy grid owner guard: findings={len(problems)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
