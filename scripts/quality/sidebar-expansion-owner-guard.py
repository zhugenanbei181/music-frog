#!/usr/bin/env python3
"""Guard the owner chain for FlClash's desktop sidebar-expansion preference.

The preference is persisted by the settings owner, changed through one typed
command, and read back in the shared settings page snapshot. This guard rejects
a UI-local boolean, an untyped key path, or a command that disappears from one
of the exhaustive dispatch/name tables.
"""

from __future__ import annotations

import argparse
import pathlib
import sys
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
COMMAND = ROOT / "crates/infiltrator-contract/src/command.rs"
SNAPSHOT = ROOT / "crates/infiltrator-contract/src/surface_snapshot.rs"
SETTINGS = ROOT / "crates/infiltrator-domain/src/settings.rs"
DISPATCH = ROOT / "crates/infiltrator-application/src/command_application/dispatch.rs"
APPLICATION = ROOT / "crates/infiltrator-application/src/command_application.rs"
COMMAND_NAME = ROOT / "crates/infiltrator-application/src/core_application/command_name.rs"
BUILDER = ROOT / "crates/infiltrator-composition/src/surface_reader/page_builders.rs"
CONTRACT_TESTS = ROOT / "crates/infiltrator-contract/src/lib.rs"
APPLICATION_TESTS = ROOT / (
    "crates/infiltrator-application/src/command_application_settings_tests.rs"
)
COMPOSITION_TESTS = ROOT / (
    "crates/infiltrator-composition/src/surface_reader/settings_observation_tests.rs"
)
GATE = ROOT / "scripts/quality/check-structure.sh"

REQUIRED = {
    "command": (
        "use crate::shell_sidebar_preference::SidebarExpansionIntent;",
        "ApplySidebarExpansion {",
        "intent: SidebarExpansionIntent,",
        "| Self::ApplySidebarExpansion { .. }",
    ),
    "snapshot": (
        "use crate::shell_sidebar_preference::SidebarExpansionPreference;",
        "pub sidebar_expansion: SidebarExpansionPreference,",
    ),
    "settings": (
        "use infiltrator_contract::shell_sidebar_preference::SidebarExpansionPreference;",
        "pub sidebar_expansion: SidebarExpansionPreference,",
        "sidebar_expansion: SidebarExpansionPreference::default(),",
    ),
    "dispatch": (
        "CommandIntent::ApplySidebarExpansion { intent } => {",
        "self.apply_sidebar_expansion(intent).await",
    ),
    "application": (
        "async fn apply_sidebar_expansion(",
        "apply_sidebar_expansion_intent(current, intent)",
        "settings.sidebar_expansion = next",
    ),
    "name": ('CommandIntent::ApplySidebarExpansion { .. } => "apply_sidebar_expansion",',),
    "builder": ("sidebar_expansion: settings.sidebar_expansion,",),
    "contract_tests": (
        "use super::shell_sidebar_preference::SidebarExpansionIntent;",
        "CommandIntent::ApplySidebarExpansion {",
    ),
    "application_tests": (
        "async fn sidebar_expansion_intents_persist_one_typed_preference()",
        "SidebarExpansionPreference::Collapsed",
    ),
    "composition_tests": (
        "facts.sidebar_expansion",
        "SidebarExpansionPreference::Collapsed",
    ),
}

FILES = {
    "command": COMMAND,
    "snapshot": SNAPSHOT,
    "settings": SETTINGS,
    "dispatch": DISPATCH,
    "application": APPLICATION,
    "name": COMMAND_NAME,
    "builder": BUILDER,
    "contract_tests": CONTRACT_TESTS,
    "application_tests": APPLICATION_TESTS,
    "composition_tests": COMPOSITION_TESTS,
}


def findings(texts: dict[str, str], gate: str) -> list[str]:
    found: list[str] = []
    for key, needles in REQUIRED.items():
        source = texts[key]
        for needle in needles:
            if needle not in source:
                found.append(f"{key} is missing {needle!r}")
    if "sidebar_expansion" in texts["application"] and "ApplySidebarExpansion" not in texts["dispatch"]:
        found.append("the typed preference command is no longer dispatched")
    if "sidebar-expansion-owner-guard.py --self-test" not in gate:
        found.append("the guard self-test is not part of the structure gate")
    if "sidebar-expansion-owner-guard.py --mode enforce" not in gate:
        found.append("the guard enforce leg is not part of the structure gate")
    return found


def check() -> list[str]:
    paths = list(FILES.values()) + [GATE]
    missing = [str(path.relative_to(ROOT)) for path in paths if not path.is_file()]
    if missing:
        return [f"missing owner-chain file: {path}" for path in missing]
    texts = {key: path.read_text(encoding="utf-8") for key, path in FILES.items()}
    return findings(texts, GATE.read_text(encoding="utf-8"))


class GuardSelfTest(unittest.TestCase):
    def setUp(self) -> None:
        self.texts = {key: path.read_text(encoding="utf-8") for key, path in FILES.items()}
        self.gate = GATE.read_text(encoding="utf-8")

    def found(self, **changes: str) -> list[str]:
        texts = dict(self.texts)
        texts.update(changes)
        return findings(texts, changes.get("gate", self.gate))

    def test_live_tree_is_clean(self) -> None:
        self.assertEqual(self.found(), [])

    def test_a_missing_typed_command_is_caught(self) -> None:
        planted = "\n".join(
            line for line in self.texts["command"].splitlines()
            if "ApplySidebarExpansion {" not in line
        )
        self.assertTrue(any("missing 'ApplySidebarExpansion {'" in item for item in self.found(command=planted)))

    def test_a_missing_readback_is_caught(self) -> None:
        planted = self.texts["builder"].replace(
            "sidebar_expansion: settings.sidebar_expansion,\n", "", 1
        )
        self.assertTrue(any("builder is missing" in item for item in self.found(builder=planted)))

    def test_a_missing_gate_leg_is_caught(self) -> None:
        planted = self.gate.replace(
            "sidebar-expansion-owner-guard.py --mode enforce\n", "", 1
        )
        self.assertTrue(any("enforce leg" in item for item in self.found(gate=planted)))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("report", "enforce"), default="enforce")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(GuardSelfTest)
        result = unittest.TextTestRunner(verbosity=1).run(suite)
        return 0 if result.wasSuccessful() else 1
    problems = check()
    for problem in problems:
        print(f"sidebar expansion owner guard: {problem}", file=sys.stderr)
    if problems and args.mode == "enforce":
        return 1
    print(
        f"sidebar expansion owner guard: {len(REQUIRED)} owner-chain files adjudicated, "
        f"findings={len(problems)}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
