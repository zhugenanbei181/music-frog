package com.musicfrog.infiltrator.ui.proxies

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.musicfrog.infiltrator.ui.common.CoreScopeObservation
import com.musicfrog.infiltrator.ui.common.DEFAULT_FFI_TIMEOUT_MS
import com.musicfrog.infiltrator.ui.common.AndroidComposeRepository
import com.musicfrog.infiltrator.ui.common.CallResult
import com.musicfrog.infiltrator.ui.common.FailureTextKey
import com.musicfrog.infiltrator.ui.common.LocalizedFailure
import com.musicfrog.infiltrator.ui.common.AndroidProductCommandOutcome
import com.musicfrog.infiltrator.ui.common.OutboundDetailsPageState
import com.musicfrog.infiltrator.ui.common.OutboundDetailsSnapshot
import com.musicfrog.infiltrator.ui.common.OutboundInventoryPageState
import com.musicfrog.infiltrator.ui.common.OutboundInventorySnapshot
import com.musicfrog.infiltrator.ui.common.OutboundProbeSnapshot
import com.musicfrog.infiltrator.ui.common.OutboundProbeOperationCapability
import com.musicfrog.infiltrator.ui.common.OutboundSelectionCollection
import com.musicfrog.infiltrator.ui.common.OutboundIdObservation
import com.musicfrog.infiltrator.ui.common.OutboundUiPreferencesPageState
import com.musicfrog.infiltrator.ui.common.OutboundUiPreferencesSnapshot
import com.musicfrog.infiltrator.ui.common.OwnerErrorCode
import com.musicfrog.infiltrator.ui.common.OwnerFailure
import com.musicfrog.infiltrator.ui.common.OwnerSurfaceRead
import com.musicfrog.infiltrator.ui.common.OwnerSurfaceSnapshot
import com.musicfrog.infiltrator.ui.common.ProtocolStudioPageState
import com.musicfrog.infiltrator.ui.common.parseAndroidProductCommandOutcome
import com.musicfrog.infiltrator.ui.common.runCall
import com.musicfrog.infiltrator.ui.common.localizedFailure
import com.musicfrog.infiltrator.ui.common.toLocalizedFailure
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock

