package com.musicfrog.infiltrator.ui.settings.fakeip

import com.musicfrog.infiltrator.ui.common.AppFailure
import com.musicfrog.infiltrator.ui.common.ObservationRevision
import infiltrator_android.FakeIpSettings
import infiltrator_android.FakeIpSettingsPatch

internal data class FakeIpSettingsRevision(val ownerSessionId: String, val coreGeneration: ULong, val dataRevision: ULong)

internal data class FakeIpSettingsObservationPoint(val value: FakeIpSettings, val revision: FakeIpSettingsRevision)

internal sealed interface FakeIpSettingsReadState {
    data class Loading(val lastAccepted: FakeIpSettingsObservationPoint? = null) : FakeIpSettingsReadState
    data class Observed(val value: FakeIpSettings, val revision: FakeIpSettingsRevision) : FakeIpSettingsReadState
    data class Unavailable(val lastAccepted: FakeIpSettingsObservationPoint?, val failure: AppFailure) : FakeIpSettingsReadState
}

internal sealed interface FakeIpSettingsOperationState {
    data object Idle : FakeIpSettingsOperationState
    data object Reading : FakeIpSettingsOperationState
    data object Saving : FakeIpSettingsOperationState
    data class Saved(val revision: FakeIpSettingsRevision) : FakeIpSettingsOperationState
    data class Failed(val failure: AppFailure) : FakeIpSettingsOperationState
    data class Cancelled(val failure: AppFailure) : FakeIpSettingsOperationState
    data class TimedOut(val failure: AppFailure) : FakeIpSettingsOperationState
    data class Unconfirmed(val failure: AppFailure) : FakeIpSettingsOperationState
}

internal enum class FakeIpSettingsActivity {
    Busy,
    Interactive,
}

internal enum class FakeIpBooleanSetting {
    Unspecified,
    Enabled,
    Disabled,
}

internal fun Boolean?.toFakeIpBooleanSetting(): FakeIpBooleanSetting = when (this) {
    null -> FakeIpBooleanSetting.Unspecified
    true -> FakeIpBooleanSetting.Enabled
    false -> FakeIpBooleanSetting.Disabled
}

internal fun FakeIpBooleanSetting.toPatchValue(): Boolean? = when (this) {
    FakeIpBooleanSetting.Unspecified -> null
    FakeIpBooleanSetting.Enabled -> true
    FakeIpBooleanSetting.Disabled -> false
}

internal sealed interface HostFakeIpFileCacheState {
    data object Idle : HostFakeIpFileCacheState
    data object Clearing : HostFakeIpFileCacheState
    data object Cleared : HostFakeIpFileCacheState
    data object AlreadyEmpty : HostFakeIpFileCacheState
    data class Failed(val failure: AppFailure) : HostFakeIpFileCacheState
}

internal enum class FakeIpSettingsReadbackResult {
    Confirmed,
    NotConfirmed,
}

internal enum class FakeIpSnapshotAdmission {
    Accepted,
    Stale,
}

internal enum class FakeIpRequestAdmission {
    Current,
    Superseded,
}

internal fun hostFakeIpFileCacheReceipt(removedFile: Boolean): HostFakeIpFileCacheState = when (removedFile) {
    true -> HostFakeIpFileCacheState.Cleared
    false -> HostFakeIpFileCacheState.AlreadyEmpty
}

internal fun lastAcceptedFakeIp(state: FakeIpSettingsReadState): FakeIpSettingsObservationPoint? = when (state) {
    is FakeIpSettingsReadState.Loading -> state.lastAccepted
    is FakeIpSettingsReadState.Observed -> FakeIpSettingsObservationPoint(state.value, state.revision)
    is FakeIpSettingsReadState.Unavailable -> state.lastAccepted
}

internal fun unavailableFakeIp(previous: FakeIpSettingsReadState, failure: AppFailure): FakeIpSettingsReadState =
    lastAcceptedFakeIp(previous).let { accepted ->
        FakeIpSettingsReadState.Unavailable(accepted, failure)
    }

internal fun acceptsFakeIpRevision(current: FakeIpSettingsReadState, incoming: FakeIpSettingsRevision): FakeIpSnapshotAdmission {
    val accepted = lastAcceptedFakeIp(current)?.revision ?: return FakeIpSnapshotAdmission.Accepted
    if (accepted.ownerSessionId != incoming.ownerSessionId) return FakeIpSnapshotAdmission.Accepted
    return when {
        incoming.coreGeneration > accepted.coreGeneration -> FakeIpSnapshotAdmission.Accepted
        incoming.coreGeneration < accepted.coreGeneration -> FakeIpSnapshotAdmission.Stale
        incoming.dataRevision >= accepted.dataRevision -> FakeIpSnapshotAdmission.Accepted
        else -> FakeIpSnapshotAdmission.Stale
    }
}

internal fun admitsFakeIpRequest(latest: ObservationRevision.Request, incoming: ObservationRevision.Request): FakeIpRequestAdmission =
    when (incoming) {
        latest -> FakeIpRequestAdmission.Current
        else -> FakeIpRequestAdmission.Superseded
    }

internal fun FakeIpSettings.matches(patch: FakeIpSettingsPatch, expectedRange: String?): FakeIpSettingsReadbackResult {
    val matches =
        fakeIpRange == expectedRange &&
            (patch.fakeIpFilter == null || fakeIpFilter == patch.fakeIpFilter) &&
            (patch.storeFakeIp == null || storeFakeIp == patch.storeFakeIp)
    return when (matches) {
        true -> FakeIpSettingsReadbackResult.Confirmed
        false -> FakeIpSettingsReadbackResult.NotConfirmed
    }
}
