package com.musicfrog.infiltrator

/** A caller-owned request identity that is unique across client process sessions. */
internal data class VpnOwnerRequestKey(
    val clientSessionId: String,
    val requestId: Long,
) {
    init {
        require(clientSessionId.isNotBlank())
        require(requestId > 0L)
    }
}

/** Absolute Android elapsed-realtime deadline, shared across Binder and service work. */
internal data class VpnOwnerRequestDeadline(val elapsedRealtimeMillis: Long) {
    init {
        require(elapsedRealtimeMillis > 0L)
    }

    fun remainingAt(nowElapsedRealtimeMillis: Long): VpnDeadlineState = when {
        nowElapsedRealtimeMillis >= elapsedRealtimeMillis -> VpnDeadlineState.Expired
        else -> VpnDeadlineState.Remaining(elapsedRealtimeMillis - nowElapsedRealtimeMillis)
    }

    companion object {
        fun after(nowElapsedRealtimeMillis: Long, timeoutMillis: Long): VpnOwnerRequestDeadline {
            require(nowElapsedRealtimeMillis > 0L)
            require(timeoutMillis > 0L)
            val deadline = if (Long.MAX_VALUE - nowElapsedRealtimeMillis < timeoutMillis) {
                Long.MAX_VALUE
            } else {
                nowElapsedRealtimeMillis + timeoutMillis
            }
            return VpnOwnerRequestDeadline(deadline)
        }
    }
}

internal sealed interface VpnDeadlineState {
    data object Expired : VpnDeadlineState

    data class Remaining(val millis: Long) : VpnDeadlineState {
        init {
            require(millis > 0L)
        }
    }
}

internal sealed interface VpnOwnerStartAdmission {
    data object Admitted : VpnOwnerStartAdmission

    data object Duplicate : VpnOwnerStartAdmission

    data object QueueFull : VpnOwnerStartAdmission

    data object Expired : VpnOwnerStartAdmission
}

internal sealed interface VpnOwnerStartExecution {
    data class Started(val deadline: VpnOwnerRequestDeadline) : VpnOwnerStartExecution

    data object Canceled : VpnOwnerStartExecution

    data object Expired : VpnOwnerStartExecution

    data object Unknown : VpnOwnerStartExecution
}

internal sealed interface VpnOwnerStartGenerationBinding {
    data object Bound : VpnOwnerStartGenerationBinding

    data object ExecutionAlreadyStarted : VpnOwnerStartGenerationBinding

    data object Canceled : VpnOwnerStartGenerationBinding

    data object Unknown : VpnOwnerStartGenerationBinding
}

internal sealed interface VpnOwnerStartDispatchOutcome {
    data object Admitted : VpnOwnerStartDispatchOutcome

    data object ExecutionStarted : VpnOwnerStartDispatchOutcome

    data object Dispatched : VpnOwnerStartDispatchOutcome

    data object Canceled : VpnOwnerStartDispatchOutcome

    data object Unknown : VpnOwnerStartDispatchOutcome
}

internal enum class VpnOwnerStartDispatchPhase {
    NotDispatched,
    DispatchPending,
    Dispatching,
    Dispatched,
}

internal sealed interface VpnOwnerStartCancellation {
    data object Unknown : VpnOwnerStartCancellation

    data object CanceledBeforeGeneration : VpnOwnerStartCancellation

    data class CancelGeneration(
        val generation: Long,
    ) : VpnOwnerStartCancellation

    data object ExecutionAlreadyStarted : VpnOwnerStartCancellation

    data object TerminalReadback : VpnOwnerStartCancellation

    data object AlreadyCanceled : VpnOwnerStartCancellation
}

internal sealed interface VpnOwnerStartCompletion {
    data object Publish : VpnOwnerStartCompletion

    data object SuppressLateReceipt : VpnOwnerStartCompletion

    data object Unknown : VpnOwnerStartCompletion
}

internal sealed interface VpnOwnerStartAcknowledgment {
    data object Acknowledged : VpnOwnerStartAcknowledgment

    data object Unknown : VpnOwnerStartAcknowledgment
}

internal sealed interface VpnOwnerStartExpiry {
    data object NotExpired : VpnOwnerStartExpiry