internal class ProxiesViewModel internal constructor(
    private val repository: AndroidComposeRepository,
) : ViewModel() {
    constructor() : this(com.musicfrog.infiltrator.ui.common.UniFfiAndroidComposeRepository)

    private val _inventory = MutableStateFlow<OutboundInventoryPageState>(OutboundInventoryPageState.Loading)
    internal val inventory: StateFlow<OutboundInventoryPageState> = _inventory.asStateFlow()

    private val _details = MutableStateFlow<OutboundDetailsPageState>(OutboundDetailsPageState.Loading)
    internal val details: StateFlow<OutboundDetailsPageState> = _details.asStateFlow()

    private val _preferences = MutableStateFlow<OutboundUiPreferencesPageState>(OutboundUiPreferencesPageState.Loading)
    internal val preferences: StateFlow<OutboundUiPreferencesPageState> = _preferences.asStateFlow()

    private val _protocolStudio = MutableStateFlow<ProtocolStudioPageState>(ProtocolStudioPageState.Loading)
    internal val protocolStudio: StateFlow<ProtocolStudioPageState> = _protocolStudio.asStateFlow()

    private val _probeReadback = MutableStateFlow<OutboundProbeSnapshot?>(null)
    internal val probeReadback: StateFlow<OutboundProbeSnapshot?> = _probeReadback.asStateFlow()

    private val _probeCapability = MutableStateFlow<OutboundProbeOperationCapability>(OutboundProbeOperationCapability.Unknown)
    internal val probeCapability: StateFlow<OutboundProbeOperationCapability> = _probeCapability.asStateFlow()

    private val _isLoading = MutableStateFlow(false)
    val isLoading: StateFlow<Boolean> = _isLoading.asStateFlow()

    private val _commandInProgress = MutableStateFlow(false)
    internal val commandInProgress: StateFlow<Boolean> = _commandInProgress.asStateFlow()

    private val _error = MutableStateFlow<LocalizedFailure?>(null)
    val error: StateFlow<LocalizedFailure?> = _error.asStateFlow()

    private val _commandFailure = MutableStateFlow<OwnerFailure?>(null)
    internal val commandFailure: StateFlow<OwnerFailure?> = _commandFailure.asStateFlow()

    private val refreshMutex = Mutex()
    private var lastAcceptedSnapshot: OutboundInventorySnapshot? = null

    init {
        loadInventory(showLoading = true)
        startRefreshLoop()
    }

    private fun startRefreshLoop() {
        viewModelScope.launch {
            repository.ownerSurfaceFlow(3000L).collect { read ->
                when (read) {
                    is OwnerSurfaceRead.Available -> acceptSurface(read.snapshot)
                    is OwnerSurfaceRead.Unavailable -> publishUnavailable(read.failure)
                }
            }
        }
    }

    fun loadInventory(showLoading: Boolean = false) {
        viewModelScope.launch {
            refreshMutex.withLock {
                if (showLoading) _isLoading.value = true
                try {
                    when (val call = runCall(timeoutMs = DEFAULT_FFI_TIMEOUT_MS) {
                        repository.readOwnerSurface()
                    }) {
                        is CallResult.Failure -> publishUnavailable(call.failure.toLocalizedFailure())
                        is CallResult.Success -> when (val read = call.value) {
                            is OwnerSurfaceRead.Available -> acceptSurface(read.snapshot)
                            is OwnerSurfaceRead.Unavailable -> publishUnavailable(read.failure)
                        }
                    }
                } catch (error: CancellationException) {
                    throw error
                } catch (error: Exception) {
                    publishUnavailable(error.toLocalizedFailure())
                } finally {
                    if (showLoading) _isLoading.value = false
                }
            }
        }
    }

    /** The row passes the exact observation it rendered; a later scope read cannot replace it. */
    internal fun selectOutbound(
        observedSnapshot: OutboundInventorySnapshot,
        collection: OutboundSelectionCollection,
        outboundId: OutboundIdObservation,
    ) {
        val commandJson = try {
            buildSelectOutboundCommandJson(observedSnapshot, collection, outboundId)
        } catch (error: Exception) {
            publishCommandError(error.toLocalizedFailure())
            return
        }
        submitCommand(commandJson, observedSnapshot.scope) { outcome ->
            evaluateSelectOutboundOutcome(outcome, observedSnapshot, collection, outboundId)
        }
    }

    internal fun applyPreference(
        observedInventory: OutboundInventorySnapshot,
        observedPreferences: OutboundUiPreferencesSnapshot,
        intent: OutboundUiPreferenceIntent,
    ) {
        val commandJson = try {
            buildApplyOutboundUiPreferenceCommandJson(
                observedInventory,
                observedPreferences,
                intent,
            )
        } catch (error: Exception) {
            publishCommandError(error.toLocalizedFailure())
            return
        }
        submitCommand(commandJson, observedInventory.scope) { outcome ->
            evaluateOutboundUiPreferenceOutcome(
                outcome = outcome,
                observedScope = observedInventory.scope,
                inventory = observedInventory,
                preferences = observedPreferences,
                intent = intent,
            )
        }
    }

    internal fun probeOutbound(
        observedInventory: OutboundInventorySnapshot,
        observedDetails: OutboundDetailsSnapshot,
        observedProbeCapability: OutboundProbeOperationCapability,
        outboundId: OutboundIdObservation,
    ) {
        val commandJson = try {
            buildProbeOutboundCommandJson(
                observedInventory,
                observedDetails,
                observedProbeCapability,
                outboundId,
            )
        } catch (error: Exception) {
            publishCommandError(error.toLocalizedFailure())
            return
        }
        submitCommand(commandJson, observedInventory.scope) { outcome ->
            evaluateProbeOutboundOutcome(outcome, observedInventory, outboundId)
        }
    }

    private fun submitCommand(
        commandJson: String,
        observedScope: CoreScopeObservation,
        evaluate: (AndroidProductCommandOutcome) -> OutboundCommandHandling,
    ) {
        if (_commandInProgress.value) return
        _commandInProgress.value = true
        viewModelScope.launch {
            try {
                _error.value = null
                _commandFailure.value = null
                val outcome = when (val call = runCall(timeoutMs = DEFAULT_FFI_TIMEOUT_MS) {
                    repository.executeProductCommand(commandJson)
                }) {
                    is CallResult.Failure -> {
                        publishCommandError(call.failure.toLocalizedFailure())
                        loadInventory()
                        return@launch
                    }
                    is CallResult.Success -> try {
                        parseAndroidProductCommandOutcome(call.value)
                    } catch (error: CancellationException) {
                        throw error
                    } catch (error: Exception) {
                        publishCommandError(error.toLocalizedFailure())
                        loadInventory()
                        return@launch
                    }
                }
                when (val handling = evaluate(outcome)) {
                    OutboundCommandHandling.Success -> loadInventory()
                    is OutboundCommandHandling.SelectionApplied -> {
                        when (acceptInventory(OutboundInventoryPageState.Observed(handling.snapshot))) {
                            OutboundInventoryAdmission.Accepted,
                            OutboundInventoryAdmission.RejectedStale -> Unit
                        }
                        loadInventory()
                    }
                    is OutboundCommandHandling.PreferencesApplied -> {
                        val currentInventory = (_inventory.value as? OutboundInventoryPageState.Observed)?.snapshot
                        if (currentInventory != null && sameScope(currentInventory.scope, handling.snapshot.scope)) {
                            _preferences.value = fencePreferences(
                                OutboundUiPreferencesPageState.Observed(handling.snapshot),
                                currentInventory.scope,
                                _preferences.value,
                            )
                        }
                        loadInventory()
                    }
                    is OutboundCommandHandling.ProbeCompleted -> {
                        if (sameScope(observedScope, handling.snapshot.scope)) {
                            _probeReadback.value = handling.snapshot
                        }
                        loadInventory()
                    }
                    is OutboundCommandHandling.OwnerFailure -> {
                        _commandFailure.value = handling.failure
                        publishCommandError(handling.failure.toLocalizedFailure())
                        loadInventory()
                    }
                    OutboundCommandHandling.StaleScope -> {
                        publishCommandError(localizedFailure(FailureTextKey.InvalidState))
                        loadInventory()
                    }
                    is OutboundCommandHandling.UnexpectedResult -> {
                        publishCommandError(localizedFailure(FailureTextKey.Internal))
                        loadInventory()
                    }
                }
            } finally {
                _commandInProgress.value = false
            }
        }
    }

    fun clearError() {
        _error.value = null
        _commandFailure.value = null
    }

    private fun acceptSurface(surface: OwnerSurfaceSnapshot) {
        when (acceptInventory(surface.outboundInventory)) {
            OutboundInventoryAdmission.Accepted -> Unit
            OutboundInventoryAdmission.RejectedStale -> return
        }
        val inventorySnapshot = (surface.outboundInventory as? OutboundInventoryPageState.Observed)?.snapshot
        if (inventorySnapshot == null) {
            _details.value = surface.outboundDetails
            _preferences.value = surface.outboundUiPreferences
            _probeCapability.value = surface.outboundProbeCapability
            _protocolStudio.value = surface.protocolStudio
            _probeReadback.value = null
            return
        }
        _details.value = fenceDetails(surface.outboundDetails, inventorySnapshot.scope, _details.value)
        _preferences.value = fencePreferences(
            surface.outboundUiPreferences,
            inventorySnapshot.scope,
            _preferences.value,
        )
        _probeCapability.value = surface.outboundProbeCapability
        _protocolStudio.value = surface.protocolStudio
        if (_probeReadback.value?.let { !sameScope(inventorySnapshot.scope, it.scope) } == true) {
            _probeReadback.value = null
        }
    }

    private fun acceptInventory(incoming: OutboundInventoryPageState): OutboundInventoryAdmission {
        when (incoming) {
            is OutboundInventoryPageState.Observed -> {
                when (val current = lastAcceptedSnapshot) {
                    null -> Unit
                    else -> when (classifyInventoryObservation(current, incoming.snapshot)) {
                        OutboundInventoryAdmission.Accepted -> Unit
                        OutboundInventoryAdmission.RejectedStale -> return OutboundInventoryAdmission.RejectedStale
                    }
                }
                lastAcceptedSnapshot = incoming.snapshot
            }
            else -> Unit
        }
        val retainedIncoming = retainLastAcceptedInventory(incoming, lastAcceptedSnapshot)
        _inventory.value = retainedIncoming
        when (incoming) {
            is OutboundInventoryPageState.Failed,
            is OutboundInventoryPageState.Unavailable -> Unit
            else -> _error.value = null
        }
        return OutboundInventoryAdmission.Accepted
    }

    private fun publishUnavailable(failure: LocalizedFailure) {
        val ownerFailure = OwnerFailure(
            code = OwnerErrorCode.NotReady,
            message = failure.diagnostic.orEmpty(),
            retryable = true,
            reason = null,
        )
        _inventory.value = OutboundInventoryPageState.Unavailable(
            ownerFailure,
            lastValue = lastAcceptedSnapshot,
        )
        _details.value = OutboundDetailsPageState.Unavailable(ownerFailure)
        _preferences.value = OutboundUiPreferencesPageState.Unavailable(ownerFailure)
        _probeCapability.value = OutboundProbeOperationCapability.Unknown
        _protocolStudio.value = ProtocolStudioPageState.Unavailable(ownerFailure)
        _error.value = failure
    }

    private fun publishCommandError(failure: LocalizedFailure) {
        _error.value = failure
    }
}

