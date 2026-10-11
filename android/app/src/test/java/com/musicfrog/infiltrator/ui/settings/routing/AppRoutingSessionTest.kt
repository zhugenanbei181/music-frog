package com.musicfrog.infiltrator.ui.settings.routing

import com.musicfrog.infiltrator.ui.common.AppFailure
import com.musicfrog.infiltrator.ui.common.AppFailureCode
import com.musicfrog.infiltrator.ui.common.FailureTextKey
import com.musicfrog.infiltrator.ui.common.ObservationRevision
import infiltrator_android.AndroidAppRoutingMode
import infiltrator_android.AndroidAppRoutingPackageSelection
import infiltrator_android.AndroidOwnerOperation
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.yield
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class AppRoutingSessionTest {
    @Test
    fun initialOwnerReadFailureLeavesSnapshotUnavailable() = runBlocking {
        val repository = FakeAppRoutingRepository(
            AppRoutingRepositoryResult.Failed(failure(), AcceptedAppRoutingSnapshot.Missing),
        )
        val session = AppRoutingSession(repository, FakeInstalledAppsProvider())

        session.load()

        assertEquals(AppRoutingOperationState.Failed(failure()), session.operation.value)
        val snapshot = session.snapshot.value as AppRoutingSnapshotState.Unavailable
        assertEquals(AcceptedAppRoutingSnapshot.Missing, snapshot.lastAccepted)
    }

    @Test
    fun refreshFailureRetainsAcceptedSnapshotAndMarksItUnavailable() = runBlocking {
        val accepted = observation(revision = 4uL)
        val repository = FakeAppRoutingRepository(
            AppRoutingRepositoryResult.Completed(accepted),
            AppRoutingRepositoryResult.Failed(failure(), AcceptedAppRoutingSnapshot.Missing),
        )
        val session = AppRoutingSession(repository, FakeInstalledAppsProvider())

        session.load()
        session.load()

        val snapshot = session.snapshot.value as AppRoutingSnapshotState.Unavailable
        assertEquals(AcceptedAppRoutingSnapshot.Present(accepted), snapshot.lastAccepted)
        assertEquals(AppRoutingOperationState.Failed(failure()), session.operation.value)
    }

    @Test
    fun ownerCancellationAndTimeoutHaveDistinctTypedOutcomes() = runBlocking {
        val canceledSession = AppRoutingSession(
            FakeAppRoutingRepository(
                AppRoutingRepositoryResult.Cancelled(
                    failure(AppFailureCode.Canceled, FailureTextKey.Canceled),
                    AcceptedAppRoutingSnapshot.Missing,
                ),
            ),
            FakeInstalledAppsProvider(),
        )
        canceledSession.load()
        assertTrue(canceledSession.operation.value is AppRoutingOperationState.Cancelled)

        val timedOutSession = AppRoutingSession(
            FakeAppRoutingRepository(
                AppRoutingRepositoryResult.TimedOut(failure(AppFailureCode.NotReady, FailureTextKey.NotReady)),
            ),
            FakeInstalledAppsProvider(),
        )
        timedOutSession.load()
        assertTrue(timedOutSession.operation.value is AppRoutingOperationState.TimedOut)
    }

    @Test
    fun staleAndConflictingOwnerRevisionsCannotReplaceAcceptedSnapshot() = runBlocking {
        val accepted = observation(revision = 8uL)
        val stale = observation(revision = 7uL, packages = setOf("org.example.stale"))
        val conflict = observation(revision = 8uL, packages = setOf("org.example.conflict"))
        val repository = FakeAppRoutingRepository(
            AppRoutingRepositoryResult.Completed(accepted),
            AppRoutingRepositoryResult.Completed(stale),
            AppRoutingRepositoryResult.Completed(conflict),
        )
        val session = AppRoutingSession(repository, FakeInstalledAppsProvider())

        session.load()
        session.load()
        assertEquals(
            AppRoutingOperationState.Failed(failure(AppFailureCode.InvalidState, FailureTextKey.InvalidState)),
            session.operation.value,
        )
        assertEquals(
            AcceptedAppRoutingSnapshot.Present(accepted),
            (session.snapshot.value as AppRoutingSnapshotState.Unavailable).lastAccepted,
        )

        session.load()
        assertEquals(
            AppRoutingOperationState.Failed(failure(AppFailureCode.InvalidState, FailureTextKey.InvalidState)),
            session.operation.value,
        )
        assertEquals(
            AcceptedAppRoutingSnapshot.Present(accepted),
            (session.snapshot.value as AppRoutingSnapshotState.Unavailable).lastAccepted,
        )
    }

    @Test
    fun packageToggleUsesTypedSelectionAndOnlyInstalledPackage() = runBlocking {
        val repository = FakeAppRoutingRepository(
            AppRoutingRepositoryResult.Completed(
                observation(mode = AndroidAppRoutingMode.PROXY_SELECTED, packages = emptySet()),
            ),
            AppRoutingRepositoryResult.Completed(
                observation(
                    mode = AndroidAppRoutingMode.PROXY_SELECTED,
                    packages = setOf("org.example.user"),
                    revision = 2uL,
                ),
            ),
        )
        val session = AppRoutingSession(repository, FakeInstalledAppsProvider(apps()))

        session.load()
        session.toggleApp("org.not.installed")
        session.toggleApp("org.example.user")

        assertEquals(2, repository.operations.size)
        assertEquals(
            AndroidOwnerOperation.AppRoutingPackagesSet(
                listOf("org.example.user"),
                AndroidAppRoutingPackageSelection.SELECTED,
            ),
            repository.operations.last(),
        )
    }

    @Test
    fun packageManagerFailureAndOwnerReadFailureAreNotCollapsedIntoEmptySuccess() = runBlocking {
        val providerFailure = object : InstalledAppsProvider {
            override suspend fun loadInstalledApps(): List<AppItem> = error("PackageManager unavailable")
        }
        val repository = FakeAppRoutingRepository(AppRoutingRepositoryResult.Completed(observation()))
        val session = AppRoutingSession(repository, providerFailure)

        session.load()

        assertTrue(session.installedApps.value is InstalledAppsState.Unavailable)
        assertEquals(AppRoutingOperationState.Failed(failure()), session.operation.value)
        assertEquals(0, repository.operations.size)
    }

    @Test
    fun parentCancellationIsRethrown() = runBlocking {
        val repository = object : AppRoutingRepository {
            override suspend fun execute(
                operation: AndroidOwnerOperation,
                timeoutMs: Long,
            ): AppRoutingRepositoryResult = throw CancellationException("parent canceled")
        }
        val session = AppRoutingSession(repository, FakeInstalledAppsProvider())

        try {
            session.load()
            throw AssertionError("parent cancellation was swallowed")
        } catch (error: CancellationException) {
            assertEquals("parent canceled", error.message)
        }
    }

    @Test
    fun olderInFlightRefreshCannotOverwriteNewerAcceptedRevision() = runBlocking {
        val firstStarted = CompletableDeferred<Unit>()
        val releaseFirst = CompletableDeferred<AppRoutingRepositoryResult>()
        val newest = observation(revision = 12uL, packages = setOf("org.example.newest"))
        var callCount = 0
        val repository = object : AppRoutingRepository {
            override suspend fun execute(
                operation: AndroidOwnerOperation,
                timeoutMs: Long,
            ): AppRoutingRepositoryResult = when (callCount++) {
                0 -> {
                    firstStarted.complete(Unit)
                    releaseFirst.await()
                }
                else -> AppRoutingRepositoryResult.Completed(newest)
            }
        }
        val session = AppRoutingSession(repository, FakeInstalledAppsProvider())
        val older = launch { session.load() }
        firstStarted.await()
        val newer = launch { session.load() }
        yield()
        releaseFirst.complete(AppRoutingRepositoryResult.Completed(observation(revision = 11uL)))
        older.join()
        newer.join()

        assertEquals(newest, (session.snapshot.value as AppRoutingSnapshotState.Observed).observation)
    }

    private class FakeAppRoutingRepository(
        vararg results: AppRoutingRepositoryResult,
    ) : AppRoutingRepository {
        private val pending = ArrayDeque(results.toList())
        val operations = mutableListOf<AndroidOwnerOperation>()

        override suspend fun execute(
            operation: AndroidOwnerOperation,
            timeoutMs: Long,
        ): AppRoutingRepositoryResult {
            operations += operation
            return pending.removeFirst()
        }
    }

    private class FakeInstalledAppsProvider(
        private val values: List<AppItem> = apps(),
    ) : InstalledAppsProvider {
        override suspend fun loadInstalledApps(): List<AppItem> = values
    }

    private fun apps() = listOf(
        AppItem("User App", "org.example.user", null, AppClassification.User),
        AppItem("System App", "org.example.system", null, AppClassification.System),
    )

    private fun observation(
        mode: AndroidAppRoutingMode = AndroidAppRoutingMode.PROXY_SELECTED,
        packages: Set<String> = setOf("org.example.user"),
        revision: ULong = 1uL,
    ) = AppRoutingObservation(
        mode = mode,
        selectedPackages = packages,
        ownerSessionId = "owner-session-1",
        revision = ObservationRevision.Owner(generation = 2uL, revision = revision),
    )

    private fun failure(
        code: AppFailureCode = AppFailureCode.NotReady,
        key: FailureTextKey = FailureTextKey.NotReady,
    ) = AppFailure(code = code, localization = key)
}
