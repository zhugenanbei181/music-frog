package com.musicfrog.infiltrator

import infiltrator_android.FfiErrorCode
import infiltrator_android.FfiStatus
import java.util.concurrent.TimeUnit
import java.util.concurrent.locks.ReentrantLock
import kotlin.concurrent.withLock

/** Process-local facts for the native VPN service startup transaction. */
internal sealed interface VpnProcessState {
    data object Stopped : VpnProcessState

    data object StartRequested : VpnProcessState

    data object Starting : VpnProcessState

    data object Foreground : VpnProcessState

    data object TunnelEstablished : VpnProcessState

    data object Running : VpnProcessState

    data object Stopping : VpnProcessState

    data object Revoked : VpnProcessState

    data class Failed(val message: String) : VpnProcessState
}

internal sealed interface VpnStartResult {
    data object Running : VpnStartResult

    data object PermissionRequired : VpnStartResult

    data class Failed(val message: String) : VpnStartResult

    data object TimedOut : VpnStartResult
}

internal sealed interface VpnDataPlaneStartReceipt {
    /** Rust confirms the duplicated TUN device exists and tun2proxy reached its packet-read loop. */
    data object Started : VpnDataPlaneStartReceipt

    data class Failed(val message: String) : VpnDataPlaneStartReceipt
}

internal sealed interface VpnTransitionOutcome {
    data object Applied : VpnTransitionOutcome

    data object StaleGeneration : VpnTransitionOutcome

    data object RunningRequiresDataPlaneReceipt : VpnTransitionOutcome
}

internal sealed interface VpnDataPlaneStartPublication {
    data object Published : VpnDataPlaneStartPublication

    data object StaleGeneration : VpnDataPlaneStartPublication

    data object RoutingLeaseNotHeld : VpnDataPlaneStartPublication

    data class UnexpectedState(val state: VpnProcessState) : VpnDataPlaneStartPublication
}

/** The Rust FFI returns OK only after its typed worker readiness receipt and owner readback agree. */
internal fun FfiStatus.toVpnDataPlaneStartReceipt(): VpnDataPlaneStartReceipt =
    when (code) {
        FfiErrorCode.OK -> VpnDataPlaneStartReceipt.Started
        else -> VpnDataPlaneStartReceipt.Failed(
            message ?: "Rust failed to start the VPN data path",
        )
    }

internal sealed interface VpnStopResult {
    data object Stopped : VpnStopResult

    data class Failed(val message: String) : VpnStopResult

    data object TimedOut : VpnStopResult

    data object Superseded : VpnStopResult

    data object Canceled : VpnStopResult
}

internal data class VpnLifecycleSnapshot(
    val generation: Long,
    val state: VpnProcessState,
)

internal sealed interface VpnStartPlan {
    data class Dispatch(val generation: Long) : VpnStartPlan

    data class Join(val generation: Long) : VpnStartPlan

    data object AlreadyRunning : VpnStartPlan

    data object StopInProgress : VpnStartPlan
}

internal sealed interface VpnStopTarget {
    data object Current : VpnStopTarget

    data class Generation(val value: Long) : VpnStopTarget {
        init {
            require(value > 0L)
        }
    }

    companion object {
        fun fromWire(targetCode: Int, generation: Long): VpnStopTarget? =
            when (VpnStopTargetWire.fromWireCode(targetCode)) {
                VpnStopTargetWire.Current -> when (generation) {
                    0L -> VpnStopTarget.Current
                    else -> null
                }
                VpnStopTargetWire.Generation -> when {
                    generation > 0L -> VpnStopTarget.Generation(generation)
                    else -> null
                }
                null -> null
            }
    }
}

internal enum class VpnStopTargetWire(val wireCode: Int) {
    Current(0),
    Generation(1);

    companion object {
        fun fromWireCode(code: Int): VpnStopTargetWire? = entries.firstOrNull { it.wireCode == code }
    }
}

/** Serializes VPN lifecycle facts and rejects completions from older commands. */
internal class VpnLifecycleCoordinator {
    private val lock = ReentrantLock()
    private val changed = lock.newCondition()
    private var current = VpnLifecycleSnapshot(0, VpnProcessState.Stopped)
    private var pendingCleanup: VpnCleanupRequest? = null

    fun snapshot(): VpnLifecycleSnapshot = lock.withLock { current }

