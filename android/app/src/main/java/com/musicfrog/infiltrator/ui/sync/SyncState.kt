package com.musicfrog.infiltrator.ui.sync

import com.musicfrog.infiltrator.ui.common.LocalizedFailure
import infiltrator_android.WebDavSettings

internal data class SyncSettingsRevision(val ownerSessionId: String, val coreGeneration: ULong, val dataRevision: ULong)

internal data class SyncSettingsSnapshot(val settings: WebDavSettings, val revision: SyncSettingsRevision)

internal data class SyncSettingsDraft(
    val enabled: Boolean,
    val url: String,
    val username: String,
    val password: String,
    val intervalMinutes: String,
    val syncOnStartup: Boolean,
)

internal data class SyncForm(val accepted: SyncSettingsSnapshot, val draft: SyncSettingsDraft, val requestSequence: ULong)

internal sealed interface SyncSettingsState {
    data class Loading(val lastAccepted: SyncForm? = null) : SyncSettingsState
    data class Observed(val form: SyncForm) : SyncSettingsState
    data class Unavailable(val lastAccepted: SyncForm?) : SyncSettingsState
}

internal enum class SyncOperation {
    LoadSettings,
    SaveSettings,
    TestConnection,
    SyncNow,
}

internal sealed interface SyncActionState {
    data object Idle : SyncActionState
    data class Working(val operation: SyncOperation) : SyncActionState
    data object Saved : SyncActionState
    data object ConnectionTested : SyncActionState
    data class Synchronized(val summary: SyncSummary) : SyncActionState
    data class Failed(val operation: SyncOperation, val failure: LocalizedFailure) : SyncActionState
    data class Cancelled(val operation: SyncOperation, val failure: LocalizedFailure) : SyncActionState
    data class TimedOut(val operation: SyncOperation, val failure: LocalizedFailure) : SyncActionState
    data class Unconfirmed(val operation: SyncOperation, val failure: LocalizedFailure) : SyncActionState
}

internal data class SyncSummary(val successCount: UInt, val failedCount: UInt, val totalActions: UInt)

internal data class SyncUiState(
    val settings: SyncSettingsState = SyncSettingsState.Loading(),
    val action: SyncActionState = SyncActionState.Idle,
)

internal enum class SyncRequestAvailability {
    Available,
    Busy,
}

@JvmInline
internal value class SyncRequestSequence(val value: ULong)

internal sealed interface SyncRequestAllocation {
    data class Allocated(val sequence: SyncRequestSequence) : SyncRequestAllocation
    data object Exhausted : SyncRequestAllocation
}

internal fun allocateSyncRequest(current: ULong): SyncRequestAllocation = when (current) {
    ULong.MAX_VALUE -> SyncRequestAllocation.Exhausted
    else -> SyncRequestAllocation.Allocated(SyncRequestSequence(current + 1u))
}

internal enum class SyncScreenActivity {
    Loading,
    Busy,
    Ready,
    Unavailable,
}

internal enum class SyncInputAvailability {
    Enabled,
    Disabled,
}

internal enum class SyncSnapshotAdmission {
    Accepted,
    Stale,
    Conflicting,
}

internal sealed interface SyncIntervalParse {
    data class Valid(val minutes: UInt) : SyncIntervalParse
    data object Invalid : SyncIntervalParse
}

internal fun parseSyncInterval(raw: String): SyncIntervalParse =
    raw.trim().toUIntOrNull()?.let(SyncIntervalParse::Valid) ?: SyncIntervalParse.Invalid

internal fun SyncSettingsSnapshot.toDraft(): SyncSettingsDraft = SyncSettingsDraft(
    enabled = settings.enabled,
    url = settings.url,
    username = settings.username,
    password = settings.password,
    intervalMinutes = settings.syncIntervalMins.toString(),
    syncOnStartup = settings.syncOnStartup,
)

internal fun SyncSettingsDraft.toWebDavSettings(intervalMinutes: UInt): WebDavSettings = WebDavSettings(
    enabled = enabled,
    url = url.trim(),
    username = username.trim(),
    password = password,
    syncIntervalMins = intervalMinutes,
    syncOnStartup = syncOnStartup,
)

internal fun SyncSettingsState.lastAccepted(): SyncForm? = when (this) {
    is SyncSettingsState.Loading -> lastAccepted
    is SyncSettingsState.Observed -> form
    is SyncSettingsState.Unavailable -> lastAccepted
}

internal fun SyncSettingsState.displayForm(): SyncForm? = lastAccepted()

internal fun SyncUiState.screenActivity(): SyncScreenActivity = when (action) {
    is SyncActionState.Working -> SyncScreenActivity.Busy

    SyncActionState.Idle,
    SyncActionState.Saved,
    SyncActionState.ConnectionTested,
    is SyncActionState.Synchronized,
    is SyncActionState.Failed,
    is SyncActionState.Cancelled,
    is SyncActionState.TimedOut,
    is SyncActionState.Unconfirmed,
    -> when (settings) {
        is SyncSettingsState.Loading -> SyncScreenActivity.Loading
        is SyncSettingsState.Observed -> SyncScreenActivity.Ready
        is SyncSettingsState.Unavailable -> SyncScreenActivity.Unavailable
    }
}

internal fun SyncUiState.requestAvailability(): SyncRequestAvailability = when (action) {
    is SyncActionState.Working -> SyncRequestAvailability.Busy

    SyncActionState.Idle,
    SyncActionState.Saved,
    SyncActionState.ConnectionTested,
    is SyncActionState.Synchronized,
    is SyncActionState.Failed,
    is SyncActionState.Cancelled,
    is SyncActionState.TimedOut,
    is SyncActionState.Unconfirmed,
    -> SyncRequestAvailability.Available
}

internal fun SyncUiState.inputAvailability(): SyncInputAvailability = when {
    settings is SyncSettingsState.Observed && action !is SyncActionState.Working ->
        SyncInputAvailability.Enabled

    else -> SyncInputAvailability.Disabled
}

internal fun admitSyncSettings(current: SyncForm?, incoming: SyncSettingsSnapshot, requestSequence: ULong): SyncSnapshotAdmission {
    if (current == null) return SyncSnapshotAdmission.Accepted
    if (requestSequence < current.requestSequence) return SyncSnapshotAdmission.Stale

    val previous = current.accepted.revision
    val next = incoming.revision
    if (next.ownerSessionId != previous.ownerSessionId) return SyncSnapshotAdmission.Accepted
    if (next.coreGeneration > previous.coreGeneration) return SyncSnapshotAdmission.Accepted
    if (next.coreGeneration < previous.coreGeneration) return SyncSnapshotAdmission.Stale
    if (next.dataRevision > previous.dataRevision) return SyncSnapshotAdmission.Accepted
    if (next.dataRevision < previous.dataRevision) return SyncSnapshotAdmission.Stale
    return when (incoming.settings == current.accepted.settings) {
        true -> SyncSnapshotAdmission.Accepted
        false -> SyncSnapshotAdmission.Conflicting
    }
}

internal fun SyncSettingsSnapshot.isReadbackOf(requested: WebDavSettings): SyncReadbackConfirmation = when (settings == requested) {
    true -> SyncReadbackConfirmation.Confirmed
    false -> SyncReadbackConfirmation.Mismatch
}

internal enum class SyncReadbackConfirmation {
    Confirmed,
    Mismatch,
}
