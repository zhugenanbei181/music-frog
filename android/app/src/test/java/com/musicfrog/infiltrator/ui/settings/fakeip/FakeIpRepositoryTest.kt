package com.musicfrog.infiltrator.ui.settings.fakeip

import com.musicfrog.infiltrator.testFfiStatus
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
import infiltrator_android.FakeIpSettings
import infiltrator_android.FakeIpSettingsPatch
import infiltrator_android.FakeIpSettingsResult
import infiltrator_android.FfiBoolResult
import infiltrator_android.FfiErrorCode
import infiltrator_android.FfiStatus
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class FakeIpRepositoryTest {
    @Test
    fun cacheClearUsesOwnerOperationAndMatchesRevisionToObservedMutation() = runBlocking {
        val removedGateway = ReplyGateway(exchange(cacheClearReply(removedFile = true, dataRevision = 11uL)))
        val removed = UniFfiFakeIpRepository(removedGateway)
            .clearHostFakeIpFileCache(1_000L)

        assertEquals(FakeIpCacheClearResult.Completed(removedFile = true), removed)
        assertEquals(AndroidOwnerOperation.FakeIpCacheClear, removedGateway.lastOperation)

        val emptyGateway = ReplyGateway(exchange(cacheClearReply(removedFile = false, dataRevision = 10uL)))
        val empty = UniFfiFakeIpRepository(emptyGateway)
            .clearHostFakeIpFileCache(1_000L)

        assertEquals(FakeIpCacheClearResult.Completed(removedFile = false), empty)
        assertEquals(AndroidOwnerOperation.FakeIpCacheClear, emptyGateway.lastOperation)
    }

    @Test
    fun cacheClearRejectsUnfencedOwnerReadback() = runBlocking {
        val gateway = ReplyGateway(exchange(cacheClearReply(removedFile = true, dataRevision = 10uL)))

        val result = UniFfiFakeIpRepository(gateway).clearHostFakeIpFileCache(1_000L)

        assertTrue(result is FakeIpCacheClearResult.Unconfirmed)
        assertEquals(AndroidOwnerOperation.FakeIpCacheClear, gateway.lastOperation)
    }

    @Test
    fun readUsesV6OwnerSessionCoreGenerationAndDataRevision() = runBlocking {
        val settings = settings()
        val gateway = ReplyGateway(exchange(reply(settings, dataRevision = 10uL)))

        val result = UniFfiFakeIpRepository(gateway).execute(AndroidOwnerOperation.FakeIpSettings, 1_000L)

        assertEquals(
            FakeIpRepositoryResult.Completed(
                FakeIpSettingsObservationPoint(settings, FakeIpSettingsRevision("owner-a", 7uL, 10uL)),
            ),
            result,
        )
        assertEquals(AndroidOwnerOperation.FakeIpSettings, gateway.lastOperation)
    }

    @Test
    fun successfulSaveRequiresOwnerDataRevisionAdvanceAndConsistentReadback() = runBlocking {
        val patch = FakeIpSettingsPatch(
            fakeIpRange = "198.18.0.1/16",
            fakeIpFilter = listOf("localhost"),
            storeFakeIp = true,
        )
        val operation = AndroidOwnerOperation.FakeIpSettingsSave(patch)
        val settings = settings().copy(
            fakeIpRange = "198.18.0.1/16",
            fakeIpFilter = listOf("localhost"),
            storeFakeIp = true,
        )
        val gateway = ReplyGateway(exchange(reply(settings, dataRevision = 11uL)))

        val result = UniFfiFakeIpRepository(gateway).execute(operation, 1_000L)

        assertEquals(
            FakeIpRepositoryResult.Completed(
                FakeIpSettingsObservationPoint(settings, FakeIpSettingsRevision("owner-a", 7uL, 11uL)),
            ),
            result,
        )
        assertEquals(operation, gateway.lastOperation)
    }

    @Test
    fun missingRevisionAdvanceAndSnapshotReadbackMismatchAreUnconfirmed() = runBlocking {
        val operation = AndroidOwnerOperation.FakeIpSettingsSave(
            FakeIpSettingsPatch(fakeIpRange = null, fakeIpFilter = emptyList(), storeFakeIp = null),
        )
        val noAdvance = UniFfiFakeIpRepository(
            ReplyGateway(exchange(reply(settings(), dataRevision = 10uL))),
        ).execute(operation, 1_000L)
        val mismatch = UniFfiFakeIpRepository(
            ReplyGateway(exchange(reply(settings(), dataRevision = 11uL, readback = settings().copy(storeFakeIp = false)))),
        ).execute(operation, 1_000L)

        assertTrue(noAdvance is FakeIpRepositoryResult.Unconfirmed)
        assertTrue(mismatch is FakeIpRepositoryResult.Unconfirmed)
    }

    @Test
    fun mismatchedOwnerSessionOrCoreGenerationFailsClosed() = runBlocking {
        val staleOwner = reply(
            settings(),
            dataRevision = 10uL,
            ownerSessionId = "owner-b",
            expectedOwnerSessionId = "owner-a",
        )
        val staleCore = reply(
            settings(),
            dataRevision = 10uL,
            observedCoreGeneration = 8uL,
            expectedCoreGeneration = 7uL,
        )
        val ownerResult = UniFfiFakeIpRepository(ReplyGateway(exchange(staleOwner)))
            .execute(AndroidOwnerOperation.FakeIpSettings, 1_000L)
        val coreResult = UniFfiFakeIpRepository(ReplyGateway(exchange(staleCore)))
            .execute(AndroidOwnerOperation.FakeIpSettings, 1_000L)

        assertEquals(
            FakeIpRepositoryResult.Unconfirmed(
                localizedFailure(FailureTextKey.NotReady).toAppFailure(),
                null,
            ),
            ownerResult,
        )
        assertTrue(coreResult is FakeIpRepositoryResult.Unconfirmed)
    }

    @Test
    fun gatewayCancellationTimeoutAndFailureStayTyped() = runBlocking {
        val failure = localizedFailure(FailureTextKey.NotReady).toAppFailure()
        val operation = AndroidOwnerOperation.FakeIpSettings

        val cancelled = UniFfiFakeIpRepository(ReplyGateway(AndroidOwnerOperationExchange.Cancelled(failure)))
            .execute(operation, 1_000L)
        val timedOut = UniFfiFakeIpRepository(ReplyGateway(AndroidOwnerOperationExchange.TimedOut(failure)))
            .execute(operation, 1_000L)
        val failed = UniFfiFakeIpRepository(ReplyGateway(AndroidOwnerOperationExchange.Failed(failure)))
            .execute(operation, 1_000L)

        assertEquals(FakeIpRepositoryResult.Cancelled(failure, null), cancelled)
        assertEquals(FakeIpRepositoryResult.TimedOut(failure), timedOut)
        assertEquals(FakeIpRepositoryResult.Failed(failure, null), failed)
        assertEquals(AppFailureCode.NotReady, failure.code)
    }

    private fun exchange(reply: AndroidOwnerOperationReply) = AndroidOwnerOperationExchange.Completed(reply)

    private fun reply(
        settings: FakeIpSettings,
        dataRevision: ULong,
        readback: FakeIpSettings = settings,
        ownerSessionId: String = "owner-a",
        expectedOwnerSessionId: String = ownerSessionId,
        observedCoreGeneration: ULong = 7uL,
        expectedCoreGeneration: ULong = observedCoreGeneration,
    ) = AndroidOwnerOperationReply(
        protocol = AndroidOwnerOperationProtocolVersion.V6,
        context = AndroidOwnerOperationContext(
            clientSessionId = "client-a",
            expectedOwnerSessionId = expectedOwnerSessionId,
            ownerSessionId = ownerSessionId,
            expectedCoreGeneration = expectedCoreGeneration,
            expectedDataRevision = 10uL,
        ),
        observedCoreGeneration = observedCoreGeneration,
        dataRevision = dataRevision,
        dataDomain = AndroidOwnerDataDomain.FAKE_IP_SETTINGS,
        snapshot = AndroidOwnerDataSnapshot(
            domain = AndroidOwnerDataDomain.FAKE_IP_SETTINGS,
            data = AndroidOwnerDataSnapshotData.FakeIpSettings(
                FakeIpSettingsResult(testFfiStatus(FfiErrorCode.OK), settings),
            ),
        ),
        readback = AndroidOwnerOperationReadback.FakeIpSettings(
            FakeIpSettingsResult(testFfiStatus(FfiErrorCode.OK), readback),
        ),
    )

    private fun cacheClearReply(removedFile: Boolean, dataRevision: ULong) = AndroidOwnerOperationReply(
        protocol = AndroidOwnerOperationProtocolVersion.V6,
        context = AndroidOwnerOperationContext(
            clientSessionId = "client-a",
            expectedOwnerSessionId = "owner-a",
            ownerSessionId = "owner-a",
            expectedCoreGeneration = 7uL,
            expectedDataRevision = 10uL,
        ),
        observedCoreGeneration = 7uL,
        dataRevision = dataRevision,
        dataDomain = AndroidOwnerDataDomain.FAKE_IP_CACHE,
        snapshot = null,
        readback = AndroidOwnerOperationReadback.FakeIpCacheClear(
            FfiBoolResult(testFfiStatus(FfiErrorCode.OK), removedFile),
        ),
    )

    private fun settings() = FakeIpSettings(
        fakeIpRange = null,
        fakeIpFilter = emptyList(),
        storeFakeIp = null,
    )

    private class ReplyGateway(private val exchange: AndroidOwnerOperationExchange) : AndroidOwnerOperationGateway {
        var lastOperation: AndroidOwnerOperation? = null

        override suspend fun execute(operation: AndroidOwnerOperation, timeoutMs: Long): AndroidOwnerOperationExchange {
            lastOperation = operation
            return exchange
        }
    }
}
