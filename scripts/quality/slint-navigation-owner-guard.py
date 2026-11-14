#!/usr/bin/env python3
"""Hold Slint's desktop sidebar to the shared navigation owner.

FlClash's `Navigation.getItems()` is one typed assembly point, and this
repository's equivalent is `shell_navigation::desktop_items(surface)`: it is
the only place that decides which surface-local rows a surface can reach. The
Slint projection used to read the raw `NavigationSurface::Desktop` registry,
which happened to produce the same eight rows but bypassed that owner.

The compact dock has the same rule: `NavigationSurface::Dock` is the only row
set a four-item bar may mount. Destinations, routes, icons and active
selection are published by the projection, never re-declared or re-derived by
`AppBottomBar`.

The WORD on a dock tab is a different fact, and it has a different owner.
Copy belongs to the surface's compact vocabulary (`PageTitles`), which is the
same owner the compact page headers read, so a tab can never be named
differently from the header of the page it opens. The projection therefore
publishes no label: a published word would make the host spell a second copy
of the dialect, in the host language.

This bounded lexical reader checks the facts a re-edit is most likely to break:
the host publishes a typed surface, the initial frame carries it, every
snapshot uses its own published surface, and the projection reads
`desktop_items(surface)` instead of a private destination list.

Usage:
    python3 scripts/quality/slint-navigation-owner-guard.py [--mode report|enforce]
    python3 scripts/quality/slint-navigation-owner-guard.py --self-test
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
HOST = ROOT / "crates/infiltrator-slint/src/lib.rs"
PROJECTION = ROOT / "crates/infiltrator-slint/src/lumen_projection.rs"
DOCK_PROJECTION = ROOT / "crates/infiltrator-slint/src/lumen_projection/dock.rs"
CHROME = ROOT / "crates/infiltrator-slint/ui/components/app_chrome.slint"
APP = ROOT / "crates/infiltrator-slint/ui/lumen-app.slint"
MODELS = ROOT / "crates/infiltrator-slint/ui/components/models.slint"
HOST_IMPLEMENTATIONS = (
    ROOT / "crates/infiltrator-slint/src/desktop.rs",
    ROOT / "crates/infiltrator-slint/src/demo.rs",
)
GATE = ROOT / "scripts/quality/check-structure.sh"


def read(path: pathlib.Path) -> str:
    return "\n".join(line.split("//", 1)[0] for line in path.read_text(encoding="utf-8").splitlines())


def findings(host: pathlib.Path = HOST,
             projection: pathlib.Path = PROJECTION,
             dock_projection: pathlib.Path = DOCK_PROJECTION,
             chrome: pathlib.Path = CHROME,
             app: pathlib.Path = APP,
             models: pathlib.Path = MODELS,
             implementations: tuple[pathlib.Path, ...] = HOST_IMPLEMENTATIONS,
             gate: pathlib.Path = GATE) -> list[str]:
    found: list[str] = []
    host_source = read(host)
    projection_source = read(projection)
    dock_projection_source = read(dock_projection)
    chrome_source = read(chrome)
    app_source = read(app)
    models_source = read(models)
    gate_source = gate.read_text(encoding="utf-8")

    if not re.search(r"fn surface_kind\(&self\) -> SurfaceKind;", host_source):
        found.append(f"{host}: SlintProductHost no longer publishes a typed SurfaceKind")
    if "host.surface_kind()" not in host_source:
        found.append(f"{host}: the initial frame no longer receives the host's SurfaceKind")
    if "let rows = desktop_items(surface)" not in projection_source:
        found.append(
            f"{projection}: the sidebar projection no longer calls desktop_items(surface); "
            "a raw registry read bypasses surface-local ownership"
        )
    if "NavigationItem::items(NavigationSurface::Desktop)" in projection_source:
        found.append(
            f"{projection}: the raw desktop registry is still a navigation source"
        )
    if "project_navigation(window, &lang, snapshot.surface);" not in projection_source:
        found.append(
            f"{projection}: a snapshot no longer passes its own published surface to the owner"
        )
    if "NavigationItem::items(NavigationSurface::Dock)" not in dock_projection_source:
        found.append(
            f"{dock_projection}: the compact dock no longer reads the typed Dock registry"
        )
    if "window.set_dock_items(slint_model(rows));" not in dock_projection_source:
        found.append(
            f"{dock_projection}: the dock rows no longer reach the Slint window"
        )
    if "label:" in dock_projection_source:
        found.append(
            f"{dock_projection}: the dock projection publishes a word again; the "
            "compact vocabulary owns the tab copy"
        )
    for call in ("dock::project_dock(window);",):
        if projection_source.count(call) != 2:
            found.append(
                f"{projection}: initial and snapshot frames must both publish the typed dock"
            )
    if "export struct DockSlot {" not in models_source:
        found.append(f"{models}: the typed dock row model is missing")
    dock_row = re.search(r"export struct DockSlot \{(?P<body>[^}]*)\}", models_source)
    if dock_row is not None and re.search(r"\blabel\s*:", dock_row.group("body")):
        found.append(
            f"{models}: the dock row publishes a word again; the compact "
            "vocabulary owns the tab copy"
        )
    if "for item in root.dock-items: NavButton {" not in chrome_source:
        found.append(
            f"{chrome}: AppBottomBar no longer renders the projected dock rows"
        )
    if "label: PageTitles.phone-title(item.route);" not in chrome_source:
        found.append(
            f"{chrome}: the dock no longer names its rows from the compact copy owner"
        )
    for retired in (
        'PageTitles.phone-title("',
        "label: item.label",
        '@image-url("../../../infiltrator-shared/assets/ui/iced/svg/layout-grid.svg")',
    ):
        if retired in chrome_source:
            found.append(f"{chrome}: the dock keeps a second navigation literal ({retired})")
    if "active: item.destination == root.active-destination;" not in chrome_source:
        found.append(
            f"{chrome}: the dock re-derives active state from page strings"
        )
    if "dock-items: root.dock-items; active-destination: root.active-destination;" not in app_source:
        found.append(
            f"{app}: the dock does not receive the projected rows and active destination"
        )
    for implementation in implementations:
        source = read(implementation)
        if "fn surface_kind(&self) -> SurfaceKind" not in source:
            found.append(f"{implementation}: the host does not publish its surface")
    for leg in ("slint-navigation-owner-guard.py --self-test",
                "slint-navigation-owner-guard.py --mode enforce"):
        if leg not in gate_source:
            found.append(f"{gate}: the structure gate is missing '{leg}'")
    return found


class GuardSelfTest(unittest.TestCase):
    def test_live_sources_are_clean(self) -> None:
        self.assertEqual(findings(), [])

    def test_a_raw_registry_read_is_caught(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            planted = pathlib.Path(directory) / "projection.rs"
            planted.write_text(
                read(PROJECTION).replace(
                    "let rows = desktop_items(surface)",
                    "let rows = NavigationItem::items(NavigationSurface::Desktop)",
                ),
                encoding="utf-8",
            )
            found = findings(projection=planted)
            self.assertTrue(any("raw desktop registry" in item for item in found), found)
            self.assertTrue(any("no longer calls" in item for item in found), found)

    def test_a_snapshot_without_the_typed_surface_is_caught(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            planted = pathlib.Path(directory) / "projection.rs"
            planted.write_text(
                read(PROJECTION).replace(
                    "project_navigation(window, &lang, snapshot.surface);",
                    "project_navigation(window, &lang, SurfaceKind::SlintDesktop);",
                ),
                encoding="utf-8",
            )
            self.assertTrue(
                any("snapshot no longer passes" in item for item in findings(projection=planted))
            )

    def test_a_dock_that_stops_reading_the_registry_is_caught(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            planted = pathlib.Path(directory) / "dock.rs"
            planted.write_text(
                read(DOCK_PROJECTION).replace(
                    "NavigationItem::items(NavigationSurface::Dock)",
                    "NavigationItem::items(NavigationSurface::Desktop)",
                ),
                encoding="utf-8",
            )
            self.assertTrue(
                any("typed Dock registry" in item for item in findings(dock_projection=planted))
            )

    def test_a_hardcoded_dock_tab_is_caught(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            planted = pathlib.Path(directory) / "app_chrome.slint"
            planted.write_text(
                read(CHROME).replace(
                    "for item in root.dock-items: NavButton {",
                    "NavButton {",
                ),
                encoding="utf-8",
            )
            self.assertTrue(
                any("no longer renders the projected dock rows" in item
                    for item in findings(chrome=planted))
            )

    def test_a_dock_that_spell_its_own_word_is_caught(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            planted = pathlib.Path(directory) / "app_chrome.slint"
            planted.write_text(
                read(CHROME).replace(
                    "label: PageTitles.phone-title(item.route);",
                    'label: "Dashboard";',
                ),
                encoding="utf-8",
            )
            found = findings(chrome=planted)
            self.assertTrue(
                any("compact copy owner" in item for item in found), found
            )

    def test_a_dock_that_names_a_hardcoded_route_is_caught(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            planted = pathlib.Path(directory) / "app_chrome.slint"
            planted.write_text(
                read(CHROME).replace(
                    "label: PageTitles.phone-title(item.route);",
                    'label: PageTitles.phone-title("Home");',
                ),
                encoding="utf-8",
            )
            found = findings(chrome=planted)
            self.assertTrue(
                any("second navigation literal" in item for item in found), found
            )

    def test_a_projection_that_publishes_a_word_again_is_caught(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            planted = pathlib.Path(directory) / "dock.rs"
            planted.write_text(
                read(DOCK_PROJECTION).replace(
                    "destination: item.id().into(),",
                    "destination: item.id().into(),\n            label: item.id().into(),",
                ),
                encoding="utf-8",
            )
            found = findings(dock_projection=planted)
            self.assertTrue(
                any("publishes a word again" in item for item in found), found
            )

    def test_an_initial_frame_without_the_host_surface_is_caught(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            planted = pathlib.Path(directory) / "lib.rs"
            planted.write_text(
                read(HOST).replace("host.surface_kind()", "SurfaceKind::SlintDesktop"),
                encoding="utf-8",
            )
            self.assertTrue(
                any("initial frame no longer" in item
                    for item in findings(host=planted))
            )


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
        found = findings()
    except OSError as error:
        print(f"slint navigation owner guard: {error}", file=sys.stderr)
        return 2
    for problem in found:
        print(f"slint navigation owner guard: {problem}", file=sys.stderr)
    print(f"slint navigation owner guard: findings={len(found)}")
    return 1 if found and args.mode == "enforce" else 0


if __name__ == "__main__":
    raise SystemExit(main())
