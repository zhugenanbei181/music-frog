package com.musicfrog.infiltrator.ui.profiles

import com.musicfrog.infiltrator.ui.common.AppFailure
import infiltrator_android.AndroidProfileSelectionTarget
import infiltrator_android.ProfileDetail
import infiltrator_android.ProfileSummary

internal data class ProfilesRevision(val ownerSessionId: String, val coreGeneration: ULong, val dataRevision: ULong)

internal data class ProfilesSnapshot(val profiles: List<ProfileSummary>, val revision: ProfilesRevision)

internal data class ProfilesObservationPoint(val value: ProfilesSnapshot, val requestSequence: ULong)

internal data class ProfileDetailSnapshot(val profile: ProfileDetail, val revision: ProfilesRevision)

internal data class ProfileDetailPoint(val value: ProfileDetailSnapshot, val requestSequence: ULong)

internal sealed interface ProfilesReadState {
    data class Loading(val lastAccepted: ProfilesObservationPoint? = null) : ProfilesReadState
    data class Observed(val point: ProfilesObservationPoint) : ProfilesReadState
    data class Unavailable(val lastAccepted: ProfilesObservationPoint?, val failure: AppFailure) : ProfilesReadState
}

internal sealed interface ProfileDetailReadState {
    data object Idle : ProfileDetailReadState
    data class Loading(val lastAccepted: ProfileDetailPoint? = null) : ProfileDetailReadState
    data class Observed(val point: ProfileDetailPoint) : ProfileDetailReadState
    data class Unavailable(val lastAccepted: ProfileDetailPoint?, val failure: AppFailure) : ProfileDetailReadState
}

internal enum class ProfileOperation {
    ReadProfiles,
    ReadDetail,
    Create,
    Select,
    Update,
    Save,
    Delete,
    SaveSubscription,
    ClearSubscription,
}

internal enum class ProfileRequestAvailability {
    Available,
    Busy,
}

internal sealed interface ProfileActionState {
    data object Idle : ProfileActionState
    data class Working(val operation: ProfileOperation) : ProfileActionState
    data class Failed(val operation: ProfileOperation, val failure: AppFailure) : ProfileActionState
    data class Cancelled(val operation: ProfileOperation, val failure: AppFailure) : ProfileActionState
    data class TimedOut(val operation: ProfileOperation, val failure: AppFailure) : ProfileActionState
    data class Unconfirmed(val operation: ProfileOperation, val failure: AppFailure) : ProfileActionState
}

data class LocalImportDraft(val suggestedName: String, val content: String)

data class ProfileDraftsState(
    val showAddDialog: Boolean = false,
    val prefilledUrl: String? = null,
    val deletingProfile: ProfileSummary? = null,
    val subscriptionProfile: ProfileSummary? = null,
    val localImportDraft: LocalImportDraft? = null,
)

internal data class ProfilesUiState(
    val collection: ProfilesReadState = ProfilesReadState.Loading(),
    val detail: ProfileDetailReadState = ProfileDetailReadState.Idle,
    val action: ProfileActionState = ProfileActionState.Idle,
    val drafts: ProfileDraftsState = ProfileDraftsState(),
)

internal sealed interface ProfileReadbackExpectation {
    data class Present(val name: String) : ProfileReadbackExpectation
    data class Active(val name: String) : ProfileReadbackExpectation
    data class ActiveReference(val target: AndroidProfileSelectionTarget) : ProfileReadbackExpectation
    data class Absent(val name: String) : ProfileReadbackExpectation
    data class Subscription(val name: String, val url: String, val updatePolicy: SubscriptionUpdatePolicy, val intervalHours: Int?) :
        ProfileReadbackExpectation
    data class SubscriptionCleared(val name: String) : ProfileReadbackExpectation
    data class SavedContent(val name: String, val content: String, val activation: ProfileActivation) : ProfileReadbackExpectation
}

internal enum class ProfileActivation {
    Active,
    Inactive,
}

internal enum class SubscriptionUpdatePolicy {
    Enabled,
    Disabled,
}

internal enum class ProfileReadbackResult {
    Confirmed,
    NotConfirmed,
}

internal enum class ProfilesSnapshotAdmission {
    Accepted,
    Stale,
    Conflicting,
}

internal fun ProfilesSnapshot.satisfies(expectation: ProfileReadbackExpectation): ProfileReadbackResult = when (expectation) {
    is ProfileReadbackExpectation.Present -> profiles.any { it.name == expectation.name }.toReadbackResult()

    is ProfileReadbackExpectation.Active -> profiles.any { it.name == expectation.name && it.active }.toReadbackResult()

    is ProfileReadbackExpectation.ActiveReference -> ProfileReadbackResult.NotConfirmed

    is ProfileReadbackExpectation.Absent -> profiles.none { it.name == expectation.name }.toReadbackResult()

    is ProfileReadbackExpectation.Subscription ->
        profiles
            .firstOrNull { it.name == expectation.name }
            ?.matches(expectation)
            ?: ProfileReadbackResult.NotConfirmed

    is ProfileReadbackExpectation.SubscriptionCleared -> profiles.any {
        it.name == expectation.name &&
            it.subscriptionUrl == null &&
            !it.autoUpdateEnabled &&
            it.updateIntervalHours == null
    }.toReadbackResult()

    is ProfileReadbackExpectation.SavedContent -> profiles.any {
        it.name == expectation.name && it.active == (expectation.activation == ProfileActivation.Active)
    }.toReadbackResult()
}

