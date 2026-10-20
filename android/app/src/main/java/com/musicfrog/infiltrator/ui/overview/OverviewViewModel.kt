package com.musicfrog.infiltrator.ui.overview

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.musicfrog.infiltrator.AndroidActivityState
import com.musicfrog.infiltrator.AndroidVpnAuthorizationUiState
import com.musicfrog.infiltrator.VpnOwnerDataPlaneState
import com.musicfrog.infiltrator.VpnOwnerLifecycleWire
import com.musicfrog.infiltrator.ui.common.AndroidComposeRepository
import com.musicfrog.infiltrator.ui.common.AppFailure
import com.musicfrog.infiltrator.ui.common.AppFailureCode
import com.musicfrog.infiltrator.ui.common.CallResult
import com.musicfrog.infiltrator.ui.common.DEFAULT_FFI_TIMEOUT_MS
import com.musicfrog.infiltrator.ui.common.FailureTextKey
import com.musicfrog.infiltrator.ui.common.LONG_FFI_TIMEOUT_MS
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
import com.musicfrog.infiltrator.ui.common.toAppFailure
import com.musicfrog.infiltrator.ui.common.toLocalizedFailure
import com.musicfrog.infiltrator.ui.common.trafficSnapshot
import infiltrator_android.AndroidCoreLifecycle
import infiltrator_android.FfiErrorCode
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

internal sealed interface ObservationState<out T> {
    data class Loading<out T>(val lastAccepted: ObservationPoint<T>? = null) : ObservationState<T>

    data object Empty : ObservationState<Nothing>

    data class Unavailable<out T>(val lastValue: T?, val revision: ObservationRevision?, val failure: AppFailure) : ObservationState<T>

    data class Observed<out T>(val value: T, val revision: ObservationRevision) : ObservationState<T>
}

internal data class ObservationPoint<out T>(val value: T, val revision: ObservationRevision)

data class EgressIpObservation(val ip: String, val location: String?)

