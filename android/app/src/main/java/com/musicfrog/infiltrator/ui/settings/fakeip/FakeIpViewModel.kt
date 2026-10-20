package com.musicfrog.infiltrator.ui.settings.fakeip

import androidx.lifecycle.ViewModel
import com.musicfrog.infiltrator.ui.common.AppFailure
import com.musicfrog.infiltrator.ui.common.DEFAULT_FFI_TIMEOUT_MS
import com.musicfrog.infiltrator.ui.common.FailureTextKey
import com.musicfrog.infiltrator.ui.common.LONG_FFI_TIMEOUT_MS
import com.musicfrog.infiltrator.ui.common.ObservationRevision
import com.musicfrog.infiltrator.ui.common.localizedFailure
import com.musicfrog.infiltrator.ui.common.toAppFailure
import infiltrator_android.AndroidOwnerOperation
import infiltrator_android.FakeIpSettings
import infiltrator_android.FakeIpSettingsPatch
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

internal data class FakeIpUiState(
    val fakeIpRange: String = "",
    val fakeIpFilter: String = "",
    val storeFakeIp: FakeIpBooleanSetting = FakeIpBooleanSetting.Unspecified,
    val observation: FakeIpSettingsReadState = FakeIpSettingsReadState.Loading(),
    val settingsOperation: FakeIpSettingsOperationState = FakeIpSettingsOperationState.Idle,
    val hostCacheOperation: HostFakeIpFileCacheState = HostFakeIpFileCacheState.Idle,
) {
    val settingsActivity: FakeIpSettingsActivity
        get() = when (settingsOperation) {
            FakeIpSettingsOperationState.Reading,
            FakeIpSettingsOperationState.Saving,
            -> FakeIpSettingsActivity.Busy

            FakeIpSettingsOperationState.Idle,
            is FakeIpSettingsOperationState.Saved,
            is FakeIpSettingsOperationState.Failed,
            is FakeIpSettingsOperationState.Cancelled,
            is FakeIpSettingsOperationState.TimedOut,
            is FakeIpSettingsOperationState.Unconfirmed,
            -> FakeIpSettingsActivity.Interactive
        }
}

internal data class FakeIpSaveIntent(val patch: FakeIpSettingsPatch, val expectedRange: String?)

internal sealed interface FakeIpRequestOutcome {
    data object Accepted : FakeIpRequestOutcome
    data object Rejected : FakeIpRequestOutcome
    data object Superseded : FakeIpRequestOutcome
}