private fun ProfileSummary.matches(expectation: ProfileReadbackExpectation.Subscription): ProfileReadbackResult {
    if (subscriptionUrl != expectation.url) return ProfileReadbackResult.NotConfirmed
    if (autoUpdateEnabled != (expectation.updatePolicy == SubscriptionUpdatePolicy.Enabled)) {
        return ProfileReadbackResult.NotConfirmed
    }
    val intervalHours = expectation.intervalHours ?: return when (updateIntervalHours) {
        null -> ProfileReadbackResult.Confirmed
        else -> ProfileReadbackResult.NotConfirmed
    }
    if (intervalHours < 0) return ProfileReadbackResult.NotConfirmed
    return when (updateIntervalHours == intervalHours.toUInt()) {
        true -> ProfileReadbackResult.Confirmed
        false -> ProfileReadbackResult.NotConfirmed
    }
}

internal fun ProfileDetailSnapshot.satisfies(expectation: ProfileReadbackExpectation.SavedContent): ProfileReadbackResult = when {
    profile.name != expectation.name -> ProfileReadbackResult.NotConfirmed
    profile.content != expectation.content -> ProfileReadbackResult.NotConfirmed
    profile.active != (expectation.activation == ProfileActivation.Active) -> ProfileReadbackResult.NotConfirmed
    else -> ProfileReadbackResult.Confirmed
}

private fun Boolean.toReadbackResult(): ProfileReadbackResult = when (this) {
    true -> ProfileReadbackResult.Confirmed
    false -> ProfileReadbackResult.NotConfirmed
}

internal fun acceptedProfiles(state: ProfilesReadState): ProfilesObservationPoint? = when (state) {
    is ProfilesReadState.Loading -> state.lastAccepted
    is ProfilesReadState.Observed -> state.point
    is ProfilesReadState.Unavailable -> state.lastAccepted
}

internal fun acceptedProfileDetail(state: ProfileDetailReadState): ProfileDetailPoint? = when (state) {
    ProfileDetailReadState.Idle -> null
    is ProfileDetailReadState.Loading -> state.lastAccepted
    is ProfileDetailReadState.Observed -> state.point
    is ProfileDetailReadState.Unavailable -> state.lastAccepted
}

internal fun admitsProfilesSnapshot(current: ProfilesObservationPoint?, incoming: ProfilesObservationPoint): ProfilesSnapshotAdmission {
    if (current == null) return ProfilesSnapshotAdmission.Accepted
    if (incoming.requestSequence < current.requestSequence) return ProfilesSnapshotAdmission.Stale

    val previousRevision = current.value.revision
    val incomingRevision = incoming.value.revision
    if (incomingRevision.ownerSessionId != previousRevision.ownerSessionId) {
        return ProfilesSnapshotAdmission.Accepted
    }
    if (incomingRevision.coreGeneration > previousRevision.coreGeneration) {
        return ProfilesSnapshotAdmission.Accepted
    }
    if (incomingRevision.coreGeneration < previousRevision.coreGeneration) {
        return ProfilesSnapshotAdmission.Stale
    }
    if (incomingRevision.dataRevision > previousRevision.dataRevision) {
        return ProfilesSnapshotAdmission.Accepted
    }
    if (incomingRevision.dataRevision < previousRevision.dataRevision) {
        return ProfilesSnapshotAdmission.Stale
    }
    return when (incoming.value.profiles == current.value.profiles) {
        true -> ProfilesSnapshotAdmission.Accepted
        false -> ProfilesSnapshotAdmission.Conflicting
    }
}

internal fun admitsProfileDetail(current: ProfileDetailPoint?, incoming: ProfileDetailPoint): ProfilesSnapshotAdmission {
    if (current == null) return ProfilesSnapshotAdmission.Accepted
    if (incoming.requestSequence < current.requestSequence) return ProfilesSnapshotAdmission.Stale

    val previousRevision = current.value.revision
    val incomingRevision = incoming.value.revision
    if (incomingRevision.ownerSessionId != previousRevision.ownerSessionId) {
        return ProfilesSnapshotAdmission.Accepted
    }
    if (incomingRevision.coreGeneration > previousRevision.coreGeneration) {
        return ProfilesSnapshotAdmission.Accepted
    }
    if (incomingRevision.coreGeneration < previousRevision.coreGeneration) {
        return ProfilesSnapshotAdmission.Stale
    }
    if (incomingRevision.dataRevision > previousRevision.dataRevision) {
        return ProfilesSnapshotAdmission.Accepted
    }
    if (incomingRevision.dataRevision < previousRevision.dataRevision) {
        return ProfilesSnapshotAdmission.Stale
    }
    if (incoming.value.profile.name != current.value.profile.name) {
        return ProfilesSnapshotAdmission.Accepted
    }
    return when (incoming.value.profile == current.value.profile) {
        true -> ProfilesSnapshotAdmission.Accepted
        false -> ProfilesSnapshotAdmission.Conflicting
    }
}
