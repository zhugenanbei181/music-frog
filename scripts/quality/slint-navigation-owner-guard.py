#!/usr/bin/env python3
"""Hold Slint's desktop sidebar to the shared navigation owner.

FlClash's `Navigation.getItems()` is one typed assembly point, and this
repository's equivalent is `shell_navigation::desktop_items(surface)`: it is
the only place that decides which surface-local rows a surface can reach. The
Slint projection used to read the raw `NavigationSurface::Desktop` registry,
which happened to produce the same eight rows but bypassed that owner.

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
HOST_IMPLEMENTATIONS = (
    ROOT / "crates/infiltrator-slint/src/desktop.rs",
    ROOT / "crates/infiltrator-slint/src/demo.rs",
)
GATE = ROOT / "scripts/quality/check-structure.sh"


def read(path: pathlib.Path) -> str:
    return "\n".join(line.split("//", 1)[0] for line in path.read_text(encoding="utf-8").splitlines())


def findings(host: pathlib.Path = HOST,
             projection: pathlib.Path = PROJECTION,
             implementations: tuple[pathlib.Path, ...] = HOST_IMPLEMENTATIONS,
             gate: pathlib.Path = GATE) -> list[str]:
    found: list[str] = []
    host_source = read(host)
    projection_source = read(projection)
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
