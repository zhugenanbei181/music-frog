package com.musicfrog.infiltrator

import java.util.ArrayDeque
import org.junit.Assert.assertEquals
import org.junit.Test

class VpnLifecycleWakeLockTest {
    @Test
    fun serviceLeaseLimitIsOneSharedBoundedDeadline() {
        assertEquals(45_000L, VpnLifecycleWakeLock.MAX_LEASE_DURATION_MILLIS)
    }

    @Test
    fun lifecycleLeaseUsesTheConfiguredTimeoutAndReleasesAtTerminal() {
        val handle = FakeVpnWakeLockHandle()
        val owner = VpnLifecycleWakeLock(handle, timeoutMillis = 30_000L)

        assertEquals(VpnWakeLockAcquire.Acquired(31_000L), owner.acquire(7L, 1_000L))
        assertEquals(listOf(30_000L), handle.acquireTimeouts)
        assertEquals(
            VpnWakeLockRelease.Released,
            owner.release(7L, nowElapsedRealtime = 5_000L),
        )
        assertEquals(FakeWakeLockState.Released, handle.state)
        assertEquals(VpnWakeLockState.Released, owner.state())
    }

    @Test
    fun lifecycleOperationReleasesItsLeaseBeforeReturning() {
        val handle = FakeVpnWakeLockHandle()
        val owner = VpnLifecycleWakeLock(handle, timeoutMillis = 30L)
        val acquisitions = mutableListOf<VpnWakeLockAcquire>()
        val releases = mutableListOf<VpnWakeLockRelease>()

        val result = owner.withLease(
            generation = 7L,
            elapsedRealtimeNow = { 100L },
            onAcquire = acquisitions::add,
            onRelease = releases::add,
        ) { "owner-terminal" }

        assertEquals("owner-terminal", result)
        assertEquals(listOf(VpnWakeLockAcquire.Acquired(130L)), acquisitions)
        assertEquals(listOf(VpnWakeLockRelease.Released), releases)
        assertEquals(FakeWakeLockState.Released, handle.state)
    }

    @Test
    fun lifecycleOperationFailureStillReleasesItsLeaseAndPreservesTheFailure() {
        val handle = FakeVpnWakeLockHandle()
        val owner = VpnLifecycleWakeLock(handle, timeoutMillis = 30L)
        val releases = mutableListOf<VpnWakeLockRelease>()
        var failureMessage: String? = null

        try {
            owner.withLease(
                generation = 7L,
                elapsedRealtimeNow = { 100L },
                onAcquire = {},
                onRelease = releases::add,
            ) { error("start transaction failed") }
        } catch (failure: IllegalStateException) {
            failureMessage = failure.message
        }

        assertEquals("start transaction failed", failureMessage)
        assertEquals(listOf(VpnWakeLockRelease.Released), releases)
        assertEquals(FakeWakeLockState.Released, handle.state)
    }

    @Test
    fun duplicateWorkForTheSameGenerationJoinsWithoutAcquiringTwice() {
        val handle = FakeVpnWakeLockHandle()
        val owner = VpnLifecycleWakeLock(handle, timeoutMillis = 20L)
        owner.acquire(4L, 100L)

        assertEquals(VpnWakeLockAcquire.Joined(120L), owner.acquire(4L, 110L))
        assertEquals(listOf(20L), handle.acquireTimeouts)
    }

    @Test
    fun aDifferentGenerationCannotBorrowAnUnexpiredLease() {
        val handle = FakeVpnWakeLockHandle()
        val owner = VpnLifecycleWakeLock(handle, timeoutMillis = 20L)
        owner.acquire(4L, 100L)

        assertEquals(VpnWakeLockAcquire.Busy(4L), owner.acquire(5L, 110L))
        assertEquals(listOf(20L), handle.acquireTimeouts)
        assertEquals(FakeWakeLockState.Held, handle.state)
    }

    @Test
    fun expiredLeaseIsReleasedBeforeTheNextGenerationAcquires() {
        val handle = FakeVpnWakeLockHandle()
        val owner = VpnLifecycleWakeLock(handle, timeoutMillis = 10L)
        owner.acquire(4L, 100L)

        assertEquals(VpnWakeLockAcquire.Acquired(120L), owner.acquire(5L, 110L))
        assertEquals(listOf(10L, 10L), handle.acquireTimeouts)
        assertEquals(1, handle.releaseCalls)
        assertEquals(VpnWakeLockState.Held(5L, 120L), owner.state())
    }

