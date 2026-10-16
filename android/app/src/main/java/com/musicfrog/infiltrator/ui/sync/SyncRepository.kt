package com.musicfrog.infiltrator.ui.sync

import com.musicfrog.infiltrator.ui.common.AndroidOwnerOperationExchange
import com.musicfrog.infiltrator.ui.common.AndroidOwnerOperationGateway
import com.musicfrog.infiltrator.ui.common.AppFailure
import com.musicfrog.infiltrator.ui.common.FailureTextKey
import com.musicfrog.infiltrator.ui.common.UniFfiAndroidOwnerOperationGateway
import com.musicfrog.infiltrator.ui.common.localizedFailure
import com.musicfrog.infiltrator.ui.common.toAppFailure
import infiltrator_android.AndroidOwnerDataDomain
import infiltrator_android.AndroidOwnerDataSnapshotData
import infiltrator_android.AndroidOwnerOperation
import infiltrator_android.AndroidOwnerOperationProtocolVersion
import infiltrator_android.AndroidOwnerOperationReadback
import infiltrator_android.AndroidOwnerOperationReply
import infiltrator_android.FfiErrorCode

internal sealed interface SyncRepositoryOutcome {
    data object SettingsObserved : SyncRepositoryOutcome
    data object SettingsSaved : SyncRepositoryOutcome
    data object ConnectionTested : SyncRepositoryOutcome
    data class Synchronized(val summary: SyncSummary) : SyncRepositoryOutcome
}

internal sealed interface SyncRepositoryResult {
    data class Completed(
        val snapshot: SyncSettingsSnapshot,
        val outcome: SyncRepositoryOutcome,
    ) : SyncRepositoryResult

    data class Failed(val failure: AppFailure, val latest: SyncSettingsSnapshot?) : SyncRepositoryResult
    data class Cancelled(val failure: AppFailure, val latest: SyncSettingsSnapshot?) : SyncRepositoryResult
    data class TimedOut(val failure: AppFailure) : SyncRepositoryResult
    data class Unconfirmed(val failure: AppFailure, val latest: SyncSettingsSnapshot?) : SyncRepositoryResult
}

internal interface SyncRepository {
    suspend fun execute(operation: AndroidOwnerOperation, timeoutMs: Long): SyncRepositoryResult
}

internal class UniFfiSyncRepository(
    private val gateway: AndroidOwnerOperationGateway = UniFfiAndroidOwnerOperationGateway,
) : SyncRepository {
    override suspend fun execute(
        operation: AndroidOwnerOperation,
        timeoutMs: Long,
    ): SyncRepositoryResult = when (val exchange = gateway.execute(operation, timeoutMs)) {
        is AndroidOwnerOperationExchange.Completed -> exchange.reply.toSyncRepositoryResult(operation)
        is AndroidOwnerOperationExchange.Failed -> SyncRepositoryResult.Failed(exchange.failure, null)
        is AndroidOwnerOperationExchange.Cancelled -> SyncRepositoryResult.Cancelled(exchange.failure, null)
        is AndroidOwnerOperationExchange.TimedOut -> SyncRepositoryResult.TimedOut(exchange.failure)
    }
}

