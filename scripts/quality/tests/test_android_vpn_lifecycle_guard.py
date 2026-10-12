from __future__ import annotations

import importlib.util
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[3]
SCRIPT = ROOT / "scripts/quality/android-vpn-lifecycle-guard.py"
SPEC = importlib.util.spec_from_file_location("android_vpn_lifecycle_guard", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
guard = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(guard)


class AndroidVpnLifecycleGuardTests(unittest.TestCase):
    def setUp(self) -> None:
        self.sources = {}
        for relative in (guard.SERVICE, guard.LEASE, guard.ANDROID_HANDLE):
            self.sources[relative.as_posix()] = (ROOT / relative).read_text(encoding="utf-8")

    def test_production_vpn_tasks_use_the_bounded_lease(self) -> None:
        self.assertEqual(guard.source_problems_from_sources(self.sources), [])

    def test_start_task_must_wrap_work_with_the_bounded_lease(self) -> None:
        path = guard.SERVICE.as_posix()
        self.sources[path] = self.sources[path].replace("lifecycleWakeLock.withLease(", "", 1)
        self.assertTrue(
            any("StartTransactionTask must execute exactly once" in issue
                for issue in guard.source_problems_from_sources(self.sources))
        )

    def test_cleanup_task_must_release_admission_in_its_finally_path(self) -> None:
        path = guard.SERVICE.as_posix()
        self.sources[path] = self.sources[path].replace(
            "serviceAdmission.finishCleanup(request)",
            "serviceAdmission.finishStart(request.generation)",
            1,
        )
        self.assertTrue(
            any("CleanupTransactionTask must finish service admission in a finally path" in issue
                for issue in guard.source_problems_from_sources(self.sources))
        )

    def test_platform_adapter_must_keep_system_timeout_and_guard_release(self) -> None:
        path = guard.ANDROID_HANDLE.as_posix()
        self.sources[path] = self.sources[path].replace("wakeLock.acquire(timeoutMillis)", "wakeLock.acquire()")
        self.assertTrue(
            any("Android platform adapter must contain `wakeLock.acquire(timeoutMillis)`" in issue
                for issue in guard.source_problems_from_sources(self.sources))
        )


if __name__ == "__main__":
    unittest.main()
