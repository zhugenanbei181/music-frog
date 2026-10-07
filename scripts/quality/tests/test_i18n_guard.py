"""Regression checks for the Rust localization scanner's real coverage boundaries."""
import importlib.util
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'scripts/quality'))
spec = importlib.util.spec_from_file_location('i18n_guard', ROOT / 'scripts/quality/i18n-guard.py')
guard = importlib.util.module_from_spec(spec)
spec.loader.exec_module(guard)


class LocalizationCoverageTests(unittest.TestCase):
    def test_runtime_handlers_and_shared_interpolation_are_covered(self):
        self.assertIn(Path('crates/infiltrator-iced/src/update'), guard.VIEW_SCAN_DIRS)
        self.assertIn(Path('crates/infiltrator-iced/src/state'), guard.VIEW_SCAN_DIRS)
        source = 'localize(&locale, "runtime_failure", &[("reason", data)])'
        self.assertEqual(guard.INTERPOLATION_REF_PATTERN.findall(source), ['runtime_failure'])
        source = 'localize(locale, "runtime_failure", &[])'
        self.assertEqual(guard.INTERPOLATION_REF_PATTERN.findall(source), ['runtime_failure'])

    def test_test_gated_import_does_not_hide_the_following_production_view(self):
        source = '#[cfg(test)]\nuse fixture::{One, Two};\nfn view() { Text("真实文案"); }'
        code = guard.production_code(source)
        self.assertNotIn('fixture', code)
        self.assertIn('真实文案', code)

    def test_test_module_is_skipped_without_hiding_production_after_it(self):
        source = '#[cfg(test)]\nmod tests { fn sample() { let input = "夹具"; } }\nfn view() { Text("真实文案"); }'
        code = guard.production_code(source)
        self.assertNotIn('夹具', code)
        self.assertIn('真实文案', code)

    def test_urls_raw_strings_and_nested_comments_keep_their_correct_boundaries(self):
        source = '/* outside /* "注释" */ still outside */\nfn view() { Text("粘贴 https://example.test/链接"); Text(r#"多行\n文案"#); }'
        code = guard.production_code(source)
        self.assertNotIn('注释', code)
        self.assertIn('https://example.test/链接', code)
        self.assertIn('多行\n文案', code)

    def test_production_speedtest_is_scanned_and_real_test_files_are_skipped(self):
        self.assertFalse(guard.is_test_path(Path('crates/ui/src/pages/speedtest_details.rs')))
        self.assertTrue(guard.is_test_path(Path('crates/ui/tests/headless/scene.rs')))
        self.assertTrue(guard.is_test_path(Path('crates/ui/src/scene_tests.rs')))
        self.assertTrue(guard.is_test_path(Path('crates/ui/src/scene_test_cases/part.rs')))

    def test_duplicate_keys_in_another_table_cannot_silently_shadow_shared_copy(self):
        primary, extension = Path('primary.rs'), Path('extension.rs')
        sources = {primary: '"action_save" => "Save".into(),',
                   extension: '"action_save" => "Overwrite".into(),'}
        with patch.object(guard, 'ZH_TABLES', [primary, extension]), \
                patch.object(guard, 'EN_TABLES', []), \
                patch.object(guard, 'read', side_effect=sources.__getitem__):
            failures = guard.check_duplicate_keys()
        self.assertEqual(len(failures), 1)
        self.assertIn("duplicate locale key 'action_save'", failures[0])
        self.assertIn('primary.rs:1', failures[0])
        self.assertIn('extension.rs:1', failures[0])

    def test_translation_parameter_rename_or_removal_is_rejected_but_order_is_free(self):
        chinese, english = Path('zh.rs'), Path('en.rs')
        sources = {
            chinese: '"result" => "{profile} · {count}".into(),',
            english: '"result" => "{count} for {profile}".into(),',
        }
        with patch.object(guard, 'ZH_TABLES', [chinese]), \
                patch.object(guard, 'EN_TABLES', [english]), \
                patch.object(guard, 'read', side_effect=sources.__getitem__):
            self.assertEqual(guard.check_placeholder_parity(), [])
            sources[english] = '"result" => "{total} for {profile}".into(),'
            failure = guard.check_placeholder_parity()
            self.assertEqual(len(failure), 1)
            self.assertIn("locale key 'result'", failure[0])
            self.assertIn("total", failure[0])
            sources[english] = '"result" => "Result for {profile}".into(),'
            self.assertEqual(len(guard.check_placeholder_parity()), 1)



if __name__ == '__main__':
    unittest.main()
