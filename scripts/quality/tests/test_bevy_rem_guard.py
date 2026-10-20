"""Enforce the Bevy rem-channel authoring law without measuring real pixels."""
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest

QUALITY = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(QUALITY))
SPEC = importlib.util.spec_from_file_location("bevy_rem_guard", QUALITY / "bevy_rem_guard.py")
GUARD = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = GUARD
SPEC.loader.exec_module(GUARD)


class RemRules(unittest.TestCase):
    def scan(self, files, debt=None):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / GUARD.ROOTS[0]
            source.mkdir(parents=True)
            for name, content in files.items():
                path = source / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(content, encoding="utf-8")
            inventory = root / GUARD.INVENTORY
            inventory.parent.mkdir(parents=True, exist_ok=True)
            inventory.write_text(
                json.dumps({"version": 1, "debt": debt or {}}) + "\n", encoding="utf-8"
            )
            violations, notices = GUARD.scan(root)
        return sorted(v.code for v in violations), notices

    def test_the_rem_channel_is_the_sanctioned_route(self):
        self.assertEqual(
            self.scan({
                "scene.rs": "fn scene() { let width = rem(space::S8); let size = FontSize::Rem(1.0); }",
            })[0],
            [],
        )

    def test_the_physical_whitelist_is_allowed(self):
        self.assertEqual(
            self.scan({
                "hairline.rs": (
                    "fn hairline(palette: &UiPalette) {"
                    " let border = Val::Px(palette.hairline_px);"
                    " let caret = Val::Px(metrics::CARET_WIDTH);"
                    " let touch = Val::Px(metrics::MIN_TOUCH_TARGET);"
                    " let breakpoint = Val::Px(breakpoint::COMPACT_MAX_PX);"
                    " let inset = Val::Px(insets_left); }"
                ),
            })[0],
            [],
        )

    def test_a_named_px_dimension_outside_the_whitelist_is_rejected(self):
        self.assertEqual(
            self.scan({"card.rs": "fn card() { let radius = Val::Px(palette.control_radius_px); }"})[0],
            ["BEVY-REM-001"],
        )

    def test_a_hardcoded_px_literal_is_rejected(self):
        self.assertEqual(
            self.scan({"card.rs": "fn card() { let radius = Val::Px(4.0); }"})[0],
            ["BEVY-REM-001"],
        )

    def test_a_registered_legacy_site_is_allowed_until_it_is_migrated(self):
        codes, _ = self.scan(
            {"card.rs": "fn card() { let radius = Val::Px(palette.control_radius_px); }"},
            debt={"crates/infiltrator-bevy-widgets/src/card.rs": {"palette.control_radius_px": 1}},
        )
        self.assertEqual(codes, [])

    def test_a_legacy_site_beyond_the_registered_count_is_rejected(self):
        codes, _ = self.scan(
            {
                "card.rs": (
                    "fn card() { let a = Val::Px(palette.control_radius_px);"
                    " let b = Val::Px(palette.control_radius_px); }"
                )
            },
            debt={"crates/infiltrator-bevy-widgets/src/card.rs": {"palette.control_radius_px": 1}},
        )
        self.assertEqual(codes, ["BEVY-REM-001"])

    def test_a_non_rem_font_size_is_rejected(self):
        self.assertEqual(
            self.scan({"type.rs": "fn scale() { let size = FontSize::Px(15.0); }"})[0],
            ["BEVY-REM-002"],
        )

    def test_a_viewport_font_size_is_rejected(self):
        self.assertEqual(
            self.scan({"type.rs": "fn scale() { let size = FontSize::Vw(2.0); }"})[0],
            ["BEVY-REM-002"],
        )

    def test_a_literal_em_size_is_rejected(self):
        self.assertEqual(
            self.scan({"icon.rs": "fn icon() { let size = Val::Em(1.5); }"})[0],
            ["BEVY-REM-003"],
        )

    def test_comments_and_string_literals_cannot_hide_or_fake_a_px_site(self):
        self.assertEqual(
            self.scan({
                "notes.rs": '// Val::Px(palette.control_radius_px)\n/// let radius = Val::Px(6.0);\nlet doc = "Val::Px(8.0)";',
            })[0],
            [],
        )

    def test_an_inventory_entry_for_a_removed_file_is_rejected(self):
        codes, _ = self.scan(
            {"card.rs": "fn card() {}"},
            debt={"crates/infiltrator-bevy-widgets/src/gone.rs": {"palette.control_radius_px": 1}},
        )
        self.assertEqual(codes, ["BEVY-REM-004"])

    def test_pruning_refuses_to_register_new_debt(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / GUARD.ROOTS[0]
            source.mkdir(parents=True)
            (source / "card.rs").write_text(
                "fn card() { let radius = Val::Px(palette.control_radius_px); }", encoding="utf-8"
            )
            inventory = root / GUARD.INVENTORY
            inventory.parent.mkdir(parents=True, exist_ok=True)
            inventory.write_text(
                json.dumps({"version": 1, "debt": {}}) + "\n", encoding="utf-8"
            )
            self.assertEqual(GUARD.prune_inventory(root), 1)
            self.assertEqual(
                json.loads(inventory.read_text(encoding="utf-8"))["debt"], {}
            )

    def test_pruning_shrinks_the_registered_debt(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / GUARD.ROOTS[0]
            source.mkdir(parents=True)
            (source / "card.rs").write_text(
                "fn card() { let radius = Val::Px(palette.control_radius_px); }", encoding="utf-8"
            )
            inventory = root / GUARD.INVENTORY
            inventory.parent.mkdir(parents=True, exist_ok=True)
            inventory.write_text(
                json.dumps(
                    {
                        "version": 1,
                        "debt": {
                            "crates/infiltrator-bevy-widgets/src/card.rs": {"palette.control_radius_px": 2},
                            "crates/infiltrator-bevy-widgets/src/gone.rs": {"palette.control_radius_px": 1},
                        },
                    }
                )
                + "\n",
                encoding="utf-8",
            )
            self.assertEqual(GUARD.prune_inventory(root), 0)
            self.assertEqual(
                json.loads(inventory.read_text(encoding="utf-8"))["debt"],
                {"crates/infiltrator-bevy-widgets/src/card.rs": {"palette.control_radius_px": 1}},
            )

    def test_the_real_production_trees_pass(self):
        repo_root = Path(__file__).resolve().parents[3]
        violations, _ = GUARD.scan(repo_root)
        self.assertEqual([f"{v.path}:{v.line}: {v.code}" for v in violations], [])


if __name__ == "__main__":
    unittest.main()
