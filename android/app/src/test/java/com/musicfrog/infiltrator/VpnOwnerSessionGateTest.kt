package com.musicfrog.infiltrator

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class VpnOwnerSessionGateTest {
    @Test
    fun preparingSessionPublishesVpnOwnerButKeepsCoreReadbackInactive() {
        val gate = VpnOwnerSessionGate<String>()
        gate.prepare("service-1", "publisher-1")

        assertEquals(VpnOwnerSession("service-1", "publisher-1"), gate.snapshotSession())
        assertNull(gate.activeSession())
        assertEquals(
            VpnOwnerSessionActivationOutcome.Activated,
            gate.activate("service-1"),
        )
        assertEquals(VpnOwnerSession("service-1", "publisher-1"), gate.activeSession())
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

        gate.prepare("service-2", "publisher-2")
        assertEquals(
            VpnOwnerSessionActivationOutcome.Activated,
            gate.activate("service-2"),
        )
        gate.end("service-2")
        assertNull(gate.activeSession())
    }
}
