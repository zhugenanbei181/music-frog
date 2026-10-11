package com.musicfrog.infiltrator.ui.profiles

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.musicfrog.infiltrator.ui.common.AppFailure
import com.musicfrog.infiltrator.ui.common.FailureTextKey
import com.musicfrog.infiltrator.ui.common.DEFAULT_FFI_TIMEOUT_MS
import com.musicfrog.infiltrator.ui.common.LONG_FFI_TIMEOUT_MS
import com.musicfrog.infiltrator.ui.common.localizedFailure
import com.musicfrog.infiltrator.ui.common.toAppFailure
import infiltrator_android.AndroidOwnerOperation
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

class ProfilesViewModel internal constructor(
    private val repository: ProfilesRepository,
) : ViewModel() {
    constructor() : this(UniFfiProfilesRepository())

    private val _state = MutableStateFlow(ProfilesUiState())
    internal val state: StateFlow<ProfilesUiState> = _state.asStateFlow()

    private var requestSequence = 0uL
    private var latestRequestSequence = 0uL

    init {
        loadProfiles()
    }

    fun loadProfiles() {
        when (requestAvailability()) {
            ProfileRequestAvailability.Busy -> return
            ProfileRequestAvailability.Available -> Unit
        }
        val sequence = nextRequestSequence()
        _state.value = _state.value.copy(
            collection = ProfilesReadState.Loading(acceptedProfiles(_state.value.collection)),
            action = ProfileActionState.Working(ProfileOperation.ReadProfiles),
        )
        viewModelScope.launch {
            when (val result = repository.execute(AndroidOwnerOperation.ProfilesList, DEFAULT_FFI_TIMEOUT_MS)) {
                is ProfilesRepositoryResult.Completed -> when (val observation = result.observation) {
                    is ProfilesRepositoryObservation.Profiles -> when (
                        acceptProfilesSnapshot(observation.snapshot, sequence)
                    ) {
                        ProfilesSnapshotAdmission.Accepted -> setIdle()
                        ProfilesSnapshotAdmission.Stale,
                        ProfilesSnapshotAdmission.Conflicting -> setUnconfirmed(
                            ProfileOperation.ReadProfiles,
                            notReadyFailure(),
                        )
                    }
                    is ProfilesRepositoryObservation.Detail -> setUnconfirmed(
                        ProfileOperation.ReadProfiles,
                        notReadyFailure(),
                    )
                }
                is ProfilesRepositoryResult.Failed -> {
                    acceptLatestProfiles(result.latest, sequence)
                    markProfilesUnavailable(result.failure)
                    setFailed(ProfileOperation.ReadProfiles, result.failure)
                }
                is ProfilesRepositoryResult.Cancelled -> {
                    acceptLatestProfiles(result.latest, sequence)
                    markProfilesUnavailable(result.failure)
                    setCancelled(ProfileOperation.ReadProfiles, result.failure)
                }
                is ProfilesRepositoryResult.TimedOut -> {
                    markProfilesUnavailable(result.failure)
                    setTimedOut(ProfileOperation.ReadProfiles, result.failure)
                }
                is ProfilesRepositoryResult.Unconfirmed -> {
                    acceptLatestProfiles(result.latest, sequence)
                    markProfilesUnavailable(result.failure)
                    setUnconfirmed(ProfileOperation.ReadProfiles, result.failure)
                }
            }
        }
    }

    fun addProfile(name: String, url: String) = executeProfileCommand(
        operation = ProfileOperation.Create,
        expectation = ProfileReadbackExpectation.Present(name),
        command = AndroidOwnerOperation.ProfileCreate(name, url),
    )

    fun selectProfile(name: String) = executeProfileCommand(
        operation = ProfileOperation.Select,
        expectation = ProfileReadbackExpectation.Active(name),
        command = AndroidOwnerOperation.ProfileSelect(name),
    )

    fun updateProfile(name: String) = executeProfileCommand(
        operation = ProfileOperation.Update,
        expectation = ProfileReadbackExpectation.Present(name),
        command = AndroidOwnerOperation.ProfileUpdate(name),
    )

    fun loadProfileDetail(name: String) {
        when (requestAvailability()) {
            ProfileRequestAvailability.Busy -> return
            ProfileRequestAvailability.Available -> Unit
        }
        val sequence = nextRequestSequence()
        val previous = acceptedProfileDetail(_state.value.detail)
            ?.takeIf { it.value.profile.name == name }
        _state.value = _state.value.copy(
            detail = ProfileDetailReadState.Loading(previous),
            action = ProfileActionState.Working(ProfileOperation.ReadDetail),
        )
        viewModelScope.launch {
            handleDetailResult(
                result = repository.execute(AndroidOwnerOperation.ProfileDetail(name), DEFAULT_FFI_TIMEOUT_MS),
                sequence = sequence,
                operation = ProfileOperation.ReadDetail,
            )
        }
    }

    fun saveProfileContent(name: String, content: String, activation: ProfileActivation) = executeProfileCommand(
        operation = ProfileOperation.Save,
        expectation = ProfileReadbackExpectation.SavedContent(name, content, activation),
        command = AndroidOwnerOperation.ProfileSave(
            name = name,
            content = content,
            activate = activation.toWireValue(),
        ),
    )

    fun deleteProfile(name: String) = executeProfileCommand(
        operation = ProfileOperation.Delete,
        expectation = ProfileReadbackExpectation.Absent(name),
        command = AndroidOwnerOperation.ProfileDelete(name),
        onConfirmed = {
            if (acceptedProfileDetail(_state.value.detail)?.value?.profile?.name == name) {
                _state.value = _state.value.copy(detail = ProfileDetailReadState.Idle)
            }
        },
    )

    fun saveSubscriptionSettings(
        name: String,
        url: String,
        updatePolicy: SubscriptionUpdatePolicy,
        updateIntervalHours: Int?,
    ) {
        when (requestAvailability()) {
            ProfileRequestAvailability.Busy -> return
            ProfileRequestAvailability.Available -> Unit
        }
        val intervalHours = updateIntervalHours?.let { hours ->
            when {
                hours >= 0 -> hours.toUInt()
                else -> {
                    setFailed(ProfileOperation.SaveSubscription, invalidProfileInputFailure())
                    return
                }
            }
        }
        executeProfileCommand(
            operation = ProfileOperation.SaveSubscription,
            expectation = ProfileReadbackExpectation.Subscription(
                name = name,
                url = url,
                updatePolicy = updatePolicy,
                intervalHours = updateIntervalHours,
            ),
            command = AndroidOwnerOperation.ProfileSubscriptionSave(
                name = name,
                url = url,
                autoUpdateEnabled = updatePolicy.toWireValue(),
                updateIntervalHours = intervalHours,
            ),
        )
    }

    fun clearSubscriptionSettings(name: String) = executeProfileCommand(
        operation = ProfileOperation.ClearSubscription,
        expectation = ProfileReadbackExpectation.SubscriptionCleared(name),
        command = AndroidOwnerOperation.ProfileSubscriptionClear(name),
    )

    fun clearProfileDetail() {
        _state.value = _state.value.copy(detail = ProfileDetailReadState.Idle)
    }

    fun clearError() {
        val current = _state.value
        val nextAction = when (current.action) {
            is ProfileActionState.Failed,
            is ProfileActionState.Cancelled,
            is ProfileActionState.TimedOut,
            is ProfileActionState.Unconfirmed -> ProfileActionState.Idle
            ProfileActionState.Idle,
            is ProfileActionState.Working -> current.action
        }
        val nextDetail = when (current.detail) {
            is ProfileDetailReadState.Unavailable -> ProfileDetailReadState.Idle
            ProfileDetailReadState.Idle,
            is ProfileDetailReadState.Loading,
            is ProfileDetailReadState.Observed -> current.detail
        }
        val nextCollection = when (val collection = current.collection) {
            is ProfilesReadState.Unavailable -> collection.lastAccepted?.let { ProfilesReadState.Observed(it) }
                ?: ProfilesReadState.Loading()
            is ProfilesReadState.Loading,
            is ProfilesReadState.Observed -> collection
        }
        _state.value = current.copy(action = nextAction, collection = nextCollection, detail = nextDetail)
    }

    private fun executeProfileCommand(
        operation: ProfileOperation,
        expectation: ProfileReadbackExpectation,
        command: AndroidOwnerOperation,
        onConfirmed: () -> Unit = {},
    ) {
        when (requestAvailability()) {
            ProfileRequestAvailability.Busy -> return
            ProfileRequestAvailability.Available -> Unit
        }
        val sequence = nextRequestSequence()
        _state.value = _state.value.copy(action = ProfileActionState.Working(operation))
        viewModelScope.launch {
            when (val result = repository.execute(command, LONG_FFI_TIMEOUT_MS)) {
                is ProfilesRepositoryResult.Completed -> when (val observation = result.observation) {
                    is ProfilesRepositoryObservation.Profiles -> {
                        when (acceptProfilesSnapshot(observation.snapshot, sequence)) {
                            ProfilesSnapshotAdmission.Accepted -> when (
                                observation.snapshot.satisfies(expectation)
                            ) {
                                ProfileReadbackResult.Confirmed -> when (expectation) {
                                    is ProfileReadbackExpectation.SavedContent -> confirmSavedContent(
                                        expectation,
                                        operation,
                                    )
                                    else -> {
                                        onConfirmed()
                                        setIdle()
                                    }
                                }
                                ProfileReadbackResult.NotConfirmed -> setUnconfirmed(
                                    operation,
                                    notReadyFailure(),
                                )
                            }
                            ProfilesSnapshotAdmission.Stale,
                            ProfilesSnapshotAdmission.Conflicting -> setUnconfirmed(
                                operation,
                                notReadyFailure(),
                            )
                        }
                    }
                    is ProfilesRepositoryObservation.Detail -> setUnconfirmed(operation, notReadyFailure())
                }
                is ProfilesRepositoryResult.Failed -> {
                    acceptLatestProfiles(result.latest, sequence)
                    setFailed(operation, result.failure)
                }
                is ProfilesRepositoryResult.Cancelled -> {
                    acceptLatestProfiles(result.latest, sequence)
                    setCancelled(operation, result.failure)
                }
                is ProfilesRepositoryResult.TimedOut -> setTimedOut(operation, result.failure)
                is ProfilesRepositoryResult.Unconfirmed -> {
                    acceptLatestProfiles(result.latest, sequence)
                    setUnconfirmed(operation, result.failure)
                }
            }
        }
    }

    private suspend fun confirmSavedContent(
        expectation: ProfileReadbackExpectation.SavedContent,
        operation: ProfileOperation,
    ) {
        val sequence = nextRequestSequence()
        when (val result = repository.execute(
            AndroidOwnerOperation.ProfileDetail(expectation.name),
            DEFAULT_FFI_TIMEOUT_MS,
        )) {
            is ProfilesRepositoryResult.Completed -> when (val observation = result.observation) {
                is ProfilesRepositoryObservation.Detail -> {
                    when (acceptProfilesSnapshot(observation.profiles, sequence)) {
                        ProfilesSnapshotAdmission.Accepted -> when (
                            observation.detail.satisfies(expectation)
                        ) {
                            ProfileReadbackResult.Confirmed -> when (
                                acceptProfileDetail(observation.detail, sequence)
                            ) {
                                ProfilesSnapshotAdmission.Accepted -> setIdle()
                                ProfilesSnapshotAdmission.Stale,
                                ProfilesSnapshotAdmission.Conflicting -> setUnconfirmed(
                                    operation,
                                    notReadyFailure(),
                                )
                            }
                            ProfileReadbackResult.NotConfirmed -> setUnconfirmed(
                                operation,
                                notReadyFailure(),
                            )
                        }
                        ProfilesSnapshotAdmission.Stale,
                        ProfilesSnapshotAdmission.Conflicting -> setUnconfirmed(operation, notReadyFailure())
                    }
                }
                is ProfilesRepositoryObservation.Profiles -> setUnconfirmed(operation, notReadyFailure())
            }
            is ProfilesRepositoryResult.Failed -> {
                acceptLatestProfiles(result.latest, sequence)
                setUnconfirmed(operation, result.failure)
            }
            is ProfilesRepositoryResult.Cancelled -> {
                acceptLatestProfiles(result.latest, sequence)
                setCancelled(operation, result.failure)
            }
            is ProfilesRepositoryResult.TimedOut -> setTimedOut(operation, result.failure)
            is ProfilesRepositoryResult.Unconfirmed -> {
                acceptLatestProfiles(result.latest, sequence)
                setUnconfirmed(operation, result.failure)
            }
        }
    }

    private suspend fun handleDetailResult(
        result: ProfilesRepositoryResult,
        sequence: ULong,
        operation: ProfileOperation,
    ) {
        if (sequence != latestRequestSequence) return
        when (result) {
            is ProfilesRepositoryResult.Completed -> when (val observation = result.observation) {
                is ProfilesRepositoryObservation.Detail -> {
                    when (acceptProfilesSnapshot(observation.profiles, sequence)) {
                        ProfilesSnapshotAdmission.Accepted -> when (
                            acceptProfileDetail(observation.detail, sequence)
                        ) {
                            ProfilesSnapshotAdmission.Accepted -> setIdle()
                            ProfilesSnapshotAdmission.Stale,
                            ProfilesSnapshotAdmission.Conflicting -> {
                                markDetailUnavailable(notReadyFailure())
                                setUnconfirmed(operation, notReadyFailure())
                            }
                        }
                        ProfilesSnapshotAdmission.Stale,
                        ProfilesSnapshotAdmission.Conflicting -> {
                            markDetailUnavailable(notReadyFailure())
                            setUnconfirmed(operation, notReadyFailure())
                        }
                    }
                }
                is ProfilesRepositoryObservation.Profiles -> {
                    markDetailUnavailable(notReadyFailure())
                    setUnconfirmed(operation, notReadyFailure())
                }
            }
            is ProfilesRepositoryResult.Failed -> {
                acceptLatestProfiles(result.latest, sequence)
                markDetailUnavailable(result.failure)
                setFailed(operation, result.failure)
            }
            is ProfilesRepositoryResult.Cancelled -> {
                acceptLatestProfiles(result.latest, sequence)
                markDetailUnavailable(result.failure)
                setCancelled(operation, result.failure)
            }
            is ProfilesRepositoryResult.TimedOut -> {
                markDetailUnavailable(result.failure)
                setTimedOut(operation, result.failure)
            }
            is ProfilesRepositoryResult.Unconfirmed -> {
                acceptLatestProfiles(result.latest, sequence)
                markDetailUnavailable(result.failure)
                setUnconfirmed(operation, result.failure)
            }
        }
    }

    private fun acceptProfilesSnapshot(
        snapshot: ProfilesSnapshot,
        sequence: ULong,
    ): ProfilesSnapshotAdmission {
        if (sequence != latestRequestSequence) return ProfilesSnapshotAdmission.Stale
        val point = ProfilesObservationPoint(snapshot, sequence)
        return when (val admission = admitsProfilesSnapshot(acceptedProfiles(_state.value.collection), point)) {
            ProfilesSnapshotAdmission.Accepted -> {
                _state.value = _state.value.copy(collection = ProfilesReadState.Observed(point))
                ProfilesSnapshotAdmission.Accepted
            }
            ProfilesSnapshotAdmission.Stale -> ProfilesSnapshotAdmission.Stale
            ProfilesSnapshotAdmission.Conflicting -> ProfilesSnapshotAdmission.Conflicting
        }
    }

    private fun acceptProfileDetail(
        snapshot: ProfileDetailSnapshot,
        sequence: ULong,
    ): ProfilesSnapshotAdmission {
        if (sequence != latestRequestSequence) return ProfilesSnapshotAdmission.Stale
        val point = ProfileDetailPoint(snapshot, sequence)
        return when (val admission = admitsProfileDetail(acceptedProfileDetail(_state.value.detail), point)) {
            ProfilesSnapshotAdmission.Accepted -> {
                _state.value = _state.value.copy(detail = ProfileDetailReadState.Observed(point))
                ProfilesSnapshotAdmission.Accepted
            }
            ProfilesSnapshotAdmission.Stale -> ProfilesSnapshotAdmission.Stale
            ProfilesSnapshotAdmission.Conflicting -> ProfilesSnapshotAdmission.Conflicting
        }
    }

    private fun acceptLatestProfiles(snapshot: ProfilesSnapshot?, sequence: ULong) {
        when (snapshot) {
            null -> Unit
            else -> acceptProfilesSnapshot(snapshot, sequence)
        }
    }

    private fun markProfilesUnavailable(failure: AppFailure) {
        _state.value = _state.value.copy(
            collection = ProfilesReadState.Unavailable(acceptedProfiles(_state.value.collection), failure),
        )
    }

    private fun markDetailUnavailable(failure: AppFailure) {
        _state.value = _state.value.copy(
            detail = ProfileDetailReadState.Unavailable(acceptedProfileDetail(_state.value.detail), failure),
        )
    }

    private fun setIdle() {
        _state.value = _state.value.copy(action = ProfileActionState.Idle)
    }

    private fun setFailed(operation: ProfileOperation, failure: AppFailure) {
        _state.value = _state.value.copy(action = ProfileActionState.Failed(operation, failure))
    }

    private fun setCancelled(operation: ProfileOperation, failure: AppFailure) {
        _state.value = _state.value.copy(action = ProfileActionState.Cancelled(operation, failure))
    }

    private fun setTimedOut(operation: ProfileOperation, failure: AppFailure) {
        _state.value = _state.value.copy(action = ProfileActionState.TimedOut(operation, failure))
    }

    private fun setUnconfirmed(operation: ProfileOperation, failure: AppFailure) {
        _state.value = _state.value.copy(action = ProfileActionState.Unconfirmed(operation, failure))
    }

    private fun requestAvailability(): ProfileRequestAvailability = when (_state.value.action) {
        is ProfileActionState.Working -> ProfileRequestAvailability.Busy
        ProfileActionState.Idle,
        is ProfileActionState.Failed,
        is ProfileActionState.Cancelled,
        is ProfileActionState.TimedOut,
        is ProfileActionState.Unconfirmed -> ProfileRequestAvailability.Available
    }

    private fun nextRequestSequence(): ULong {
        requestSequence += 1uL
        latestRequestSequence = requestSequence
        return requestSequence
    }

    private fun notReadyFailure(): AppFailure = localizedFailure(FailureTextKey.NotReady).toAppFailure()

    private fun invalidProfileInputFailure(): AppFailure = localizedFailure(FailureTextKey.InvalidInput).toAppFailure()
}

private fun ProfileActivation.toWireValue(): Boolean = when (this) {
    ProfileActivation.Active -> true
    ProfileActivation.Inactive -> false
}

private fun SubscriptionUpdatePolicy.toWireValue(): Boolean = when (this) {
    SubscriptionUpdatePolicy.Enabled -> true
    SubscriptionUpdatePolicy.Disabled -> false
}