internal class FakeIpViewModel internal constructor(private val repository: FakeIpRepository, private val coroutineScope: CoroutineScope) :
    ViewModel() {
    constructor() : this(
        repository = UniFfiFakeIpRepository(),
        coroutineScope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate),
    )

    private val _state = MutableStateFlow(FakeIpUiState())
    val state: StateFlow<FakeIpUiState> = _state.asStateFlow()

    private var requestSequence = 0uL
    private var latestRequest = ObservationRevision.Request(0uL)

    init {
        load()
    }

    fun load() {
        when (_state.value.settingsActivity) {
            FakeIpSettingsActivity.Busy -> return
            FakeIpSettingsActivity.Interactive -> Unit
        }
        val request = nextRequest()
        val accepted = lastAcceptedFakeIp(_state.value.observation)
        _state.value = _state.value.copy(
            observation = FakeIpSettingsReadState.Loading(accepted),
            settingsOperation = FakeIpSettingsOperationState.Reading,
        )
        coroutineScope.launch {
            val result = repository.execute(AndroidOwnerOperation.FakeIpSettings, DEFAULT_FFI_TIMEOUT_MS)
            handleReadResult(result, request)
        }
    }

    fun updateRange(value: String) = updateDraft { copy(fakeIpRange = value) }
    fun updateFilter(value: String) = updateDraft { copy(fakeIpFilter = value) }
    fun updateStoreFakeIp(value: FakeIpBooleanSetting) = updateDraft { copy(storeFakeIp = value) }

    fun save() {
        when (_state.value.settingsActivity) {
            FakeIpSettingsActivity.Busy -> return
            FakeIpSettingsActivity.Interactive -> Unit
        }
        if (lastAcceptedFakeIp(_state.value.observation) == null) {
            setUnconfirmed(notReadyFailure())
            return
        }
        val current = _state.value
        val intent = current.toSaveIntent()
        val request = nextRequest()
        _state.value = current.copy(settingsOperation = FakeIpSettingsOperationState.Saving)
        coroutineScope.launch {
            val result = repository.execute(
                AndroidOwnerOperation.FakeIpSettingsSave(intent.patch),
                LONG_FFI_TIMEOUT_MS,
            )
            handleWriteResult(result, request, intent)
        }
    }

    /** This clears the owner's host file cache, never the controller's live Fake-IP table. */
    fun clearHostFakeIpFileCache() {
        if (_state.value.hostCacheOperation is HostFakeIpFileCacheState.Clearing) return
        _state.value = _state.value.copy(hostCacheOperation = HostFakeIpFileCacheState.Clearing)
        coroutineScope.launch {
            when (val result = repository.clearHostFakeIpFileCache(LONG_FFI_TIMEOUT_MS)) {
                is FakeIpCacheClearResult.Completed -> {
                    _state.value = _state.value.copy(
                        hostCacheOperation = hostFakeIpFileCacheReceipt(result.removedFile),
                    )
                }

                is FakeIpCacheClearResult.Failed -> {
                    _state.value = _state.value.copy(
                        hostCacheOperation = HostFakeIpFileCacheState.Failed(result.failure),
                    )
                }

                is FakeIpCacheClearResult.Cancelled -> {
                    _state.value = _state.value.copy(
                        hostCacheOperation = HostFakeIpFileCacheState.Failed(result.failure),
                    )
                }

                is FakeIpCacheClearResult.TimedOut -> {
                    _state.value = _state.value.copy(
                        hostCacheOperation = HostFakeIpFileCacheState.Failed(result.failure),
                    )
                }

                is FakeIpCacheClearResult.Unconfirmed -> {
                    _state.value = _state.value.copy(
                        hostCacheOperation = HostFakeIpFileCacheState.Failed(result.failure),
                    )
                }
            }
        }
    }

    fun clearError() {
        val current = _state.value
        val settingsOperation = when (current.settingsOperation) {
            is FakeIpSettingsOperationState.Failed,
            is FakeIpSettingsOperationState.Cancelled,
            is FakeIpSettingsOperationState.TimedOut,
            is FakeIpSettingsOperationState.Unconfirmed,
            -> FakeIpSettingsOperationState.Idle

            FakeIpSettingsOperationState.Idle,
            FakeIpSettingsOperationState.Reading,
            FakeIpSettingsOperationState.Saving,
            is FakeIpSettingsOperationState.Saved,
            -> current.settingsOperation
        }
        val hostCacheOperation = when (current.hostCacheOperation) {
            is HostFakeIpFileCacheState.Failed -> HostFakeIpFileCacheState.Idle

            HostFakeIpFileCacheState.Idle,
            HostFakeIpFileCacheState.Clearing,
            HostFakeIpFileCacheState.Cleared,
            HostFakeIpFileCacheState.AlreadyEmpty,
            -> current.hostCacheOperation
        }
        _state.value = current.copy(
            settingsOperation = settingsOperation,
            hostCacheOperation = hostCacheOperation,
        )
    }

    private fun updateDraft(update: FakeIpUiState.() -> FakeIpUiState) {
        val current = _state.value
        when (current.settingsActivity) {
            FakeIpSettingsActivity.Busy -> return
            FakeIpSettingsActivity.Interactive -> Unit
        }
        when (lastAcceptedFakeIp(current.observation)) {
            null -> return
            else -> _state.value = current.update().copy(settingsOperation = FakeIpSettingsOperationState.Idle)
        }
    }

    private fun handleReadResult(result: FakeIpRepositoryResult, request: ObservationRevision.Request): FakeIpRequestOutcome {
        when (requestAdmission(request)) {
            FakeIpRequestAdmission.Superseded -> return FakeIpRequestOutcome.Superseded
            FakeIpRequestAdmission.Current -> Unit
        }
        return when (result) {
            is FakeIpRepositoryResult.Completed -> when (
                accept(result.point, FakeIpSettingsOperationState.Idle)
            ) {
                FakeIpSnapshotAdmission.Accepted -> FakeIpRequestOutcome.Accepted

                FakeIpSnapshotAdmission.Stale -> {
                    val failure = notReadyFailure()
                    markUnavailable(failure, FakeIpSettingsOperationState.Unconfirmed(failure))
                    FakeIpRequestOutcome.Rejected
                }
            }

            is FakeIpRepositoryResult.Failed -> {
                acceptLatest(result.latest)
                markUnavailable(result.failure, FakeIpSettingsOperationState.Failed(result.failure))
                FakeIpRequestOutcome.Rejected
            }

            is FakeIpRepositoryResult.Cancelled -> {
                acceptLatest(result.latest)
                markUnavailable(result.failure, FakeIpSettingsOperationState.Cancelled(result.failure))
                FakeIpRequestOutcome.Rejected
            }

            is FakeIpRepositoryResult.TimedOut -> {
                markUnavailable(result.failure, FakeIpSettingsOperationState.TimedOut(result.failure))
                FakeIpRequestOutcome.Rejected
            }

            is FakeIpRepositoryResult.Unconfirmed -> {
                acceptLatest(result.latest)
                markUnavailable(result.failure, FakeIpSettingsOperationState.Unconfirmed(result.failure))
                FakeIpRequestOutcome.Rejected
            }
        }
    }

    private fun handleWriteResult(
        result: FakeIpRepositoryResult,
        request: ObservationRevision.Request,
        intent: FakeIpSaveIntent,
    ): FakeIpRequestOutcome {
        when (requestAdmission(request)) {
            FakeIpRequestAdmission.Superseded -> return FakeIpRequestOutcome.Superseded
            FakeIpRequestAdmission.Current -> Unit
        }
        return when (result) {
            is FakeIpRepositoryResult.Completed -> when {
                result.point.value.matches(intent.patch, intent.expectedRange) != FakeIpSettingsReadbackResult.Confirmed -> {
                    acceptLatest(result.point)
                    setUnconfirmed(notReadyFailure())
                    FakeIpRequestOutcome.Rejected
                }

                else -> when (accept(result.point, FakeIpSettingsOperationState.Saved(result.point.revision))) {
                    FakeIpSnapshotAdmission.Accepted -> FakeIpRequestOutcome.Accepted

                    FakeIpSnapshotAdmission.Stale -> {
                        setUnconfirmed(notReadyFailure())
                        FakeIpRequestOutcome.Rejected
                    }
                }
            }

            is FakeIpRepositoryResult.Failed -> {
                acceptLatest(result.latest)
                _state.value = _state.value.copy(settingsOperation = FakeIpSettingsOperationState.Failed(result.failure))
                FakeIpRequestOutcome.Rejected
            }

            is FakeIpRepositoryResult.Cancelled -> {
                acceptLatest(result.latest)
                _state.value = _state.value.copy(settingsOperation = FakeIpSettingsOperationState.Cancelled(result.failure))
                FakeIpRequestOutcome.Rejected
            }

            is FakeIpRepositoryResult.TimedOut -> {
                _state.value = _state.value.copy(settingsOperation = FakeIpSettingsOperationState.TimedOut(result.failure))
                FakeIpRequestOutcome.Rejected
            }

            is FakeIpRepositoryResult.Unconfirmed -> {
                acceptLatest(result.latest)
                setUnconfirmed(result.failure)
                FakeIpRequestOutcome.Rejected
            }
        }
    }

    private fun requestAdmission(request: ObservationRevision.Request): FakeIpRequestAdmission = admitsFakeIpRequest(latestRequest, request)

    private fun acceptLatest(point: FakeIpSettingsObservationPoint?) {
        when (point) {
            null -> Unit

            else -> when (acceptsFakeIpRevision(_state.value.observation, point.revision)) {
                FakeIpSnapshotAdmission.Accepted -> acceptSettings(point, _state.value.settingsOperation)
                FakeIpSnapshotAdmission.Stale -> Unit
            }
        }
    }

    private fun accept(point: FakeIpSettingsObservationPoint, operation: FakeIpSettingsOperationState): FakeIpSnapshotAdmission =
        when (acceptsFakeIpRevision(_state.value.observation, point.revision)) {
            FakeIpSnapshotAdmission.Accepted -> {
                acceptSettings(point, operation)
                FakeIpSnapshotAdmission.Accepted
            }

            FakeIpSnapshotAdmission.Stale -> FakeIpSnapshotAdmission.Stale
        }

    private fun acceptSettings(point: FakeIpSettingsObservationPoint, operation: FakeIpSettingsOperationState) {
        val settings = point.value
        _state.value = FakeIpUiState(
            fakeIpRange = settings.fakeIpRange.orEmpty(),
            fakeIpFilter = settings.fakeIpFilter.joinToString(separator = "\n"),
            storeFakeIp = settings.storeFakeIp.toFakeIpBooleanSetting(),
            observation = FakeIpSettingsReadState.Observed(point.value, point.revision),
            settingsOperation = operation,
            hostCacheOperation = _state.value.hostCacheOperation,
        )
    }

    private fun markUnavailable(failure: AppFailure, operation: FakeIpSettingsOperationState) {
        val current = _state.value
        _state.value = current.copy(
            observation = unavailableFakeIp(current.observation, failure),
            settingsOperation = operation,
        )
    }

    private fun setUnconfirmed(failure: AppFailure) = markUnavailable(
        failure,
        FakeIpSettingsOperationState.Unconfirmed(failure),
    )

    private fun FakeIpUiState.toSaveIntent(): FakeIpSaveIntent = FakeIpSaveIntent(
        patch = toPatch(),
        expectedRange = fakeIpRange.trim().ifBlank { null },
    )

    private fun FakeIpUiState.toPatch(): FakeIpSettingsPatch = FakeIpSettingsPatch(
        fakeIpRange.trim().ifBlank { null },
        parseList(fakeIpFilter),
        storeFakeIp.toPatchValue(),
    )

    private fun parseList(raw: String): List<String> = raw.split('\n', ',')
        .map { it.trim() }
        .filter { it.isNotEmpty() }

    private fun nextRequest(): ObservationRevision.Request {
        requestSequence += 1uL
        return ObservationRevision.Request(requestSequence).also { latestRequest = it }
    }

    private fun notReadyFailure(): AppFailure = localizedFailure(FailureTextKey.NotReady).toAppFailure()

    override fun onCleared() {
        coroutineScope.cancel()
    }
}
