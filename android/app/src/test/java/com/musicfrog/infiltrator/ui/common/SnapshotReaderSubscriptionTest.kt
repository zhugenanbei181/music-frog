package com.musicfrog.infiltrator.ui.common

import infiltrator_android.AndroidCoreLifecycleResult
import infiltrator_android.IpResult
import kotlinx.coroutines.cancelAndJoin
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.take
import kotlinx.coroutines.flow.toList
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.util.concurrent.atomic.AtomicInteger

class SnapshotReaderSubscriptionTest {

    private class CountingComposeRepository : AndroidComposeRepository {
        val surfaceReadCount = AtomicInteger(0)
        val coreLifecycleReadCount = AtomicInteger(0)

        override suspend fun readOwnerSurface(): OwnerSurfaceRead {
            surfaceReadCount.incrementAndGet()
            return OwnerSurfaceRead.Unavailable(localizedFailure(FailureTextKey.NotReady))
        }

        override suspend fun executeProductCommand(commandJson: String): String = "{}"

        override suspend fun readCoreLifecycle(): AndroidCoreLifecycleResult {
            coreLifecycleReadCount.incrementAndGet()
            return error("unused in this test")
        }

        override suspend fun checkPublicIp(): IpResult = error("unused in this test")
    }

    @Test
    fun ownerSurfaceFlowStopsPollingWhenSubscriptionCancels() = runBlocking {
        val repository = CountingComposeRepository()
        val flow = repository.ownerSurfaceFlow(50L)

        val job = launch {
            flow.collect {}
        }

        delay(120L)
        val countBeforeCancel = repository.surfaceReadCount.get()
        assertTrue("Surface flow should have polled at least once", countBeforeCancel >= 1)

        job.cancelAndJoin()

        delay(150L)
        val countAfterCancel = repository.surfaceReadCount.get()
        assertEquals(
            "Surface reader must not continue polling after subscription cancellation",
            countBeforeCancel,
            countAfterCancel,
        )
    }

    @Test
    fun singleReaderEmitsExpectedValuesBoundedByTake() = runBlocking {
        val repository = CountingComposeRepository()
        val items = repository.ownerSurfaceFlow(10L).take(3).toList()

        assertEquals(3, items.size)
        assertEquals(3, repository.surfaceReadCount.get())
    }
}
