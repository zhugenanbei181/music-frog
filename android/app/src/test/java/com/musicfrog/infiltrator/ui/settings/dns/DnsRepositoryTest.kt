package com.musicfrog.infiltrator.ui.settings.dns

import com.musicfrog.infiltrator.ui.common.AndroidOwnerOperationExchange
import com.musicfrog.infiltrator.ui.common.AndroidOwnerOperationGateway
import com.musicfrog.infiltrator.ui.common.AppFailureCode
import com.musicfrog.infiltrator.ui.common.FailureTextKey
import com.musicfrog.infiltrator.ui.common.localizedFailure
import com.musicfrog.infiltrator.ui.common.toAppFailure
import infiltrator_android.AndroidOwnerDataDomain
import infiltrator_android.AndroidOwnerDataSnapshot
import infiltrator_android.AndroidOwnerDataSnapshotData
import infiltrator_android.AndroidOwnerOperation
import infiltrator_android.AndroidOwnerOperationContext
import infiltrator_android.AndroidOwnerOperationProtocolVersion
import infiltrator_android.AndroidOwnerOperationReadback
import infiltrator_android.AndroidOwnerOperationReply
import infiltrator_android.DnsSettings
import infiltrator_android.DnsSettingsResult
import infiltrator_android.FfiErrorCode
import infiltrator_android.FfiStatus
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class DnsRepositoryTest {
    @Test
    fun readUsesOwnerSessionGenerationAndDataRevision() = runBlocking {
        val settings = settings()
        val gateway = ReplyGateway(reply(settings, revision = 10uL))

        val result = UniFfiDnsRepository(gateway).execute(AndroidOwnerOperation.DnsSettings, 1000L)

        assertEquals(
            DnsRepositoryResult.Completed(
                DnsSettingsObservationPoint(settings, DnsSettingsRevision("owner-a", 7uL, 10uL)),
            ),
            result,
        )
        assertEquals(AndroidOwnerOperation.DnsSettings, gateway.lastOperation)
    }

    @Test
    fun successfulSaveRequiresOwnerRevisionAdvanceAndMatchingSnapshot() = runBlocking {
        val settings = settings().copy(nameserver = listOf("1.1.1.1"))
        val operation = AndroidOwnerOperation.DnsSettingsSave(
            infiltrator_android.DnsSettingsPatch(
                enable = null,
                ipv6 = null,
                enhancedMode = null,
                nameserver = listOf("1.1.1.1"),
                defaultNameserver = null,
                fallback = null,
                fallbackFilter = null,
            ),
        )
        val gateway = ReplyGateway(reply(settings, revision = 11uL))

        val result = UniFfiDnsRepository(gateway).execute(operation, 1000L)

        assertTrue(result is DnsRepositoryResult.Completed)
        assertEquals(
            DnsSettingsRevision("owner-a", 7uL, 11uL),
            (result as DnsRepositoryResult.Completed).point.revision,
        )
    }

    @Test
    fun missingOwnerRevisionAdvanceIsUnconfirmed() = runBlocking {
        val operation = AndroidOwnerOperation.DnsSettingsSave(
            infiltrator_android.DnsSettingsPatch(null, null, null, null, null, null, null),
        )
        val gateway = ReplyGateway(reply(settings(), revision = 10uL))

        val result = UniFfiDnsRepository(gateway).execute(operation, 1000L)

        assertTrue(result is DnsRepositoryResult.Unconfirmed)
    }

    @Test
    fun cancellationAndTimeoutRemainDistinctTypedResults() = runBlocking {
        val failure = localizedFailure(FailureTextKey.NotReady).toAppFailure()
        val operation = AndroidOwnerOperation.DnsSettings

        val cancelled = UniFfiDnsRepository(ReplyGateway(AndroidOwnerOperationExchange.Cancelled(failure)))
            .execute(operation, 1000L)
        val timedOut = UniFfiDnsRepository(ReplyGateway(AndroidOwnerOperationExchange.TimedOut(failure)))
            .execute(operation, 1000L)

        assertEquals(DnsRepositoryResult.Cancelled(failure, null), cancelled)
        assertEquals(DnsRepositoryResult.TimedOut(failure), timedOut)
        assertEquals(AppFailureCode.NotReady, failure.code)
    }

    private fun reply(
        settings: DnsSettings,
        revision: ULong,
    ): AndroidOwnerOperationExchange = AndroidOwnerOperationExchange.Completed(
        AndroidOwnerOperationReply(
            protocol = AndroidOwnerOperationProtocolVersion.V3,
            context = AndroidOwnerOperationContext(
                clientSessionId = "client-a",
                expectedOwnerSessionId = "owner-a",
                ownerSessionId = "owner-a",
                expectedCoreGeneration = 7uL,
                expectedDataRevision = 10uL,
            ),
            observedCoreGeneration = 7uL,
            dataRevision = revision,
            dataDomain = AndroidOwnerDataDomain.DNS_SETTINGS,
            snapshot = AndroidOwnerDataSnapshot(
                domain = AndroidOwnerDataDomain.DNS_SETTINGS,
                data = AndroidOwnerDataSnapshotData.DnsSettings(
                    DnsSettingsResult(FfiStatus(FfiErrorCode.OK, null), settings),
                ),
            ),
            readback = AndroidOwnerOperationReadback.DnsSettings(
                DnsSettingsResult(FfiStatus(FfiErrorCode.OK, null), settings),
            ),
        ),
    )

    private fun settings() = DnsSettings(
        enable = true,
        ipv6 = false,
        enhancedMode = "fake-ip",
        nameserverConfigured = true,
        nameserver = listOf("9.9.9.9"),
        defaultNameserver = emptyList(),
        fallback = emptyList(),
        fallbackFilter = null,
    )

    private class ReplyGateway(private val exchange: AndroidOwnerOperationExchange) : AndroidOwnerOperationGateway {
        var lastOperation: AndroidOwnerOperation? = null

        override suspend fun execute(
            operation: AndroidOwnerOperation,
            timeoutMs: Long,
        ): AndroidOwnerOperationExchange {
            lastOperation = operation
            return exchange
        }
    }
}
