package com.musicfrog.infiltrator.ui.connections

import com.musicfrog.infiltrator.ui.common.AppFailure
import com.musicfrog.infiltrator.ui.common.ConnectionRecord
import com.musicfrog.infiltrator.ui.common.ObservationRevision

internal data class ConnectionSnapshot(
    val records: List<ConnectionRecord>,
    val totalConnections: Int,
    val uploadTotal: ULong,
    val downloadTotal: ULong,
)

internal data class ConnectionObservationPoint(
    val value: ConnectionSnapshot,
    val revision: ObservationRevision.Owner,
)

internal sealed interface ConnectionReadState {
    data class Loading(val lastAccepted: ConnectionObservationPoint? = null) : ConnectionReadState
    data class Observed(
        val value: ConnectionSnapshot,
        val revision: ObservationRevision.Owner,
    ) : ConnectionReadState
    data class Unavailable(
        val lastValue: ConnectionSnapshot?,
        val revision: ObservationRevision.Owner?,
        val failure: AppFailure,
    ) : ConnectionReadState
}

internal sealed interface ConnectionActionState {
    data object Idle : ConnectionActionState
    data object Refreshing : ConnectionActionState
    data object ClosingConnection : ConnectionActionState
    data object ClosingAll : ConnectionActionState
    data class Failed(val failure: AppFailure) : ConnectionActionState
}

internal sealed interface ConnectionCloseExpectation {
    data class TargetClosed(val id: String) : ConnectionCloseExpectation
    data object AllClosed : ConnectionCloseExpectation
}

internal enum class ConnectionCloseReadback {
    Confirmed,
    StillActive,
}

internal enum class ConnectionsListStatus {
    Loading,
    Unavailable,
    Empty,
    NoMatches,
    Content,
}

internal fun connectionsListStatus(
    readState: ConnectionReadState,
    visibleConnectionCount: Int,
): ConnectionsListStatus = when (readState) {
    is ConnectionReadState.Loading -> ConnectionsListStatus.Loading
    is ConnectionReadState.Unavailable -> ConnectionsListStatus.Unavailable
    is ConnectionReadState.Observed -> when {
        readState.value.records.isEmpty() -> ConnectionsListStatus.Empty
        visibleConnectionCount == 0 -> ConnectionsListStatus.NoMatches
        else -> ConnectionsListStatus.Content
    }
}

internal fun lastAcceptedConnection(
    state: ConnectionReadState,
): ConnectionObservationPoint? = when (state) {
    is ConnectionReadState.Loading -> state.lastAccepted
    is ConnectionReadState.Observed -> ConnectionObservationPoint(state.value, state.revision)
    is ConnectionReadState.Unavailable -> state.lastValue?.let { value ->
        state.revision?.let { revision -> ConnectionObservationPoint(value, revision) }
    }
}

internal fun unavailableConnections(
    previous: ConnectionReadState,
    failure: AppFailure,
): ConnectionReadState = lastAcceptedConnection(previous).let { accepted ->
    ConnectionReadState.Unavailable(accepted?.value, accepted?.revision, failure)
}

internal fun acceptsConnectionRevision(
    current: ConnectionReadState,
    incoming: ObservationRevision.Owner,
): Boolean {
    val previous = lastAcceptedConnection(current)?.revision ?: return true
    return incoming.generation > previous.generation ||
        (incoming.generation == previous.generation && incoming.revision >= previous.revision)
}

internal fun ConnectionSnapshot.checks(
    expectation: ConnectionCloseExpectation,
): ConnectionCloseReadback = when (expectation) {
    is ConnectionCloseExpectation.TargetClosed -> if (records.none { it.id == expectation.id }) {
        ConnectionCloseReadback.Confirmed
    } else {
        ConnectionCloseReadback.StillActive
    }
    ConnectionCloseExpectation.AllClosed -> if (records.isEmpty()) {
        ConnectionCloseReadback.Confirmed
    } else {
        ConnectionCloseReadback.StillActive
    }
}
