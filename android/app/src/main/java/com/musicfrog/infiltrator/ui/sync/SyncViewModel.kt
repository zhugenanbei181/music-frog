package com.musicfrog.infiltrator.ui.sync

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.musicfrog.infiltrator.ui.common.DEFAULT_FFI_TIMEOUT_MS
import com.musicfrog.infiltrator.ui.common.FailureTextKey
import com.musicfrog.infiltrator.ui.common.LONG_FFI_TIMEOUT_MS
import com.musicfrog.infiltrator.ui.common.localizedFailure
import com.musicfrog.infiltrator.ui.common.toLocalizedFailure
import infiltrator_android.AndroidOwnerOperation
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

internal class SyncViewModel(private val repository: SyncRepository) : ViewModel() {
    constructor() : this(UniFfiSyncRepository())

    private val _state = MutableStateFlow(SyncUiState())
    val state: StateFlow<SyncUiState> = _state.asStateFlow()

    private var requestSequence = 0uL

    init {
        load()
    }

    fun load() {
        val request = begin(SyncOperation.LoadSettings) ?: return
        _state.value = _state.value.copy(
            settings = SyncSettingsState.Loading(_state.value.settings.lastAccepted()),
            action = SyncActionState.Working(SyncOperation.LoadSettings),
        )
        viewModelScope.launch {
            handleResult(
                operation = SyncOperation.LoadSettings,
                request = request,
                result = repository.execute(AndroidOwnerOperation.WebDavSettings, DEFAULT_FFI_TIMEOUT_MS),
                draftPolicy = SyncDraftPolicy.ReplaceWithAccepted,
            )
        }
    }

    fun updateEnabled(value: Boolean) = updateDraft { copy(enabled = value) }
    fun updateUrl(value: String) = updateDraft { copy(url = value) }
    fun updateUsername(value: String) = updateDraft { copy(username = value) }
    fun updatePassword(value: String) = updateDraft { copy(password = value) }
    fun updateSyncInterval(value: String) = updateDraft { copy(intervalMinutes = value) }
    fun updateSyncOnStartup(value: Boolean) = updateDraft { copy(syncOnStartup = value) }

    fun save() {
        val form = currentEditableForm() ?: return unavailable(SyncOperation.SaveSettings)
        val interval = when (val parsed = parseSyncInterval(form.draft.intervalMinutes)) {
            is SyncIntervalParse.Valid -> parsed.minutes

            SyncIntervalParse.Invalid -> {
                _state.value = _state.value.copy(
                    action = SyncActionState.Failed(
                        SyncOperation.SaveSettings,
                        localizedFailure(FailureTextKey.SyncIntervalInvalid),
                    ),
                )
                return
            }
        }
        val request = begin(SyncOperation.SaveSettings) ?: return
        val settings = form.draft.toWebDavSettings(interval)
        _state.value = _state.value.copy(action = SyncActionState.Working(SyncOperation.SaveSettings))
        viewModelScope.launch {
            handleResult(
                operation = SyncOperation.SaveSettings,
                request = request,
                result = repository.execute(
                    AndroidOwnerOperation.WebDavSettingsSave(settings),
                    LONG_FFI_TIMEOUT_MS,
                ),
                draftPolicy = SyncDraftPolicy.ReplaceWithAccepted,
                requestedSettings = settings,
            )
        }
    }

    fun testConnection() {
        val form = currentEditableForm() ?: return unavailable(SyncOperation.TestConnection)
        val interval = when (val parsed = parseSyncInterval(form.draft.intervalMinutes)) {
            is SyncIntervalParse.Valid -> parsed.minutes

            SyncIntervalParse.Invalid -> {
                _state.value = _state.value.copy(
                    action = SyncActionState.Failed(
                        SyncOperation.TestConnection,
                        localizedFailure(FailureTextKey.SyncIntervalInvalid),
                    ),
                )
                return
            }
        }
        val request = begin(SyncOperation.TestConnection) ?: return
        val settings = form.draft.toWebDavSettings(interval)
        _state.value = _state.value.copy(action = SyncActionState.Working(SyncOperation.TestConnection))
        viewModelScope.launch {
            handleResult(
                operation = SyncOperation.TestConnection,
                request = request,
                result = repository.execute(
                    AndroidOwnerOperation.WebDavTest(settings),
                    LONG_FFI_TIMEOUT_MS,
                ),
                draftPolicy = SyncDraftPolicy.Preserve,
            )
        }
    }