    fun hasPendingCleanup(): Boolean = lock.withLock { pendingCleanup != null }

    fun preparedConfigurationGate(generation: Long): VpnPreparedConfigurationGate = lock.withLock {
        if (pendingCleanup != null) return@withLock VpnPreparedConfigurationGate.CleanupPending
        if (current.generation != generation) return@withLock VpnPreparedConfigurationGate.StaleGeneration
        when (val state = current.state) {
            VpnProcessState.StartRequested,
            VpnProcessState.Starting,
            VpnProcessState.Foreground -> VpnPreparedConfigurationGate.Accepted
            else -> VpnPreparedConfigurationGate.UnexpectedState(state)
        }
    }

    fun requestStart(resources: VpnOwnedResourcesPresence = VpnOwnedResourcesPresence.Clear): VpnStartPlan = lock.withLock {
        if (pendingCleanup != null) {
            return@withLock VpnStartPlan.StopInProgress
        }
        when (current.state) {
            VpnProcessState.Running -> VpnStartPlan.AlreadyRunning
            VpnProcessState.StartRequested,
            VpnProcessState.Starting,
            VpnProcessState.Foreground,
            VpnProcessState.TunnelEstablished -> VpnStartPlan.Join(current.generation)
            VpnProcessState.Stopping -> VpnStartPlan.StopInProgress
            is VpnProcessState.Failed -> {
                if (resources == VpnOwnedResourcesPresence.Retained) {
                    return@withLock VpnStartPlan.StopInProgress
                }
                val generation = current.generation + 1
                current = VpnLifecycleSnapshot(generation, VpnProcessState.StartRequested)
                changed.signalAll()
                VpnStartPlan.Dispatch(generation)
            }
            VpnProcessState.Stopped,
            VpnProcessState.Revoked -> {
                if (resources == VpnOwnedResourcesPresence.Retained) {
                    return@withLock VpnStartPlan.StopInProgress
                }
                val generation = current.generation + 1
                current = VpnLifecycleSnapshot(generation, VpnProcessState.StartRequested)
                changed.signalAll()
                VpnStartPlan.Dispatch(generation)
            }
        }
    }

    fun requestCleanup(
        intent: VpnCleanupIntent,
        expectedGeneration: Long? = null,
        resources: VpnOwnedResourcesPresence = VpnOwnedResourcesPresence.Clear,
    ): VpnCleanupPlan = lock.withLock {
        if (expectedGeneration != null && expectedGeneration != current.generation) {
            return@withLock VpnCleanupPlan.Superseded
        }
        val existing = pendingCleanup
        if (existing != null) {
            val merged = existing.merge(intent)
            pendingCleanup = merged
            return@withLock when (current.state) {
                VpnProcessState.Stopping -> VpnCleanupPlan.Join(merged)
                is VpnProcessState.Failed -> {
                    current = VpnLifecycleSnapshot(merged.generation, VpnProcessState.Stopping)
                    changed.signalAll()
                    VpnCleanupPlan.Retry(merged)
                }
                else -> VpnCleanupPlan.Join(merged)
            }
        }

        val alreadyTerminal = when (intent) {
            VpnCleanupIntent.Stop -> current.state == VpnProcessState.Stopped || current.state == VpnProcessState.Revoked
            VpnCleanupIntent.Revoke -> current.state == VpnProcessState.Revoked
            is VpnCleanupIntent.StartFailure -> false
        }
        if (resources == VpnOwnedResourcesPresence.Clear && alreadyTerminal) {
            return@withLock VpnCleanupPlan.AlreadyClean(current.generation)
        }

        val resourceGeneration = current.generation
        val generation = current.generation + 1
        val request = VpnCleanupRequest(generation, resourceGeneration, intent)
        pendingCleanup = request
        current = VpnLifecycleSnapshot(generation, VpnProcessState.Stopping)
        changed.signalAll()
        VpnCleanupPlan.Dispatch(request)
    }

    fun requestCleanup(
        intent: VpnCleanupIntent,
        target: VpnStopTarget,
        resources: VpnOwnedResourcesPresence = VpnOwnedResourcesPresence.Clear,
    ): VpnCleanupPlan = when (target) {
        VpnStopTarget.Current -> requestCleanup(intent, resources = resources)
        is VpnStopTarget.Generation -> requestCleanup(
            intent,
            expectedGeneration = target.value,
            resources = resources,
        )
    }

