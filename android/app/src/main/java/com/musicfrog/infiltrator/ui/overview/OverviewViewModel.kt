package com.musicfrog.infiltrator.ui.overview

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.musicfrog.infiltrator.AndroidActivityState
import com.musicfrog.infiltrator.AndroidVpnAuthorizationUiState
import com.musicfrog.infiltrator.ui.common.DEFAULT_FFI_TIMEOUT_MS
import com.musicfrog.infiltrator.ui.common.AndroidComposeRepository
import com.musicfrog.infiltrator.ui.common.AppFailure
import com.musicfrog.infiltrator.ui.common.AppFailureCode
import com.musicfrog.infiltrator.ui.common.CallResult
import com.musicfrog.infiltrator.ui.common.FailureTextKey
import com.musicfrog.infiltrator.ui.common.LocalizedFailure
import com.musicfrog.infiltrator.ui.common.LogEntry
import com.musicfrog.infiltrator.ui.common.ObservationRevision
import com.musicfrog.infiltrator.ui.common.OwnerSurfaceRead
import com.musicfrog.infiltrator.ui.common.OwnerSurfaceSnapshot
import com.musicfrog.infiltrator.ui.common.OwnerVpnObservation
import com.musicfrog.infiltrator.ui.common.RoutingIntentStatusObservation
import com.musicfrog.infiltrator.ui.common.TrafficSnapshot
import com.musicfrog.infiltrator.ui.common.localizedFailure
import com.musicfrog.infiltrator.ui.common.runCall
import com.musicfrog.infiltrator.ui.common.trafficSnapshot
import com.musicfrog.infiltrator.ui.common.toAppFailure
import com.musicfrog.infiltrator.ui.common.toLocalizedFailure
import infiltrator_android.AndroidCoreLifecycle
import infiltrator_android.FfiErrorCode
import com.musicfrog.infiltrator.VpnOwnerDataPlaneState
import com.musicfrog.infiltrator.VpnOwnerLifecycleWire
import com.musicfrog.infiltrator.ui.common.LONG_FFI_TIMEOUT_MS
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

internal sealed interface ObservationState<out T> {
    data class Loading<out T>(
        val lastAccepted: ObservationPoint<T>? = null,
    ) : ObservationState<T>

    data object Empty : ObservationState<Nothing>

    data class Unavailable<out T>(
        val lastValue: T?,
        val revision: ObservationRevision?,
        val failure: AppFailure,
    ) : ObservationState<T>

    data class Observed<out T>(
        val value: T,
        val revision: ObservationRevision,
    ) : ObservationState<T>
}

internal data class ObservationPoint<out T>(
    val value: T,
    val revision: ObservationRevision,
)

data class EgressIpObservation(
    val ip: String,
    val location: String?,
)

internal fun <T> unavailableObservation(
    previous: ObservationState<T>,
    failure: AppFailure,
): ObservationState<T> {
    val last = lastObservedPoint(previous)
    return ObservationState.Unavailable(last?.value, last?.revision, failure)
}

private fun <T> lastObservedPoint(state: ObservationState<T>): ObservationPoint<T>? = when (state) {
    is ObservationState.Loading -> state.lastAccepted
    ObservationState.Empty -> null
    is ObservationState.Unavailable -> state.lastValue?.let { value ->
        state.revision?.let { revision -> ObservationPoint(value, revision) }
    }
    is ObservationState.Observed -> ObservationPoint(state.value, state.revision)
}

internal fun vpnSessionIsRunning(
    observation: ObservationState<OwnerVpnObservation>,
    authorization: AndroidVpnAuthorizationUiState,
): Boolean = observation is ObservationState.Observed &&
    (observation.value as? OwnerVpnObservation.AndroidOwner)
        ?.snapshot
        ?.dataPlaneState() == VpnOwnerDataPlaneState.Running &&
    authorization == AndroidVpnAuthorizationUiState.Granted