    fun syncNow() {
        if (currentEditableForm() == null) return unavailable(SyncOperation.SyncNow)
        val request = begin(SyncOperation.SyncNow) ?: return
        _state.value = _state.value.copy(action = SyncActionState.Working(SyncOperation.SyncNow))
        viewModelScope.launch {
            handleResult(
                operation = SyncOperation.SyncNow,
                request = request,
                result = repository.execute(AndroidOwnerOperation.WebDavSyncNow, LONG_FFI_TIMEOUT_MS),
                draftPolicy = SyncDraftPolicy.Preserve,
            )
        }
    }

    fun clearError() {
        when (_state.value.action) {
            is SyncActionState.Failed,
            is SyncActionState.Cancelled,
            is SyncActionState.TimedOut,
            is SyncActionState.Unconfirmed,
            -> _state.value = _state.value.copy(action = SyncActionState.Idle)

            SyncActionState.Idle,
            is SyncActionState.Working,
            SyncActionState.Saved,
            SyncActionState.ConnectionTested,
            is SyncActionState.Synchronized,
            -> Unit
        }
    }

    private fun updateDraft(update: SyncSettingsDraft.() -> SyncSettingsDraft) {
        val current = _state.value
        if (current.action is SyncActionState.Working) return
        val observed = current.settings as? SyncSettingsState.Observed ?: return
        _state.value = current.copy(
            settings = SyncSettingsState.Observed(
                observed.form.copy(draft = observed.form.draft.update()),
            ),
            action = SyncActionState.Idle,
        )
    }

    private fun currentEditableForm(): SyncForm? = when (val settings = _state.value.settings) {
        is SyncSettingsState.Observed -> settings.form

        is SyncSettingsState.Loading,
        is SyncSettingsState.Unavailable,
        -> null
    }

    private fun begin(operation: SyncOperation): SyncRequest? {
        when (_state.value.requestAvailability()) {
            SyncRequestAvailability.Busy -> return null
            SyncRequestAvailability.Available -> Unit
        }
        return when (val allocation = allocateSyncRequest(requestSequence)) {
            is SyncRequestAllocation.Allocated -> {
                requestSequence = allocation.sequence.value
                SyncRequest(allocation.sequence)
            }

            SyncRequestAllocation.Exhausted -> {
                _state.value = _state.value.copy(
                    action = SyncActionState.Unconfirmed(
                        operation,
                        localizedFailure(FailureTextKey.InvalidState, "Sync request sequence exhausted"),
                    ),
                )
                null
            }
        }
    }

