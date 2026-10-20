package com.musicfrog.infiltrator.ui.settings.dns

import com.musicfrog.infiltrator.ui.common.AppFailure
import infiltrator_android.DnsSettings
import infiltrator_android.DnsSettingsPatch

internal data class DnsSettingsObservationPoint(val value: DnsSettings, val revision: DnsSettingsRevision)

internal data class DnsSettingsRevision(val ownerSessionId: String, val coreGeneration: ULong, val dataRevision: ULong)

internal sealed interface DnsSettingsReadState {
    data class Loading(val lastAccepted: DnsSettingsObservationPoint? = null) : DnsSettingsReadState
    data class Observed(val value: DnsSettings, val revision: DnsSettingsRevision) : DnsSettingsReadState
    data class Unavailable(val lastAccepted: DnsSettingsObservationPoint?, val failure: AppFailure) : DnsSettingsReadState
}

internal enum class DnsGeoIpPolicy(val selectionValue: String) {
    Automatic(""),
    Enabled("true"),
    Disabled("false"),
    ;

    companion object {
        fun fromSelection(value: String): DnsGeoIpPolicy = when (value) {
            "" -> Automatic
            "true" -> Enabled
            "false" -> Disabled
            else -> error("Unsupported DNS GeoIP policy selection")
        }
    }
}

internal sealed interface DnsEnhancedMode {
    data object Unspecified : DnsEnhancedMode
    data object Disabled : DnsEnhancedMode
    data object FakeIp : DnsEnhancedMode
    data object RedirHost : DnsEnhancedMode
    data class Custom(val value: String) : DnsEnhancedMode
}

internal fun DnsEnhancedMode.selectionValue(): String = when (this) {
    DnsEnhancedMode.Unspecified,
    DnsEnhancedMode.Disabled,
    -> ""

    DnsEnhancedMode.FakeIp -> "fake-ip"

    DnsEnhancedMode.RedirHost -> "redir-host"

    is DnsEnhancedMode.Custom -> value
}

internal fun DnsEnhancedMode.toPatchValue(): String? = when (this) {
    DnsEnhancedMode.Unspecified,
    DnsEnhancedMode.Disabled,
    -> null

    DnsEnhancedMode.FakeIp -> "fake-ip"

    DnsEnhancedMode.RedirHost -> "redir-host"

    is DnsEnhancedMode.Custom -> value
}

internal fun DnsEnhancedMode.expectedReadback(previous: String?): String? = when (this) {
    DnsEnhancedMode.Unspecified -> previous
    DnsEnhancedMode.Disabled -> null
    DnsEnhancedMode.FakeIp -> "fake-ip"
    DnsEnhancedMode.RedirHost -> "redir-host"
    is DnsEnhancedMode.Custom -> value
}

internal fun dnsEnhancedModeFromReadback(value: String?): DnsEnhancedMode = when (value) {
    null, "" -> DnsEnhancedMode.Unspecified
    "fake-ip" -> DnsEnhancedMode.FakeIp
    "redir-host" -> DnsEnhancedMode.RedirHost
    else -> DnsEnhancedMode.Custom(value)
}

internal fun dnsEnhancedModeFromSelection(value: String): DnsEnhancedMode = when (value) {
    "" -> DnsEnhancedMode.Disabled
    "fake-ip" -> DnsEnhancedMode.FakeIp
    "redir-host" -> DnsEnhancedMode.RedirHost
    else -> error("Unsupported DNS enhanced mode selection")
}

internal enum class DnsToggleState {
    Unspecified,
    Enabled,
    Disabled,
}

internal fun Boolean?.toDnsToggleState(): DnsToggleState = when (this) {
    null -> DnsToggleState.Unspecified
    true -> DnsToggleState.Enabled
    false -> DnsToggleState.Disabled
}

internal fun DnsToggleState.toPatchValue(): Boolean? = when (this) {
    DnsToggleState.Unspecified -> null
    DnsToggleState.Enabled -> true
    DnsToggleState.Disabled -> false
}

internal sealed interface DnsSettingsOperationState {
    data object Idle : DnsSettingsOperationState
    data object Reading : DnsSettingsOperationState
    data object Saving : DnsSettingsOperationState
    data class Saved(val revision: DnsSettingsRevision) : DnsSettingsOperationState
    data class Failed(val failure: AppFailure) : DnsSettingsOperationState
    data class Cancelled(val failure: AppFailure) : DnsSettingsOperationState
    data class TimedOut(val failure: AppFailure) : DnsSettingsOperationState
    data class Unconfirmed(val failure: AppFailure) : DnsSettingsOperationState
}

internal enum class DnsActivity {
    Busy,
    Interactive,
}

internal enum class DnsSettingsAvailability {
    Available,
    Unavailable,
}

internal enum class DnsSnapshotAdmission {
    Accepted,
    Stale,
}

internal enum class DnsSettingsReadbackResult {
    Confirmed,
    NotConfirmed,
}

internal fun lastAcceptedDns(state: DnsSettingsReadState): DnsSettingsObservationPoint? = when (state) {
    is DnsSettingsReadState.Loading -> state.lastAccepted
    is DnsSettingsReadState.Observed -> DnsSettingsObservationPoint(state.value, state.revision)
    is DnsSettingsReadState.Unavailable -> state.lastAccepted
}

internal fun unavailableDns(previous: DnsSettingsReadState, failure: AppFailure): DnsSettingsReadState =
    lastAcceptedDns(previous).let { accepted ->
        DnsSettingsReadState.Unavailable(accepted, failure)
    }

internal fun acceptsDnsRevision(current: DnsSettingsReadState, incoming: DnsSettingsRevision): DnsSnapshotAdmission {
    val accepted = lastAcceptedDns(current)?.revision ?: return DnsSnapshotAdmission.Accepted
    if (accepted.ownerSessionId != incoming.ownerSessionId) return DnsSnapshotAdmission.Accepted
    return when {
        incoming.coreGeneration > accepted.coreGeneration -> DnsSnapshotAdmission.Accepted
        incoming.coreGeneration < accepted.coreGeneration -> DnsSnapshotAdmission.Stale
        incoming.dataRevision >= accepted.dataRevision -> DnsSnapshotAdmission.Accepted
        else -> DnsSnapshotAdmission.Stale
    }
}

internal fun DnsSettings.matches(
    patch: DnsSettingsPatch,
    expectedEnhancedMode: String?,
    expectedFallbackFilter: infiltrator_android.DnsFallbackFilterSettings?,
): DnsSettingsReadbackResult {
    val matches =
        (patch.enable == null || enable == patch.enable) &&
            (patch.ipv6 == null || ipv6 == patch.ipv6) &&
            enhancedMode == expectedEnhancedMode &&
            (patch.nameserver == null || nameserver == patch.nameserver) &&
            (patch.defaultNameserver == null || defaultNameserver == patch.defaultNameserver) &&
            (patch.fallback == null || fallback == patch.fallback) &&
            fallbackFilter == expectedFallbackFilter
    return if (matches) DnsSettingsReadbackResult.Confirmed else DnsSettingsReadbackResult.NotConfirmed
}
