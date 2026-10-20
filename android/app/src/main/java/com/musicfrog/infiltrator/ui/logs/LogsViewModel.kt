package com.musicfrog.infiltrator.ui.logs

import androidx.lifecycle.ViewModel
import com.musicfrog.infiltrator.ui.common.AppFailure
import com.musicfrog.infiltrator.ui.common.CallResult
import com.musicfrog.infiltrator.ui.common.DEFAULT_FFI_TIMEOUT_MS
import com.musicfrog.infiltrator.ui.common.FailureTextKey
import com.musicfrog.infiltrator.ui.common.LONG_FFI_TIMEOUT_MS
import com.musicfrog.infiltrator.ui.common.ObservationRevision
import com.musicfrog.infiltrator.ui.common.OwnerPageState
import com.musicfrog.infiltrator.ui.common.localizedFailure
import com.musicfrog.infiltrator.ui.common.runCall
import com.musicfrog.infiltrator.ui.common.toAppFailure
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch

internal const val DEFAULT_LOGS_POLL_INTERVAL_MS = 2_000L

internal class LogsViewModel internal constructor(
    private val repository: LogsRepository,
    private val pollingIntervalMs: Long = DEFAULT_LOGS_POLL_INTERVAL_MS,
    private val coroutineScope: CoroutineScope,
) : ViewModel() {
    constructor() : this(
        repository = UniFfiLogsRepository(),
        pollingIntervalMs = DEFAULT_LOGS_POLL_INTERVAL_MS,
        coroutineScope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate),
    )

    private val _state = MutableStateFlow(LogsUiState())
    val state: StateFlow<LogsUiState> = _state.asStateFlow()

    private var pageVisibility = LogsPageVisibility.Hidden
    private var pollingJob: Job? = null
    private val activeReadJobs = mutableSetOf<Job>()
    private var readRequestRevision = ObservationRevision.Request(0uL)
    private var lastAcceptedRevision: ObservationRevision.Owner? = null
    private var clearOperationSequence = 0uL

    internal fun dispatch(intent: LogsIntent) {
        when (intent) {
            LogsIntent.PageVisible -> showPage()
            LogsIntent.PageHidden -> hidePage()
            LogsIntent.Refresh -> requestRefresh()
            LogsIntent.Clear -> requestClear()
        }
    }

    private fun showPage() {
        if (pageVisibility == LogsPageVisibility.Visible) return
        pageVisibility = LogsPageVisibility.Visible
        if (_state.value.observation is LogsObservationState.Cancelled) {
            _state.value = _state.value.copy(
                observation = LogsObservationState.Loading(
                    lastAccepted = _state.value.observation.lastAccepted(),
                    revision = lastAcceptedRevision,
                ),
            )
        }
        startPolling()
    }

    private fun hidePage() {
        if (pageVisibility == LogsPageVisibility.Hidden) return
        pageVisibility = LogsPageVisibility.Hidden
        cancelReads()
        _state.value = _state.value.copy(
            observation = LogsObservationState.Cancelled(
                lastAccepted = _state.value.observation.lastAccepted(),
                revision = lastAcceptedRevision,
            ),
        )
    }

    private fun requestRefresh() {
        if (pageVisibility != LogsPageVisibility.Visible) return
        if (_state.value.operation is LogsOperationState.Clearing) return
        val job = coroutineScope.launch(start = CoroutineStart.LAZY) {
            refreshSnapshot()
        }
        activeReadJobs += job
        job.invokeOnCompletion { activeReadJobs -= job }
        job.start()
    }

    private fun requestClear() {
        if (pageVisibility != LogsPageVisibility.Visible) return
        if (_state.value.operation is LogsOperationState.Clearing) return

        val coreScope = _state.value.observation.lastAccepted()?.coreScope
        if (coreScope == null) {
            _state.value = _state.value.copy(operation = LogsOperationState.Failed(logsNotReadyFailure()))
            return
        }

        cancelReads()
        clearOperationSequence += 1uL
        val operationSequence = clearOperationSequence
        _state.value = _state.value.copy(operation = LogsOperationState.Clearing)
        coroutineScope.launch {
            try {
                when (val call = runCall(LONG_FFI_TIMEOUT_MS) { repository.clearLogs(coreScope) }) {
                    is CallResult.Failure -> publishClearFailure(operationSequence, call.failure)

                    is CallResult.Success -> when (val result = call.value) {
                        LogsClearResult.Accepted -> confirmClear(operationSequence)
                        is LogsClearResult.Failed -> publishClearFailure(operationSequence, result.failure)
                    }
                }
            } catch (error: CancellationException) {
                if (operationSequence == clearOperationSequence) {
                    _state.value = _state.value.copy(operation = LogsOperationState.Cancelled)
                }
                throw error
            } catch (error: Exception) {
                publishClearFailure(operationSequence, error.toAppFailure())
            } finally {
                if (operationSequence == clearOperationSequence &&
                    pageVisibility == LogsPageVisibility.Visible &&
                    _state.value.operation !is LogsOperationState.Clearing
                ) {
                    startPolling(LogsPollingStart.AfterInterval)
                }
            }
        }
    }

    private suspend fun confirmClear(operationSequence: ULong) {
        when (val readback = refreshSnapshot()) {
            is LogsRefreshResult.Accepted -> {
                val operation = when (readback.point.logs.clearReadback()) {
                    LogsClearReadback.Empty -> LogsOperationState.Cleared(readback.point.revision)
                    LogsClearReadback.EntriesRemain -> LogsOperationState.Failed(logsNotReadyFailure())
                }
                if (operationSequence == clearOperationSequence) {
                    _state.value = _state.value.copy(operation = operation)
                }
            }

            is LogsRefreshResult.Unavailable -> publishClearFailure(operationSequence, readback.failure)

            LogsRefreshResult.Pending,
            LogsRefreshResult.Superseded,
            -> publishClearFailure(operationSequence, logsNotReadyFailure())
        }
    }

    private fun publishClearFailure(sequence: ULong, failure: AppFailure) {
        if (sequence == clearOperationSequence) {
            _state.value = _state.value.copy(operation = LogsOperationState.Failed(failure))
        }
    }

    private fun startPolling(start: LogsPollingStart = LogsPollingStart.Immediately) {
        if (pageVisibility != LogsPageVisibility.Visible) return
        if (pollingJob != null) return
        if (_state.value.operation is LogsOperationState.Clearing) return
        pollingJob = coroutineScope.launch {
            if (start == LogsPollingStart.AfterInterval) delay(pollingIntervalMs)
            while (isActive && pageVisibility == LogsPageVisibility.Visible) {
                refreshSnapshot()
                delay(pollingIntervalMs)
            }
        }
    }

    private fun cancelReads() {
        advanceReadRequestRevision()
        pollingJob?.cancel()
        pollingJob = null
        activeReadJobs.toList().forEach { it.cancel() }
        activeReadJobs.clear()
    }

    private suspend fun refreshSnapshot(): LogsRefreshResult {
        val requestRevision = advanceReadRequestRevision()
        val previousPoint = _state.value.observation.lastAccepted()
        if (previousPoint == null) {
            _state.value = _state.value.copy(
                observation = LogsObservationState.Loading(
                    lastAccepted = null,
                    revision = lastAcceptedRevision,
                ),
            )
        }

        return try {
            when (val call = runCall(DEFAULT_FFI_TIMEOUT_MS) { repository.readSnapshot() }) {
                is CallResult.Failure -> if (requestRevision == readRequestRevision) {
                    publishUnavailable(call.failure)
                    LogsRefreshResult.Unavailable(call.failure)
                } else {
                    LogsRefreshResult.Superseded
                }

                is CallResult.Success -> acceptRead(requestRevision, call.value)
            }
        } catch (error: CancellationException) {
            if (requestRevision == readRequestRevision) {
                _state.value = _state.value.copy(
                    observation = LogsObservationState.Cancelled(
                        lastAccepted = _state.value.observation.lastAccepted(),
                        revision = lastAcceptedRevision,
                    ),
                )
            }
            throw error
        }
    }

    private fun acceptRead(requestRevision: ObservationRevision.Request, read: LogsSnapshotRead): LogsRefreshResult {
        if (requestRevision != readRequestRevision) return LogsRefreshResult.Superseded
        return when (read) {
            is LogsSnapshotRead.Unavailable -> {
                publishUnavailable(read.failure)
                LogsRefreshResult.Unavailable(read.failure)
            }

            is LogsSnapshotRead.OwnerSnapshot -> {
                if (!acceptsLogsRevision(lastAcceptedRevision, read.revision)) {
                    return LogsRefreshResult.Superseded
                }
                lastAcceptedRevision = read.revision
                when (read.page.state) {
                    OwnerPageState.Ready,
                    OwnerPageState.Empty,
                    -> {
                        val logs = read.page.data
                        if (logs == null) {
                            val failure = logsNotReadyFailure()
                            publishUnavailable(failure)
                            LogsRefreshResult.Unavailable(failure)
                        } else {
                            val point = LogsObservationPoint(logs, read.revision, read.coreScope)
                            _state.value = _state.value.copy(
                                observation = LogsObservationState.Content(point),
                            )
                            LogsRefreshResult.Accepted(point)
                        }
                    }

                    OwnerPageState.Loading -> {
                        _state.value = _state.value.copy(
                            observation = LogsObservationState.Loading(
                                lastAccepted = _state.value.observation.lastAccepted(),
                                revision = read.revision,
                            ),
                        )
                        LogsRefreshResult.Pending
                    }

                    OwnerPageState.Unavailable,
                    OwnerPageState.Failed,
                    -> {
                        val failure = read.page.failure?.toAppFailure() ?: logsNotReadyFailure()
                        publishUnavailable(failure)
                        LogsRefreshResult.Unavailable(failure)
                    }
                }
            }
        }
    }

    private fun publishUnavailable(failure: AppFailure) {
        _state.value = _state.value.copy(
            observation = LogsObservationState.Unavailable(
                lastAccepted = _state.value.observation.lastAccepted(),
                revision = lastAcceptedRevision,
                failure = failure,
            ),
        )
    }

    private fun advanceReadRequestRevision(): ObservationRevision.Request {
        val next = ObservationRevision.Request(readRequestRevision.sequence + 1uL)
        readRequestRevision = next
        return next
    }

    override fun onCleared() {
        coroutineScope.cancel()
    }
}

private sealed interface LogsRefreshResult {
    data class Accepted(val point: LogsObservationPoint) : LogsRefreshResult
    data object Pending : LogsRefreshResult
    data class Unavailable(val failure: AppFailure) : LogsRefreshResult
    data object Superseded : LogsRefreshResult
}
