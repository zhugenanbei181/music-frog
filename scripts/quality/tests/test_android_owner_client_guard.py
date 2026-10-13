from __future__ import annotations

import importlib.util
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[3]
SCRIPT = ROOT / "scripts/quality/android-owner-client-guard.py"
SPEC = importlib.util.spec_from_file_location("android_owner_client_guard", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
guard = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(guard)


class AndroidOwnerClientGuardTests(unittest.TestCase):
    def test_current_android_sources_have_one_application_owned_client(self) -> None:
        self.assertEqual(guard.source_problems(), [])

    def test_guard_rejects_second_constructor_call(self) -> None:
        sources = {
            guard.OWNER_SOURCE: (
                "internal val ownerIpcClient: MihomoControlClient by ownerIpcClientDelegate\n"
                "MihomoControlClient(applicationContext)"
            ),
            guard.SHARED_CLIENT_CONSUMERS[0]: "application.ownerIpcClient",
            guard.SHARED_CLIENT_CONSUMERS[1]: "application.ownerIpcClient",
            guard.SHARED_CLIENT_CONSUMERS[2]: "application.ownerIpcClient\nMihomoControlClient(context)",
            "MihomoControlClient.kt": "class MihomoControlClient(context: Context) {}",
        }

        self.assertTrue(
            any(
                "exactly one MihomoControlClient constructor" in problem
                for problem in guard.source_problems_from_sources(sources)
            )
        )

    def test_guard_rejects_consumer_that_stops_using_shared_client(self) -> None:
        sources = {
            guard.OWNER_SOURCE: (
                "internal val ownerIpcClient: MihomoControlClient by ownerIpcClientDelegate\n"
                "MihomoControlClient(applicationContext)"
            ),
            guard.SHARED_CLIENT_CONSUMERS[0]: "application.ownerIpcClient",
            guard.SHARED_CLIENT_CONSUMERS[1]: "MihomoControlClient(context).readSnapshot()",
            guard.SHARED_CLIENT_CONSUMERS[2]: "application.ownerIpcClient",
            "MihomoControlClient.kt": "class MihomoControlClient(context: Context) {}",
        }

        self.assertTrue(
            any(
                guard.SHARED_CLIENT_CONSUMERS[1] in problem
                for problem in guard.source_problems_from_sources(sources)
            )
        )

    def test_guard_rejects_surface_reader_that_bypasses_owner_snapshot_ffi(self) -> None:
        sources = {
            guard.OWNER_SOURCE: (
                "internal val ownerIpcClient: MihomoControlClient by ownerIpcClientDelegate\n"
                "MihomoControlClient(applicationContext)"
            ),
            guard.SHARED_CLIENT_CONSUMERS[0]: "application.ownerIpcClient",
            guard.SHARED_CLIENT_CONSUMERS[1]: "application.ownerIpcClient",
            guard.SHARED_CLIENT_CONSUMERS[2]: "application.ownerIpcClient",
            "MihomoControlClient.kt": "class MihomoControlClient(context: Context) {}",
        }
        for path, fragments in guard.SNAPSHOT_ROUTE_REQUIREMENTS.items():
            sources[path] = "\n".join(fragments)
        sources["android/app/src/main/java/com/musicfrog/infiltrator/ui/common/OwnerSurfaceSnapshotReader.kt"] = (
            "localLifecycleRegistry.readSnapshot()"
        )

        self.assertTrue(
            any(
                "OwnerSurfaceSnapshotReader.kt" in problem
                and "canonical owner SurfaceSnapshot route" in problem
                for problem in guard.source_problems_from_sources(sources)
            )
        )


if __name__ == "__main__":
    unittest.main()
