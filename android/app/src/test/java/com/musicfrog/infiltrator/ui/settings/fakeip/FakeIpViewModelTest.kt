package com.musicfrog.infiltrator.ui.settings.fakeip

import com.musicfrog.infiltrator.ui.common.AppFailure
import com.musicfrog.infiltrator.ui.common.AppFailureCode
import com.musicfrog.infiltrator.ui.common.FailureTextKey
import infiltrator_android.AndroidOwnerOperation
import infiltrator_android.FakeIpSettings
import infiltrator_android.FakeIpSettingsPatch
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.util.ArrayDeque

class FakeIpViewModelTest {
    @Test
    fun saveIsPublishedOnlyAfterMatchingOwnerReadbackAndRevisionAdvance() {
        val initial = settings()
        val updated = settings().copy(
            fakeIpRange = "198.18.0.1/16",
            fakeIpFilter = listOf("localhost", "*.internal"),
            storeFakeIp = true,
        )
        val repository = FakeRepository().apply {
            enqueue(FakeIpRepositoryResult.Completed(point(initial, revision(dataRevision = 10uL))))
        }
        val scope = testScope()
        val viewModel = FakeIpViewModel(repository, scope)

        viewModel.updateRange("198.18.0.1/16")
        viewModel.updateFilter("localhost\n*.internal")
        viewModel.updateStoreFakeIp(FakeIpBooleanSetting.Enabled)
        repository.enqueue(FakeIpRepositoryResult.Completed(point(updated, revision(dataRevision = 11uL))))
        viewModel.save()

        assertEquals(
            AndroidOwnerOperation.FakeIpSettingsSave(
                FakeIpSettingsPatch("198.18.0.1/16", listOf("localhost", "*.internal"), true),
            ),
            repository.operations.last(),
        )
        assertEquals(
            FakeIpSettingsOperationState.Saved(revision(dataRevision = 11uL)),
            viewModel.state.value.settingsOperation,
        )
        assertEquals(updated, lastAcceptedFakeIp(viewModel.state.value.observation)?.value)
        scope.cancel()
    }

    @Test
    fun mismatchingSaveReadbackIsUnconfirmedAndKeepsOwnerSnapshot() {
        val initial = settings()
        val ownerReadback = settings().copy(fakeIpRange = "198.18.0.2/16")
        val repository = FakeRepository().apply {
            enqueue(FakeIpRepositoryResult.Completed(point(initial, revision(dataRevision = 10uL))))
        }
        val scope = testScope()
        val viewModel = FakeIpViewModel(repository, scope)
        viewModel.updateRange("198.18.0.1/16")
        repository.enqueue(
            FakeIpRepositoryResult.Completed(point(ownerReadback, revision(dataRevision = 11uL))),
        )

        viewModel.save()

        assertTrue(viewModel.state.value.settingsOperation is FakeIpSettingsOperationState.Unconfirmed)
        assertEquals(ownerReadback, lastAcceptedFakeIp(viewModel.state.value.observation)?.value)
        assertTrue(viewModel.state.value.observation is FakeIpSettingsReadState.Unavailable)
        scope.cancel()
    }

    @Test
    fun timeoutCancellationAndFailurePreserveLastAcceptedSettings() {
        val initial = settings().copy(fakeIpRange = "198.18.0.1/16")
        val accepted = point(initial, revision(dataRevision = 10uL))
        val failure = failure()
        val repository = FakeRepository().apply {
            enqueue(FakeIpRepositoryResult.Completed(accepted))
        }
        val scope = testScope()
        val viewModel = FakeIpViewModel(repository, scope)

        repository.enqueue(FakeIpRepositoryResult.TimedOut(failure))
        viewModel.load()
        assertEquals(FakeIpSettingsOperationState.TimedOut(failure), viewModel.state.value.settingsOperation)
        assertEquals(accepted, lastAcceptedFakeIp(viewModel.state.value.observation))

        repository.enqueue(FakeIpRepositoryResult.Cancelled(failure, null))
        viewModel.load()
        assertEquals(FakeIpSettingsOperationState.Cancelled(failure), viewModel.state.value.settingsOperation)
        assertEquals(accepted, lastAcceptedFakeIp(viewModel.state.value.observation))

        repository.enqueue(FakeIpRepositoryResult.Failed(failure, null))
        viewModel.load()
        assertEquals(FakeIpSettingsOperationState.Failed(failure), viewModel.state.value.settingsOperation)
        assertEquals(accepted, lastAcceptedFakeIp(viewModel.state.value.observation))
        scope.cancel()
    }

    @Test
    fun staleOwnerGenerationCannotReplaceAcceptedSettings() {
        val accepted = point(settings().copy(fakeIpRange = "current"), revision(coreGeneration = 7uL, dataRevision = 10uL))
        val repository = FakeRepository().apply {
            enqueue(FakeIpRepositoryResult.Completed(accepted))
        }
        val scope = testScope()
        val viewModel = FakeIpViewModel(repository, scope)
        repository.enqueue(
            FakeIpRepositoryResult.Completed(
                point(settings().copy(fakeIpRange = "stale"), revision(coreGeneration = 6uL, dataRevision = 99uL)),
            ),
        )

        viewModel.load()

        assertTrue(viewModel.state.value.settingsOperation is FakeIpSettingsOperationState.Unconfirmed)
        assertEquals(accepted, lastAcceptedFakeIp(viewModel.state.value.observation))
        scope.cancel()
    }
}

private class FakeRepository : FakeIpRepository {
    private val results = ArrayDeque<CompletableDeferred<FakeIpRepositoryResult>>()
    val operations = mutableListOf<AndroidOwnerOperation>()

    fun enqueue(result: FakeIpRepositoryResult) {
        results.addLast(CompletableDeferred(result))
    }

    override suspend fun execute(operation: AndroidOwnerOperation, timeoutMs: Long): FakeIpRepositoryResult {
        operations += operation
        return results.removeFirst().await()
    }

    override suspend fun clearHostFakeIpFileCache(timeoutMs: Long) = FakeIpCacheClearResult.Completed(removedFile = false)
}

private fun testScope() = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined)

private fun settings() = FakeIpSettings(
    fakeIpRange = null,
    fakeIpFilter = emptyList(),
    storeFakeIp = null,
)

private fun revision(ownerSessionId: String = "owner-a", coreGeneration: ULong = 7uL, dataRevision: ULong) =
    FakeIpSettingsRevision(ownerSessionId, coreGeneration, dataRevision)

private fun point(settings: FakeIpSettings, revision: FakeIpSettingsRevision) = FakeIpSettingsObservationPoint(settings, revision)

private fun failure() = AppFailure(
    code = AppFailureCode.NotReady,
    localization = FailureTextKey.NotReady,
)
