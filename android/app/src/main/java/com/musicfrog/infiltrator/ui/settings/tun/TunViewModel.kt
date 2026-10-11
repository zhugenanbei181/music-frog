package com.musicfrog.infiltrator.ui.settings.tun

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.musicfrog.infiltrator.ui.common.AppFailure
import com.musicfrog.infiltrator.ui.common.DEFAULT_FFI_TIMEOUT_MS
import com.musicfrog.infiltrator.ui.common.FailureTextKey
import com.musicfrog.infiltrator.ui.common.LONG_FFI_TIMEOUT_MS
import com.musicfrog.infiltrator.ui.common.localizedFailure
import com.musicfrog.infiltrator.ui.common.toAppFailure
import infiltrator_android.AndroidOwnerOperation
import infiltrator_android.VpnTunSettingsPatch
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

internal class TunViewModel(
    private val repository: TunRepository,
) : ViewModel() {
    constructor() : this(UniFfiTunRepository)

    private val _state = MutableStateFlow(TunUiState())
    val state: StateFlow<TunUiState> = _state.asStateFlow()

    private var requestSequence = 0uL
    private var latestRequestSequence = 0uL

    init {
        load()
    }

    fun load() {
        val sequence = nextRequestSequence()
        val accepted = lastAcceptedTun(_state.value.settings)
        _state.value = _state.value.copy(
            settings = TunSettingsReadState.Loading(accepted),
            operation = TunOperationState.Loading,
        )
        viewModelScope.launch {
            when (val result = repository.execute(
                AndroidOwnerOperation.TunSettings,
                DEFAULT_FFI_TIMEOUT_MS,
            )) {
                is TunRepositoryResult.Completed -> if (isLatest(sequence)) {
                    acceptOrFail(result.point, TunOperationState.Idle)
                }
                is TunRepositoryResult.Failed -> if (isLatest(sequence)) {
                    updateLatest(result.latestSnapshot)
                    fail(result.failure)
                }
                is TunRepositoryResult.Cancelled -> if (isLatest(sequence)) {
                    updateLatest(result.latestSnapshot)
                    _state.value = _state.value.copy(
                        operation = TunOperationState.Cancelled(result.failure),
                    )
                }
                is TunRepositoryResult.TimedOut -> if (isLatest(sequence)) {
                    fail(result.failure)
                }
                is TunRepositoryResult.Unconfirmed -> if (isLatest(sequence)) {
                    updateLatest(result.latestSnapshot)
                    _state.value = _state.value.copy(
                        operation = TunOperationState.Unconfirmed(result.failure),
                    )
                }
            }
        }
    }

    fun updateMtu(value: String) = updateDraft { copy(mtu = value) }
    fun updateAutoRoute(value: Boolean) = updateDraft { copy(autoRoute = value) }
    fun updateStack(value: String) = updateDraft { copy(stack = value) }
    fun updateStrictRoute(value: Boolean) = updateDraft { copy(strictRoute = value) }
    fun updateAutoDetectInterface(value: Boolean) = updateDraft { copy(autoDetectInterface = value) }

    fun save() {
        if (_state.value.isBusy) return
        if (lastAcceptedTun(_state.value.settings) == null) {
            fail(localizedFailure(FailureTextKey.NotReady).toAppFailure())
            return
        }
        val draft = _state.value.draft
        val mtu = parseMtu(draft.mtu)
        if (draft.mtu.isNotBlank() && mtu == null) {
            fail(localizedFailure(FailureTextKey.TunMtuInvalid).toAppFailure())
            return
        }
        if (mtu == 0u) {
            fail(localizedFailure(FailureTextKey.TunMtuZero).toAppFailure())
            return
        }

        val patch = VpnTunSettingsPatch(
            mtu = mtu,
            autoRoute = draft.autoRoute,
            strictRoute = draft.strictRoute,
            stack = draft.stack.trim().ifBlank { null },
            autoDetectInterface = draft.autoDetectInterface,
        )
        val sequence = nextRequestSequence()
        _state.value = _state.value.copy(operation = TunOperationState.Saving)
        viewModelScope.launch {
            when (val result = repository.execute(
                AndroidOwnerOperation.TunSettingsSave(patch),
                LONG_FFI_TIMEOUT_MS,
            )) {
                is TunRepositoryResult.Completed -> if (isLatest(sequence)) {
                    acceptOrFail(result.point, TunOperationState.Saved(result.point.revision))
                }
                is TunRepositoryResult.Failed -> if (isLatest(sequence)) {
                    updateLatest(result.latestSnapshot)
                    fail(result.failure)
                }
                is TunRepositoryResult.Cancelled -> if (isLatest(sequence)) {
                    updateLatest(result.latestSnapshot)
                    _state.value = _state.value.copy(
                        operation = TunOperationState.Cancelled(result.failure),
                    )
                }
                is TunRepositoryResult.TimedOut -> if (isLatest(sequence)) {
                    fail(result.failure)
                }
                is TunRepositoryResult.Unconfirmed -> if (isLatest(sequence)) {
                    updateLatest(result.latestSnapshot)
                    _state.value = _state.value.copy(
                        operation = TunOperationState.Unconfirmed(result.failure),
                    )
                }
            }
        }
    }

    fun clearError() {
        _state.value = _state.value.copy(operation = TunOperationState.Idle)
    }

    private fun updateDraft(transform: TunSettingsDraft.() -> TunSettingsDraft) {
        val current = _state.value
        if (current.isBusy) return
        _state.value = current.copy(
            draft = current.draft.transform(),
            operation = TunOperationState.Idle,
        )
    }

    private fun accept(point: TunSettingsObservationPoint, operation: TunOperationState) {
        if (!acceptsTunRevision(_state.value.settings, point.revision)) return
        _state.value = _state.value.copy(
            settings = TunSettingsReadState.Observed(point),
            draft = point.toDraft(),
            operation = operation,
        )
    }

    private fun acceptOrFail(point: TunSettingsObservationPoint, operation: TunOperationState) {
        if (acceptsTunRevision(_state.value.settings, point.revision)) {
            accept(point, operation)
        } else {
            fail(localizedFailure(FailureTextKey.NotReady).toAppFailure())
        }
    }

    private fun updateLatest(point: TunSettingsObservationPoint?) {
        if (point != null && acceptsTunRevision(_state.value.settings, point.revision)) {
            _state.value = _state.value.copy(
                settings = TunSettingsReadState.Observed(point),
                draft = point.toDraft(),
            )
        }
    }

    private fun fail(failure: AppFailure) {
        _state.value = _state.value.copy(
            settings = unavailableTun(_state.value.settings, failure),
            operation = TunOperationState.Failed(failure),
        )
    }

    @Synchronized
    private fun nextRequestSequence(): ULong {
        check(requestSequence < ULong.MAX_VALUE) { "TUN request sequence is exhausted" }
        requestSequence += 1u
        latestRequestSequence = requestSequence
        return requestSequence
    }

    @Synchronized
    private fun isLatest(sequence: ULong): Boolean = latestRequestSequence == sequence

    private fun parseMtu(value: String): UInt? = value.trim().takeIf(String::isNotEmpty)?.toUIntOrNull()
}
