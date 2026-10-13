package com.musicfrog.infiltrator

import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class AndroidBridgeCallDispatcherTest {
    @Test
    fun executorBoundsWorkersAndQueueAndCancelsQueuedWorkBeforeExecution() {
        val dispatcher = AndroidBridgeCallDispatcher(workerCount = 1, queueCapacity = 1)
        val firstStarted = CountDownLatch(1)
        val releaseFirst = CountDownLatch(1)
        val executions = AtomicInteger()
        val completed = mutableListOf<Long>()
        val completionLock = Any()
        try {
            assertEquals(
                AndroidBridgeDispatchState.Accepted,
                dispatcher.submit(
                    requestId = 1L,
                    call = AndroidBridgeCall.SurfaceSnapshot,
                    execute = {
                        executions.incrementAndGet()
                        firstStarted.countDown()
                        releaseFirst.await()
                        AndroidBridgeCallReply.SurfaceSnapshot("snapshot")
                    },
                    complete = { id, _ -> synchronized(completionLock) { completed += id } },
                    fail = { _, _ -> error("unexpected host failure") },
                ),
            )
            assertTrue(firstStarted.await(2, TimeUnit.SECONDS))
            assertEquals(
                AndroidBridgeDispatchState.Accepted,
                dispatcher.submit(
                    requestId = 2L,
                    call = AndroidBridgeCall.SurfaceSnapshot,
                    execute = {
                        executions.incrementAndGet()
                        AndroidBridgeCallReply.SurfaceSnapshot("must not execute")
                    },
                    complete = { id, _ -> synchronized(completionLock) { completed += id } },
                    fail = { _, _ -> error("unexpected host failure") },
                ),
            )
            assertEquals(
                AndroidBridgeDispatchState.QueueFull,
                dispatcher.submit(
                    requestId = 3L,
                    call = AndroidBridgeCall.SurfaceSnapshot,
                    execute = { error("capacity must reject before execution") },
                    complete = { _, _ -> error("capacity must reject before completion") },
                    fail = { _, _ -> error("capacity must reject before failure callback") },
                ),
            )
            assertEquals(AndroidBridgeCallCancellation.Canceled, dispatcher.cancel(2L))
            releaseFirst.countDown()
            assertTrue(waitUntil { synchronized(completionLock) { completed == listOf(1L) } })
            assertEquals(1, executions.get())
        } finally {
            releaseFirst.countDown()
            dispatcher.close()
        }
    }

    @Test
    fun cancelRunningRequestInterruptsWorkerAndSuppressesLateCompletion() {
        val dispatcher = AndroidBridgeCallDispatcher(workerCount = 1, queueCapacity = 1)
        val started = CountDownLatch(1)
        val interrupted = CountDownLatch(1)
        val completions = AtomicInteger()
        try {
            assertEquals(
                AndroidBridgeDispatchState.Accepted,
                dispatcher.submit(
                    requestId = 10L,
                    call = AndroidBridgeCall.ExecuteProductCommand("typed-command"),
                    execute = {
                        started.countDown()
                        try {
                            CountDownLatch(1).await()
                            AndroidBridgeCallReply.ExecuteProductCommand("unexpected")
                        } catch (error: InterruptedException) {
                            interrupted.countDown()
                            throw error
                        }
                    },
                    complete = { _, _ -> completions.incrementAndGet() },
                    fail = { _, _ -> completions.incrementAndGet() },
                ),
            )
            assertTrue(started.await(2, TimeUnit.SECONDS))
            assertEquals(AndroidBridgeCallCancellation.Canceled, dispatcher.cancel(10L))
            assertTrue(interrupted.await(2, TimeUnit.SECONDS))
            assertEquals(0, completions.get())
        } finally {
            dispatcher.close()
        }
    }

    private fun waitUntil(predicate: () -> Boolean): Boolean {
        val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(2)
        while (System.nanoTime() < deadline) {
            if (predicate()) return true
            Thread.yield()
        }
        return predicate()
    }
}
