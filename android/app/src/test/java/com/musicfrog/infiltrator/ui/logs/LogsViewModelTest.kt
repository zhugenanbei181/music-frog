package com.musicfrog.infiltrator.ui.logs

import com.musicfrog.infiltrator.ui.common.AppFailure
import com.musicfrog.infiltrator.ui.common.AppFailureCode
import com.musicfrog.infiltrator.ui.common.CoreIdentityObservation
import com.musicfrog.infiltrator.ui.common.CoreScopeObservation
import com.musicfrog.infiltrator.ui.common.FailureTextKey
import com.musicfrog.infiltrator.ui.common.LogEntry
import com.musicfrog.infiltrator.ui.common.LogLevel
import com.musicfrog.infiltrator.ui.common.ObservationRevision
import com.musicfrog.infiltrator.ui.common.OwnerLogStreamState
import com.musicfrog.infiltrator.ui.common.OwnerLogsPage
import com.musicfrog.infiltrator.ui.common.OwnerPage
import com.musicfrog.infiltrator.ui.common.OwnerPageState
import java.util.ArrayDeque
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class LogsViewModelTest {
    @Test
    fun clearPublishesClearedOnlyAfterOwnerReadbackIsEmpty() = runBlocking {
        val repository = FakeLogsRepository().apply {
            enqueue(ownerLogs(revision = 4uL, entries = listOf(logEntry("old"))))
            enqueue(ownerLogs(revision = 5uL, entries = emptyList()))
        }
        val scope = testScope()
        val viewModel = testViewModel(repository, scope)

        viewModel.dispatch(LogsIntent.PageVisible)
        assertTrue(viewModel.state.value.observation is LogsObservationState.Content)

        viewModel.dispatch(LogsIntent.Clear)

        assertEquals(1, repository.clearCount)
        assertEquals(testCoreScope(), repository.lastClearScope)
        assertEquals(
            LogsOperationState.Cleared(ObservationRevision.Owner(3uL, 5uL)),
            viewModel.state.value.operation,
        )
        assertTrue(viewModel.state.value.observation.lastAccepted()?.logs?.entries.isNullOrEmpty())
        scope.cancel()
    }

    @Test
    fun clearReadbackFailureStaysFailedAndPreservesLastAcceptedOwnerLogs() = runBlocking {
        val accepted = ownerLogs(revision = 4uL, entries = listOf(logEntry("kept")))
        val failure = internalFailure()
        val repository = FakeLogsRepository().apply {
            enqueue(accepted)
            enqueue(LogsSnapshotRead.Unavailable(failure))
        }
        val scope = testScope()
        val viewModel = testViewModel(repository, scope)

        viewModel.dispatch(LogsIntent.PageVisible)
        viewModel.dispatch(LogsIntent.Clear)

        assertEquals(LogsOperationState.Failed(failure), viewModel.state.value.operation)
        assertEquals(
            accepted.page.data,
            viewModel.state.value.observation.lastAccepted()?.logs,
        )
        assertTrue(viewModel.state.value.observation is LogsObservationState.Unavailable)
        scope.cancel()
    }

    @Test
    fun clearWithEntriesStillPresentInOwnerReadbackStaysFailed() = runBlocking {
        val repository = FakeLogsRepository().apply {
            enqueue(ownerLogs(revision = 4uL, entries = listOf(logEntry("old"))))
            enqueue(ownerLogs(revision = 5uL, entries = listOf(logEntry("still present"))))
        }
        val scope = testScope()
        val viewModel = testViewModel(repository, scope)

        viewModel.dispatch(LogsIntent.PageVisible)
        viewModel.dispatch(LogsIntent.Clear)

        assertEquals(LogsOperationState.Failed(logsNotReadyFailure()), viewModel.state.value.operation)
        assertEquals(
            listOf(logEntry("still present")),
            viewModel.state.value.observation.lastAccepted()?.logs?.entries,
        )
        scope.cancel()
    }

    @Test
    fun clearCommandFailureDoesNotStartReadbackOrClaimSuccess() = runBlocking {
        val failure = internalFailure()
        val repository = FakeLogsRepository().apply {
            enqueue(ownerLogs(revision = 4uL, entries = listOf(logEntry("kept"))))
            clearResult = LogsClearResult.Failed(failure)
        }
        val scope = testScope()
        val viewModel = testViewModel(repository, scope)

        viewModel.dispatch(LogsIntent.PageVisible)
        viewModel.dispatch(LogsIntent.Clear)

        assertEquals(LogsOperationState.Failed(failure), viewModel.state.value.operation)
        assertEquals(1, repository.readCount)
        assertEquals(1, repository.clearCount)
        assertEquals("kept", viewModel.state.value.observation.lastAccepted()?.logs?.entries?.single()?.message)
        scope.cancel()
    }

    @Test
    fun clearWithoutObservedCoreScopeFailsClosed() = runBlocking {
        val repository = FakeLogsRepository().apply {
            enqueue(ownerLogs(revision = 4uL, entries = listOf(logEntry("kept")), coreScope = null))
        }
        val scope = testScope()
        val viewModel = testViewModel(repository, scope)

        viewModel.dispatch(LogsIntent.PageVisible)
        viewModel.dispatch(LogsIntent.Clear)

        assertEquals(0, repository.clearCount)
        assertEquals(LogsOperationState.Failed(logsNotReadyFailure()), viewModel.state.value.operation)
        scope.cancel()
    }

    @Test
    fun newerRequestAndOwnerRevisionWinOverOutOfOrderReadbacks() = runBlocking {
        val olderRequest = CompletableDeferred<LogsSnapshotRead>()
        val newerRequest = CompletableDeferred<LogsSnapshotRead>()
        val repository = FakeLogsRepository().apply {
            enqueue(olderRequest)
            enqueue(newerRequest)
        }
        val scope = testScope()
        val viewModel = testViewModel(repository, scope)

        viewModel.dispatch(LogsIntent.PageVisible)
        viewModel.dispatch(LogsIntent.Refresh)
        newerRequest.complete(ownerLogs(revision = 10uL, entries = listOf(logEntry("new"))))
        assertEquals("new", viewModel.state.value.observation.lastAccepted()?.logs?.entries?.single()?.message)

        olderRequest.complete(ownerLogs(revision = 11uL, entries = listOf(logEntry("late"))))
        repository.enqueue(ownerLogs(generation = 2uL, revision = 999uL, entries = listOf(logEntry("stale generation"))))
        viewModel.dispatch(LogsIntent.Refresh)
        repository.enqueue(ownerLogs(revision = 9uL, entries = listOf(logEntry("stale revision"))))
        viewModel.dispatch(LogsIntent.Refresh)

        assertEquals(
            listOf(logEntry("new")),
            viewModel.state.value.observation.lastAccepted()?.logs?.entries,
        )
        assertEquals(ObservationRevision.Owner(3uL, 10uL), viewModel.state.value.observation.lastAccepted()?.revision)
        scope.cancel()
    }

    @Test
    fun readFailureKeepsAcceptedEntriesAndOwnerRevision() = runBlocking {
        val accepted = ownerLogs(revision = 7uL, entries = listOf(logEntry("preserved")))
        val failure = internalFailure()
        val repository = FakeLogsRepository().apply {
            enqueue(accepted)
            enqueue(LogsSnapshotRead.Unavailable(failure))
        }
        val scope = testScope()
        val viewModel = testViewModel(repository, scope)

        viewModel.dispatch(LogsIntent.PageVisible)
        viewModel.dispatch(LogsIntent.Refresh)

        val unavailable = viewModel.state.value.observation as LogsObservationState.Unavailable
        assertEquals(failure, unavailable.failure)
        assertEquals(accepted.page.data, unavailable.lastAccepted?.logs)
        assertEquals(ObservationRevision.Owner(3uL, 7uL), unavailable.revision)
        scope.cancel()
    }

    @Test
    fun hidingPageCancelsPendingPollAndLeavesTypedCancelledObservation() = runBlocking {
        val pending = CompletableDeferred<LogsSnapshotRead>()
        val repository = FakeLogsRepository().apply { enqueue(pending) }
        val scope = testScope()
        val viewModel = testViewModel(repository, scope, pollingIntervalMs = 5L)

        viewModel.dispatch(LogsIntent.PageVisible)
        assertEquals(1, repository.readCount)
        assertTrue(viewModel.state.value.observation is LogsObservationState.Loading)
        viewModel.dispatch(LogsIntent.PageHidden)
        delay(20L)

        assertEquals(1, repository.readCount)
        assertEquals(1, repository.cancelledReadCount)
        assertTrue(viewModel.state.value.observation is LogsObservationState.Cancelled)
        scope.cancel()
    }
}