internal fun retainLastAcceptedInventory(
    incoming: OutboundInventoryPageState,
    lastAccepted: OutboundInventorySnapshot?,
): OutboundInventoryPageState = when (incoming) {
    is OutboundInventoryPageState.Unavailable -> incoming.copy(
        lastValue = incoming.lastValue ?: lastAccepted,
    )
    else -> incoming
}

internal enum class OutboundInventoryAdmission {
    Accepted,
    RejectedStale,
}

internal fun classifyInventoryObservation(
    current: OutboundInventorySnapshot,
    incoming: OutboundInventorySnapshot,
): OutboundInventoryAdmission {
    val currentGeneration = current.scope.generation.toULongOrNull()
        ?: return OutboundInventoryAdmission.RejectedStale
    val incomingGeneration = incoming.scope.generation.toULongOrNull()
        ?: return OutboundInventoryAdmission.RejectedStale
    when {
        incomingGeneration < currentGeneration -> return OutboundInventoryAdmission.RejectedStale
        incomingGeneration > currentGeneration -> return OutboundInventoryAdmission.Accepted
    }
    if (!sameScope(current.scope, incoming.scope)) return OutboundInventoryAdmission.RejectedStale
    val currentRevision = current.revision.toULongOrNull()
        ?: return OutboundInventoryAdmission.RejectedStale
    val incomingRevision = incoming.revision.toULongOrNull()
        ?: return OutboundInventoryAdmission.RejectedStale
    return when {
        incomingRevision < currentRevision -> OutboundInventoryAdmission.RejectedStale
        else -> OutboundInventoryAdmission.Accepted
    }
}

