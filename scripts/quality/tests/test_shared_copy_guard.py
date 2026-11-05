"""Regression checks for the shared UI copy catalog."""
import importlib.util
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "scripts/quality"))
spec = importlib.util.spec_from_file_location(
    "shared_copy_guard", ROOT / "scripts/quality/shared-copy-guard.py"
)
guard = importlib.util.module_from_spec(spec)
spec.loader.exec_module(guard)


class SharedCopyRules(unittest.TestCase):
    def test_registry_matches_both_locale_catalogs_and_android_resources(self):
        self.assertEqual(guard.check(), [])

    def test_missing_android_shared_key_is_rejected(self):
        original = guard.android_keys
        with patch.object(
            guard,
            "android_keys",
            side_effect=lambda path: original(path) - {"layout_density"},
        ):
            failures = guard.check()
        self.assertTrue(any("missing shared key 'layout_density'" in item for item in failures))

    def test_bare_chinese_a11y_label_is_rejected(self):
        original = Path.read_text
        with patch.object(
            Path,
            "read_text",
            autospec=True,
            side_effect=lambda path, *args, **kwargs: "label_zh"
            if path.name == "a11y.rs"
            else original(path, *args, **kwargs),
        ):
            failures = guard.check()
        self.assertTrue(any("bare-Chinese label" in item for item in failures))

    def test_new_word_piling_is_rejected_and_repaid_debt_is_reported(self):
        original = guard.verbose_glosses
        with patch.object(
            guard,
            "verbose_glosses",
            side_effect=lambda: original() | {"crates/infiltrator-shared/src/x.rs|padded_key"},
        ):
            failures = guard.check()
        self.assertTrue(any("repeats a Latin gloss" in item for item in failures))
        self.assertEqual(guard.conciseness_failures()[1], [])

    def test_new_overlong_iced_label_is_rejected(self):
        original = guard.long_copy
        with patch.object(
            guard,
            "long_copy",
            side_effect=lambda: original() | {"crates/infiltrator-shared/src/x.rs|wordy_key"},
        ):
            failures = guard.check()
        self.assertTrue(any("over-long Iced label" in item for item in failures))

    def test_iced_visible_literal_is_rejected(self):
        original = guard.ICED_SOURCES
        with patch.object(
            guard,
            "ICED_SOURCES",
            [ROOT / "crates/infiltrator-iced/src/view/sidebar.rs"],
        ):
            violations = guard.iced_visible_copy_failures()
        self.assertEqual(violations, [])
        with patch.object(
            guard,
            "VISIBLE_COPY_ALLOWLIST",
            set(),
        ):
            violations = guard.iced_visible_copy_failures()
        self.assertTrue(any("hard-codes visible copy" in item for item in violations))


if __name__ == "__main__":
    unittest.main()
