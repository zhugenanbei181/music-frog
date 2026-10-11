package com.musicfrog.infiltrator.ui.settings.rules

import com.musicfrog.infiltrator.ui.common.AppFailure
import infiltrator_android.AndroidRuleEntry
import infiltrator_android.AndroidRuleProviderEntry

internal data class RulesRevision(
    val ownerSessionId: String,
    val coreGeneration: ULong,
    val dataRevision: ULong,
)

internal data class RulesSnapshot(
    val entries: List<AndroidRuleEntry>,
    val revision: RulesRevision,
)

internal data class RuleProvidersSnapshot(
    val entries: List<AndroidRuleProviderEntry>,
    val json: String,
    val revision: RulesRevision,
)

internal sealed interface RulesReadState {
    data class Loading(val lastAccepted: RulesSnapshot? = null) : RulesReadState
    data class Observed(val snapshot: RulesSnapshot) : RulesReadState
    data class Unavailable(
        val lastAccepted: RulesSnapshot?,
        val failure: AppFailure,
    ) : RulesReadState
}

internal sealed interface RuleProvidersReadState {
    data class Loading(val lastAccepted: RuleProvidersSnapshot? = null) : RuleProvidersReadState
    data class Observed(val snapshot: RuleProvidersSnapshot) : RuleProvidersReadState
    data class Unavailable(
        val lastAccepted: RuleProvidersSnapshot?,
        val failure: AppFailure,
    ) : RuleProvidersReadState
}

internal sealed interface RulesOperationState {
    data object Loading : RulesOperationState
    data object Ready : RulesOperationState
    data object SavingRules : RulesOperationState
    data object RulesSaved : RulesOperationState
    data object SavingProviders : RulesOperationState
    data object ProvidersSaved : RulesOperationState
    data class Failed(val failure: AppFailure) : RulesOperationState
    data class Cancelled(val failure: AppFailure) : RulesOperationState
    data class TimedOut(val failure: AppFailure) : RulesOperationState
    data class Unconfirmed(val failure: AppFailure) : RulesOperationState
}

internal enum class RulesActivity {
    Busy,
    Interactive,
}

internal enum class RulesObservationAvailability {
    Available,
    Unavailable,
}

internal enum class RulesDraftStatus {
    Unavailable,
    Clean,
    Changed,
}

internal enum class RulesReloadAvailability {
    Available,
    Busy,
    UnsavedChanges,
}

internal enum class RulesSnapshotAdmission {
    Accepted,
    Stale,
}

internal enum class RulesDomain {
    Rules,
    Providers,
    Unsupported,
}

internal enum class RulesRequestKind(val domain: RulesDomain) {
    ReadRules(RulesDomain.Rules),
    WriteRules(RulesDomain.Rules),
    ReadProviders(RulesDomain.Providers),
    WriteProviders(RulesDomain.Providers),
    Unsupported(RulesDomain.Unsupported),
}

internal sealed interface RulesFailureState {
    data object None : RulesFailureState
    data class Present(val failure: AppFailure) : RulesFailureState
}