    fun retryPendingCleanup(): VpnCleanupPlan = lock.withLock {
        val pending = pendingCleanup ?: return@withLock VpnCleanupPlan.AlreadyClean(current.generation)
        when (current.state) {
            VpnProcessState.Stopping -> VpnCleanupPlan.Join(pending)
            else -> {
                current = VpnLifecycleSnapshot(pending.generation, VpnProcessState.Stopping)
                changed.signalAll()
                VpnCleanupPlan.Retry(pending)
            }
        }
    }

    fun cleanupQueueRejected(
        request: VpnCleanupRequest,
        message: String,
        resource: VpnCleanupResourceKind = VpnCleanupResourceKind.ExecutorQueue,
    ): VpnCleanupPublication = lock.withLock {
        val pending = matchingCleanup(request) ?: return@withLock VpnCleanupPublication.StaleGeneration
        val failure = VpnCleanupFailure(resource, message)
        current = VpnLifecycleSnapshot(
            pending.generation,
            VpnProcessState.Failed("VPN cleanup pending: ${failure.message}"),
        )
        changed.signalAll()
        VpnCleanupPublication.CleanupPending(failure)
    }

    fun completeCleanup(
        request: VpnCleanupRequest,
        outcome: VpnCleanupOutcome,
        ownerReadback: VpnCleanupOwnerReadback,
    ): VpnCleanupPublication = lock.withLock {
        val pending = matchingCleanup(request) ?: return@withLock VpnCleanupPublication.StaleGeneration
        val failure = when (outcome) {
            VpnCleanupOutcome.Completed -> when {
                ownerReadback.generation != pending.generation ||
                    ownerReadback.state != VpnProcessState.Stopping -> VpnCleanupFailure(
                    VpnCleanupResourceKind.OwnerReadback,
                    "VPN cleanup owner readback did not match the active cleanup generation",
                )
                ownerReadback.resources != VpnOwnedResourcesPresence.Clear -> VpnCleanupFailure(
                    VpnCleanupResourceKind.OwnerReadback,
                    "VPN owner still retains resources after cleanup",
                )
                else -> null
            }
            is VpnCleanupOutcome.CleanupPending -> outcome.failure
        }
        if (failure != null) {
            current = VpnLifecycleSnapshot(
                pending.generation,
                VpnProcessState.Failed("VPN cleanup pending: ${failure.message}"),
            )
            changed.signalAll()
            return@withLock VpnCleanupPublication.CleanupPending(failure)
        }

        val terminalState = when (val intent = pending.intent) {
            VpnCleanupIntent.Stop -> VpnProcessState.Stopped
            VpnCleanupIntent.Revoke -> VpnProcessState.Revoked
            is VpnCleanupIntent.StartFailure -> VpnProcessState.Failed(intent.message)
        }
        current = VpnLifecycleSnapshot(pending.generation, terminalState)
        pendingCleanup = null
        changed.signalAll()
        VpnCleanupPublication.Published(terminalState)
    }

    private fun matchingCleanup(request: VpnCleanupRequest): VpnCleanupRequest? = when (val pending = pendingCleanup) {
        null -> null
        else -> pending.takeIf {
            current.generation == request.generation &&
                pending.generation == request.generation &&
                pending.resourceGeneration == request.resourceGeneration
        }
    }

    fun transition(generation: Long, state: VpnProcessState): VpnTransitionOutcome = lock.withLock {
        if (current.generation != generation) return@withLock VpnTransitionOutcome.StaleGeneration
        if (state == VpnProcessState.Running) {
            return@withLock VpnTransitionOutcome.RunningRequiresDataPlaneReceipt
        }
        current = VpnLifecycleSnapshot(generation, state)
        changed.signalAll()
        VpnTransitionOutcome.Applied
    }

