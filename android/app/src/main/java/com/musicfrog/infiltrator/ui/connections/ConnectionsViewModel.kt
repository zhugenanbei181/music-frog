package com.musicfrog.infiltrator.ui.connections

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.musicfrog.infiltrator.ui.common.AppFailure
import com.musicfrog.infiltrator.ui.common.CallResult
import com.musicfrog.infiltrator.ui.common.ConnectionRecord
import com.musicfrog.infiltrator.ui.common.CoreScopeObservation
import com.musicfrog.infiltrator.ui.common.DEFAULT_FFI_TIMEOUT_MS
import com.musicfrog.infiltrator.ui.common.FailureTextKey
import com.musicfrog.infiltrator.ui.common.LONG_FFI_TIMEOUT_MS
import com.musicfrog.infiltrator.ui.common.ObservationRevision
import com.musicfrog.infiltrator.ui.common.OwnerSurfaceRead
import com.musicfrog.infiltrator.ui.common.ScopedProductCommandResult
import com.musicfrog.infiltrator.ui.common.localizedFailure
import com.musicfrog.infiltrator.ui.common.runCall
import com.musicfrog.infiltrator.ui.common.toAppFailure
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch

internal data class ConnectionsUiState(
    val connections: List<ConnectionRecord> = emptyList(),
    val totalConnectionCount: Int = 0,
    val uploadTotal: ULong = 0uL,
    val downloadTotal: ULong = 0uL,
    val listStatus: ConnectionsListStatus = ConnectionsListStatus.Loading,
    val actionState: ConnectionActionState = ConnectionActionState.Idle,
    val failure: AppFailure? = null,
)

class ConnectionsViewModel internal constructor(private val repository: ConnectionsRepository) : ViewModel() {
    constructor() : this(UniFfiConnectionsRepository())

    private val connectionReadState = MutableStateFlow<ConnectionReadState>(ConnectionReadState.Loading())
    private val actionState = MutableStateFlow<ConnectionActionState>(ConnectionActionState.Idle)
    private var readRequestSequence = 0uL

    private val _hostFilter = MutableStateFlow("")
    private val _processFilter = MutableStateFlow("")

    val hostFilter: StateFlow<String> = _hostFilter
    val processFilter: StateFlow<String> = _processFilter