internal fun canStartVpnSession(
    observation: ObservationState<OwnerVpnObservation>,
    authorization: AndroidVpnAuthorizationUiState,
): Boolean {
    val ownerSnapshot = ((observation as? ObservationState.Observed)?.value
        as? OwnerVpnObservation.AndroidOwner)?.snapshot
    val ownerState = ownerSnapshot?.dataPlaneState()
    val ownerAllowsStart = when (ownerState) {
        VpnOwnerDataPlaneState.PermissionRequired -> ownerSnapshot.resourcesReleased()
        VpnOwnerDataPlaneState.Stopped,
        VpnOwnerDataPlaneState.Revoked -> ownerSnapshot.isTerminalStopped()
        is VpnOwnerDataPlaneState.Failed ->
            ownerSnapshot.vpnState is VpnOwnerLifecycleWire.Failed && ownerSnapshot.resourcesReleased()
        VpnOwnerDataPlaneState.Starting,
        VpnOwnerDataPlaneState.Running,
        VpnOwnerDataPlaneState.Stopping,
        is VpnOwnerDataPlaneState.CleanupPending,
        is VpnOwnerDataPlaneState.Unavailable,
        null -> false
    }
    if (ownerSnapshot == null) {
        return observation is ObservationState.Loading &&
            authorization == AndroidVpnAuthorizationUiState.Required
    }
    if (!ownerAllowsStart) return false
    return authorization == AndroidVpnAuthorizationUiState.Required ||
        authorization == AndroidVpnAuthorizationUiState.Granted
}

internal data class OverviewUiState(
    internal val routingIntent: RoutingIntentUiState = RoutingIntentUiState.Loading,
    val traffic: ObservationState<TrafficSnapshot> = ObservationState.Loading(),
    val logs: ObservationState<List<LogEntry>> = ObservationState.Loading(),
    val egressIp: ObservationState<EgressIpObservation> = ObservationState.Loading(),
    val coreLifecycle: ObservationState<AndroidCoreLifecycle> = ObservationState.Loading(),
    val coreLifecycleError: LocalizedFailure? = null,
    val vpnOwnerObservation: ObservationState<OwnerVpnObservation> = ObservationState.Loading(),
    val vpnStateError: LocalizedFailure? = null,
    val error: LocalizedFailure? = null
)