    /** Running publication requires both Rust readiness and an owner-held routing lease. */
    fun completeDataPlaneStart(
        generation: Long,
        receipt: VpnDataPlaneStartReceipt,
        routingLease: VpnRoutingLeaseState,
    ): VpnDataPlaneStartPublication = lock.withLock {
        if (current.generation != generation) {
            return@withLock VpnDataPlaneStartPublication.StaleGeneration
        }
        if (current.state != VpnProcessState.TunnelEstablished) {
            return@withLock VpnDataPlaneStartPublication.UnexpectedState(current.state)
        }
        val nextState = when (receipt) {
            VpnDataPlaneStartReceipt.Started -> when (routingLease) {
                is VpnRoutingLeaseState.Held -> VpnProcessState.Running
                VpnRoutingLeaseState.Released,
                is VpnRoutingLeaseState.ReleasePending -> {
                    current = VpnLifecycleSnapshot(
                        generation,
                        VpnProcessState.Failed("VPN Running requires a held routing activation lease"),
                    )
                    changed.signalAll()
                    return@withLock VpnDataPlaneStartPublication.RoutingLeaseNotHeld
                }
            }
            is VpnDataPlaneStartReceipt.Failed -> VpnProcessState.Failed(receipt.message)
        }
        current = VpnLifecycleSnapshot(generation, nextState)
        changed.signalAll()
        VpnDataPlaneStartPublication.Published
    }

    fun isCurrent(generation: Long): Boolean = lock.withLock {
        current.generation == generation
    }

    fun awaitStart(generation: Long, timeoutMillis: Long): VpnStartResult = lock.withLock {
        var remainingNanos = TimeUnit.MILLISECONDS.toNanos(timeoutMillis)
        while (true) {
            when (val state = current.state) {
                VpnProcessState.Running -> {
                    return@withLock if (current.generation == generation) {
                        VpnStartResult.Running
                    } else {
                        VpnStartResult.Failed("VPN start request was superseded")
                    }
                }
                VpnProcessState.Revoked -> return@withLock VpnStartResult.PermissionRequired
                is VpnProcessState.Failed -> return@withLock VpnStartResult.Failed(state.message)
                VpnProcessState.Stopped,
                VpnProcessState.Stopping -> return@withLock when (pendingCleanup?.intent) {
                    VpnCleanupIntent.Revoke -> VpnStartResult.PermissionRequired
                    VpnCleanupIntent.Stop,
                    is VpnCleanupIntent.StartFailure,
                    null -> VpnStartResult.Failed("VPN service stopped before proxy became active")
                }
                VpnProcessState.StartRequested,
                VpnProcessState.Starting,
                VpnProcessState.Foreground,
                VpnProcessState.TunnelEstablished -> {
                    if (current.generation != generation) {
                        return@withLock VpnStartResult.Failed("VPN start request was superseded")
                    }
                    if (remainingNanos <= 0) return@withLock VpnStartResult.TimedOut
                    try {
                        remainingNanos = changed.awaitNanos(remainingNanos)
                    } catch (error: InterruptedException) {
                        Thread.currentThread().interrupt()
                        return@withLock VpnStartResult.Failed("VPN startup wait was interrupted")
                    }
                }
            }
        }
        @Suppress("UNREACHABLE_CODE")
        VpnStartResult.TimedOut
    }

    /** Wait using the caller's absolute deadline so queue time is not reset at owner admission. */
    fun awaitStartUntil(
        generation: Long,
        deadline: VpnOwnerRequestDeadline,
        elapsedRealtimeMillis: () -> Long,
    ): VpnStartResult = lock.withLock {
        while (true) {
            when (val state = current.state) {
                VpnProcessState.Running -> {
                    return@withLock if (current.generation == generation) {
                        VpnStartResult.Running
                    } else {
                        VpnStartResult.Failed("VPN start request was superseded")
                    }
                }
                VpnProcessState.Revoked -> return@withLock VpnStartResult.PermissionRequired
                is VpnProcessState.Failed -> return@withLock VpnStartResult.Failed(state.message)
                VpnProcessState.Stopped,
                VpnProcessState.Stopping -> return@withLock when (pendingCleanup?.intent) {
                    VpnCleanupIntent.Revoke -> VpnStartResult.PermissionRequired
                    VpnCleanupIntent.Stop,
                    is VpnCleanupIntent.StartFailure,
                    null -> VpnStartResult.Failed("VPN service stopped before proxy became active")
                }
                VpnProcessState.StartRequested,
                VpnProcessState.Starting,
                VpnProcessState.Foreground,
                VpnProcessState.TunnelEstablished -> {
                    if (current.generation != generation) {
                        return@withLock VpnStartResult.Failed("VPN start request was superseded")
                    }
                    val remainingMillis = when (
                        val budget = deadline.remainingAt(elapsedRealtimeMillis())
                    ) {
                        VpnDeadlineState.Expired -> return@withLock VpnStartResult.TimedOut
                        is VpnDeadlineState.Remaining -> budget.millis
                    }
                    try {
                        changed.awaitNanos(TimeUnit.MILLISECONDS.toNanos(remainingMillis))
                    } catch (error: InterruptedException) {
                        Thread.currentThread().interrupt()
                        return@withLock VpnStartResult.Failed("VPN startup wait was interrupted")
                    }
                }
            }
        }
        @Suppress("UNREACHABLE_CODE")
        VpnStartResult.TimedOut
    }