private fun fenceDetails(
    incoming: OutboundDetailsPageState,
    scope: CoreScopeObservation,
    current: OutboundDetailsPageState,
): OutboundDetailsPageState {
    if (incoming is OutboundDetailsPageState.Observed) {
        if (!sameScope(scope, incoming.snapshot.scope)) {
            return OutboundDetailsPageState.Unavailable(scopeMismatchFailure())
        }
        val previous = (current as? OutboundDetailsPageState.Observed)?.snapshot
        if (previous != null && sameScope(scope, previous.scope) &&
            (incoming.snapshot.detailRevision.toULongOrNull() ?: return current) <
            (previous.detailRevision.toULongOrNull() ?: return current)
        ) {
            return current
        }
    }
    return incoming
}

private fun fencePreferences(
    incoming: OutboundUiPreferencesPageState,
    scope: CoreScopeObservation,
    current: OutboundUiPreferencesPageState,
): OutboundUiPreferencesPageState {
    if (incoming is OutboundUiPreferencesPageState.Observed) {
        if (!sameScope(scope, incoming.snapshot.scope)) {
            return OutboundUiPreferencesPageState.Unavailable(scopeMismatchFailure())
        }
        val previous = (current as? OutboundUiPreferencesPageState.Observed)?.snapshot
        if (previous != null && sameScope(scope, previous.scope) &&
            (incoming.snapshot.revision.toULongOrNull() ?: return current) <
            (previous.revision.toULongOrNull() ?: return current)
        ) {
            return current
        }
    }
    return incoming
}

private fun scopeMismatchFailure() = OwnerFailure(
    code = OwnerErrorCode.InvalidState,
    message = "",
    retryable = false,
    reason = null,
)

private fun sameScope(first: CoreScopeObservation, second: CoreScopeObservation): Boolean =
    first.identity == second.identity &&
        first.generation == second.generation &&
        first.sessionToken == second.sessionToken