internal data class RulesUiState(
    val rules: RulesReadState = RulesReadState.Loading(),
    val providers: RuleProvidersReadState = RuleProvidersReadState.Loading(),
    val rulesDraft: List<AndroidRuleEntry> = emptyList(),
    val providersDraft: String = "",
    val newRule: String = "",
    val operation: RulesOperationState = RulesOperationState.Loading,
) {
    val activity: RulesActivity
        get() = when (operation) {
            RulesOperationState.Loading,
            RulesOperationState.SavingRules,
            RulesOperationState.SavingProviders -> RulesActivity.Busy
            RulesOperationState.Ready,
            RulesOperationState.RulesSaved,
            RulesOperationState.ProvidersSaved,
            is RulesOperationState.Failed,
            is RulesOperationState.Cancelled,
            is RulesOperationState.TimedOut,
            is RulesOperationState.Unconfirmed -> RulesActivity.Interactive
        }

    val failure: RulesFailureState
        get() = when (val current = operation) {
            is RulesOperationState.Failed -> RulesFailureState.Present(current.failure)
            is RulesOperationState.Cancelled -> RulesFailureState.Present(current.failure)
            is RulesOperationState.TimedOut -> RulesFailureState.Present(current.failure)
            is RulesOperationState.Unconfirmed -> RulesFailureState.Present(current.failure)
            RulesOperationState.Loading,
            RulesOperationState.Ready,
            RulesOperationState.SavingRules,
            RulesOperationState.RulesSaved,
            RulesOperationState.SavingProviders,
            RulesOperationState.ProvidersSaved -> RulesFailureState.None
        }

    val rulesAvailability: RulesObservationAvailability
        get() = acceptedRules(rules)?.let { RulesObservationAvailability.Available }
            ?: RulesObservationAvailability.Unavailable

    val providersAvailability: RulesObservationAvailability
        get() = acceptedRuleProviders(providers)?.let { RulesObservationAvailability.Available }
            ?: RulesObservationAvailability.Unavailable

    val rulesDraftStatus: RulesDraftStatus
        get() = when (val accepted = acceptedRules(rules)) {
            null -> RulesDraftStatus.Unavailable
            else -> if (accepted.entries == rulesDraft) RulesDraftStatus.Clean else RulesDraftStatus.Changed
        }

    val providersDraftStatus: RulesDraftStatus
        get() = when (val accepted = acceptedRuleProviders(providers)) {
            null -> RulesDraftStatus.Unavailable
            else -> if (accepted.json == providersDraft) RulesDraftStatus.Clean else RulesDraftStatus.Changed
        }

    val reloadAvailability: RulesReloadAvailability
        get() = when (activity) {
            RulesActivity.Busy -> RulesReloadAvailability.Busy
            RulesActivity.Interactive -> when (rulesDraftStatus) {
                RulesDraftStatus.Changed -> RulesReloadAvailability.UnsavedChanges
                RulesDraftStatus.Unavailable,
                RulesDraftStatus.Clean -> when (providersDraftStatus) {
                    RulesDraftStatus.Changed -> RulesReloadAvailability.UnsavedChanges
                    RulesDraftStatus.Unavailable,
                    RulesDraftStatus.Clean -> RulesReloadAvailability.Available
                }
            }
        }
}

internal fun acceptedRules(state: RulesReadState): RulesSnapshot? = when (state) {
    is RulesReadState.Loading -> state.lastAccepted
    is RulesReadState.Observed -> state.snapshot
    is RulesReadState.Unavailable -> state.lastAccepted
}

internal fun acceptedRuleProviders(state: RuleProvidersReadState): RuleProvidersSnapshot? = when (state) {
    is RuleProvidersReadState.Loading -> state.lastAccepted
    is RuleProvidersReadState.Observed -> state.snapshot
    is RuleProvidersReadState.Unavailable -> state.lastAccepted
}

internal fun admitsRulesRevision(
    current: RulesSnapshot?,
    incoming: RulesRevision,
): RulesSnapshotAdmission {
    if (current == null || current.revision.ownerSessionId != incoming.ownerSessionId) {
        return RulesSnapshotAdmission.Accepted
    }
    val previous = current.revision
    return when {
        incoming.coreGeneration > previous.coreGeneration -> RulesSnapshotAdmission.Accepted
        incoming.coreGeneration < previous.coreGeneration -> RulesSnapshotAdmission.Stale
        incoming.dataRevision >= previous.dataRevision -> RulesSnapshotAdmission.Accepted
        else -> RulesSnapshotAdmission.Stale
    }
}

internal fun admitsProvidersRevision(
    current: RuleProvidersSnapshot?,
    incoming: RulesRevision,
): RulesSnapshotAdmission {
    if (current == null || current.revision.ownerSessionId != incoming.ownerSessionId) {
        return RulesSnapshotAdmission.Accepted
    }
    val previous = current.revision
    return when {
        incoming.coreGeneration > previous.coreGeneration -> RulesSnapshotAdmission.Accepted
        incoming.coreGeneration < previous.coreGeneration -> RulesSnapshotAdmission.Stale
        incoming.dataRevision >= previous.dataRevision -> RulesSnapshotAdmission.Accepted
        else -> RulesSnapshotAdmission.Stale
    }
}
