package com.musicfrog.infiltrator.ui.logs

import com.musicfrog.infiltrator.ui.common.AppFailure
import com.musicfrog.infiltrator.ui.common.CoreScopeObservation
import com.musicfrog.infiltrator.ui.common.ObservationRevision
import com.musicfrog.infiltrator.ui.common.OwnerLogsPage
import com.musicfrog.infiltrator.ui.common.OwnerPage

internal data class LogsObservationPoint(
    val logs: OwnerLogsPage,
    val revision: ObservationRevision.Owner,
    val coreScope: CoreScopeObservation?,
)

internal sealed interface LogsObservationState {
    data class Loading(
        val lastAccepted: LogsObservationPoint?,
        val revision: ObservationRevision.Owner?,
    ) : LogsObservationState

    data class Content(val point: LogsObservationPoint) : LogsObservationState

    data class Unavailable(
        val lastAccepted: LogsObservationPoint?,
        val revision: ObservationRevision.Owner?,
        val failure: AppFailure,
    ) : LogsObservationState

    data class Cancelled(
        val lastAccepted: LogsObservationPoint?,
        val revision: ObservationRevision.Owner?,
    ) : LogsObservationState
}

internal sealed interface LogsOperationState {
    data object Idle : LogsOperationState
    data object Clearing : LogsOperationState
    data class Cleared(val revision: ObservationRevision.Owner) : LogsOperationState
    data class Failed(val failure: AppFailure) : LogsOperationState
    data object Cancelled : LogsOperationState
}

internal sealed interface LogsClearResult {
    data object Accepted : LogsClearResult
    data class Failed(val failure: AppFailure) : LogsClearResult
}

internal enum class LogsClearReadback {
    Empty,
    EntriesRemain,
}

internal enum class LogsStreamState {
    Starting,
    Streaming,
    Paused,
    Reconnecting,
    Failed,
    Unavailable,
}

internal sealed interface LogsIntent {
    data object PageVisible : LogsIntent
    data object PageHidden : LogsIntent
    data object Refresh : LogsIntent
    data object Clear : LogsIntent
}

internal enum class LogsPageVisibility {
    Visible,
    Hidden,
}

internal enum class LogsPollingStart {
    Immediately,
    AfterInterval,
}

internal data class LogsUiState(
    val observation: LogsObservationState = LogsObservationState.Loading(
        lastAccepted = null,
        revision = null,
    ),
    val operation: LogsOperationState = LogsOperationState.Idle,
)

internal sealed interface LogsSnapshotRead {
    data class OwnerSnapshot(
        val coreScope: CoreScopeObservation?,
        val revision: ObservationRevision.Owner,
        val page: OwnerPage<OwnerLogsPage>,
    ) : LogsSnapshotRead

    data class Unavailable(val failure: AppFailure) : LogsSnapshotRead
}

internal fun LogsObservationState.lastAccepted(): LogsObservationPoint? = when (this) {
    is LogsObservationState.Loading -> lastAccepted
    is LogsObservationState.Content -> point
    is LogsObservationState.Unavailable -> lastAccepted
    is LogsObservationState.Cancelled -> lastAccepted
}

internal fun acceptsLogsRevision(
    current: ObservationRevision.Owner?,
    incoming: ObservationRevision.Owner,
): Boolean {
    if (current == null) return true
    return incoming.generation > current.generation ||
        (incoming.generation == current.generation && incoming.revision >= current.revision)
}

internal fun OwnerLogsPage.clearReadback(): LogsClearReadback = when {
    entries.isEmpty() -> LogsClearReadback.Empty
    else -> LogsClearReadback.EntriesRemain
}
