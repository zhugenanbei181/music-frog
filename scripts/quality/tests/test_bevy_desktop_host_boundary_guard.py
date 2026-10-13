from __future__ import annotations

import importlib.util
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[3]
SCRIPT = ROOT / "scripts/quality/bevy-desktop-host-boundary-guard.py"
SPEC = importlib.util.spec_from_file_location("bevy_desktop_host_boundary_guard", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
guard = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(guard)


class BevyDesktopHostBoundaryGuardTests(unittest.TestCase):
    def test_manifest_dependency_reader_reaches_target_tables(self) -> None:
        self.assertTrue(
            guard.named_dependencies(
                {"target": {"cfg(unix)": {"dependencies": {"infiltrator-desktop": {}}}}},
                "infiltrator-desktop",
            )
        )

    def test_workspace_host_and_ui_boundaries(self) -> None:
        self.assertEqual(guard.manifest_problems(ROOT), [])
        self.assertEqual(guard.source_problems(ROOT), [])


if __name__ == "__main__":
    unittest.main()