    data object ExpiredTerminal : VpnOwnerStartExpiry

    data object Unknown : VpnOwnerStartExpiry
}

/**
 * Bounded admission and cancellation fence for owner Start requests. At most
 * one running and one queued start can be retained; canceled entries remain
 * until their worker observes the terminal state and releases its slot.
 */
internal class VpnOwnerStartRequestAdmission(private val capacity: Int = 2) {
    init {
        require(capacity in 1..2)
    }

    private val requests = linkedMapOf<VpnOwnerRequestKey, VpnOwnerStartRequestState>()

    @Synchronized
    fun admit(
        key: VpnOwnerRequestKey,
        deadline: VpnOwnerRequestDeadline,
        nowElapsedRealtimeMillis: Long,
    ): VpnOwnerStartAdmission {
        if (requests.containsKey(key)) return VpnOwnerStartAdmission.Duplicate
        if (deadline.remainingAt(nowElapsedRealtimeMillis) == VpnDeadlineState.Expired) {
            return VpnOwnerStartAdmission.Expired
        }
        if (requests.size >= capacity) return VpnOwnerStartAdmission.QueueFull
        requests[key] = VpnOwnerStartRequestState.Queued(deadline)
        return VpnOwnerStartAdmission.Admitted
    }

    @Synchronized
    fun begin(
        key: VpnOwnerRequestKey,
        nowElapsedRealtimeMillis: Long,
    ): VpnOwnerStartExecution = when (val state = requests[key]) {
        null -> VpnOwnerStartExecution.Unknown
        is VpnOwnerStartRequestState.Queued -> when (
            state.deadline.remainingAt(nowElapsedRealtimeMillis)
        ) {
            is VpnDeadlineState.Remaining -> {
                requests[key] = VpnOwnerStartRequestState.Executing(state.deadline)
                VpnOwnerStartExecution.Started(state.deadline)
            }
            VpnDeadlineState.Expired -> VpnOwnerStartExecution.Expired
        }
        is VpnOwnerStartRequestState.CanceledBeforeGeneration -> VpnOwnerStartExecution.Canceled
        is VpnOwnerStartRequestState.GenerationBound,
        is VpnOwnerStartRequestState.ServiceDispatchPending,
        is VpnOwnerStartRequestState.ServiceDispatching,
        is VpnOwnerStartRequestState.ServiceDispatched,
        is VpnOwnerStartRequestState.TerminalAwaitingAcknowledgment,
        is VpnOwnerStartRequestState.CanceledForGeneration,
        is VpnOwnerStartRequestState.Executing -> VpnOwnerStartExecution.Unknown
    }

    @Synchronized
    fun associateGeneration(
        key: VpnOwnerRequestKey,
        generation: Long,
    ): VpnOwnerStartGenerationBinding {
        require(generation > 0L)
        return when (val state = requests[key]) {
            null -> VpnOwnerStartGenerationBinding.Unknown
            is VpnOwnerStartRequestState.Executing -> {
                requests[key] = VpnOwnerStartRequestState.GenerationBound(state.deadline, generation)
                VpnOwnerStartGenerationBinding.Bound
            }
            is VpnOwnerStartRequestState.CanceledBeforeGeneration -> {
                requests[key] = VpnOwnerStartRequestState.CanceledForGeneration(generation)
                VpnOwnerStartGenerationBinding.Canceled
            }
            is VpnOwnerStartRequestState.Queued,
            is VpnOwnerStartRequestState.GenerationBound,
            is VpnOwnerStartRequestState.ServiceDispatchPending,
            is VpnOwnerStartRequestState.ServiceDispatching,
            is VpnOwnerStartRequestState.ServiceDispatched,
            is VpnOwnerStartRequestState.TerminalAwaitingAcknowledgment,
            is VpnOwnerStartRequestState.CanceledForGeneration ->
                VpnOwnerStartGenerationBinding.Unknown
        }
    }

