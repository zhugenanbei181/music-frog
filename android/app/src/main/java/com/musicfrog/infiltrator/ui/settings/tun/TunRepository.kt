package com.musicfrog.infiltrator.ui.settings.tun

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
import infiltrator_android.AndroidOwnerOperationReadback
import infiltrator_android.AndroidOwnerOperationReply
import infiltrator_android.FfiErrorCode

internal sealed interface TunRepositoryResult {
    data class Completed(val point: TunSettingsObservationPoint) : TunRepositoryResult
    data class Failed(val failure: AppFailure, val latestSnapshot: TunSettingsObservationPoint?) : TunRepositoryResult
    data class Cancelled(val failure: AppFailure, val latestSnapshot: TunSettingsObservationPoint?) : TunRepositoryResult
    data class TimedOut(val failure: AppFailure) : TunRepositoryResult
    data class Unconfirmed(val failure: AppFailure, val latestSnapshot: TunSettingsObservationPoint?) : TunRepositoryResult
}

internal interface TunRepository {
    suspend fun execute(operation: AndroidOwnerOperation, timeoutMs: Long): TunRepositoryResult
}

internal object UniFfiTunRepository : TunRepository {
    private val gateway: AndroidOwnerOperationGateway = UniFfiAndroidOwnerOperationGateway

    override suspend fun execute(operation: AndroidOwnerOperation, timeoutMs: Long): TunRepositoryResult =
        when (val result = gateway.execute(operation, timeoutMs)) {
            is AndroidOwnerOperationExchange.Completed -> result.reply.toTunRepositoryResult(operation)
            is AndroidOwnerOperationExchange.Failed -> TunRepositoryResult.Failed(result.failure, null)
            is AndroidOwnerOperationExchange.Cancelled -> TunRepositoryResult.Cancelled(result.failure, null)
            is AndroidOwnerOperationExchange.TimedOut -> TunRepositoryResult.TimedOut(result.failure)
        }
}

private fun AndroidOwnerOperationReply.toTunRepositoryResult(operation: AndroidOwnerOperation): TunRepositoryResult {
    val latest = latestTunSnapshot()
    val readback = when (val value = readback) {
        is AndroidOwnerOperationReadback.TunSettings -> value.v1
        else -> return unconfirmedTun(latest)
    }
    val expectedRevision = when (operation) {
        is AndroidOwnerOperation.TunSettingsSave -> {
            if (context.expectedDataRevision == ULong.MAX_VALUE) return unconfirmedTun(latest)
            context.expectedDataRevision + 1u
        }

        AndroidOwnerOperation.TunSettings -> context.expectedDataRevision

        else -> return unconfirmedTun(latest)
    }
    if (dataDomain != AndroidOwnerDataDomain.TUN_SETTINGS ||
        context.expectedOwnerSessionId != context.ownerSessionId ||
        context.ownerSessionId.isBlank() ||
        context.expectedCoreGeneration != observedCoreGeneration ||
        dataRevision != expectedRevision
    ) {
        return unconfirmedTun(latest)
    }
    return when (readback.status.code) {
        FfiErrorCode.OK -> {
            val settings = readback.settings ?: return unconfirmedTun(latest)
            val snapshot = latest ?: return unconfirmedTun(null)
            if (snapshot.revision != revision() || snapshot.value != settings) {
                return unconfirmedTun(latest)
            }
            if (operation is AndroidOwnerOperation.TunSettingsSave &&
                settings.matches(operation.patch) != TunReadbackMatch.Confirmed
            ) {
                return unconfirmedTun(latest)
            }
            TunRepositoryResult.Completed(snapshot)
        }

        FfiErrorCode.CANCELED -> TunRepositoryResult.Cancelled(readback.status.toAppFailure(), latest)

        else -> TunRepositoryResult.Failed(readback.status.toAppFailure(), latest)
    }
}

private fun AndroidOwnerOperationReply.latestTunSnapshot(): TunSettingsObservationPoint? {
    val result = when (val data = snapshot?.data) {
        is AndroidOwnerDataSnapshotData.TunSettings -> data.v1
        else -> return null
    }
    if (snapshot?.domain != AndroidOwnerDataDomain.TUN_SETTINGS ||
        dataDomain != AndroidOwnerDataDomain.TUN_SETTINGS ||
        result.status.code != FfiErrorCode.OK
    ) {
        return null
    }
    val settings = result.settings ?: return null
    return TunSettingsObservationPoint(settings, revision())
}

private fun AndroidOwnerOperationReply.revision() = TunSettingsRevision(
    ownerSessionId = context.ownerSessionId,
    coreGeneration = observedCoreGeneration,
    dataRevision = dataRevision,
)

private fun unconfirmedTun(latest: TunSettingsObservationPoint?): TunRepositoryResult.Unconfirmed = TunRepositoryResult.Unconfirmed(
    localizedFailure(FailureTextKey.NotReady).toAppFailure(),
    latest,
)