internal fun <T> unavailableObservation(previous: ObservationState<T>, failure: AppFailure): ObservationState<T> {
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
    val ownerSnapshot = (
        (observation as? ObservationState.Observed)?.value
            as? OwnerVpnObservation.AndroidOwner
        )?.snapshot
    val ownerState = ownerSnapshot?.dataPlaneState()
    val ownerAllowsStart = when (ownerState) {
        VpnOwnerDataPlaneState.PermissionRequired -> ownerSnapshot.resourcesReleased()

        VpnOwnerDataPlaneState.Stopped,
        VpnOwnerDataPlaneState.Revoked,
        -> ownerSnapshot.isTerminalStopped()

        is VpnOwnerDataPlaneState.Failed ->
            ownerSnapshot.vpnState is VpnOwnerLifecycleWire.Failed && ownerSnapshot.resourcesReleased()

        VpnOwnerDataPlaneState.Starting,
        VpnOwnerDataPlaneState.Running,
        VpnOwnerDataPlaneState.Stopping,
        is VpnOwnerDataPlaneState.CleanupPending,
        is VpnOwnerDataPlaneState.Unavailable,
        null,
        -> false
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
    val error: LocalizedFailure? = null,
)

internal class OverviewViewModel(private val repository: AndroidComposeRepository) : ViewModel() {
    constructor() : this(com.musicfrog.infiltrator.ui.common.UniFfiAndroidComposeRepository)

    private val routingIntentObservationFence = RoutingIntentObservationFence()
    private var ipCheckRevision = 0uL
    private val _state = MutableStateFlow(OverviewUiState())
    val state: StateFlow<OverviewUiState> = _state.asStateFlow()

    init {
        refreshRoutingIntent()
        startCoreLifecyclePolling()
        startSurfacePolling()
        refreshIpCheck()
    }

    private fun startCoreLifecyclePolling() {
        viewModelScope.launch {
            repository.coreLifecycleFlow(1500L).collect { result ->
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
    }

    private fun startSurfacePolling() {
        viewModelScope.launch {
            repository.ownerSurfaceFlow(2000L).collect { read ->
                val epoch = routingIntentObservationFence.beginRead()
                val current = _state.value
                when (read) {
                    is OwnerSurfaceRead.Unavailable -> {
                        val failure = read.failure.toAppFailure()
                        _state.value = current.copy(
                            vpnOwnerObservation = unavailableObservation(current.vpnOwnerObservation, failure),
                            vpnStateError = failure.toLocalizedFailure(),
                            traffic = unavailableObservation(current.traffic, failure),
                            logs = unavailableObservation(current.logs, failure),
                        )
                    }

                    is OwnerSurfaceRead.Available -> {
                        val snapshot = read.snapshot

                        // 1. VPN observation
                        val vpnRevision = when (val vpn = snapshot.vpn) {
                            is OwnerVpnObservation.AndroidOwner -> ObservationRevision.VpnService(
                                vpn.snapshot.ownerSessionId,
                                vpn.snapshot.vpnGeneration,
                                vpn.snapshot.revision,
                            )

                            is OwnerVpnObservation.Unsupported,
                            OwnerVpnObservation.NotApplicable,
                            is OwnerVpnObservation.Unavailable,
                            -> ObservationRevision.Owner(
                                snapshot.generation,
                                snapshot.revision,
                            )
                        }
                        val vpnFailure = (snapshot.vpn as? OwnerVpnObservation.Unavailable)
                            ?.failure
                            ?.toAppFailure()
                        val nextVpn = ObservationState.Observed(snapshot.vpn, vpnRevision)

                        // 2. Traffic observation
                        val trafficSnapshot = snapshot.trafficSnapshot()
                        val pageFailure = snapshot.overview.failure?.toAppFailure()
                        val nextTraffic = when {
                            trafficSnapshot != null -> ObservationState.Observed(
                                trafficSnapshot,
                                ObservationRevision.Owner(snapshot.generation, snapshot.revision),
                            )

                            else -> unavailableObservation(current.traffic, pageFailure ?: notReadyFailure())
                        }

                        // 3. Logs observation
                        val logData = snapshot.logs.currentData()
                        val nextLogs = when {
                            logData == null -> unavailableObservation(
                                current.logs,
                                snapshot.logs.failure?.toAppFailure() ?: notReadyFailure(),
                            )

                            logData.entries.isEmpty() -> ObservationState.Empty

                            else -> ObservationState.Observed(
                                logData.entries.takeLast(5),
                                ObservationRevision.Owner(snapshot.generation, snapshot.revision),
                            )
                        }

                        // 4. Routing intent preservation
                        val nextRoutingIntent = if (routingIntentObservationFence.accepts(epoch)) {
                            preservePendingRoutingIntent(current.routingIntent, snapshot)
                        } else {
                            current.routingIntent
                        }

                        _state.value = current.copy(
                            vpnOwnerObservation = nextVpn,
                            vpnStateError = vpnFailure?.toLocalizedFailure(),
                            traffic = nextTraffic,
                            logs = nextLogs,
                            routingIntent = nextRoutingIntent,
                        )
                    }
                }
            }
        }
    }

    fun refreshRoutingIntent() {
        val epoch = routingIntentObservationFence.beginRead()
        viewModelScope.launch {
            when (
                val call = runCall(timeoutMs = DEFAULT_FFI_TIMEOUT_MS) {
                    repository.readOwnerSurface()
                }
            ) {
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
                                is OwnerSurfaceRead.Unavailable ->
                                    read.failure
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
                when (
                    val call = runCall(timeoutMs = DEFAULT_FFI_TIMEOUT_MS) {
                        repository.executeProductCommand(commandJson)
                    }
                ) {
                    is CallResult.Failure -> {
                        refreshRoutingIntentAfterFailure(call.failure.toLocalizedFailure())
                    }

                    is CallResult.Success -> when (
                        val result = parseRoutingIntentCommandResult(
                            call.value,
                            observed.scope,
                            target,
                        )
                    ) {
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
        when (
            val call = runCall(timeoutMs = DEFAULT_FFI_TIMEOUT_MS) {
                repository.readOwnerSurface()
            }
        ) {
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

    private fun preservePendingRoutingIntent(current: RoutingIntentUiState, snapshot: OwnerSurfaceSnapshot?): RoutingIntentUiState {
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
            val next = when (
                val call = runCall(timeoutMs = DEFAULT_FFI_TIMEOUT_MS) {
                    repository.checkPublicIp()
                }
            ) {
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