private fun AndroidOwnerOperationReply.toSyncRepositoryResult(
    operation: AndroidOwnerOperation,
): SyncRepositoryResult {
    val purpose = when (operation) {
        AndroidOwnerOperation.WebDavSettings -> SyncOperationPurpose.Read
        is AndroidOwnerOperation.WebDavSettingsSave -> SyncOperationPurpose.Save
        is AndroidOwnerOperation.WebDavTest -> SyncOperationPurpose.Test
        AndroidOwnerOperation.WebDavSyncNow -> SyncOperationPurpose.Sync
        else -> return unconfirmedSync(null)
    }
    val latest = latestSyncSnapshot()
    if (!hasSyncOwnerFence()) return unconfirmedSync(null)

    val readbackStatus = when (val value = readback) {
        is AndroidOwnerOperationReadback.WebDavSettings -> value.v1.status
        is AndroidOwnerOperationReadback.WebDavTest -> value.v1
        is AndroidOwnerOperationReadback.WebDavSyncNow -> value.v1.status
        else -> return unconfirmedSync(latest)
    }
    val expectedRevision = when (purpose) {
        SyncOperationPurpose.Read,
        SyncOperationPurpose.Test,
        SyncOperationPurpose.Sync -> context.expectedDataRevision
        SyncOperationPurpose.Save -> context.expectedDataRevision
            .takeIf { it < ULong.MAX_VALUE }
            ?.plus(1u)
            ?: return unconfirmedSync(latest)
        SyncOperationPurpose.Unsupported -> return unconfirmedSync(latest)
    }
    if (dataRevision != expectedRevision) return unconfirmedSync(null)

    if (readbackStatus.code != FfiErrorCode.OK) {
        return when (readbackStatus.code) {
            FfiErrorCode.CANCELED -> SyncRepositoryResult.Cancelled(readbackStatus.toAppFailure(), latest)
            else -> SyncRepositoryResult.Failed(readbackStatus.toAppFailure(), latest)
        }
    }

    val snapshot = latest ?: return unconfirmedSync(null)
    val snapshotData = snapshotData() ?: return unconfirmedSync(snapshot)
    val outcome = when (purpose) {
        SyncOperationPurpose.Read -> {
            val readback = readback as? AndroidOwnerOperationReadback.WebDavSettings
                ?: return unconfirmedSync(snapshot)
            if (readback.v1 != snapshotData) return unconfirmedSync(snapshot)
            SyncRepositoryOutcome.SettingsObserved
        }
        SyncOperationPurpose.Save -> {
            val save = operation as? AndroidOwnerOperation.WebDavSettingsSave
                ?: return unconfirmedSync(snapshot)
            val readback = readback as? AndroidOwnerOperationReadback.WebDavSettings
                ?: return unconfirmedSync(snapshot)
            val requestedSettings = save.settings
            val acceptedSettings = snapshotData.settings ?: return unconfirmedSync(snapshot)
            if (readback.v1 != snapshotData || acceptedSettings != requestedSettings) {
                return unconfirmedSync(snapshot)
            }
            SyncRepositoryOutcome.SettingsSaved
        }
        SyncOperationPurpose.Test -> {
            if (readback !is AndroidOwnerOperationReadback.WebDavTest) return unconfirmedSync(snapshot)
            SyncRepositoryOutcome.ConnectionTested
        }
        SyncOperationPurpose.Sync -> {
            val result = (readback as? AndroidOwnerOperationReadback.WebDavSyncNow)?.v1
                ?: return unconfirmedSync(snapshot)
            SyncRepositoryOutcome.Synchronized(
                SyncSummary(
                    successCount = result.successCount,
                    failedCount = result.failedCount,
                    totalActions = result.totalActions,
                ),
            )
        }
        SyncOperationPurpose.Unsupported -> return unconfirmedSync(snapshot)
    }
    return SyncRepositoryResult.Completed(snapshot, outcome)
}

private fun AndroidOwnerOperationReply.hasSyncOwnerFence(): Boolean =
    protocol == AndroidOwnerOperationProtocolVersion.V6 &&
        context.clientSessionId.isNotBlank() &&
        context.ownerSessionId.isNotBlank() &&
        context.expectedOwnerSessionId == context.ownerSessionId &&
        context.expectedCoreGeneration == observedCoreGeneration &&
        dataDomain == AndroidOwnerDataDomain.WEB_DAV &&
        snapshot?.domain == AndroidOwnerDataDomain.WEB_DAV &&
        snapshot?.data is AndroidOwnerDataSnapshotData.WebDavSettings

private fun AndroidOwnerOperationReply.snapshotData() =
    (snapshot?.data as? AndroidOwnerDataSnapshotData.WebDavSettings)?.v1

private fun AndroidOwnerOperationReply.latestSyncSnapshot(): SyncSettingsSnapshot? {
    if (dataDomain != AndroidOwnerDataDomain.WEB_DAV || snapshot?.domain != AndroidOwnerDataDomain.WEB_DAV) {
        return null
    }
    val result = snapshotData() ?: return null
    if (result.status.code != FfiErrorCode.OK) return null
    val settings = result.settings ?: return null
    return SyncSettingsSnapshot(
        settings = settings,
        revision = SyncSettingsRevision(
            ownerSessionId = context.ownerSessionId,
            coreGeneration = observedCoreGeneration,
            dataRevision = dataRevision,
        ),
    )
}

private enum class SyncOperationPurpose {
    Read,
    Save,
    Test,
    Sync,
    Unsupported,
}

private fun unconfirmedSync(latest: SyncSettingsSnapshot?) = SyncRepositoryResult.Unconfirmed(
    failure = localizedFailure(FailureTextKey.NotReady).toAppFailure(),
    latest = latest,
)
