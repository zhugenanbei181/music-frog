package com.musicfrog.infiltrator.ui.sync

import com.musicfrog.infiltrator.ui.common.AndroidOwnerOperationExchange
import com.musicfrog.infiltrator.ui.common.AndroidOwnerOperationGateway
import com.musicfrog.infiltrator.ui.common.AppFailure
import com.musicfrog.infiltrator.ui.common.AppFailureCode
import com.musicfrog.infiltrator.ui.common.FailureTextKey
import infiltrator_android.AndroidOwnerDataDomain
import infiltrator_android.AndroidOwnerDataSnapshot
import infiltrator_android.AndroidOwnerDataSnapshotData
import infiltrator_android.AndroidOwnerOperation
import infiltrator_android.AndroidOwnerOperationContext
import infiltrator_android.AndroidOwnerOperationProtocolVersion
import infiltrator_android.AndroidOwnerOperationReadback
import infiltrator_android.AndroidOwnerOperationReply
import infiltrator_android.FfiErrorCode
import infiltrator_android.FfiStatus
import infiltrator_android.WebDavSettings
import infiltrator_android.WebDavSettingsResult
import infiltrator_android.WebDavSyncResult
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class SyncRepositoryTest {
    @Test
    fun readAcceptsOnlyTypedOwnerSnapshotAtExpectedRevision() = runBlocking {
        val operation = AndroidOwnerOperation.WebDavSettings
        val expected = settings()
        val repository = repository { completed(reply(operation, expected)) }

        val result = repository.execute(operation, 1000L)

        val completed = result as SyncRepositoryResult.Completed
        assertEquals(expected, completed.snapshot.settings)
        assertEquals(SyncSettingsRevision("owner-session-1", 7uL, 10uL), completed.snapshot.revision)
        assertEquals(SyncRepositoryOutcome.SettingsObserved, completed.outcome)
    }

    @Test
    fun invalidOwnerFenceAndStaleRevisionFailClosed() = runBlocking {
        val operation = AndroidOwnerOperation.WebDavSettings
        val valid = reply(operation, settings())
        val stale = valid.copy(dataRevision = 9uL)
        val foreignOwner = valid.copy(
            context = context().copy(
                ownerSessionId = "owner-session-old",
                expectedOwnerSessionId = "owner-session-1",
            ),
        )
        val staleResult = repository { completed(stale) }.execute(operation, 1000L)
        val foreignResult = repository { completed(foreignOwner) }.execute(operation, 1000L)

        assertTrue(staleResult is SyncRepositoryResult.Unconfirmed)
        assertEquals(null, (staleResult as SyncRepositoryResult.Unconfirmed).latest)
        assertTrue(foreignResult is SyncRepositoryResult.Unconfirmed)
    }

    @Test
    fun saveRequiresAdvancedRevisionAndExactOwnerReadback() = runBlocking {
        val requested = settings(url = "https://dav.example.invalid/new")
        val operation = AndroidOwnerOperation.WebDavSettingsSave(requested)
        val confirmed = repository { completed(reply(operation, requested)) }
            .execute(operation, 1000L)
        assertEquals(SyncRepositoryOutcome.SettingsSaved, (confirmed as SyncRepositoryResult.Completed).outcome)
        assertEquals(11uL, confirmed.snapshot.revision.dataRevision)

        val mismatchedSnapshot = reply(operation, requested.copy(url = "https://dav.example.invalid/other"))
        val rejected = repository { completed(mismatchedSnapshot) }.execute(operation, 1000L)
        assertTrue(rejected is SyncRepositoryResult.Unconfirmed)

        val staleWrite = reply(operation, requested).copy(dataRevision = 10uL)
        assertTrue(repository { completed(staleWrite) }.execute(operation, 1000L) is SyncRepositoryResult.Unconfirmed)
    }

    @Test
    fun testAndSyncUseReadbackOutcomesWithoutChangingSettingsRevision() = runBlocking {
        val testOperation = AndroidOwnerOperation.WebDavTest(settings(url = "https://draft.invalid"))
        val tested = repository { completed(reply(testOperation, settings())) }
            .execute(testOperation, 1000L) as SyncRepositoryResult.Completed
        assertEquals(SyncRepositoryOutcome.ConnectionTested, tested.outcome)
        assertEquals(10uL, tested.snapshot.revision.dataRevision)

        val syncOperation = AndroidOwnerOperation.WebDavSyncNow
        val synced = repository { completed(reply(syncOperation, settings())) }
            .execute(syncOperation, 1000L) as SyncRepositoryResult.Completed
        assertEquals(SyncRepositoryOutcome.Synchronized(SyncSummary(3u, 1u, 4u)), synced.outcome)
        assertEquals(10uL, synced.snapshot.revision.dataRevision)
    }

    @Test
    fun cancellationTimeoutAndCallerCancellationRemainDistinct() = runBlocking {
        val operation = AndroidOwnerOperation.WebDavSettings
        val cancelled = repository {
            AndroidOwnerOperationExchange.Cancelled(failure())
        }.execute(operation, 1000L)
        val timedOut = repository {
            AndroidOwnerOperationExchange.TimedOut(failure())
        }.execute(operation, 1L)
        assertTrue(cancelled is SyncRepositoryResult.Cancelled)
        assertTrue(timedOut is SyncRepositoryResult.TimedOut)

        try {
            repository { throw CancellationException("caller cancelled") }
                .execute(operation, 1000L)
            throw AssertionError("caller cancellation was swallowed")
        } catch (error: CancellationException) {
            assertEquals("caller cancelled", error.message)
        }
    }

    private fun repository(
        handle: suspend (AndroidOwnerOperation) -> AndroidOwnerOperationExchange,
    ) = UniFfiSyncRepository(object : AndroidOwnerOperationGateway {
        override suspend fun execute(
            operation: AndroidOwnerOperation,
            timeoutMs: Long,
        ): AndroidOwnerOperationExchange = handle(operation)
    })

    private fun completed(reply: AndroidOwnerOperationReply) = AndroidOwnerOperationExchange.Completed(reply)

    private fun reply(
        operation: AndroidOwnerOperation,
        snapshotSettings: WebDavSettings,
    ): AndroidOwnerOperationReply {
        val successful = FfiStatus(FfiErrorCode.OK, null)
        val settingsResult = WebDavSettingsResult(successful, snapshotSettings)
        val readback = when (operation) {
            AndroidOwnerOperation.WebDavSettings,
            is AndroidOwnerOperation.WebDavSettingsSave -> AndroidOwnerOperationReadback.WebDavSettings(settingsResult)
            is AndroidOwnerOperation.WebDavTest -> AndroidOwnerOperationReadback.WebDavTest(successful)
            AndroidOwnerOperation.WebDavSyncNow -> AndroidOwnerOperationReadback.WebDavSyncNow(
                WebDavSyncResult(successful, 3u, 1u, 4u),
            )
            else -> throw IllegalArgumentException("Not a WebDAV operation: $operation")
        }
        val revision = when (operation) {
            is AndroidOwnerOperation.WebDavSettingsSave -> 11uL
            else -> 10uL
        }
        return AndroidOwnerOperationReply(
        protocol = AndroidOwnerOperationProtocolVersion.V5,
            context = context(),
            observedCoreGeneration = 7uL,
            dataRevision = revision,
            dataDomain = AndroidOwnerDataDomain.WEB_DAV,
            snapshot = AndroidOwnerDataSnapshot(
                domain = AndroidOwnerDataDomain.WEB_DAV,
                data = AndroidOwnerDataSnapshotData.WebDavSettings(settingsResult),
            ),
            readback = readback,
        )
    }

    private fun context() = AndroidOwnerOperationContext(
        clientSessionId = "client-session-1",
        expectedOwnerSessionId = "owner-session-1",
        ownerSessionId = "owner-session-1",
        expectedCoreGeneration = 7uL,
        expectedDataRevision = 10uL,
    )

    private fun settings(url: String = "https://dav.example.invalid") = WebDavSettings(
        enabled = true,
        url = url,
        username = "alice",
        password = "secret",
        syncIntervalMins = 30u,
        syncOnStartup = false,
    )

    private fun failure() = AppFailure(
        code = AppFailureCode.NotReady,
        localization = FailureTextKey.NotReady,
    )
}