    internal val uiState: StateFlow<ConnectionsUiState> = combine(
        combine(connectionReadState, actionState, ::ConnectionsState),
        combine(_hostFilter, _processFilter) { host, process ->
            host.trim().lowercase() to process.trim().lowercase()
        },
    ) { state, (hostQuery, processQuery) ->
        val snapshot = state.readState.snapshotOrLastAccepted()
        val filtered = snapshot?.records.orEmpty().filter { connection ->
            (hostQuery.isBlank() || connection.host.lowercase().contains(hostQuery)) &&
                (processQuery.isBlank() || connection.processPath.lowercase().contains(processQuery))
        }
        ConnectionsUiState(
            connections = filtered,
            totalConnectionCount = snapshot?.totalConnections ?: 0,
            uploadTotal = snapshot?.uploadTotal ?: 0uL,
            downloadTotal = snapshot?.downloadTotal ?: 0uL,
            listStatus = connectionsListStatus(state.readState, filtered.size),
            actionState = state.actionState,
            failure = when (val read = state.readState) {
                is ConnectionReadState.Unavailable -> read.failure
                else -> (state.actionState as? ConnectionActionState.Failed)?.failure
            },
        )
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5000), ConnectionsUiState())

    init {
        refresh()
    }

    fun setHostFilter(value: String) {
        _hostFilter.value = value
    }

    fun setProcessFilter(value: String) {
        _processFilter.value = value
    }

    fun refresh() {
        viewModelScope.launch {
            if (actionState.value != ConnectionActionState.Idle) return@launch
            if (lastAcceptedConnection(connectionReadState.value) == null) {
                connectionReadState.value = ConnectionReadState.Loading()
            }
            actionState.value = ConnectionActionState.Refreshing
            refreshConnections()
            if (actionState.value == ConnectionActionState.Refreshing) {
                actionState.value = ConnectionActionState.Idle
            }
        }
    }

    fun closeConnection(id: String) {
        if (actionState.value != ConnectionActionState.Idle) return
        val scope = observedCoreScope() ?: return failUnavailableClose()
        val targetId = id.trim()
        if (targetId.isEmpty()) return failUnavailableClose()
        executeClose(
            working = ConnectionActionState.ClosingConnection,
            operation = { repository.closeConnection(scope, targetId) },
            expectation = ConnectionCloseExpectation.TargetClosed(targetId),
        )
    }

    fun closeAllConnections() {
        if (actionState.value != ConnectionActionState.Idle) return
        val scope = observedCoreScope() ?: return failUnavailableClose()
        executeClose(
            working = ConnectionActionState.ClosingAll,
            operation = { repository.closeAll(scope) },
            expectation = ConnectionCloseExpectation.AllClosed,
        )
    }

    fun clearError() {
        if (actionState.value is ConnectionActionState.Failed) {
            actionState.value = ConnectionActionState.Idle
        }
    }

    private fun executeClose(
        working: ConnectionActionState,
        operation: suspend () -> ScopedProductCommandResult,
        expectation: ConnectionCloseExpectation,
    ) {
        viewModelScope.launch {
            if (actionState.value != ConnectionActionState.Idle) return@launch
            actionState.value = working
            when (val call = runCall(timeoutMs = LONG_FFI_TIMEOUT_MS, block = operation)) {
                is CallResult.Failure -> {
                    actionState.value = ConnectionActionState.Failed(call.failure)
                    return@launch
                }

                is CallResult.Success -> when (val result = call.value) {
                    is ScopedProductCommandResult.Completed -> Unit

                    is ScopedProductCommandResult.Failed -> {
                        actionState.value = ConnectionActionState.Failed(result.failure.toAppFailure())
                        return@launch
                    }

                    ScopedProductCommandResult.ScopeMismatch,
                    ScopedProductCommandResult.NonTerminal,
                    ScopedProductCommandResult.Invalid,
                    -> {
                        actionState.value = ConnectionActionState.Failed(notReadyFailure())
                        return@launch
                    }
                }
            }

            when (val readback = refreshConnections()) {
                is ConnectionReadState.Observed -> {
                    actionState.value = when (readback.value.checks(expectation)) {
                        ConnectionCloseReadback.Confirmed -> ConnectionActionState.Idle
                        ConnectionCloseReadback.StillActive -> ConnectionActionState.Failed(notReadyFailure())
                    }
                }

                is ConnectionReadState.Unavailable -> {
                    actionState.value = ConnectionActionState.Failed(readback.failure)
                }

                is ConnectionReadState.Loading -> {
                    actionState.value = ConnectionActionState.Failed(notReadyFailure())
                }
            }
        }
    }

    private suspend fun refreshConnections(): ConnectionReadState {
        readRequestSequence += 1uL
        val requestSequence = readRequestSequence
        val call = runCall(timeoutMs = DEFAULT_FFI_TIMEOUT_MS) { repository.readSurface() }
        if (requestSequence != readRequestSequence) return connectionReadState.value
        when (call) {
            is CallResult.Failure -> {
                return unavailableConnections(connectionReadState.value, call.failure)
                    .also { connectionReadState.value = it }
            }

            is CallResult.Success -> when (val read = call.value) {
                is OwnerSurfaceRead.Unavailable -> {
                    return unavailableConnections(connectionReadState.value, read.failure.toAppFailure())
                        .also { connectionReadState.value = it }
                }

                is OwnerSurfaceRead.Available -> {
                    val snapshot = read.snapshot
                    val page = snapshot.connections
                    val data = page.currentData()
                    if (data == null) {
                        val failure = page.failure?.toAppFailure() ?: notReadyFailure()
                        return unavailableConnections(connectionReadState.value, failure)
                            .also { connectionReadState.value = it }
                    }

                    val revision = ObservationRevision.Owner(snapshot.generation, snapshot.revision)
                    val latest = connectionReadState.value
                    if (!acceptsConnectionRevision(latest, revision)) return latest

                    return ConnectionReadState.Observed(
                        value = ConnectionSnapshot(
                            records = data.connections,
                            totalConnections = data.totalConnections,
                            uploadTotal = data.uploadTotal,
                            downloadTotal = data.downloadTotal,
                            coreScope = snapshot.coreScope,
                        ),
                        revision = revision,
                    ).also { connectionReadState.value = it }
                }
            }
        }
    }

    private data class ConnectionsState(val readState: ConnectionReadState, val actionState: ConnectionActionState)

    private fun ConnectionReadState.snapshotOrLastAccepted(): ConnectionSnapshot? = when (this) {
        is ConnectionReadState.Loading -> lastAccepted?.value
        is ConnectionReadState.Observed -> value
        is ConnectionReadState.Unavailable -> lastValue
    }

    private fun notReadyFailure(): AppFailure = localizedFailure(FailureTextKey.NotReady).toAppFailure()

    private fun observedCoreScope(): CoreScopeObservation? = lastAcceptedConnection(connectionReadState.value)?.value?.coreScope

    private fun failUnavailableClose() {
        actionState.value = ConnectionActionState.Failed(notReadyFailure())
    }
}
