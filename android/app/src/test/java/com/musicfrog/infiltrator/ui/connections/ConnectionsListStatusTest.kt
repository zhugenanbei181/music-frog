package com.musicfrog.infiltrator.ui.connections

import com.musicfrog.infiltrator.ui.common.ConnectionRecord
import com.musicfrog.infiltrator.ui.common.AppFailure
import com.musicfrog.infiltrator.ui.common.FailureTextKey
import com.musicfrog.infiltrator.ui.common.ObservationRevision
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class ConnectionsListStatusTest {
    @Test
    fun aReadFailureRemainsUnavailableInsteadOfBecomingAnEmptyResult() {
        assertEquals(
            ConnectionsListStatus.Loading,
            connectionsListStatus(ConnectionReadState.Loading(), visibleConnectionCount = 0),
        )
        assertEquals(
            ConnectionsListStatus.Unavailable,
            connectionsListStatus(
                ConnectionReadState.Unavailable(null, null, failure()),
                visibleConnectionCount = 0,
            ),
        )
    }

    @Test
    fun onlyASuccessfulEmptySnapshotDisplaysTheNoConnectionsState() {
        val snapshot = ConnectionSnapshot(
            records = emptyList(),
            totalConnections = 0,
            uploadTotal = 0uL,
            downloadTotal = 0uL,
        )

        assertEquals(
            ConnectionsListStatus.Empty,
            connectionsListStatus(
                ConnectionReadState.Observed(snapshot, ObservationRevision.Owner(1uL, 1uL)),
                visibleConnectionCount = 0,
            ),
        )
    }

    @Test
    fun aFilterWithNoVisibleRowsDoesNotClaimThereAreNoActiveConnections() {
        val snapshot = ConnectionSnapshot(
            records = listOf(sampleConnection()),
            totalConnections = 1,
            uploadTotal = 10uL,
            downloadTotal = 20uL,
        )

        assertEquals(
            ConnectionsListStatus.NoMatches,
            connectionsListStatus(
                ConnectionReadState.Observed(snapshot, ObservationRevision.Owner(1uL, 1uL)),
                visibleConnectionCount = 0,
            ),
        )
        assertEquals(
            ConnectionsListStatus.Content,
            connectionsListStatus(
                ConnectionReadState.Observed(snapshot, ObservationRevision.Owner(1uL, 1uL)),
                visibleConnectionCount = 1,
            ),
        )
    }

    @Test
    fun unavailableReadRetainsLastAcceptedValueAndRevision() {
        val point = ConnectionObservationPoint(
            sampleSnapshot(),
            ObservationRevision.Owner(3uL, 9uL),
        )
        val unavailable = unavailableConnections(ConnectionReadState.Loading(point), failure())

        assertEquals(point.value, (unavailable as ConnectionReadState.Unavailable).lastValue)
        assertEquals(point.revision, unavailable.revision)
        assertEquals(failure(), unavailable.failure)
    }

    @Test
    fun initialReadFailureLeavesLoadingAndHasNoStaleValue() {
        val unavailable = unavailableConnections(ConnectionReadState.Loading(), failure())

        assertEquals(ConnectionsListStatus.Unavailable, connectionsListStatus(unavailable, 0))
        assertNull((unavailable as ConnectionReadState.Unavailable).lastValue)
    }

    @Test
    fun staleOwnerRevisionIsRejectedAndNewGenerationIsAccepted() {
        val observed = ConnectionReadState.Observed(
            sampleSnapshot(),
            ObservationRevision.Owner(4uL, 10uL),
        )

        assertFalse(acceptsConnectionRevision(observed, ObservationRevision.Owner(4uL, 9uL)))
        assertTrue(acceptsConnectionRevision(observed, ObservationRevision.Owner(5uL, 0uL)))
    }

    @Test
    fun closeReceiptNeedsMatchingOwnerReadback() {
        val snapshot = sampleSnapshot()

        assertEquals(
            ConnectionCloseReadback.StillActive,
            snapshot.checks(ConnectionCloseExpectation.TargetClosed("connection-1")),
        )
        assertEquals(
            ConnectionCloseReadback.StillActive,
            snapshot.checks(ConnectionCloseExpectation.AllClosed),
        )
        assertEquals(
            ConnectionCloseReadback.Confirmed,
            snapshot.copy(records = emptyList()).checks(ConnectionCloseExpectation.AllClosed),
        )
    }

    private fun sampleConnection() = ConnectionRecord(
        id = "connection-1",
        host = "example.com",
        processPath = "/system/bin/browser",
        network = "tcp",
        connectionType = null,
        rule = "MATCH",
        upload = 10uL,
        download = 20uL,
        chains = listOf("DIRECT"),
        destinationIpAsn = "",
        destinationGeoIp = null,
    )

    private fun sampleSnapshot() = ConnectionSnapshot(
        records = listOf(sampleConnection()),
        totalConnections = 1,
        uploadTotal = 10uL,
        downloadTotal = 20uL,
    )

    private fun failure() = AppFailure(
        code = com.musicfrog.infiltrator.ui.common.AppFailureCode.NotReady,
        localization = FailureTextKey.NotReady,
    )
}
