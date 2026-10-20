package com.musicfrog.infiltrator.ui.settings.tun

import com.musicfrog.infiltrator.ui.common.AppFailure
import infiltrator_android.VpnTunSettings
import infiltrator_android.VpnTunSettingsPatch
import infiltrator_android.VpnTunStackUpdate

internal data class TunSettingsRevision(val ownerSessionId: String, val coreGeneration: ULong, val dataRevision: ULong)

internal data class TunSettingsObservationPoint(val value: VpnTunSettings, val revision: TunSettingsRevision)

internal sealed interface TunSettingsReadState {
    data class Loading(val lastAccepted: TunSettingsObservationPoint? = null) : TunSettingsReadState

    data class Observed(val point: TunSettingsObservationPoint) : TunSettingsReadState

    data class Unavailable(val lastAccepted: TunSettingsObservationPoint?, val failure: AppFailure) : TunSettingsReadState
}

internal data class TunSettingsDraft(
    val mtu: String = "",
    val stack: String = "",
    val autoRoute: Boolean = true,
    val strictRoute: Boolean = false,
    val autoDetectInterface: Boolean = false,
)

internal sealed interface TunOperationState {
    data object Loading : TunOperationState
    data object Saving : TunOperationState
    data object Idle : TunOperationState
    data class Saved(val revision: TunSettingsRevision) : TunOperationState
    data class Failed(val failure: AppFailure) : TunOperationState
    data class Cancelled(val failure: AppFailure) : TunOperationState
    data class Unconfirmed(val failure: AppFailure) : TunOperationState
}

internal data class TunUiState(
    val settings: TunSettingsReadState = TunSettingsReadState.Loading(),
    val draft: TunSettingsDraft = TunSettingsDraft(),
    val operation: TunOperationState = TunOperationState.Loading,
) {
    val isBusy: Boolean
        get() = operation is TunOperationState.Loading || operation is TunOperationState.Saving

    val failure: AppFailure?
        get() = when (val current = operation) {
            is TunOperationState.Failed -> current.failure

            is TunOperationState.Cancelled -> current.failure

            is TunOperationState.Unconfirmed -> current.failure

            TunOperationState.Idle,
            TunOperationState.Loading,
            TunOperationState.Saving,
            is TunOperationState.Saved,
            -> null
        }
}

internal enum class TunReadbackMatch {
    Confirmed,
    NotConfirmed,
}

internal fun VpnTunSettings.matches(patch: VpnTunSettingsPatch): TunReadbackMatch = if (
    (patch.mtu == null || mtu == patch.mtu) &&
    (patch.autoRoute == null || autoRoute == patch.autoRoute) &&
    (patch.strictRoute == null || strictRoute == patch.strictRoute) &&
    stackMatches(patch.stackUpdate) &&
    (patch.autoDetectInterface == null || autoDetectInterface == patch.autoDetectInterface)
) {
    TunReadbackMatch.Confirmed
} else {
    TunReadbackMatch.NotConfirmed
}

private fun VpnTunSettings.stackMatches(update: VpnTunStackUpdate): Boolean = when (update) {
    VpnTunStackUpdate.Keep -> true
    VpnTunStackUpdate.Clear -> stack == null
    is VpnTunStackUpdate.Set -> stack == update.stack.name.lowercase()
}

internal fun lastAcceptedTun(state: TunSettingsReadState): TunSettingsObservationPoint? = when (state) {
    is TunSettingsReadState.Loading -> state.lastAccepted
    is TunSettingsReadState.Observed -> state.point
    is TunSettingsReadState.Unavailable -> state.lastAccepted
}

internal fun unavailableTun(previous: TunSettingsReadState, failure: AppFailure): TunSettingsReadState =
    TunSettingsReadState.Unavailable(lastAcceptedTun(previous), failure)

internal fun acceptsTunRevision(current: TunSettingsReadState, incoming: TunSettingsRevision): Boolean {
    val accepted = lastAcceptedTun(current)?.revision ?: return true
    if (accepted.ownerSessionId != incoming.ownerSessionId) return true
    return when {
        incoming.coreGeneration > accepted.coreGeneration -> true
        incoming.coreGeneration < accepted.coreGeneration -> false
        else -> incoming.dataRevision >= accepted.dataRevision
    }
}

internal fun TunSettingsObservationPoint.toDraft(): TunSettingsDraft = TunSettingsDraft(
    mtu = value.mtu?.toString().orEmpty(),
    stack = value.stack.orEmpty(),
    autoRoute = value.autoRoute ?: true,
    strictRoute = value.strictRoute ?: false,
    autoDetectInterface = value.autoDetectInterface ?: false,
)
