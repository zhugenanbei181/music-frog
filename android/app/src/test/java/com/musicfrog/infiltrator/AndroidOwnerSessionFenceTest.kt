package com.musicfrog.infiltrator

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class AndroidOwnerSessionFenceTest {
    @Test
    fun oneOwnerSnapshotAdoptsVpnAndCoreBeforeCommandsAndFencesRetiredBinderReplies() {
        val fence = AndroidOwnerSessionFence()
        assertEquals(
            OwnerSessionAcceptance.FirstObserved,
            fence.accept(
                expectedSessionId = null,
                responseSessionId = "owner-a",
                requestId = 1L,
                evidence = OwnerSessionEvidence.VpnOwnerSnapshotReadback,
            ),
        )
        assertEquals(OwnerSessionReadiness.Ready, fence.readiness())
        assertEquals("owner-a", fence.current())

        fence.requireReadback(2L, "owner-a", OwnerClientTerminal.BinderDied)
        assertEquals(
            OwnerSessionAcceptance.Changed,
            fence.accept(
                expectedSessionId = "owner-a",
                responseSessionId = "owner-b",
                requestId = 3L,
                evidence = OwnerSessionEvidence.VpnOwnerSnapshotReadback,
            ),
        )
        assertEquals(OwnerSessionReadiness.Ready, fence.readiness())
        assertEquals("owner-b", fence.current())
        assertEquals(
            OwnerSessionAcceptance.StaleResponse,
            fence.accept(
                expectedSessionId = "owner-b",
                responseSessionId = "owner-a",
                requestId = 4L,
                evidence = OwnerSessionEvidence.VpnOwnerSnapshotReadback,
            ),
        )
    }

    @Test
    fun firstOwnerMustBeAdoptedByCoreAndVpnReadbacksBeforeItIsReady() {
        val fence = AndroidOwnerSessionFence()

        assertEquals(OwnerSessionReadiness.ReadbackRequired, fence.readiness())
        assertEquals(
            OwnerSessionAcceptance.ReadbackRequired,
            fence.accept(
                expectedSessionId = null,
                responseSessionId = "owner-a",
                requestId = 1,
                evidence = OwnerSessionEvidence.OperationReply,
            ),
        )
        assertNull(fence.current())

        assertEquals(
            OwnerSessionAcceptance.FirstObserved,
            fence.accept(
                expectedSessionId = null,
                responseSessionId = "owner-a",
                requestId = 2,
                evidence = OwnerSessionEvidence.CoreLifecycleReadback,
            ),
        )
        assertEquals("owner-a", fence.current())
        assertEquals(OwnerSessionReadiness.ReadbackRequired, fence.readiness())
        assertEquals(
            OwnerSessionAcceptance.Current,
            fence.accept(
                expectedSessionId = "owner-a",
                responseSessionId = "owner-a",
                requestId = 3,
                evidence = OwnerSessionEvidence.VpnStateReadback,
            ),
        )
        assertEquals(OwnerSessionReadiness.Ready, fence.readiness())
    }

    @Test
    fun typedVpnStopReadbackStillRequiresCoreAndVpnAdoptionAfterRestart() {
        val fence = AndroidOwnerSessionFence()

        assertEquals(
            OwnerSessionAcceptance.FirstObserved,
            fence.accept(
                expectedSessionId = null,
                responseSessionId = "owner-vpn",
                requestId = 1,
                evidence = OwnerSessionEvidence.VpnStopReadback,
            ),
        )
        assertEquals("owner-vpn", fence.current())
        assertEquals(OwnerSessionReadiness.ReadbackRequired, fence.readiness())
        assertEquals(
            OwnerSessionAcceptance.Current,
            fence.accept(
                expectedSessionId = "owner-vpn",
                responseSessionId = "owner-vpn",
                requestId = 2,
                evidence = OwnerSessionEvidence.CoreLifecycleReadback,
            ),
        )
        assertEquals(OwnerSessionReadiness.Ready, fence.readiness())

        fence.requireReadback(3, "owner-vpn", OwnerClientTerminal.BinderDied)
        assertEquals(
            OwnerSessionAcceptance.Changed,
            fence.accept(
                expectedSessionId = "owner-vpn",
                responseSessionId = "owner-recreated",
                requestId = 4,
                evidence = OwnerSessionEvidence.VpnStopReadback,
            ),
        )
        assertEquals("owner-recreated", fence.current())
        assertEquals(OwnerSessionReadiness.ReadbackRequired, fence.readiness())
        assertEquals(
            OwnerSessionAcceptance.Current,
            fence.accept(
                expectedSessionId = "owner-recreated",
                responseSessionId = "owner-recreated",
                requestId = 5,
                evidence = OwnerSessionEvidence.CoreLifecycleReadback,
            ),
        )
        assertEquals(OwnerSessionReadiness.Ready, fence.readiness())
    }

    @Test
    fun binderDeathCreatesTypedReadbackBarrierBeforeCommandsCanResume() {
        val fence = AndroidOwnerSessionFence()
        adopt(fence, "owner-a", 1)

        fence.requireReadback(3, "owner-a", OwnerClientTerminal.BinderDied)

        val state = fence.state() as OwnerSessionFenceState.ReadbackRequired
        assertEquals("owner-a", state.ownerSessionId)
        assertEquals(3L, state.requestId)
        assertEquals(OwnerClientTerminal.BinderDied, state.terminal)
        assertEquals(OwnerSessionReadiness.ReadbackRequired, fence.readiness())
        assertEquals(
            OwnerSessionAcceptance.ReadbackRequired,
            fence.accept(
                expectedSessionId = "owner-a",
                responseSessionId = "owner-a",
                requestId = 4,
                evidence = OwnerSessionEvidence.OperationReply,
            ),
        )
        assertEquals(OwnerSessionReadiness.ReadbackRequired, fence.readiness())

        assertEquals(
            OwnerSessionAcceptance.Current,
            fence.accept(
                expectedSessionId = "owner-a",
                responseSessionId = "owner-a",
                requestId = 5,
                evidence = OwnerSessionEvidence.CoreLifecycleReadback,
            ),
        )
        assertEquals(OwnerSessionReadiness.ReadbackRequired, fence.readiness())
        assertEquals(
            OwnerSessionAcceptance.Current,
            fence.accept(
                expectedSessionId = "owner-a",
                responseSessionId = "owner-a",
                requestId = 6,
                evidence = OwnerSessionEvidence.VpnStateReadback,
            ),
        )
        assertEquals(OwnerSessionReadiness.Ready, fence.readiness())
    }

    @Test
    fun serviceRestartIsAdoptedOnlyAfterNewOwnerCoreAndVpnReadbacks() {
        val fence = AndroidOwnerSessionFence()
        adopt(fence, "owner-a", 1)
        fence.requireReadback(3, "owner-a", OwnerClientTerminal.BindingDied)

        assertEquals(
            OwnerSessionAcceptance.ReadbackRequired,
            fence.accept(
                expectedSessionId = "owner-a",
                responseSessionId = "owner-b",
                requestId = 4,
                evidence = OwnerSessionEvidence.OperationReply,
            ),
        )
        assertEquals("owner-a", fence.current())
        assertEquals(OwnerSessionReadiness.ReadbackRequired, fence.readiness())

        assertEquals(
            OwnerSessionAcceptance.Changed,
            fence.accept(
                expectedSessionId = "owner-a",
                responseSessionId = "owner-b",
                requestId = 5,
                evidence = OwnerSessionEvidence.CoreLifecycleReadback,
            ),
        )
        assertEquals("owner-b", fence.current())
        assertEquals(OwnerSessionReadiness.ReadbackRequired, fence.readiness())
        assertEquals(
            OwnerSessionAcceptance.Current,
            fence.accept(
                expectedSessionId = "owner-b",
                responseSessionId = "owner-b",
                requestId = 6,
                evidence = OwnerSessionEvidence.VpnStateReadback,
            ),
        )
        assertEquals(OwnerSessionReadiness.Ready, fence.readiness())
    }

    @Test
    fun restartDetectedByOldSessionRequestAdoptsOnlyExplicitLifecycleReadback() {
        val fence = AndroidOwnerSessionFence()
        adopt(fence, "owner-a", 1)

        assertEquals(
            OwnerSessionAcceptance.Changed,
            fence.accept(
                expectedSessionId = "owner-a",
                responseSessionId = "owner-b",
                requestId = 3,
                evidence = OwnerSessionEvidence.CoreLifecycleReadback,
            ),
        )
        assertEquals("owner-b", fence.current())
        assertEquals(OwnerSessionReadiness.ReadbackRequired, fence.readiness())
        assertEquals(
            OwnerSessionAcceptance.Current,
            fence.accept(
                expectedSessionId = "owner-b",
                responseSessionId = "owner-b",
                requestId = 4,
                evidence = OwnerSessionEvidence.VpnStateReadback,
            ),
        )
        assertEquals(OwnerSessionReadiness.Ready, fence.readiness())
    }

    @Test
    fun restartBetweenCoreAndVpnReadbacksRequiresBothFactsFromTheNewestOwner() {
        val fence = AndroidOwnerSessionFence()
        adopt(fence, "owner-a", 1)
        fence.requireReadback(3, "owner-a", OwnerClientTerminal.BinderDied)

        assertEquals(
            OwnerSessionAcceptance.Changed,
            fence.accept(
                expectedSessionId = "owner-a",
                responseSessionId = "owner-b",
                requestId = 4,
                evidence = OwnerSessionEvidence.CoreLifecycleReadback,
            ),
        )
        assertEquals(OwnerSessionReadiness.ReadbackRequired, fence.readiness())
        assertEquals(
            OwnerSessionAcceptance.Changed,
            fence.accept(
                expectedSessionId = "owner-b",
                responseSessionId = "owner-c",
                requestId = 5,
                evidence = OwnerSessionEvidence.VpnStateReadback,
            ),
        )
        assertEquals("owner-c", fence.current())
        assertEquals(OwnerSessionReadiness.ReadbackRequired, fence.readiness())
        assertEquals(
            OwnerSessionAcceptance.Current,
            fence.accept(
                expectedSessionId = "owner-c",
                responseSessionId = "owner-c",
                requestId = 6,
                evidence = OwnerSessionEvidence.CoreLifecycleReadback,
            ),
        )
        assertEquals(OwnerSessionReadiness.Ready, fence.readiness())
    }

    @Test
    fun oldSessionRepliesCannotRollBackAfterRebindOrArriveOutOfOrder() {
        val fence = AndroidOwnerSessionFence()
        adopt(fence, "owner-a", 1)
        fence.requireReadback(3, "owner-a", OwnerClientTerminal.BinderDied)
        fence.accept(
            expectedSessionId = "owner-a",
            responseSessionId = "owner-b",
            requestId = 4,
            evidence = OwnerSessionEvidence.CoreLifecycleReadback,
        )
        fence.accept(
            expectedSessionId = "owner-b",
            responseSessionId = "owner-b",
            requestId = 5,
            evidence = OwnerSessionEvidence.VpnStateReadback,
        )

        assertEquals(
            OwnerSessionAcceptance.StaleResponse,
            fence.accept(
                expectedSessionId = "owner-a",
                responseSessionId = "owner-a",
                requestId = 3,
                evidence = OwnerSessionEvidence.CoreLifecycleReadback,
            ),
        )
        assertEquals(
            OwnerSessionAcceptance.StaleResponse,
            fence.accept(
                expectedSessionId = "owner-b",
                responseSessionId = "owner-a",
                requestId = 6,
                evidence = OwnerSessionEvidence.CoreLifecycleReadback,
            ),
        )
        assertEquals("owner-b", fence.current())
    }

    @Test
    fun aTimedOutReadbackCannotAdoptItsLateReply() {
        val fence = AndroidOwnerSessionFence()
        adopt(fence, "owner-a", 1)
        fence.requireReadback(3, "owner-a", OwnerClientTerminal.ReplyTimedOut)

        assertEquals(
            OwnerSessionAcceptance.StaleResponse,
            fence.accept(
                expectedSessionId = "owner-a",
                responseSessionId = "owner-a",
                requestId = 3,
                evidence = OwnerSessionEvidence.CoreLifecycleReadback,
            ),
        )
        assertEquals(OwnerSessionReadiness.ReadbackRequired, fence.readiness())
    }

    @Test
    fun lateDeathFromRetiredBindingCannotInvalidateTheReboundOwner() {
        val fence = AndroidOwnerSessionFence()
        adopt(fence, "owner-a", 1)
        fence.requireReadback(3, "owner-a", OwnerClientTerminal.BinderDied)
        fence.accept(
            expectedSessionId = "owner-a",
            responseSessionId = "owner-b",
            requestId = 4,
            evidence = OwnerSessionEvidence.CoreLifecycleReadback,
        )
        fence.accept(
            expectedSessionId = "owner-b",
            responseSessionId = "owner-b",
            requestId = 5,
            evidence = OwnerSessionEvidence.VpnStateReadback,
        )

        assertEquals(
            OwnerSessionInvalidation.RetiredBinding,
            fence.requireReadback(3, "owner-a", OwnerClientTerminal.BinderDied),
        )
        assertEquals("owner-b", fence.current())
        assertEquals(OwnerSessionReadiness.Ready, fence.readiness())
    }

    private fun adopt(fence: AndroidOwnerSessionFence, sessionId: String, requestId: Long) {
        assertEquals(
            OwnerSessionAcceptance.FirstObserved,
            fence.accept(
                expectedSessionId = null,
                responseSessionId = sessionId,
                requestId = requestId,
                evidence = OwnerSessionEvidence.CoreLifecycleReadback,
            ),
        )
        assertEquals(
            OwnerSessionAcceptance.Current,
            fence.accept(
                expectedSessionId = sessionId,
                responseSessionId = sessionId,
                requestId = requestId + 1L,
                evidence = OwnerSessionEvidence.VpnStateReadback,
            ),
        )
    }
}