    @Test
    fun staleTerminalCannotReleaseANewerGenerationLease() {
        val handle = FakeVpnWakeLockHandle()
        val owner = VpnLifecycleWakeLock(handle, timeoutMillis = 10L)
        owner.acquire(4L, 100L)
        owner.acquire(5L, 110L)

        assertEquals(
            VpnWakeLockRelease.StaleGeneration,
            owner.release(4L, nowElapsedRealtime = 111L),
        )
        assertEquals(FakeWakeLockState.Held, handle.state)
        assertEquals(VpnWakeLockState.Held(5L, 120L), owner.state())
    }

    @Test
    fun unavailablePlatformLockDoesNotCreateAHeldLease() {
        val handle = FakeVpnWakeLockHandle().apply {
            acquireResult = VpnWakeLockPlatformAcquire.Unavailable("PowerManager unavailable")
        }
        val owner = VpnLifecycleWakeLock(handle, timeoutMillis = 10L)

        assertEquals(
            VpnWakeLockAcquire.Unavailable("PowerManager unavailable"),
            owner.acquire(2L, 100L),
        )
        assertEquals(VpnWakeLockState.Released, owner.state())
        assertEquals(FakeWakeLockState.Released, handle.state)
    }

    @Test
    fun failedReleaseRetainsTypedCleanupStateAndCanBeRetried() {
        val handle = FakeVpnWakeLockHandle().apply {
            releaseResults.add(VpnWakeLockPlatformRelease.Failed("release interrupted"))
        }
        val owner = VpnLifecycleWakeLock(handle, timeoutMillis = 10L)
        owner.acquire(2L, 100L)

        assertEquals(
            VpnWakeLockRelease.CleanupPending("release interrupted"),
            owner.release(2L, nowElapsedRealtime = 105L),
        )
        assertEquals(VpnWakeLockState.Held(2L, 110L), owner.state())
        assertEquals(
            VpnWakeLockRelease.Released,
            owner.release(2L, nowElapsedRealtime = 106L),
        )
        assertEquals(VpnWakeLockState.Released, owner.state())
    }

    @Test
    fun automaticTimeoutIsASeparateTerminalFromEarlyRelease() {
        val handle = FakeVpnWakeLockHandle().apply {
            releaseResults.add(VpnWakeLockPlatformRelease.Expired)
        }
        val owner = VpnLifecycleWakeLock(handle, timeoutMillis = 10L)
        owner.acquire(2L, 100L)

        assertEquals(
            VpnWakeLockRelease.Expired,
            owner.release(2L, nowElapsedRealtime = 110L),
        )
        assertEquals(VpnWakeLockState.Released, owner.state())
    }

    private enum class FakeWakeLockState {
        Released,
        Held,
    }

    private class FakeVpnWakeLockHandle : VpnWakeLockHandle {
        var state = FakeWakeLockState.Released
            private set
        var acquireResult: VpnWakeLockPlatformAcquire = VpnWakeLockPlatformAcquire.Acquired
        val acquireTimeouts = mutableListOf<Long>()
        val releaseResults = ArrayDeque<VpnWakeLockPlatformRelease>()
        var releaseCalls = 0
            private set

        override fun acquire(timeoutMillis: Long): VpnWakeLockPlatformAcquire {
            acquireTimeouts += timeoutMillis
            if (acquireResult == VpnWakeLockPlatformAcquire.Acquired) {
                state = FakeWakeLockState.Held
            }
            return acquireResult
        }

        override fun release(): VpnWakeLockPlatformRelease {
            releaseCalls += 1
            val result = releaseResults.pollFirst() ?: VpnWakeLockPlatformRelease.Released
            when (result) {
                VpnWakeLockPlatformRelease.Released,
                VpnWakeLockPlatformRelease.Expired -> state = FakeWakeLockState.Released
                is VpnWakeLockPlatformRelease.Failed -> Unit
            }
            return result
        }
    }
}