internal class OverviewViewModel(
    private val repository: AndroidComposeRepository,
) : ViewModel() {
    constructor() : this(com.musicfrog.infiltrator.ui.common.UniFfiAndroidComposeRepository)

    private val routingIntentObservationFence = RoutingIntentObservationFence()
    private var ipCheckRevision = 0uL
    private val _state = MutableStateFlow(OverviewUiState())
    val state: StateFlow<OverviewUiState> = _state.asStateFlow()

    init {
        refreshRoutingIntent()
        startCoreLifecyclePolling()
        startVpnSessionPolling()
        startTrafficPolling()
        startLogStreaming()
        refreshIpCheck()
    }

    private fun startCoreLifecyclePolling() {
        viewModelScope.launch {
            while (true) {
                when (val call = runCall(timeoutMs = LONG_FFI_TIMEOUT_MS) {
                    repository.readCoreLifecycle()
                }) {
                    is CallResult.Failure -> {
                        val current = _state.value
                        _state.value = current.copy(
                            coreLifecycle = unavailableObservation(current.coreLifecycle, call.failure),
                            coreLifecycleError = call.failure.toLocalizedFailure(),
                        )
                    }
                    is CallResult.Success -> {
                        val result = call.value
                        val current = _state.value
                        val lifecycle = result.lifecycle
                        val failure = if (result.status.code != FfiErrorCode.OK) {
                            result.status.toAppFailure()
                        } else {
                            null
                        }
                        val nextState = when {
                            failure != null -> unavailableObservation(current.coreLifecycle, failure)
                            lifecycle == null -> ObservationState.Empty
                            else -> ObservationState.Observed(
                                lifecycle,
                                ObservationRevision.Owner(result.generation, result.revision),
                            )
                        }
                        _state.value = current.copy(
                            coreLifecycle = nextState,
                            coreLifecycleError = failure?.toLocalizedFailure(),
                        )
                    }
                }
                delay(1500)
            }
        }
    }

    private fun startVpnSessionPolling() {
        viewModelScope.launch {
            while (true) {
                when (val call = runCall(timeoutMs = DEFAULT_FFI_TIMEOUT_MS) {
                    repository.readOwnerSurface()
                }) {
                    is CallResult.Failure -> {
                        val current = _state.value
                        _state.value = current.copy(
                            vpnOwnerObservation = unavailableObservation(current.vpnOwnerObservation, call.failure),
                            vpnStateError = call.failure.toLocalizedFailure(),
                        )
                    }
                    is CallResult.Success -> {
                        val current = _state.value
                        when (val surface = call.value) {
                            is OwnerSurfaceRead.Unavailable -> {
                                val failure = surface.failure.toAppFailure()
                                _state.value = current.copy(
                                    vpnOwnerObservation = unavailableObservation(
                                        current.vpnOwnerObservation,
                                        failure,
                                    ),
                                    vpnStateError = failure.toLocalizedFailure(),
                                )
                            }
                            is OwnerSurfaceRead.Available -> {
                                val snapshot = surface.snapshot
                                val revision = when (val vpn = snapshot.vpn) {
                                    is OwnerVpnObservation.AndroidOwner -> ObservationRevision.VpnService(
                                        vpn.snapshot.ownerSessionId,
                                        vpn.snapshot.vpnGeneration,
                                        vpn.snapshot.revision,
                                    )
                                    is OwnerVpnObservation.Unsupported,
                                    OwnerVpnObservation.NotApplicable,
                                    is OwnerVpnObservation.Unavailable -> ObservationRevision.Owner(
                                        snapshot.generation,
                                        snapshot.revision,
                                    )
                                }
                                val failure = (snapshot.vpn as? OwnerVpnObservation.Unavailable)
                                    ?.failure
                                    ?.toAppFailure()
                                val nextState = ObservationState.Observed(snapshot.vpn, revision)
                                _state.value = current.copy(
                                    vpnOwnerObservation = nextState,
                                    vpnStateError = failure?.toLocalizedFailure(),
                                )
                            }
                        }
                    }
                }
                delay(1500)
            }
        }
    }

    fun refreshRoutingIntent() {
        val epoch = routingIntentObservationFence.beginRead()
        viewModelScope.launch {
            when (val call = runCall(timeoutMs = DEFAULT_FFI_TIMEOUT_MS) {
                repository.readOwnerSurface()
            }) {
                is CallResult.Failure -> if (routingIntentObservationFence.accepts(epoch)) {
                    val current = _state.value
                    _state.value = current.copy(
                        routingIntent = preservePendingRoutingIntent(current.routingIntent, null),
                        error = call.failure.toLocalizedFailure(),
                    )
                }
                is CallResult.Success -> {
                    val read = call.value
                    val snapshot = (read as? OwnerSurfaceRead.Available)?.snapshot
                    if (routingIntentObservationFence.accepts(epoch)) {
                        val current = _state.value
                        _state.value = current.copy(
                            routingIntent = preservePendingRoutingIntent(current.routingIntent, snapshot),
                            error = when (read) {
                                is OwnerSurfaceRead.Unavailable -> read.failure
                                    .toAppFailure()
                                    .toLocalizedFailure()
                                is OwnerSurfaceRead.Available -> current.error
                            },
                        )
                    }
                }
            }
        }
    }

    private fun startTrafficPolling() {
        viewModelScope.launch {
            while (true) {
                val epoch = routingIntentObservationFence.beginRead()
                val currentState = _state.value
                when (val call = runCall(timeoutMs = DEFAULT_FFI_TIMEOUT_MS) {
                    repository.readOwnerSurface()
                }) {
                    is CallResult.Failure -> _state.value = currentState.copy(
                        traffic = unavailableObservation(currentState.traffic, call.failure),
                    )
                    is CallResult.Success -> {
                        when (val read = call.value) {
                            is OwnerSurfaceRead.Unavailable -> _state.value = currentState.copy(
                                traffic = unavailableObservation(
                                    currentState.traffic,
                                    read.failure.toAppFailure(),
                                ),
                            )
                            is OwnerSurfaceRead.Available -> {
                                val ownerSnapshot = read.snapshot
                                val snapshot = ownerSnapshot.trafficSnapshot()
                                val pageFailure = ownerSnapshot.overview.failure
                                    ?.toAppFailure()
                                val next = when {
                                    snapshot != null -> ObservationState.Observed(
                                        snapshot,
                                        ObservationRevision.Owner(
                                            ownerSnapshot.generation,
                                            ownerSnapshot.revision,
                                        ),
                                    )
                                    else -> unavailableObservation(
                                        currentState.traffic,
                                        pageFailure ?: notReadyFailure(),
                                    )
                                }
                                _state.value = _state.value.copy(
                                    traffic = next,
                                    routingIntent = if (routingIntentObservationFence.accepts(epoch)) {
                                        preservePendingRoutingIntent(_state.value.routingIntent, ownerSnapshot)
                                    } else {
                                        _state.value.routingIntent
                                    },
                                )
                            }
                        }
                    }
                }
                delay(2000)
            }
        }
    }

    private fun startLogStreaming() {
        viewModelScope.launch {
            while (true) {
                val current = _state.value
                val next = when (val call = runCall { repository.readOwnerSurface() }) {
                    is CallResult.Failure -> unavailableObservation(current.logs, call.failure)
                    is CallResult.Success -> when (val read = call.value) {
                        is OwnerSurfaceRead.Unavailable -> unavailableObservation(
                            current.logs,
                            read.failure.toAppFailure(),
                        )
                        is OwnerSurfaceRead.Available -> {
                            val ownerSnapshot = read.snapshot
                            val data = ownerSnapshot.logs.currentData()
                            when {
                                data == null -> unavailableObservation(
                                    current.logs,
                                    ownerSnapshot.logs.failure?.toAppFailure() ?: notReadyFailure(),
                                )
                                data.entries.isEmpty() -> ObservationState.Empty
                                else -> ObservationState.Observed(
                                    data.entries.takeLast(5),
                                    ObservationRevision.Owner(
                                        ownerSnapshot.generation,
                                        ownerSnapshot.revision,
                                    ),
                                )
                            }
                        }
                    }
                }
                _state.value = _state.value.copy(logs = next)
                delay(3000)
            }
        }
    }

    internal fun changeRoutingIntent(target: RoutingIntentOption) {
        val observed = _state.value.routingIntent as? RoutingIntentUiState.Ready ?: return
        if (observed.current == target.observation()) return
        routingIntentObservationFence.beginMutation()
        _state.value = _state.value.copy(
            routingIntent = RoutingIntentUiState.Updating(
                current = observed.current,
                target = target,
                scope = observed.scope,
            ),
            error = null,
        )
        viewModelScope.launch {
            try {
                val commandJson = buildRoutingIntentCommandJson(observed.scope, target)
                when (val call = runCall(timeoutMs = DEFAULT_FFI_TIMEOUT_MS) {
                    repository.executeProductCommand(commandJson)
                }) {
                    is CallResult.Failure -> {
                        refreshRoutingIntentAfterFailure(call.failure.toLocalizedFailure())
                    }
                    is CallResult.Success -> when (val result = parseRoutingIntentCommandResult(
                        call.value,
                        observed.scope,
                        target,
                    )) {
                        is RoutingIntentCommandResult.Applied -> {
                            routingIntentObservationFence.finishMutation()
                            _state.value = _state.value.copy(
                                routingIntent = RoutingIntentUiState.Ready(
                                    result.observed,
                                    observed.scope,
                                ),
                                error = null,
                            )
                        }
                        is RoutingIntentCommandResult.Failed -> {
                            refreshRoutingIntentAfterFailure(result.failure.toLocalizedFailure())
                        }
                    }
                }
            } catch (error: CancellationException) {
                throw error
            } catch (e: Exception) {
                refreshRoutingIntentAfterFailure(e.toLocalizedFailure())
            }
        }
    }

    private suspend fun refreshRoutingIntentAfterFailure(failure: LocalizedFailure) {
        routingIntentObservationFence.finishMutation()
        val epoch = routingIntentObservationFence.beginRead()
        when (val call = runCall(timeoutMs = DEFAULT_FFI_TIMEOUT_MS) {
            repository.readOwnerSurface()
        }) {
            is CallResult.Failure -> if (routingIntentObservationFence.accepts(epoch)) {
                val current = _state.value
                _state.value = current.copy(
                    routingIntent = preservePendingRoutingIntent(current.routingIntent, null),
                    error = call.failure.toLocalizedFailure(),
                )
            }
            is CallResult.Success -> {
                val read = call.value
                val ownerSnapshot = (read as? OwnerSurfaceRead.Available)?.snapshot
                if (routingIntentObservationFence.accepts(epoch)) {
                    val current = _state.value
                    _state.value = current.copy(
                        routingIntent = preservePendingRoutingIntent(current.routingIntent, ownerSnapshot),
                        error = when (read) {
                            is OwnerSurfaceRead.Available -> failure
                            is OwnerSurfaceRead.Unavailable -> read.failure
                        },
                    )
                }
            }
        }
    }

    private fun routingIntentFromSnapshot(snapshot: OwnerSurfaceSnapshot?): RoutingIntentUiState {
        val observed = snapshot?.coreRoutingIntent
            ?: return RoutingIntentUiState.Unavailable
        return when (observed.status) {
            RoutingIntentStatusObservation.Ready -> {
                val current = observed.current
                    ?: return RoutingIntentUiState.Unavailable
                val scope = snapshot.coreScope
                    ?: return RoutingIntentUiState.Unavailable
                RoutingIntentUiState.Ready(current, scope)
            }
            RoutingIntentStatusObservation.Pending -> RoutingIntentUiState.Pending(observed)
            RoutingIntentStatusObservation.Unobserved -> RoutingIntentUiState.Unobserved
            RoutingIntentStatusObservation.Unsupported -> RoutingIntentUiState.Unsupported(observed)
            RoutingIntentStatusObservation.Failed -> RoutingIntentUiState.Failed(observed.failure)
        }
    }

    private fun preservePendingRoutingIntent(
        current: RoutingIntentUiState,
        snapshot: OwnerSurfaceSnapshot?,
    ): RoutingIntentUiState {
        val pending = current as? RoutingIntentUiState.Updating ?: return routingIntentFromSnapshot(snapshot)
        val scope = snapshot?.coreScope
        return if (scope != null && sameCoreScope(pending.scope, scope)) {
            pending
        } else {
            routingIntentFromSnapshot(snapshot)
        }
    }

    private fun sameCoreScope(
        first: com.musicfrog.infiltrator.ui.common.CoreScopeObservation,
        second: com.musicfrog.infiltrator.ui.common.CoreScopeObservation,
    ): Boolean = first.identity == second.identity &&
        first.generation == second.generation &&
        first.sessionToken == second.sessionToken

    fun clearError() {
        _state.value = _state.value.copy(
            error = null,
            coreLifecycleError = null,
            vpnStateError = null,
        )
        AndroidActivityState.clearError()
    }

    fun refreshIpCheck() {
        viewModelScope.launch {
            val previous = _state.value.egressIp
            _state.value = _state.value.copy(
                egressIp = ObservationState.Loading(lastObservedPoint(previous)),
            )
            val next = when (val call = runCall(timeoutMs = DEFAULT_FFI_TIMEOUT_MS) {
                repository.checkPublicIp()
            }) {
                is CallResult.Failure -> unavailableObservation(_state.value.egressIp, call.failure)
                is CallResult.Success -> {
                    val result = call.value
                    val failure = if (result.status.code != FfiErrorCode.OK) {
                        result.status.toAppFailure()
                    } else {
                        null
                    }
                    val value = result.value
                    when {
                        failure != null -> unavailableObservation(_state.value.egressIp, failure)
                        value == null -> ObservationState.Empty
                        value.ip.isBlank() -> ObservationState.Empty
                        else -> {
                            val location = listOfNotNull(
                                value.country?.takeIf { it.isNotBlank() },
                                value.region?.takeIf { it.isNotBlank() },
                                value.city?.takeIf { it.isNotBlank() },
                            ).joinToString(", ").takeIf { it.isNotBlank() }
                            ipCheckRevision += 1uL
                            ObservationState.Observed(
                                EgressIpObservation(value.ip, location),
                                ObservationRevision.Request(ipCheckRevision),
                            )
                        }
                    }
                }
            }
            _state.value = _state.value.copy(egressIp = next)
        }
    }

    private fun notReadyFailure() = AppFailure(
        code = AppFailureCode.NotReady,
        localization = FailureTextKey.NotReady,
    )
}
