package com.musicfrog.infiltrator.ui.proxies

import com.musicfrog.infiltrator.ui.common.CoreIdentityObservation
import com.musicfrog.infiltrator.ui.common.CoreScopeObservation
import com.musicfrog.infiltrator.ui.common.OutboundInventoryPageState
import com.musicfrog.infiltrator.ui.common.OutboundInventorySnapshot
import com.musicfrog.infiltrator.ui.common.OutboundInventoryState
import com.musicfrog.infiltrator.ui.common.OwnerErrorCode
import com.musicfrog.infiltrator.ui.common.OwnerFailure
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class ProxiesInventoryObservationTest {
    @Test
    fun unavailableRefreshKeepsLastAcceptedValueAndItsRevision() {
        val accepted = inventory(revision = "21")
        val failure = OwnerFailure(OwnerErrorCode.NotReady, "owner offline", true, null)

        val unavailable = retainLastAcceptedInventory(
            OutboundInventoryPageState.Unavailable(failure),
            accepted,
        ) as OutboundInventoryPageState.Unavailable

        assertEquals(accepted, unavailable.lastValue)
        assertEquals("21", unavailable.lastValue?.revision)
        assertEquals(failure, unavailable.failure)
    }

    @Test
    fun initialUnavailableRefreshLeavesLoadingAndHasNoInventedValue() {
        val failure = OwnerFailure(OwnerErrorCode.NotReady, "owner offline", true, null)

        val unavailable = retainLastAcceptedInventory(
            OutboundInventoryPageState.Unavailable(failure),
            lastAccepted = null,
        )

        assertTrue(unavailable is OutboundInventoryPageState.Unavailable)
        assertEquals(null, (unavailable as OutboundInventoryPageState.Unavailable).lastValue)
        assertEquals(failure, unavailable.failure)
    }

    @Test
    fun onlySameScopeNewerOrEqualInventoryRevisionsAreAccepted() {
        val accepted = inventory(revision = "21")

        assertEquals(
            OutboundInventoryAdmission.Accepted,
            classifyInventoryObservation(accepted, inventory(revision = "21")),
        )
        assertEquals(
            OutboundInventoryAdmission.Accepted,
            classifyInventoryObservation(accepted, inventory(revision = "22")),
        )
        assertEquals(
            OutboundInventoryAdmission.RejectedStale,
            classifyInventoryObservation(accepted, inventory(revision = "20")),
        )
    }

    @Test
    fun inventoryAdmissionRejectsOlderGenerationAndSameGenerationScopeChange() {
        val accepted = inventory(revision = "21", generation = "4")

        assertEquals(
            OutboundInventoryAdmission.Accepted,
            classifyInventoryObservation(accepted, inventory(revision = "1", generation = "5")),
        )
        assertEquals(
            OutboundInventoryAdmission.RejectedStale,
            classifyInventoryObservation(accepted, inventory(revision = "99", generation = "3")),
        )
        assertEquals(
            OutboundInventoryAdmission.RejectedStale,
            classifyInventoryObservation(
                accepted,
                inventory(revision = "99", identity = CoreIdentityObservation("mihomo", "other", "1.19.12")),
            ),
        )
    }

    @Test
    fun invalidRevisionOrGenerationIsRejectedWithoutChangingAdmissionSemantics() {
        val accepted = inventory(revision = "21")

        assertEquals(
            OutboundInventoryAdmission.RejectedStale,
            classifyInventoryObservation(accepted, inventory(revision = "invalid")),
        )
        assertEquals(
            OutboundInventoryAdmission.RejectedStale,
            classifyInventoryObservation(accepted, inventory(revision = "22", generation = "invalid")),
        )
    }

    private fun inventory(
        revision: String,
        generation: String = "4",
        identity: CoreIdentityObservation = CoreIdentityObservation("mihomo", "main", "1.19.12"),
    ) = OutboundInventorySnapshot(
        scope = CoreScopeObservation(
            identity = identity,
            generation = generation,
            sessionToken = null,
            json = "{}",
        ),
        revision = revision,
        inventory = OutboundInventoryState.Empty,
    )
}