    /** A joined generation already crossed the service-start side-effect boundary. */
    @Synchronized
    fun associateAlreadyExecutingGeneration(
        key: VpnOwnerRequestKey,
        generation: Long,
    ): VpnOwnerStartGenerationBinding {
        require(generation > 0L)
        return when (val state = requests[key]) {
            is VpnOwnerStartRequestState.Executing -> {
                requests[key] = VpnOwnerStartRequestState.ServiceDispatched(state.deadline, generation)
                VpnOwnerStartGenerationBinding.ExecutionAlreadyStarted
            }
            is VpnOwnerStartRequestState.CanceledBeforeGeneration -> {
                requests[key] = VpnOwnerStartRequestState.CanceledForGeneration(generation)
                VpnOwnerStartGenerationBinding.Canceled
            }
            null -> VpnOwnerStartGenerationBinding.Unknown
            is VpnOwnerStartRequestState.Queued,
            is VpnOwnerStartRequestState.GenerationBound,
            is VpnOwnerStartRequestState.ServiceDispatchPending,
            is VpnOwnerStartRequestState.ServiceDispatching,
            is VpnOwnerStartRequestState.ServiceDispatched,
            is VpnOwnerStartRequestState.TerminalAwaitingAcknowledgment,
            is VpnOwnerStartRequestState.CanceledForGeneration -> VpnOwnerStartGenerationBinding.Unknown
        }
    }

    @Synchronized
    fun markServiceDispatchPending(
        key: VpnOwnerRequestKey,
        generation: Long,
    ): VpnOwnerStartDispatchOutcome {
        require(generation > 0L)
        return when (val state = requests[key]) {
            is VpnOwnerStartRequestState.GenerationBound -> if (state.generation == generation) {
                requests[key] = VpnOwnerStartRequestState.ServiceDispatchPending(state.deadline, generation)
                VpnOwnerStartDispatchOutcome.Admitted
            } else {
                VpnOwnerStartDispatchOutcome.Unknown
            }
            is VpnOwnerStartRequestState.CanceledForGeneration ->
                if (state.generation == generation) VpnOwnerStartDispatchOutcome.Canceled
                else VpnOwnerStartDispatchOutcome.Unknown
            VpnOwnerStartRequestState.CanceledBeforeGeneration -> VpnOwnerStartDispatchOutcome.Canceled
            null,
            is VpnOwnerStartRequestState.Queued,
            is VpnOwnerStartRequestState.Executing,
            is VpnOwnerStartRequestState.ServiceDispatchPending,
            is VpnOwnerStartRequestState.ServiceDispatching,
            is VpnOwnerStartRequestState.ServiceDispatched,
            is VpnOwnerStartRequestState.TerminalAwaitingAcknowledgment ->
                VpnOwnerStartDispatchOutcome.Unknown
        }
    }

    /** Commit to invoking startForegroundService; cancellation may no longer roll it back. */
    @Synchronized
    fun markServiceDispatchStarted(
        key: VpnOwnerRequestKey,
        generation: Long,
    ): VpnOwnerStartDispatchOutcome {
        require(generation > 0L)
        return when (val state = requests[key]) {
            is VpnOwnerStartRequestState.ServiceDispatchPending -> if (state.generation == generation) {
                requests[key] = VpnOwnerStartRequestState.ServiceDispatching(state.deadline, generation)
                VpnOwnerStartDispatchOutcome.ExecutionStarted
            } else {
                VpnOwnerStartDispatchOutcome.Unknown
            }
            is VpnOwnerStartRequestState.CanceledForGeneration ->
                if (state.generation == generation) VpnOwnerStartDispatchOutcome.Canceled
                else VpnOwnerStartDispatchOutcome.Unknown
            VpnOwnerStartRequestState.CanceledBeforeGeneration -> VpnOwnerStartDispatchOutcome.Canceled
            null,
            is VpnOwnerStartRequestState.Queued,
            is VpnOwnerStartRequestState.Executing,
            is VpnOwnerStartRequestState.GenerationBound,
            is VpnOwnerStartRequestState.ServiceDispatching,
            is VpnOwnerStartRequestState.ServiceDispatched,
            is VpnOwnerStartRequestState.TerminalAwaitingAcknowledgment ->
                VpnOwnerStartDispatchOutcome.Unknown
        }
    }

