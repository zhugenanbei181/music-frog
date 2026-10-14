package com.musicfrog.infiltrator.ui.settings.rules

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.musicfrog.infiltrator.ui.common.AppFailure
import com.musicfrog.infiltrator.ui.common.DEFAULT_FFI_TIMEOUT_MS
import com.musicfrog.infiltrator.ui.common.FailureTextKey
import com.musicfrog.infiltrator.ui.common.LONG_FFI_TIMEOUT_MS
import com.musicfrog.infiltrator.ui.common.localizedFailure
import com.musicfrog.infiltrator.ui.common.toAppFailure
import infiltrator_android.AndroidOwnerOperation
import infiltrator_android.AndroidRuleEntry
import infiltrator_android.AndroidRuleProviderEntry
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

internal class RulesViewModel(
    private val repository: RulesRepository,
) : ViewModel() {
    constructor() : this(UniFfiRulesRepository())

    private val _state = MutableStateFlow(RulesUiState())
    val state: StateFlow<RulesUiState> = _state.asStateFlow()

    private var requestSequence = 0uL
    private var latestRequestSequence = 0uL

    init {
        load()
    }

    fun load() {
        when (_state.value.reloadAvailability) {
            RulesReloadAvailability.Available -> Unit
            RulesReloadAvailability.Busy,
            RulesReloadAvailability.UnsavedChanges -> return
        }
        val sequence = nextRequestSequence()
        val current = _state.value
        _state.value = current.copy(
            rules = RulesReadState.Loading(acceptedRules(current.rules)),
            providers = RuleProvidersReadState.Loading(acceptedRuleProviders(current.providers)),
            operation = RulesOperationState.Loading,
        )
        viewModelScope.launch {
            when (handleResult(
                repository.execute(AndroidOwnerOperation.RulesList, DEFAULT_FFI_TIMEOUT_MS),
                sequence,
                RulesRequestKind.ReadRules,
            )) {
                RulesRequestOutcome.Accepted -> Unit
                RulesRequestOutcome.Rejected,
                RulesRequestOutcome.Superseded -> return@launch
            }

            when (handleResult(
                repository.execute(AndroidOwnerOperation.RuleProviders, DEFAULT_FFI_TIMEOUT_MS),
                sequence,
                RulesRequestKind.ReadProviders,
            )) {
                RulesRequestOutcome.Accepted -> _state.value =
                    _state.value.copy(operation = RulesOperationState.Ready)
                RulesRequestOutcome.Rejected,
                RulesRequestOutcome.Superseded -> Unit
            }
        }
    }

    fun updateNewRule(value: String) = updateDraft { copy(newRule = value) }

    fun addRule() {
        val current = _state.value
        when (current.activity) {
            RulesActivity.Busy -> return
            RulesActivity.Interactive -> Unit
        }
        when (current.rulesAvailability) {
            RulesObservationAvailability.Available -> Unit
            RulesObservationAvailability.Unavailable -> {
                fail(notReadyFailure())
                return
            }
        }
        val rule = current.newRule.trim()
        if (rule.isEmpty()) {
            fail(localizedFailure(FailureTextKey.EmptyRule).toAppFailure())
            return
        }
        _state.value = current.copy(
            rulesDraft = current.rulesDraft + AndroidRuleEntry(rule, true),
            newRule = "",
            operation = RulesOperationState.Ready,
        )
    }

    fun toggleRule(index: Int, enabled: Boolean) {
        val current = _state.value
        when (current.activity) {
            RulesActivity.Busy -> return
            RulesActivity.Interactive -> Unit
        }
        if (index !in current.rulesDraft.indices) return
        _state.value = current.copy(
            rulesDraft = current.rulesDraft.toMutableList().also { entries ->
                val entry = entries[index]
                entries[index] = AndroidRuleEntry(entry.rule, enabled)
            },
            operation = RulesOperationState.Ready,
        )
    }

    fun removeRule(index: Int) {
        val current = _state.value
        when (current.activity) {
            RulesActivity.Busy -> return
            RulesActivity.Interactive -> Unit
        }
        if (index !in current.rulesDraft.indices) return
        _state.value = current.copy(
            rulesDraft = current.rulesDraft.toMutableList().also { it.removeAt(index) },
            operation = RulesOperationState.Ready,
        )
    }

    fun updateProvidersJson(value: String) = updateDraft { copy(providersDraft = value) }

    fun saveRules() {
        val current = _state.value
        when (current.activity) {
            RulesActivity.Busy -> return
            RulesActivity.Interactive -> Unit
        }
        when (current.rulesAvailability) {
            RulesObservationAvailability.Available -> Unit
            RulesObservationAvailability.Unavailable -> return
        }
        val sequence = nextRequestSequence()
        _state.value = current.copy(operation = RulesOperationState.SavingRules)
        viewModelScope.launch {
            handleResult(
                repository.execute(
                    AndroidOwnerOperation.RulesSave(current.rulesDraft),
                    LONG_FFI_TIMEOUT_MS,
                ),
                sequence,
                RulesRequestKind.WriteRules,
            )
        }
    }

    fun saveProviders() {
        val current = _state.value
        when (current.activity) {
            RulesActivity.Busy -> return
            RulesActivity.Interactive -> Unit
        }
        when (current.providersAvailability) {
            RulesObservationAvailability.Available -> Unit
            RulesObservationAvailability.Unavailable -> return
        }
        val providers = try {
            ruleProvidersJsonToEntries(current.providersDraft)
        } catch (error: Exception) {
            fail(error.toAppFailure())
            return
        }
        val sequence = nextRequestSequence()
        _state.value = current.copy(operation = RulesOperationState.SavingProviders)
        viewModelScope.launch {
            handleResult(
                repository.execute(
                    AndroidOwnerOperation.RuleProvidersSave(providers),
                    LONG_FFI_TIMEOUT_MS,
                ),
                sequence,
                RulesRequestKind.WriteProviders,
            )
        }
    }

    fun clearError() {
        when (_state.value.operation) {
            is RulesOperationState.Failed,
            is RulesOperationState.Cancelled,
            is RulesOperationState.TimedOut,
            is RulesOperationState.Unconfirmed -> {
                _state.value = _state.value.copy(operation = RulesOperationState.Ready)
            }
            RulesOperationState.Loading,
            RulesOperationState.Ready,
            RulesOperationState.SavingRules,
            RulesOperationState.RulesSaved,
            RulesOperationState.SavingProviders,
            RulesOperationState.ProvidersSaved -> Unit
        }
    }

    private suspend fun handleResult(
        result: RulesRepositoryResult,
        sequence: ULong,
        purpose: RulesRequestKind,
    ): RulesRequestOutcome {
        when (requestAdmission(sequence)) {
            RulesRequestAdmission.Current -> Unit
            RulesRequestAdmission.Superseded -> return RulesRequestOutcome.Superseded
        }
        return when (result) {
            is RulesRepositoryResult.Completed -> {
                when (accept(result.observation, purpose.domain)) {
                    RulesSnapshotAdmission.Accepted -> {
                        when (purpose) {
                            RulesRequestKind.ReadRules,
                            RulesRequestKind.ReadProviders -> Unit
                            RulesRequestKind.WriteRules -> {
                                _state.value = _state.value.copy(operation = RulesOperationState.RulesSaved)
                            }
                            RulesRequestKind.WriteProviders -> {
                                _state.value = _state.value.copy(operation = RulesOperationState.ProvidersSaved)
                            }
                            RulesRequestKind.Unsupported -> {
                                setUnconfirmed(purpose, notReadyFailure())
                                return RulesRequestOutcome.Rejected
                            }
                        }
                        RulesRequestOutcome.Accepted
                    }
                    RulesSnapshotAdmission.Stale -> {
                        setUnconfirmed(purpose, notReadyFailure())
                        RulesRequestOutcome.Rejected
                    }
                }
            }
            is RulesRepositoryResult.Failed -> {
                acceptLatest(result.latest, purpose.domain)
                setFailure(purpose, result.failure)
                RulesRequestOutcome.Rejected
            }
            is RulesRepositoryResult.Cancelled -> {
                acceptLatest(result.latest, purpose.domain)
                markUnavailableForRead(purpose, result.failure)
                _state.value = _state.value.copy(operation = RulesOperationState.Cancelled(result.failure))
                RulesRequestOutcome.Rejected
            }
            is RulesRepositoryResult.TimedOut -> {
                markUnavailableForRead(purpose, result.failure)
                _state.value = _state.value.copy(operation = RulesOperationState.TimedOut(result.failure))
                RulesRequestOutcome.Rejected
            }
            is RulesRepositoryResult.Unconfirmed -> {
                acceptLatest(result.latest, purpose.domain)
                setUnconfirmed(purpose, result.failure)
                RulesRequestOutcome.Rejected
            }
        }
    }

    private fun accept(
        observation: RulesRepositoryObservation,
        domain: RulesDomain,
    ): RulesSnapshotAdmission = when (observation) {
        is RulesRepositoryObservation.Rules -> when (domain) {
            RulesDomain.Rules -> when (
                admitsRulesRevision(acceptedRules(_state.value.rules), observation.snapshot.revision)
            ) {
                RulesSnapshotAdmission.Accepted -> {
                    _state.value = _state.value.copy(
                        rules = RulesReadState.Observed(observation.snapshot),
                        rulesDraft = observation.snapshot.entries,
                    )
                    RulesSnapshotAdmission.Accepted
                }
                RulesSnapshotAdmission.Stale -> RulesSnapshotAdmission.Stale
            }
            RulesDomain.Providers,
            RulesDomain.Unsupported -> RulesSnapshotAdmission.Stale
        }
        is RulesRepositoryObservation.Providers -> when (domain) {
            RulesDomain.Rules,
            RulesDomain.Unsupported -> RulesSnapshotAdmission.Stale
            RulesDomain.Providers -> when (
                admitsProvidersRevision(acceptedRuleProviders(_state.value.providers), observation.snapshot.revision)
            ) {
                RulesSnapshotAdmission.Accepted -> {
                    _state.value = _state.value.copy(
                        providers = RuleProvidersReadState.Observed(observation.snapshot),
                        providersDraft = observation.snapshot.json,
                    )
                    RulesSnapshotAdmission.Accepted
                }
                RulesSnapshotAdmission.Stale -> RulesSnapshotAdmission.Stale
            }
        }
    }

    private fun acceptLatest(latest: RulesRepositoryObservation?, domain: RulesDomain) {
        when (latest) {
            null -> Unit
            else -> accept(latest, domain)
        }
    }

    private fun setFailure(purpose: RulesRequestKind, failure: AppFailure) {
        markUnavailableForRead(purpose, failure)
        _state.value = _state.value.copy(operation = RulesOperationState.Failed(failure))
    }

    private fun setUnconfirmed(purpose: RulesRequestKind, failure: AppFailure) {
        markUnavailableForRead(purpose, failure)
        _state.value = _state.value.copy(operation = RulesOperationState.Unconfirmed(failure))
    }

    private fun markUnavailableForRead(purpose: RulesRequestKind, failure: AppFailure) {
        when (purpose) {
            RulesRequestKind.ReadRules -> markRulesUnavailable(failure)
            RulesRequestKind.ReadProviders -> markProvidersUnavailable(failure)
            RulesRequestKind.WriteRules,
            RulesRequestKind.WriteProviders,
            RulesRequestKind.Unsupported -> Unit
        }
    }

    private fun markRulesUnavailable(failure: AppFailure) {
        _state.value = _state.value.copy(
            rules = RulesReadState.Unavailable(acceptedRules(_state.value.rules), failure),
        )
    }

    private fun markProvidersUnavailable(failure: AppFailure) {
        _state.value = _state.value.copy(
            providers = RuleProvidersReadState.Unavailable(
                acceptedRuleProviders(_state.value.providers),
                failure,
            ),
        )
    }

    private fun updateDraft(transform: RulesUiState.() -> RulesUiState) {
        val current = _state.value
        when (current.activity) {
            RulesActivity.Busy -> return
            RulesActivity.Interactive -> Unit
        }
        _state.value = current.transform().copy(operation = RulesOperationState.Ready)
    }

    private fun fail(failure: AppFailure) {
        _state.value = _state.value.copy(operation = RulesOperationState.Failed(failure))
    }

    @Synchronized
    private fun nextRequestSequence(): ULong {
        check(requestSequence < ULong.MAX_VALUE) { "Rules request sequence is exhausted" }
        requestSequence += 1u
        latestRequestSequence = requestSequence
        return requestSequence
    }

    @Synchronized
    private fun requestAdmission(sequence: ULong): RulesRequestAdmission = when (sequence) {
        latestRequestSequence -> RulesRequestAdmission.Current
        else -> RulesRequestAdmission.Superseded
    }

    private fun notReadyFailure() = localizedFailure(FailureTextKey.NotReady).toAppFailure()
}

private enum class RulesRequestAdmission {
    Current,
    Superseded,
}

private enum class RulesRequestOutcome {
    Accepted,
    Rejected,
    Superseded,
}
