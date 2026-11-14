#!/usr/bin/env python3
"""Hold the FlClash `modes` equivalent to its Core-side typed owner.

FlClash publishes navigation items with `NavigationItemMode` and gates rows by
`openLogs` / `hasProxies`; a hidden current page falls back to the first visible
item. The repository's answer is `navigation_capability.rs`, with a persisted
setting for `open_logs`, a Core-composed `PageData` observation on
`SurfaceSnapshot`, and no UI-local capability derivation.

Usage:
    python3 scripts/quality/navigation-capability-owner-guard.py [--mode report|enforce]
    python3 scripts/quality/navigation-capability-owner-guard.py --self-test
"""

from __future__ import annotations

import argparse
import pathlib
import sys
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
CONTRACT = ROOT / "crates/infiltrator-contract/src/navigation_capability.rs"
SETTINGS = ROOT / "crates/infiltrator-domain/src/settings.rs"
DISPATCH = ROOT / "crates/infiltrator-application/src/command_application.rs"
SNAPSHOT = ROOT / "crates/infiltrator-contract/src/surface_snapshot.rs"
COMPOSITION = ROOT / "crates/infiltrator-composition/src/surface_reader.rs"
COMPOSITION_CAPABILITY = (
    ROOT / "crates/infiltrator-composition/src/surface_reader/navigation_capability.rs"
)
GATE = ROOT / "scripts/quality/check-structure.sh"


def read(path: pathlib.Path) -> str:
    return "\n".join(line.split("//", 1)[0] for line in path.read_text(encoding="utf-8").splitlines())


def findings(
    contract: pathlib.Path = CONTRACT,
    settings: pathlib.Path = SETTINGS,
    dispatch: pathlib.Path = DISPATCH,
    snapshot: pathlib.Path = SNAPSHOT,
    composition: pathlib.Path = COMPOSITION,
    composition_capability: pathlib.Path = COMPOSITION_CAPABILITY,
    gate: pathlib.Path = GATE,
) -> list[str]:
    found: list[str] = []
    contract_source = read(contract)
    settings_source = read(settings)
    dispatch_source = read(dispatch)
    snapshot_source = read(snapshot)
    composition_source = read(composition)
    composition_capability_source = read(composition_capability)
    gate_source = gate.read_text(encoding="utf-8")

    for symbol in (
        "pub enum NavigationMode",
        "pub struct NavigationFeatureState",
        "pub struct NavigationCapabilitySnapshot",
        "fn resolve(",
        "fn projected_rows(",
    ):
        if symbol not in contract_source:
            found.append(f"{contract}: the typed modes owner is missing `{symbol}`")
    if "pub open_logs: bool" not in settings_source:
        found.append(f"{settings}: the persisted open_logs owner is missing")
    if '"open_logs"' not in dispatch_source:
        found.append(f"{dispatch}: the settings command no longer owns open_logs")
    if "pub navigation_capability: PageData<NavigationCapabilitySnapshot>" not in snapshot_source:
        found.append(f"{snapshot}: the typed capability readback is missing")
    if "PageData::ready(project_navigation_capability(" not in composition_capability_source:
        found.append(f"{composition}: the Core owner no longer publishes a ready capability answer")
    if "mod navigation_capability;" not in composition_source:
        found.append(f"{composition}: the Core-side capability projection module is missing")
    for leg in (
        "navigation-capability-owner-guard.py --mode enforce",
        "navigation-capability-owner-guard.py --self-test",
    ):
        if leg not in gate_source:
            found.append(f"{gate}: the structure gate is missing '{leg}'")
    return found


class GuardSelfTest(unittest.TestCase):
    def test_live_sources_are_clean(self) -> None:
        self.assertEqual(findings(), [])

    def test_a_hidden_modes_owner_is_caught(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            planted = pathlib.Path(directory) / "navigation_capability.rs"
            planted.write_text(
                read(CONTRACT).replace("pub enum NavigationMode", "enum HiddenMode"),
                encoding="utf-8",
            )
            self.assertTrue(any("typed modes owner" in item for item in findings(contract=planted)))

    def test_a_local_default_capability_is_caught(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            planted = pathlib.Path(directory) / "navigation_capability.rs"
            planted.write_text(
                read(COMPOSITION_CAPABILITY).replace(
                    "PageData::ready(project_navigation_capability(",
                    "PageData::ready(Default::default(), project_navigation_capability(",
                ),
                encoding="utf-8",
            )
            self.assertTrue(
                any(
                    "ready capability answer" in item
                    for item in findings(composition_capability=planted)
                )
            )

    def test_a_missing_structure_leg_is_caught(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            planted = pathlib.Path(directory) / "check-structure.sh"
            planted.write_text(
                GATE.read_text(encoding="utf-8").replace(
                    "gate python3 scripts/quality/navigation-capability-owner-guard.py --self-test\n",
                    "",
                ),
                encoding="utf-8",
            )
            self.assertTrue(any("structure gate is missing" in item for item in findings(gate=planted)))


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
        print(f"navigation capability owner guard: {error}", file=sys.stderr)
        return 2
    for problem in found:
        print(f"navigation capability owner guard: {problem}", file=sys.stderr)
    print(f"navigation capability owner guard: findings={len(found)}")
    return 1 if found and args.mode == "enforce" else 0


if __name__ == "__main__":
    raise SystemExit(main())