    @Synchronized
    fun markServiceDispatched(
        key: VpnOwnerRequestKey,
        generation: Long,
    ): VpnOwnerStartDispatchOutcome {
        require(generation > 0L)
        return when (val state = requests[key]) {
            is VpnOwnerStartRequestState.ServiceDispatching -> if (state.generation == generation) {
                requests[key] = VpnOwnerStartRequestState.ServiceDispatched(state.deadline, generation)
                VpnOwnerStartDispatchOutcome.Dispatched
            } else {
                VpnOwnerStartDispatchOutcome.Unknown
            }
            is VpnOwnerStartRequestState.CanceledForGeneration ->
                if (state.generation == generation) VpnOwnerStartDispatchOutcome.Canceled
                else VpnOwnerStartDispatchOutcome.Unknown
            VpnOwnerStartRequestState.CanceledBeforeGeneration -> VpnOwnerStartDispatchOutcome.Canceled
            null,
            is VpnOwnerStartRequestState.Queued,
            is VpnOwnerStartRequestState.Executing,
            is VpnOwnerStartRequestState.GenerationBound,
            is VpnOwnerStartRequestState.ServiceDispatchPending,
            is VpnOwnerStartRequestState.ServiceDispatched,
            is VpnOwnerStartRequestState.TerminalAwaitingAcknowledgment ->
                VpnOwnerStartDispatchOutcome.Unknown
        }
    }

    @Synchronized
    fun cancel(key: VpnOwnerRequestKey): VpnOwnerStartCancellation = when (val state = requests[key]) {
        null -> VpnOwnerStartCancellation.Unknown
        is VpnOwnerStartRequestState.Queued,
        is VpnOwnerStartRequestState.Executing -> {
            requests[key] = VpnOwnerStartRequestState.CanceledBeforeGeneration
            VpnOwnerStartCancellation.CanceledBeforeGeneration
        }
        is VpnOwnerStartRequestState.GenerationBound -> {
            requests[key] = VpnOwnerStartRequestState.CanceledForGeneration(state.generation)
            VpnOwnerStartCancellation.CancelGeneration(state.generation)
        }
        is VpnOwnerStartRequestState.ServiceDispatchPending -> {
            requests[key] = VpnOwnerStartRequestState.CanceledForGeneration(state.generation)
            VpnOwnerStartCancellation.CancelGeneration(state.generation)
        }
        is VpnOwnerStartRequestState.ServiceDispatching,
        is VpnOwnerStartRequestState.ServiceDispatched -> VpnOwnerStartCancellation.ExecutionAlreadyStarted
        is VpnOwnerStartRequestState.TerminalAwaitingAcknowledgment -> when (val terminal = state.terminal) {
            VpnOwnerStartTerminal.NoGeneration -> VpnOwnerStartCancellation.TerminalReadback
            is VpnOwnerStartTerminal.Generation -> when (terminal.dispatchPhase) {
                VpnOwnerStartDispatchPhase.Dispatching,
                VpnOwnerStartDispatchPhase.Dispatched -> VpnOwnerStartCancellation.ExecutionAlreadyStarted
                VpnOwnerStartDispatchPhase.NotDispatched,
                VpnOwnerStartDispatchPhase.DispatchPending -> VpnOwnerStartCancellation.TerminalReadback
            }
        }
        is VpnOwnerStartRequestState.CanceledBeforeGeneration,
        is VpnOwnerStartRequestState.CanceledForGeneration -> VpnOwnerStartCancellation.AlreadyCanceled
    }