private class FakeLogsRepository : LogsRepository {
    private val readReplies = ArrayDeque<CompletableDeferred<LogsSnapshotRead>>()
    var readCount: Int = 0
        private set
    var clearCount: Int = 0
        private set
    var cancelledReadCount: Int = 0
        private set
    var clearResult: LogsClearResult = LogsClearResult.Accepted
    var lastClearScope: CoreScopeObservation? = null

    fun enqueue(read: LogsSnapshotRead) {
        readReplies.addLast(CompletableDeferred(read))
    }

    fun enqueue(read: CompletableDeferred<LogsSnapshotRead>) {
        readReplies.addLast(read)
    }

    override suspend fun readSnapshot(): LogsSnapshotRead {
        readCount += 1
        return try {
            readReplies.removeFirst().await()
        } catch (error: CancellationException) {
            cancelledReadCount += 1
            throw error
        }
    }

    override suspend fun clearLogs(scope: CoreScopeObservation): LogsClearResult {
        clearCount += 1
        lastClearScope = scope
        return clearResult
    }
}

private fun testScope() = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined)

private fun testViewModel(
    repository: LogsRepository,
    scope: CoroutineScope,
    pollingIntervalMs: Long = 10_000L,
) = LogsViewModel(
    repository = repository,
    pollingIntervalMs = pollingIntervalMs,
    coroutineScope = scope,
)

private fun ownerLogs(
    generation: ULong = 3uL,
    revision: ULong,
    entries: List<LogEntry>,
    coreScope: CoreScopeObservation? = testCoreScope(),
): LogsSnapshotRead.OwnerSnapshot {
    val logs = OwnerLogsPage(
        stream = OwnerLogStreamState.LIVE,
        totalEntries = entries.size,
        entries = entries,
    )
    return LogsSnapshotRead.OwnerSnapshot(
        coreScope = coreScope,
        revision = ObservationRevision.Owner(generation = generation, revision = revision),
        page = OwnerPage(
            state = if (entries.isEmpty()) OwnerPageState.Empty else OwnerPageState.Ready,
            data = logs,
            failure = null,
        ),
    )
}

private fun testCoreScope() = CoreScopeObservation(
    identity = CoreIdentityObservation("mihomo", "android-vpn", "1.19.12"),
    generation = "7",
    sessionToken = "8",
    json = """{"identity":{"engine":"mihomo","instance":"android-vpn","version":"1.19.12"},"generation":7,"session_token":8}""",
)

private fun logEntry(message: String) = LogEntry(
    level = LogLevel.INFO,
    message = message,
    timestamp = "2026-10-11T12:00:00Z",
)

private fun internalFailure() = AppFailure(
    code = AppFailureCode.Internal,
    localization = FailureTextKey.Internal,
)
