package com.musicfrog.infiltrator

/** Typed outcome of the Android partial WakeLock acquire operation. */
internal sealed interface VpnWakeLockPlatformAcquire {
    data object Acquired : VpnWakeLockPlatformAcquire

    data class Unavailable(val reason: String) : VpnWakeLockPlatformAcquire
}

/** Typed outcome of releasing a system WakeLock, including its automatic timeout. */
internal sealed interface VpnWakeLockPlatformRelease {
    data object Released : VpnWakeLockPlatformRelease

    data object Expired : VpnWakeLockPlatformRelease

    data class Failed(val reason: String) : VpnWakeLockPlatformRelease
}

internal interface VpnWakeLockHandle {
    fun acquire(timeoutMillis: Long): VpnWakeLockPlatformAcquire

    fun release(): VpnWakeLockPlatformRelease
}

internal sealed interface VpnWakeLockState {
    data object Released : VpnWakeLockState

    data class Held(val generation: Long, val deadlineElapsedRealtime: Long) : VpnWakeLockState
}

internal sealed interface VpnWakeLockAcquire {
    data class Acquired(val deadlineElapsedRealtime: Long) : VpnWakeLockAcquire

    data class Joined(val deadlineElapsedRealtime: Long) : VpnWakeLockAcquire

    data class Busy(val heldGeneration: Long) : VpnWakeLockAcquire

    data class Unavailable(val reason: String) : VpnWakeLockAcquire

    data class CleanupPending(val reason: String) : VpnWakeLockAcquire
}

internal sealed interface VpnWakeLockRelease {
    data object Released : VpnWakeLockRelease

    data object AlreadyReleased : VpnWakeLockRelease

    data object Expired : VpnWakeLockRelease

    data object StaleGeneration : VpnWakeLockRelease

    data class CleanupPending(val reason: String) : VpnWakeLockRelease
}

private enum class VpnWakeLockReleaseReason {
    CompletedBeforeDeadline,
    DeadlineReached,
}

/**
 * Owns one bounded system WakeLock lease for serialized VPN lifecycle work.
 * The Android timeout is the process-death safety net; normal terminal paths
 * release the lease early.
 */
internal class VpnLifecycleWakeLock(private val handle: VpnWakeLockHandle, private val timeoutMillis: Long) {
    companion object {
        const val MAX_LEASE_DURATION_MILLIS = 45_000L
    }

    private var state: VpnWakeLockState = VpnWakeLockState.Released

    init {
        require(timeoutMillis > 0L)
    }

    @Synchronized
    fun state(): VpnWakeLockState = state

    @Synchronized
    fun acquire(generation: Long, nowElapsedRealtime: Long): VpnWakeLockAcquire {
        require(generation > 0L)
        require(nowElapsedRealtime >= 0L)

        when (val current = state) {
            VpnWakeLockState.Released -> Unit

            is VpnWakeLockState.Held -> when {
                nowElapsedRealtime >= current.deadlineElapsedRealtime -> {
                    when (val release = releaseHeld(VpnWakeLockReleaseReason.DeadlineReached)) {
                        VpnWakeLockRelease.Released,
                        VpnWakeLockRelease.Expired,
                        -> Unit

                        VpnWakeLockRelease.AlreadyReleased,
                        VpnWakeLockRelease.StaleGeneration,
                        -> Unit

                        is VpnWakeLockRelease.CleanupPending -> {
                            return VpnWakeLockAcquire.CleanupPending(release.reason)
                        }
                    }
                }

                current.generation == generation -> {
                    return VpnWakeLockAcquire.Joined(current.deadlineElapsedRealtime)
                }

                else -> return VpnWakeLockAcquire.Busy(current.generation)
            }
        }

        return when (val platform = handle.acquire(timeoutMillis)) {
            VpnWakeLockPlatformAcquire.Acquired -> {
                val deadline = saturatingAdd(nowElapsedRealtime, timeoutMillis)
                state = VpnWakeLockState.Held(generation, deadline)
                VpnWakeLockAcquire.Acquired(deadline)
            }

            is VpnWakeLockPlatformAcquire.Unavailable ->
                VpnWakeLockAcquire.Unavailable(platform.reason)
        }
    }

    @Synchronized
    fun release(generation: Long, nowElapsedRealtime: Long): VpnWakeLockRelease {
        require(generation > 0L)
        require(nowElapsedRealtime >= 0L)
        return when (val current = state) {
            VpnWakeLockState.Released -> VpnWakeLockRelease.AlreadyReleased

            is VpnWakeLockState.Held -> when {
                generation != current.generation -> VpnWakeLockRelease.StaleGeneration

                nowElapsedRealtime >= current.deadlineElapsedRealtime ->
                    releaseHeld(VpnWakeLockReleaseReason.DeadlineReached)

                else -> releaseHeld(VpnWakeLockReleaseReason.CompletedBeforeDeadline)
            }
        }
    }

    fun <T> withLease(
        generation: Long,
        elapsedRealtimeNow: () -> Long,
        onAcquire: (VpnWakeLockAcquire) -> Unit,
        onRelease: (VpnWakeLockRelease) -> Unit,
        operation: () -> T,
    ): T {
        val admission = acquire(generation, elapsedRealtimeNow())
        try {
            onAcquire(admission)
            return operation()
        } finally {
            if (admission is VpnWakeLockAcquire.Acquired) {
                onRelease(release(generation, elapsedRealtimeNow()))
            }
        }
    }

    private fun releaseHeld(reason: VpnWakeLockReleaseReason): VpnWakeLockRelease {
        val platform = handle.release()
        return when (platform) {
            VpnWakeLockPlatformRelease.Released -> {
                state = VpnWakeLockState.Released
                when (reason) {
                    VpnWakeLockReleaseReason.CompletedBeforeDeadline -> VpnWakeLockRelease.Released
                    VpnWakeLockReleaseReason.DeadlineReached -> VpnWakeLockRelease.Expired
                }
            }

            VpnWakeLockPlatformRelease.Expired -> {
                state = VpnWakeLockState.Released
                VpnWakeLockRelease.Expired
            }

            is VpnWakeLockPlatformRelease.Failed ->
                VpnWakeLockRelease.CleanupPending(platform.reason)
        }
    }

    private fun saturatingAdd(value: Long, increment: Long): Long =
        if (value > Long.MAX_VALUE - increment) Long.MAX_VALUE else value + increment
}

internal object UnavailableVpnWakeLockHandle : VpnWakeLockHandle {
    override fun acquire(timeoutMillis: Long): VpnWakeLockPlatformAcquire =
        VpnWakeLockPlatformAcquire.Unavailable("Android PowerManager is unavailable")

    override fun release(): VpnWakeLockPlatformRelease = VpnWakeLockPlatformRelease.Expired
}