    @Synchronized
    fun complete(key: VpnOwnerRequestKey): VpnOwnerStartCompletion = when (val state = requests[key]) {
        null -> VpnOwnerStartCompletion.Unknown
        VpnOwnerStartRequestState.CanceledBeforeGeneration,
        is VpnOwnerStartRequestState.CanceledForGeneration -> {
            requests.remove(key)
            VpnOwnerStartCompletion.SuppressLateReceipt
        }
        is VpnOwnerStartRequestState.Queued,
        is VpnOwnerStartRequestState.Executing,
        is VpnOwnerStartRequestState.GenerationBound,
        is VpnOwnerStartRequestState.ServiceDispatchPending,
        is VpnOwnerStartRequestState.ServiceDispatching,
        is VpnOwnerStartRequestState.ServiceDispatched -> {
            val terminal = when (state) {
                is VpnOwnerStartRequestState.GenerationBound -> VpnOwnerStartTerminal.Generation(
                    state.generation,
                    VpnOwnerStartDispatchPhase.NotDispatched,
                )
                is VpnOwnerStartRequestState.ServiceDispatchPending -> VpnOwnerStartTerminal.Generation(
                    state.generation,
                    VpnOwnerStartDispatchPhase.DispatchPending,
                )
                is VpnOwnerStartRequestState.ServiceDispatching -> VpnOwnerStartTerminal.Generation(
                    state.generation,
                    VpnOwnerStartDispatchPhase.Dispatching,
                )
                is VpnOwnerStartRequestState.ServiceDispatched -> VpnOwnerStartTerminal.Generation(
                    state.generation,
                    VpnOwnerStartDispatchPhase.Dispatched,
                )
                is VpnOwnerStartRequestState.Queued,
                is VpnOwnerStartRequestState.Executing -> VpnOwnerStartTerminal.NoGeneration
                is VpnOwnerStartRequestState.CanceledBeforeGeneration,
                is VpnOwnerStartRequestState.CanceledForGeneration,
                is VpnOwnerStartRequestState.TerminalAwaitingAcknowledgment ->
                    return VpnOwnerStartCompletion.Unknown
            }
            val deadline = when (state) {
                is VpnOwnerStartRequestState.Queued -> state.deadline
                is VpnOwnerStartRequestState.Executing -> state.deadline
                is VpnOwnerStartRequestState.GenerationBound -> state.deadline
                is VpnOwnerStartRequestState.ServiceDispatchPending -> state.deadline
                is VpnOwnerStartRequestState.ServiceDispatching -> state.deadline
                is VpnOwnerStartRequestState.ServiceDispatched -> state.deadline
                is VpnOwnerStartRequestState.CanceledBeforeGeneration,
                is VpnOwnerStartRequestState.CanceledForGeneration,
                is VpnOwnerStartRequestState.TerminalAwaitingAcknowledgment ->
                    return VpnOwnerStartCompletion.Unknown
            }
            requests[key] = VpnOwnerStartRequestState.TerminalAwaitingAcknowledgment(deadline, terminal)
            VpnOwnerStartCompletion.Publish
        }
        is VpnOwnerStartRequestState.TerminalAwaitingAcknowledgment -> VpnOwnerStartCompletion.Unknown
    }

    @Synchronized
    fun acknowledgeTerminal(key: VpnOwnerRequestKey): VpnOwnerStartAcknowledgment =
        when (requests[key]) {
            is VpnOwnerStartRequestState.TerminalAwaitingAcknowledgment -> {
                requests.remove(key)
                VpnOwnerStartAcknowledgment.Acknowledged
            }
            null,
            is VpnOwnerStartRequestState.Queued,
            is VpnOwnerStartRequestState.Executing,
            is VpnOwnerStartRequestState.GenerationBound,
            is VpnOwnerStartRequestState.ServiceDispatchPending,
            is VpnOwnerStartRequestState.ServiceDispatching,
            is VpnOwnerStartRequestState.ServiceDispatched,
            is VpnOwnerStartRequestState.CanceledBeforeGeneration,
            is VpnOwnerStartRequestState.CanceledForGeneration -> VpnOwnerStartAcknowledgment.Unknown
        }

    @Synchronized
    fun expireTerminal(
        key: VpnOwnerRequestKey,
        nowElapsedRealtimeMillis: Long,
    ): VpnOwnerStartExpiry = when (val state = requests[key]) {
        null -> VpnOwnerStartExpiry.Unknown
        is VpnOwnerStartRequestState.TerminalAwaitingAcknowledgment -> when (
            state.deadline.remainingAt(nowElapsedRealtimeMillis)
        ) {
            is VpnDeadlineState.Remaining -> VpnOwnerStartExpiry.NotExpired
            VpnDeadlineState.Expired -> {
                requests.remove(key)
                VpnOwnerStartExpiry.ExpiredTerminal
            }
        }
        is VpnOwnerStartRequestState.Queued,
        is VpnOwnerStartRequestState.Executing,
        is VpnOwnerStartRequestState.GenerationBound,
        is VpnOwnerStartRequestState.ServiceDispatchPending,
        is VpnOwnerStartRequestState.ServiceDispatching,
        is VpnOwnerStartRequestState.ServiceDispatched,
        is VpnOwnerStartRequestState.CanceledBeforeGeneration,
        is VpnOwnerStartRequestState.CanceledForGeneration -> VpnOwnerStartExpiry.Unknown
    }

