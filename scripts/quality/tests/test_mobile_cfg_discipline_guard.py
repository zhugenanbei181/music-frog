from __future__ import annotations

import importlib.util
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[3]
SCRIPT = ROOT / "scripts/quality/mobile-cfg-discipline-guard.py"
SPEC = importlib.util.spec_from_file_location("mobile_cfg_discipline_guard", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
guard = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(guard)


class MobileCfgDisciplineGuardTests(unittest.TestCase):
    def test_protected_type_exclusion_is_flagged(self) -> None:
        code = '#[cfg(not(target_os = "android"))]\nuse infiltrator_bevy_widgets::palette::UiPalette;\n'
        problems = guard.check_protected_type_exclusions(Path("dummy.rs"), code)
        self.assertTrue(any("UiPalette" in p for p in problems))

    def test_unconditioned_pub_fn_signature_mismatch_is_flagged(self) -> None:
        code = (
            '#[cfg(not(target_os = "android"))]\n'
            'use crate::desktop::DesktopHostRef;\n\n'
            'pub fn get_desktop_host(r: &DesktopHostRef) -> bool {\n'
            '    true\n'
            '}\n'
        )
        problems = guard.check_unconditioned_pub_fn_signatures(Path("dummy.rs"), code)
        self.assertTrue(any("DesktopHostRef" in p for p in problems))

    def test_desktop_only_pub_fn_is_allowed(self) -> None:
        code = (
            '#[cfg(not(target_os = "android"))]\n'
            'use crate::desktop::DesktopHostRef;\n\n'
            '#[cfg(not(target_os = "android"))]\n'
            'pub fn get_desktop_host(r: &DesktopHostRef) -> bool {\n'
            '    true\n'
            '}\n'
        )
        problems = guard.check_unconditioned_pub_fn_signatures(Path("dummy.rs"), code)
        self.assertEqual(problems, [])

    def test_chrome_capability_honesty(self) -> None:
        problems = guard.check_chrome_capability_honesty(ROOT)
        self.assertEqual(problems, [])

    def test_all_workspace_ui_sources_pass_guard(self) -> None:
        problems = guard.find_all_problems(ROOT)
        self.assertEqual(problems, [])


if __name__ == "__main__":
    unittest.main()
