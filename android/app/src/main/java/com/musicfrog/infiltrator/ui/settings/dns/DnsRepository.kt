package com.musicfrog.infiltrator.ui.settings.dns

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

internal sealed interface DnsRepositoryResult {
    data class Completed(val point: DnsSettingsObservationPoint) : DnsRepositoryResult
    data class Failed(
        val failure: AppFailure,
        val latest: DnsSettingsObservationPoint?,
    ) : DnsRepositoryResult
    data class Cancelled(
        val failure: AppFailure,
        val latest: DnsSettingsObservationPoint?,
    ) : DnsRepositoryResult
    data class TimedOut(val failure: AppFailure) : DnsRepositoryResult
    data class Unconfirmed(
        val failure: AppFailure,
        val latest: DnsSettingsObservationPoint?,
    ) : DnsRepositoryResult
}

internal interface DnsRepository {
    suspend fun execute(
        operation: AndroidOwnerOperation,
        timeoutMs: Long,
    ): DnsRepositoryResult
}

internal class UniFfiDnsRepository(
    private val gateway: AndroidOwnerOperationGateway = UniFfiAndroidOwnerOperationGateway,
) : DnsRepository {
    override suspend fun execute(
        operation: AndroidOwnerOperation,
        timeoutMs: Long,
    ): DnsRepositoryResult = when (val exchange = gateway.execute(operation, timeoutMs)) {
        is AndroidOwnerOperationExchange.Completed -> exchange.reply.toDnsRepositoryResult(operation)
        is AndroidOwnerOperationExchange.Failed -> DnsRepositoryResult.Failed(exchange.failure, null)
        is AndroidOwnerOperationExchange.Cancelled -> DnsRepositoryResult.Cancelled(exchange.failure, null)
        is AndroidOwnerOperationExchange.TimedOut -> DnsRepositoryResult.TimedOut(exchange.failure)
    }
}

private fun AndroidOwnerOperationReply.toDnsRepositoryResult(
    operation: AndroidOwnerOperation,
): DnsRepositoryResult {
    val purpose = when (operation) {
        AndroidOwnerOperation.DnsSettings -> DnsOperationPurpose.Read
        is AndroidOwnerOperation.DnsSettingsSave -> DnsOperationPurpose.Write
        else -> DnsOperationPurpose.Unsupported
    }
    val latest = latestDnsSnapshot()
    when (purpose) {
        DnsOperationPurpose.Read,
        DnsOperationPurpose.Write -> Unit
        DnsOperationPurpose.Unsupported -> return unconfirmedDns(latest)
    }
    if (dataDomain != AndroidOwnerDataDomain.DNS_SETTINGS ||
        snapshot?.domain != AndroidOwnerDataDomain.DNS_SETTINGS ||
        context.clientSessionId.isBlank() || context.ownerSessionId.isBlank() ||
        context.expectedOwnerSessionId != context.ownerSessionId ||
        context.expectedCoreGeneration != observedCoreGeneration
    ) {
        return unconfirmedDns(latest)
    }

    val readback = when (val value = readback) {
        is AndroidOwnerOperationReadback.DnsSettings -> value.v1
        else -> return unconfirmedDns(latest)
    }
    val expectedDataRevision = when (purpose) {
        DnsOperationPurpose.Read -> context.expectedDataRevision
        DnsOperationPurpose.Write -> when (readback.status.code) {
            FfiErrorCode.OK -> when (context.expectedDataRevision) {
                ULong.MAX_VALUE -> return unconfirmedDns(latest)
                else -> context.expectedDataRevision + 1u
            }
            else -> context.expectedDataRevision
        }
        DnsOperationPurpose.Unsupported -> return unconfirmedDns(latest)
    }
    if (dataRevision != expectedDataRevision) return unconfirmedDns(latest)

    return when (readback.status.code) {
        FfiErrorCode.OK -> {
            val point = latest ?: return unconfirmedDns(null)
            val snapshot = snapshot?.data as? AndroidOwnerDataSnapshotData.DnsSettings
                ?: return unconfirmedDns(point)
            if (snapshot.v1 != readback) return unconfirmedDns(point)
            DnsRepositoryResult.Completed(point)
        }
        FfiErrorCode.CANCELED -> DnsRepositoryResult.Cancelled(readback.status.toAppFailure(), latest)
        else -> DnsRepositoryResult.Failed(readback.status.toAppFailure(), latest)
    }
}

private fun AndroidOwnerOperationReply.latestDnsSnapshot(): DnsSettingsObservationPoint? {
    if (snapshot?.domain != AndroidOwnerDataDomain.DNS_SETTINGS ||
        dataDomain != AndroidOwnerDataDomain.DNS_SETTINGS
    ) {
        return null
    }
    val result = (snapshot?.data as? AndroidOwnerDataSnapshotData.DnsSettings)?.v1 ?: return null
    if (result.status.code != FfiErrorCode.OK) return null
    val settings = result.settings ?: return null
    return DnsSettingsObservationPoint(
        settings,
        DnsSettingsRevision(context.ownerSessionId, observedCoreGeneration, dataRevision),
    )
}

private fun notReadyFailure() = localizedFailure(FailureTextKey.NotReady).toAppFailure()

private enum class DnsOperationPurpose {
    Read,
    Write,
    Unsupported,
}

private fun unconfirmedDns(latest: DnsSettingsObservationPoint?) =
    DnsRepositoryResult.Unconfirmed(notReadyFailure(), latest)
