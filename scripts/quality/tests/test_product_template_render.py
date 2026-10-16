"""Verify product metadata templates render from the central identity source."""

from pathlib import Path
import sys
import unittest

SCRIPTS = Path(__file__).resolve().parents[2]
ROOT = SCRIPTS.parent
sys.path.insert(0, str(SCRIPTS))

from product_identity import read_properties, render_template


class ProductTemplateRenderTests(unittest.TestCase):
    def test_iced_linux_launcher_uses_registered_identity(self):
        template = (ROOT / "packaging/linux/appimage/infiltrator.desktop").read_text(
            encoding="utf-8"
        )
        rendered = render_template(template, read_properties())
        self.assertIn("Name=MusicFrog Infiltrator", rendered)
        self.assertIn("Exec=infiltrator-iced %U", rendered)
        self.assertIn("Icon=infiltrator", rendered)
        self.assertIn("x-scheme-handler/infiltrator;", rendered)

    def test_bevy_linux_launcher_keeps_its_own_registered_identity(self):
        template = (ROOT / "packaging/linux/appimage/infiltrator-bevy.desktop").read_text(
            encoding="utf-8"
        )
        rendered = render_template(template, read_properties())
        self.assertIn("Name=MusicFrog Infiltrator Bevy", rendered)
        self.assertIn("Exec=infiltrator-bevy-ui %U", rendered)
        self.assertIn("Icon=infiltrator-bevy", rendered)
        self.assertIn("x-scheme-handler/infiltrator-bevy;", rendered)

    def test_unknown_placeholder_fails_closed(self):
        with self.assertRaisesRegex(ValueError, "unknown product identity placeholder"):
            render_template("Name=@desktop.future.display_name@", read_properties())


if __name__ == "__main__":
    unittest.main()