    @Synchronized
    fun retainedRequestCount(): Int = requests.size

    @Synchronized
    fun activeKeys(): List<VpnOwnerRequestKey> = requests.keys.toList()

    private sealed interface VpnOwnerStartRequestState {
        data class Queued(val deadline: VpnOwnerRequestDeadline) : VpnOwnerStartRequestState

        data class Executing(val deadline: VpnOwnerRequestDeadline) : VpnOwnerStartRequestState

        data class GenerationBound(
            val deadline: VpnOwnerRequestDeadline,
            val generation: Long,
        ) : VpnOwnerStartRequestState

        data class ServiceDispatchPending(
            val deadline: VpnOwnerRequestDeadline,
            val generation: Long,
        ) : VpnOwnerStartRequestState

        data class ServiceDispatching(
            val deadline: VpnOwnerRequestDeadline,
            val generation: Long,
        ) : VpnOwnerStartRequestState

        data class ServiceDispatched(
            val deadline: VpnOwnerRequestDeadline,
            val generation: Long,
        ) : VpnOwnerStartRequestState

        data class TerminalAwaitingAcknowledgment(
            val deadline: VpnOwnerRequestDeadline,
            val terminal: VpnOwnerStartTerminal,
        ) : VpnOwnerStartRequestState

        data object CanceledBeforeGeneration : VpnOwnerStartRequestState

        data class CanceledForGeneration(val generation: Long) : VpnOwnerStartRequestState
    }

    private sealed interface VpnOwnerStartTerminal {
        data object NoGeneration : VpnOwnerStartTerminal

        data class Generation(
            val generation: Long,
            val dispatchPhase: VpnOwnerStartDispatchPhase,
        ) : VpnOwnerStartTerminal
    }
}

internal sealed interface VpnServiceAdmissionPhase {
    data object Accepting : VpnServiceAdmissionPhase

    data object Destroying : VpnServiceAdmissionPhase

    data object Destroyed : VpnServiceAdmissionPhase
}

internal sealed interface VpnServiceTaskSlot {
    data object Empty : VpnServiceTaskSlot

    data class Start(val generation: Long) : VpnServiceTaskSlot

    data class Cleanup(val generation: Long, val resourceGeneration: Long) : VpnServiceTaskSlot
}

internal sealed interface VpnServiceTaskAdmission {
    data object Accepted : VpnServiceTaskAdmission

    data object Joined : VpnServiceTaskAdmission

    data object ReplacedQueuedStart : VpnServiceTaskAdmission

    data object QueueFull : VpnServiceTaskAdmission

    data object Destroying : VpnServiceTaskAdmission

    data object Destroyed : VpnServiceTaskAdmission

    data object StaleGeneration : VpnServiceTaskAdmission
}

/** Typed mirror of the one pending slot in MihomoVpnService's bounded executor. */
internal class VpnServiceExecutorAdmission {
    private var phase: VpnServiceAdmissionPhase = VpnServiceAdmissionPhase.Accepting
    private var running: VpnServiceTaskSlot = VpnServiceTaskSlot.Empty
    private var pending: VpnServiceTaskSlot = VpnServiceTaskSlot.Empty

    @Synchronized
    fun admitStart(generation: Long): VpnServiceTaskAdmission {
        require(generation > 0L)
        return when (phase) {
            VpnServiceAdmissionPhase.Destroying -> VpnServiceTaskAdmission.Destroying
            VpnServiceAdmissionPhase.Destroyed -> VpnServiceTaskAdmission.Destroyed
            VpnServiceAdmissionPhase.Accepting -> when {
                running == VpnServiceTaskSlot.Start(generation) ||
                    pending == VpnServiceTaskSlot.Start(generation) -> VpnServiceTaskAdmission.Joined
                running is VpnServiceTaskSlot.Cleanup || pending is VpnServiceTaskSlot.Cleanup ->
                    VpnServiceTaskAdmission.StaleGeneration
                pending != VpnServiceTaskSlot.Empty -> VpnServiceTaskAdmission.QueueFull
                else -> {
                    pending = VpnServiceTaskSlot.Start(generation)
                    VpnServiceTaskAdmission.Accepted
                }
            }
        }
    }

