package com.musicfrog.infiltrator.ui.settings.fakeip

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

internal sealed interface FakeIpRepositoryResult {
    data class Completed(val point: FakeIpSettingsObservationPoint) : FakeIpRepositoryResult
    data class Failed(val failure: AppFailure, val latest: FakeIpSettingsObservationPoint?) : FakeIpRepositoryResult
    data class Cancelled(val failure: AppFailure, val latest: FakeIpSettingsObservationPoint?) : FakeIpRepositoryResult
    data class TimedOut(val failure: AppFailure) : FakeIpRepositoryResult
    data class Unconfirmed(val failure: AppFailure, val latest: FakeIpSettingsObservationPoint?) : FakeIpRepositoryResult
}

internal sealed interface FakeIpCacheClearResult {
    data class Completed(val removedFile: Boolean) : FakeIpCacheClearResult
    data class Failed(val failure: AppFailure) : FakeIpCacheClearResult
    data class Cancelled(val failure: AppFailure) : FakeIpCacheClearResult
    data class TimedOut(val failure: AppFailure) : FakeIpCacheClearResult
    data class Unconfirmed(val failure: AppFailure) : FakeIpCacheClearResult
}

internal interface FakeIpRepository {
    suspend fun execute(operation: AndroidOwnerOperation, timeoutMs: Long): FakeIpRepositoryResult

    /** Clears the Android host's Fake-IP file cache; this does not flush Mihomo's live table. */
    suspend fun clearHostFakeIpFileCache(timeoutMs: Long): FakeIpCacheClearResult
}

internal class UniFfiFakeIpRepository(private val gateway: AndroidOwnerOperationGateway = UniFfiAndroidOwnerOperationGateway) :
    FakeIpRepository {
    override suspend fun execute(operation: AndroidOwnerOperation, timeoutMs: Long): FakeIpRepositoryResult =
        when (val exchange = gateway.execute(operation, timeoutMs)) {
            is AndroidOwnerOperationExchange.Completed -> exchange.reply.toFakeIpRepositoryResult(operation)
            is AndroidOwnerOperationExchange.Failed -> FakeIpRepositoryResult.Failed(exchange.failure, null)
            is AndroidOwnerOperationExchange.Cancelled -> FakeIpRepositoryResult.Cancelled(exchange.failure, null)
            is AndroidOwnerOperationExchange.TimedOut -> FakeIpRepositoryResult.TimedOut(exchange.failure)
        }

    override suspend fun clearHostFakeIpFileCache(timeoutMs: Long): FakeIpCacheClearResult =
        when (val exchange = gateway.execute(AndroidOwnerOperation.FakeIpCacheClear, timeoutMs)) {
            is AndroidOwnerOperationExchange.Completed -> exchange.reply.toFakeIpCacheClearResult()
            is AndroidOwnerOperationExchange.Failed -> FakeIpCacheClearResult.Failed(exchange.failure)
            is AndroidOwnerOperationExchange.Cancelled -> FakeIpCacheClearResult.Cancelled(exchange.failure)
            is AndroidOwnerOperationExchange.TimedOut -> FakeIpCacheClearResult.TimedOut(exchange.failure)
        }
}

private fun AndroidOwnerOperationReply.toFakeIpCacheClearResult(): FakeIpCacheClearResult {
    val result = readback as? AndroidOwnerOperationReadback.FakeIpCacheClear
        ?: return unconfirmedFakeIpCacheClear()
    val ownerContextIsValid = protocol == AndroidOwnerOperationProtocolVersion.V6 &&
        context.clientSessionId.isNotBlank() &&
        context.ownerSessionId.isNotBlank() &&
        context.expectedOwnerSessionId == context.ownerSessionId &&
        context.expectedCoreGeneration == observedCoreGeneration &&
        dataDomain == AndroidOwnerDataDomain.FAKE_IP_CACHE &&
        snapshot == null
    if (!ownerContextIsValid) return unconfirmedFakeIpCacheClear()

    val changed = result.v1.status.code == FfiErrorCode.OK && result.v1.value
    val expectedRevision = when (changed) {
        true -> context.expectedDataRevision.takeIf { it < ULong.MAX_VALUE }?.plus(1u)
        false -> context.expectedDataRevision
    } ?: return unconfirmedFakeIpCacheClear()
    if (dataRevision != expectedRevision) return unconfirmedFakeIpCacheClear()

    return when (result.v1.status.code) {
        FfiErrorCode.OK -> FakeIpCacheClearResult.Completed(result.v1.value)
        FfiErrorCode.CANCELED -> FakeIpCacheClearResult.Cancelled(result.v1.status.toAppFailure())
        else -> FakeIpCacheClearResult.Failed(result.v1.status.toAppFailure())
    }
}

