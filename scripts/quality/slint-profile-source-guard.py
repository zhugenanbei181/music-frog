#!/usr/bin/env python3
"""SLINT-019: the profile origin is an owner fact and one shared card row.

FlClash's `ProfilesCard` places a localized `ProfileType` label at the end of
the profile name row. MusicFrog's richer `ProfileDocumentSourceKind` is already
published in `ProfileSnapshot.filter_source`; this guard rejects a return to
URL string checks, hardcoded source words, and two separate card layouts.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys
import unittest
import unittest.mock

ROOT = pathlib.Path(__file__).resolve().parents[2]
SHARED = ROOT / "crates/infiltrator-contract/src/shared_copy.rs"
PRESENTATION = ROOT / "crates/infiltrator-presentation/src/profile_source_projection.rs"
CARD = ROOT / "crates/infiltrator-presentation/src/dashboard_hero_projection.rs"
RUST = ROOT / "crates/infiltrator-slint/src/lumen_projection/dashboard.rs"
WIDGETS = ROOT / "crates/infiltrator-slint/ui/pages/home_widgets.slint"
DESKTOP = ROOT / "crates/infiltrator-slint/ui/pages/home_desktop_grid.slint"
PHONE = ROOT / "crates/infiltrator-slint/ui/pages/home_compact_flow.slint"
MODELS = ROOT / "crates/infiltrator-slint/ui/components/models.slint"


def findings() -> list[str]:
    shared = SHARED.read_text(encoding="utf-8")
    projection = PRESENTATION.read_text(encoding="utf-8")
    card = CARD.read_text(encoding="utf-8")
    rust = RUST.read_text(encoding="utf-8")
    widgets = WIDGETS.read_text(encoding="utf-8")
    desktop = DESKTOP.read_text(encoding="utf-8")
    phone = PHONE.read_text(encoding="utf-8")
    models = MODELS.read_text(encoding="utf-8")
    found: list[str] = []
    for variant, key in (
        ("ProfileSourceLocal", "profile_source_local"),
        ("ProfileSourceSubscription", "profile_source_subscription"),
        ("ProfileSourceImported", "profile_source_imported"),
        ("ProfileSourceSynchronized", "profile_source_synchronized"),
        ("ProfileSourceUnobserved", "profile_source_unobserved"),
    ):
        if f'Self::{variant} => "{key}"' not in shared:
            found.append(f"shared_copy.rs: missing {key}")
    if "ProfileDocumentSourceKind" not in projection or "filter_source" not in projection:
        found.append("profile_source_projection.rs: must read the owner workspace identity")
    if "pub source: ProfileSourceObservation" not in card:
        found.append("dashboard_hero_projection.rs: source must be typed")
    if "is_local:" in card or "url.is_empty()" in card:
        found.append("dashboard_hero_projection.rs: URL guessing is not an owner fact")
    if "profile_source_label: profile_source_label.into()" not in rust:
        found.append("dashboard.rs: source label is not projected")
    if "profile-source-label: string," not in models:
        found.append("models.slint: source label field is missing")
    if "export component ProfileIdentityRow" not in widgets:
        found.append("home_widgets.slint: shared identity row is missing")
    for name, source in (("desktop", desktop), ("phone", phone)):
        if not re.search(r"ProfileIdentityRow\s*\{", source):
            found.append(f"{name}: profile card does not use the shared identity row")
        if "profile-source-label" not in source:
            found.append(f"{name}: source label is not bound")
    return found


class GuardSelfTest(unittest.TestCase):
    def setUp(self) -> None:
        self.shared = SHARED.read_text(encoding="utf-8")
        self.card = CARD.read_text(encoding="utf-8")
        self.rust = RUST.read_text(encoding="utf-8")

    def test_live_tree_is_clean(self) -> None:
        self.assertEqual(findings(), [])

    def test_a_missing_shared_key_is_caught(self) -> None:
        planted = self.shared.replace(
            'Self::ProfileSourceSubscription => "profile_source_subscription",',
            'Self::ProfileSourceSubscription => "profile_source_url",',
            1,
        )
        self.assertNotEqual(planted, self.shared)
        original = pathlib.Path.read_text

        def fake(self, *args, **kwargs):
            return planted if self == SHARED else original(self, *args, **kwargs)

        with unittest.mock.patch.object(pathlib.Path, "read_text", autospec=True) as read_text:
            read_text.side_effect = fake
            found = findings()
        self.assertTrue(any("missing profile_source_subscription" in item for item in found), found)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("enforce", "report"), default="enforce")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(GuardSelfTest)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1
    found = findings()
    for item in found:
        print(f"SLINT-019 [{args.mode}]: {item}")
    print(f"slint profile source guard: findings={len(found)}")
    return 1 if found and args.mode == "enforce" else 0


if __name__ == "__main__":
    sys.exit(main())
