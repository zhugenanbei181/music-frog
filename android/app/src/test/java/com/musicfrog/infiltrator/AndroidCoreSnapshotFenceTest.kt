package com.musicfrog.infiltrator

import org.junit.Assert.assertEquals
import org.junit.Test

class AndroidCoreSnapshotFenceTest {
    @Test
    fun ownerSnapshotsMayAdvanceRevisionWithinTheSameCoreSession() {
        val fence = AndroidCoreSnapshotFence()
        assertEquals(
            CoreSnapshotAcceptance.Accepted,
            fence.admit(CoreSnapshotVersion(4, 8, "session-a")),
        )
        assertEquals(
            CoreSnapshotAcceptance.Accepted,
            fence.admit(CoreSnapshotVersion(4, 9, "session-a")),
        )
    }

    @Test
    fun delayedSnapshotsCannotMoveGenerationRevisionOrSessionBackward() {
        val fence = AndroidCoreSnapshotFence()
        fence.admit(CoreSnapshotVersion(4, 9, "session-a"))

        assertEquals(
            CoreSnapshotAcceptance.Stale,
            fence.admit(CoreSnapshotVersion(3, 20, "session-old")),
        )
        assertEquals(
            CoreSnapshotAcceptance.Stale,
            fence.admit(CoreSnapshotVersion(4, 8, "session-a")),
        )
        assertEquals(
            CoreSnapshotAcceptance.Stale,
            fence.admit(CoreSnapshotVersion(4, 10, "session-b")),
        )
    }

    @Test
    fun equalRevisionMayRepeatTheSameSnapshotButCannotReplaceItsState() {
        val fence = AndroidCoreSnapshotFence()
        val current = CoreSnapshotVersion(
            generation = 4,
            revision = 9,
            sessionToken = "session-a",
            lifecycle = CoreOwnerLifecycleWire.Running,
            failure = null,
        )
        assertEquals(CoreSnapshotAcceptance.Accepted, fence.admit(current))
        assertEquals(CoreSnapshotAcceptance.Accepted, fence.admit(current.copy()))

        assertEquals(
            CoreSnapshotAcceptance.Stale,
            fence.admit(current.copy(lifecycle = CoreOwnerLifecycleWire.Failed)),
        )
        assertEquals(
            CoreSnapshotAcceptance.Stale,
            fence.admit(
                current.copy(
                    failure = CoreOwnerFailureWire(
                        CoreOwnerFailureCodeWire.NotReady,
                        "readiness failed",
                        true,
                        null,
                    ),
                ),
            ),
        )
    }

    @Test
    fun aNewerCoreGenerationStartsANewSnapshotSequence() {
        val fence = AndroidCoreSnapshotFence()
        fence.admit(CoreSnapshotVersion(4, 20, "session-a"))

        assertEquals(
            CoreSnapshotAcceptance.Accepted,
            fence.admit(CoreSnapshotVersion(5, 1, "session-b")),
        )
    }

    @Test
    fun engineIdentityChangesRequireANewerGeneration() {
        val fence = AndroidCoreSnapshotFence()
        fence.admit(
            CoreSnapshotVersion(
                4,
                20,
                "session-a",
                identity = CoreOwnerIdentityWire.Present("mihomo", "android-active", null),
            ),
        )

        assertEquals(
            CoreSnapshotAcceptance.Stale,
            fence.admit(
                CoreSnapshotVersion(
                    4,
                    21,
                    "session-b",
                    identity = CoreOwnerIdentityWire.Present("sing-box", "android-active", null),
                ),
            ),
        )
        assertEquals(
            CoreSnapshotAcceptance.Accepted,
            fence.admit(
                CoreSnapshotVersion(
                    5,
                    1,
                    "session-b",
                    identity = CoreOwnerIdentityWire.Present("sing-box", "android-active", null),
                ),
            ),
        )
    }

    @Test
    fun coreVersionChangesRequireANewerGeneration() {
        val fence = AndroidCoreSnapshotFence()
        fence.admit(
            CoreSnapshotVersion(
                4,
                20,
                "session-a",
                identity = CoreOwnerIdentityWire.Present("mihomo", "android-active", "1.19.12"),
            ),
        )

        assertEquals(
            CoreSnapshotAcceptance.Stale,
            fence.admit(
                CoreSnapshotVersion(
                    4,
                    21,
                    "session-b",
                    identity = CoreOwnerIdentityWire.Present("mihomo", "android-active", "1.20.0"),
                ),
            ),
        )
        assertEquals(
            CoreSnapshotAcceptance.Accepted,
            fence.admit(
                CoreSnapshotVersion(
                    5,
                    1,
                    "session-b",
                    identity = CoreOwnerIdentityWire.Present("mihomo", "android-active", "1.20.0"),
                ),
            ),
        )
    }

    @Test
    fun ownerRestartCanResetTheFenceBeforeReadingTheNewSession() {
        val fence = AndroidCoreSnapshotFence()
        fence.admit(CoreSnapshotVersion(9, 40, "old-owner"))
        fence.reset()

        assertEquals(
            CoreSnapshotAcceptance.Accepted,
            fence.admit(CoreSnapshotVersion(0, 1, "new-owner")),
        )
    }
}