    @Synchronized
    fun admitCleanup(request: VpnCleanupRequest): VpnServiceTaskAdmission {
        val cleanup = VpnServiceTaskSlot.Cleanup(request.generation, request.resourceGeneration)
        return when (phase) {
            VpnServiceAdmissionPhase.Destroyed -> VpnServiceTaskAdmission.Destroyed
            VpnServiceAdmissionPhase.Accepting,
            VpnServiceAdmissionPhase.Destroying -> when {
                running == cleanup || pending == cleanup -> VpnServiceTaskAdmission.Joined
                pending is VpnServiceTaskSlot.Start -> {
                    pending = cleanup
                    VpnServiceTaskAdmission.ReplacedQueuedStart
                }
                pending != VpnServiceTaskSlot.Empty -> VpnServiceTaskAdmission.QueueFull
                else -> {
                    pending = cleanup
                    VpnServiceTaskAdmission.Accepted
                }
            }
        }
    }

    @Synchronized
    fun beginStart(generation: Long): VpnServiceTaskAdmission = when (pending) {
        VpnServiceTaskSlot.Empty -> VpnServiceTaskAdmission.StaleGeneration
        is VpnServiceTaskSlot.Cleanup -> VpnServiceTaskAdmission.StaleGeneration
        is VpnServiceTaskSlot.Start -> if ((pending as VpnServiceTaskSlot.Start).generation == generation) {
            pending = VpnServiceTaskSlot.Empty
            running = VpnServiceTaskSlot.Start(generation)
            VpnServiceTaskAdmission.Accepted
        } else {
            VpnServiceTaskAdmission.StaleGeneration
        }
    }

    @Synchronized
    fun beginCleanup(request: VpnCleanupRequest): VpnServiceTaskAdmission {
        val cleanup = VpnServiceTaskSlot.Cleanup(request.generation, request.resourceGeneration)
        return when (pending) {
            VpnServiceTaskSlot.Empty -> VpnServiceTaskAdmission.StaleGeneration
            is VpnServiceTaskSlot.Start -> VpnServiceTaskAdmission.StaleGeneration
            is VpnServiceTaskSlot.Cleanup -> if (pending == cleanup) {
                pending = VpnServiceTaskSlot.Empty
                running = cleanup
                VpnServiceTaskAdmission.Accepted
            } else {
                VpnServiceTaskAdmission.StaleGeneration
            }
        }
    }

    @Synchronized
    fun rejectStart(generation: Long) {
        if (pending == VpnServiceTaskSlot.Start(generation)) pending = VpnServiceTaskSlot.Empty
    }

    @Synchronized
    fun rejectCleanup(request: VpnCleanupRequest) {
        val cleanup = VpnServiceTaskSlot.Cleanup(request.generation, request.resourceGeneration)
        if (pending == cleanup) pending = VpnServiceTaskSlot.Empty
    }

    @Synchronized
    fun finishStart(generation: Long) {
        if (running == VpnServiceTaskSlot.Start(generation)) running = VpnServiceTaskSlot.Empty
    }

    @Synchronized
    fun finishCleanup(request: VpnCleanupRequest) {
        val cleanup = VpnServiceTaskSlot.Cleanup(request.generation, request.resourceGeneration)
        if (running == cleanup) running = VpnServiceTaskSlot.Empty
    }

    @Synchronized
    fun beginDestroy(): VpnServiceAdmissionPhase {
        if (phase == VpnServiceAdmissionPhase.Accepting) phase = VpnServiceAdmissionPhase.Destroying
        return phase
    }

    @Synchronized
    fun finishDestroy(): VpnServiceAdmissionPhase {
        if (phase == VpnServiceAdmissionPhase.Destroying &&
            running == VpnServiceTaskSlot.Empty && pending == VpnServiceTaskSlot.Empty
        ) {
            phase = VpnServiceAdmissionPhase.Destroyed
        }
        return phase
    }

    @Synchronized
    fun phase(): VpnServiceAdmissionPhase = phase

    @Synchronized
    fun pendingTask(): VpnServiceTaskSlot = pending
}
