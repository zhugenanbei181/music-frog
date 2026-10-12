package com.musicfrog.infiltrator

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class VpnOwnerSessionGateTest {
    @Test
    fun preparingSessionPublishesVpnOwnerButKeepsCoreReadbackInactive() {
        val gate = VpnOwnerSessionGate<String>()
        assertEquals(VpnOwnerSessionReadiness.NotStarted, gate.readiness())
        gate.prepare("service-1", "publisher-1")

        assertEquals(VpnOwnerSession("service-1", "publisher-1"), gate.snapshotSession())
        assertNull(gate.activeSession())
        assertEquals(VpnOwnerSessionReadiness.Preparing("service-1"), gate.readiness())
        assertEquals(
            VpnOwnerSessionActivationOutcome.Activated,
            gate.activate("service-1"),
        )
        assertEquals(VpnOwnerSession("service-1", "publisher-1"), gate.activeSession())
        assertEquals(VpnOwnerSessionReadiness.Ready("service-1"), gate.readiness())
    }

    @Test
    fun staleActivationAndEndCannotChangeAnotherSession() {
        val gate = VpnOwnerSessionGate<String>()
        gate.prepare("service-current", "publisher-current")

        assertEquals(
            VpnOwnerSessionActivationOutcome.StaleSession,
            gate.activate("service-old"),
        )
        gate.end("service-old")
        assertEquals(
            VpnOwnerSession("service-current", "publisher-current"),
            gate.snapshotSession(),
        )
        assertEquals(
            VpnOwnerSessionActivationOutcome.Activated,
            gate.activate("service-current"),
        )
        assertEquals(
            VpnOwnerSessionActivationOutcome.AlreadyActive,
            gate.activate("service-current"),
        )
    }

    @Test
    fun endingPreparingOrActiveSessionMakesItUnavailable() {
        val gate = VpnOwnerSessionGate<String>()
        gate.prepare("service-1", "publisher-1")
        gate.end("service-1")
        assertNull(gate.snapshotSession())
        assertEquals(VpnOwnerSessionReadiness.Ended("service-1"), gate.readiness())

        gate.prepare("service-2", "publisher-2")
        assertEquals(
            VpnOwnerSessionActivationOutcome.Activated,
            gate.activate("service-2"),
        )
        gate.end("service-2")
        assertNull(gate.activeSession())
        assertEquals(VpnOwnerSessionReadiness.Ended("service-2"), gate.readiness())
    }

    @Test
    fun failedInitializationIsTypedAndCannotActivateOrReplaceANewerSession() {
        val gate = VpnOwnerSessionGate<String>()
        gate.prepare("service-failed", "publisher-failed")
        gate.fail("service-failed", "Core owner initialization failed")

        assertEquals(
            VpnOwnerSessionReadiness.Failed(
                "service-failed",
                "Core owner initialization failed",
            ),
            gate.readiness(),
        )
        assertNull(gate.activeSession())
        assertEquals(
            VpnOwnerSessionActivationOutcome.StaleSession,
            gate.activate("service-failed"),
        )

        gate.prepare("service-next", "publisher-next")
        gate.fail("service-failed", "stale failure must not replace the next session")
        assertEquals(
            VpnOwnerSessionReadiness.Preparing("service-next"),
            gate.readiness(),
        )
    }
}