    fun awaitStop(generation: Long, timeoutMillis: Long): VpnStopResult = lock.withLock {
        var remainingNanos = TimeUnit.MILLISECONDS.toNanos(timeoutMillis)
        while (true) {
            when (val state = current.state) {
                VpnProcessState.Stopped,
                VpnProcessState.Revoked -> {
                    return@withLock if (current.generation == generation) {
                        VpnStopResult.Stopped
                    } else {
                        VpnStopResult.Superseded
                    }
                }
                is VpnProcessState.Failed -> return@withLock VpnStopResult.Failed(state.message)
                VpnProcessState.StartRequested,
                VpnProcessState.Starting,
                VpnProcessState.Foreground,
                VpnProcessState.TunnelEstablished,
                VpnProcessState.Running,
                VpnProcessState.Stopping -> {
                    if (current.generation != generation) {
                        return@withLock VpnStopResult.Superseded
                    }
                    if (remainingNanos <= 0) return@withLock VpnStopResult.TimedOut
                    try {
                        remainingNanos = changed.awaitNanos(remainingNanos)
                    } catch (error: InterruptedException) {
                        Thread.currentThread().interrupt()
                        return@withLock VpnStopResult.Canceled
                    }
                }
            }
        }
        @Suppress("UNREACHABLE_CODE")
        VpnStopResult.TimedOut
    }

    /** Stop waits share their original caller deadline; cleanup admission remains owner-owned. */
    fun awaitStopUntil(
        generation: Long,
        deadline: VpnOwnerRequestDeadline,
        elapsedRealtimeMillis: () -> Long,
    ): VpnStopResult = lock.withLock {
        while (true) {
            when (val state = current.state) {
                VpnProcessState.Stopped,
                VpnProcessState.Revoked -> {
                    return@withLock if (current.generation == generation) {
                        VpnStopResult.Stopped
                    } else {
                        VpnStopResult.Superseded
                    }
                }
                is VpnProcessState.Failed -> return@withLock VpnStopResult.Failed(state.message)
                VpnProcessState.StartRequested,
                VpnProcessState.Starting,
                VpnProcessState.Foreground,
                VpnProcessState.TunnelEstablished,
                VpnProcessState.Running,
                VpnProcessState.Stopping -> {
                    if (current.generation != generation) return@withLock VpnStopResult.Superseded
                    val remainingMillis = when (
                        val budget = deadline.remainingAt(elapsedRealtimeMillis())
                    ) {
                        VpnDeadlineState.Expired -> return@withLock VpnStopResult.TimedOut
                        is VpnDeadlineState.Remaining -> budget.millis
                    }
                    try {
                        changed.awaitNanos(TimeUnit.MILLISECONDS.toNanos(remainingMillis))
                    } catch (error: InterruptedException) {
                        Thread.currentThread().interrupt()
                        return@withLock VpnStopResult.Canceled
                    }
                }
            }
        }
        @Suppress("UNREACHABLE_CODE")
        VpnStopResult.TimedOut
    }

    fun failUnrequestedRestart(message: String): VpnLifecycleSnapshot = lock.withLock {
        if (pendingCleanup != null) return@withLock current
        val generation = current.generation + 1
        current = VpnLifecycleSnapshot(generation, VpnProcessState.Failed(message))
        changed.signalAll()
        current
    }
}

internal fun VpnProcessState.isForegroundProtected(): Boolean =
    when (this) {
        VpnProcessState.Foreground,
        VpnProcessState.TunnelEstablished,
        VpnProcessState.Running -> true
        VpnProcessState.Stopped,
        VpnProcessState.StartRequested,
        VpnProcessState.Starting,
        VpnProcessState.Stopping,
        VpnProcessState.Revoked,
        is VpnProcessState.Failed -> false
    }