    private suspend fun handleResult(
        operation: SyncOperation,
        request: SyncRequest,
        result: SyncRepositoryResult,
        draftPolicy: SyncDraftPolicy,
        requestedSettings: infiltrator_android.WebDavSettings? = null,
    ) {
        when (result) {
            is SyncRepositoryResult.Completed -> {
                if (requestedSettings != null &&
                    result.snapshot.isReadbackOf(requestedSettings) != SyncReadbackConfirmation.Confirmed
                ) {
                    markUnconfirmed(operation)
                    return
                }
                val currentForm = _state.value.settings.lastAccepted()
                when (admitSyncSettings(currentForm, result.snapshot, request.sequence.value)) {
                    SyncSnapshotAdmission.Stale,
                    SyncSnapshotAdmission.Conflicting,
                    -> markUnconfirmed(operation)

                    SyncSnapshotAdmission.Accepted -> {
                        val draft = when (draftPolicy) {
                            SyncDraftPolicy.ReplaceWithAccepted -> result.snapshot.toDraft()
                            SyncDraftPolicy.Preserve -> currentForm?.draft ?: result.snapshot.toDraft()
                        }
                        val form = SyncForm(result.snapshot, draft, request.sequence.value)
                        _state.value = _state.value.copy(
                            settings = SyncSettingsState.Observed(form),
                            action = when (result.outcome) {
                                SyncRepositoryOutcome.SettingsObserved -> SyncActionState.Idle
                                SyncRepositoryOutcome.SettingsSaved -> SyncActionState.Saved
                                SyncRepositoryOutcome.ConnectionTested -> SyncActionState.ConnectionTested
                                is SyncRepositoryOutcome.Synchronized -> SyncActionState.Synchronized(result.outcome.summary)
                            },
                        )
                    }
                }
            }

            is SyncRepositoryResult.Failed -> {
                acceptLatest(result.latest, request.sequence, draftPolicy)
                _state.value = _state.value.copy(action = SyncActionState.Failed(operation, result.failure.toLocalizedFailure()))
            }

            is SyncRepositoryResult.Cancelled -> {
                acceptLatest(result.latest, request.sequence, draftPolicy)
                _state.value = _state.value.copy(action = SyncActionState.Cancelled(operation, result.failure.toLocalizedFailure()))
            }

            is SyncRepositoryResult.TimedOut -> {
                _state.value = _state.value.copy(
                    settings = SyncSettingsState.Unavailable(_state.value.settings.lastAccepted()),
                    action = SyncActionState.TimedOut(operation, result.failure.toLocalizedFailure()),
                )
            }

            is SyncRepositoryResult.Unconfirmed -> {
                _state.value = _state.value.copy(
                    settings = SyncSettingsState.Unavailable(_state.value.settings.lastAccepted()),
                    action = SyncActionState.Unconfirmed(operation, result.failure.toLocalizedFailure()),
                )
            }
        }
    }

    private fun acceptLatest(snapshot: SyncSettingsSnapshot?, sequence: SyncRequestSequence, draftPolicy: SyncDraftPolicy) {
        if (snapshot == null) {
            _state.value = _state.value.copy(
                settings = SyncSettingsState.Unavailable(_state.value.settings.lastAccepted()),
            )
            return
        }
        when (admitSyncSettings(_state.value.settings.lastAccepted(), snapshot, sequence.value)) {
            SyncSnapshotAdmission.Accepted -> {
                val previous = _state.value.settings.lastAccepted()
                val draft = when (draftPolicy) {
                    SyncDraftPolicy.ReplaceWithAccepted -> snapshot.toDraft()
                    SyncDraftPolicy.Preserve -> previous?.draft ?: snapshot.toDraft()
                }
                _state.value = _state.value.copy(
                    settings = SyncSettingsState.Observed(SyncForm(snapshot, draft, sequence.value)),
                )
            }

            SyncSnapshotAdmission.Stale,
            SyncSnapshotAdmission.Conflicting,
            -> {
                _state.value = _state.value.copy(
                    settings = SyncSettingsState.Unavailable(_state.value.settings.lastAccepted()),
                )
            }
        }
    }

    private fun markUnconfirmed(operation: SyncOperation) {
        val accepted = _state.value.settings.lastAccepted()
        _state.value = _state.value.copy(
            settings = SyncSettingsState.Unavailable(accepted),
            action = SyncActionState.Unconfirmed(
                operation,
                localizedFailure(FailureTextKey.NotReady),
            ),
        )
    }

    private fun unavailable(operation: SyncOperation) {
        _state.value = _state.value.copy(
            action = SyncActionState.Unconfirmed(
                operation,
                localizedFailure(FailureTextKey.NotReady),
            ),
        )
    }
}

private enum class SyncDraftPolicy {
    ReplaceWithAccepted,
    Preserve,
}

private data class SyncRequest(val sequence: SyncRequestSequence)