private fun unconfirmedFakeIpCacheClear() = FakeIpCacheClearResult.Unconfirmed(
    localizedFailure(FailureTextKey.NotReady).toAppFailure(),
)

private fun AndroidOwnerOperationReply.toFakeIpRepositoryResult(operation: AndroidOwnerOperation): FakeIpRepositoryResult {
    val purpose = operation.fakeIpPurpose()
    val latest = latestFakeIpSnapshot()
    when (purpose) {
        FakeIpOperationPurpose.Read,
        FakeIpOperationPurpose.Save,
        -> Unit

        FakeIpOperationPurpose.Unsupported -> return unconfirmedFakeIp(latest)
    }

    if (protocol != AndroidOwnerOperationProtocolVersion.V6 ||
        dataDomain != AndroidOwnerDataDomain.FAKE_IP_SETTINGS ||
        snapshot?.domain != AndroidOwnerDataDomain.FAKE_IP_SETTINGS ||
        context.clientSessionId.isBlank() ||
        context.ownerSessionId.isBlank() ||
        context.expectedOwnerSessionId != context.ownerSessionId ||
        context.expectedCoreGeneration != observedCoreGeneration
    ) {
        return unconfirmedFakeIp(latest)
    }

    val readback = when (val value = readback) {
        is AndroidOwnerOperationReadback.FakeIpSettings -> value.v1
        else -> return unconfirmedFakeIp(latest)
    }
    val expectedDataRevision = when (purpose) {
        FakeIpOperationPurpose.Read -> context.expectedDataRevision

        FakeIpOperationPurpose.Save -> when (readback.status.code) {
            FfiErrorCode.OK ->
                context.expectedDataRevision
                    .takeIf { it < ULong.MAX_VALUE }
                    ?.plus(1u)
                    ?: return unconfirmedFakeIp(latest)

            else -> context.expectedDataRevision
        }

        FakeIpOperationPurpose.Unsupported -> return unconfirmedFakeIp(latest)
    }
    if (dataRevision != expectedDataRevision) return unconfirmedFakeIp(latest)

    return when (readback.status.code) {
        FfiErrorCode.OK -> {
            val point = latest ?: return unconfirmedFakeIp(null)
            val settings = readback.settings ?: return unconfirmedFakeIp(point)
            when (point.value == settings) {
                true -> FakeIpRepositoryResult.Completed(point)
                false -> unconfirmedFakeIp(point)
            }
        }

        FfiErrorCode.CANCELED -> FakeIpRepositoryResult.Cancelled(readback.status.toAppFailure(), latest)

        else -> FakeIpRepositoryResult.Failed(readback.status.toAppFailure(), latest)
    }
}

private fun AndroidOwnerOperation.fakeIpPurpose(): FakeIpOperationPurpose = when (this) {
    AndroidOwnerOperation.FakeIpSettings -> FakeIpOperationPurpose.Read
    is AndroidOwnerOperation.FakeIpSettingsSave -> FakeIpOperationPurpose.Save
    else -> FakeIpOperationPurpose.Unsupported
}

private fun AndroidOwnerOperationReply.latestFakeIpSnapshot(): FakeIpSettingsObservationPoint? {
    if (protocol != AndroidOwnerOperationProtocolVersion.V6 ||
        dataDomain != AndroidOwnerDataDomain.FAKE_IP_SETTINGS ||
        snapshot?.domain != AndroidOwnerDataDomain.FAKE_IP_SETTINGS ||
        context.clientSessionId.isBlank() ||
        context.ownerSessionId.isBlank() ||
        context.expectedOwnerSessionId != context.ownerSessionId ||
        context.expectedCoreGeneration != observedCoreGeneration
    ) {
        return null
    }
    val result = (snapshot?.data as? AndroidOwnerDataSnapshotData.FakeIpSettings)?.v1 ?: return null
    if (result.status.code != FfiErrorCode.OK) return null
    val settings = result.settings ?: return null
    return FakeIpSettingsObservationPoint(settings, revision())
}

private fun AndroidOwnerOperationReply.revision() = FakeIpSettingsRevision(
    ownerSessionId = context.ownerSessionId,
    coreGeneration = observedCoreGeneration,
    dataRevision = dataRevision,
)

private fun unconfirmedFakeIp(latest: FakeIpSettingsObservationPoint?) = FakeIpRepositoryResult.Unconfirmed(
    localizedFailure(FailureTextKey.NotReady).toAppFailure(),
    latest,
)

private enum class FakeIpOperationPurpose {
    Read,
    Save,
    Unsupported,
}
