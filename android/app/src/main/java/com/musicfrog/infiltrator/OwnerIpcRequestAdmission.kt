package com.musicfrog.infiltrator

internal data class OwnerIpcRequestKey(val clientSessionId: String, val requestId: Long)

internal sealed interface OwnerIpcAdmission<out T> {
    data object Admitted : OwnerIpcAdmission<Nothing>

    data object InFlight : OwnerIpcAdmission<Nothing>

    data object CanceledBeforeExecution : OwnerIpcAdmission<Nothing>

    data class TerminalReadback<T>(val response: T) : OwnerIpcAdmission<T>

    data object QueueFull : OwnerIpcAdmission<Nothing>

    data object Expired : OwnerIpcAdmission<Nothing>

    data object RequestIdConflict : OwnerIpcAdmission<Nothing>
}

internal sealed interface OwnerIpcExecutionStart<out T> {
    data object Started : OwnerIpcExecutionStart<Nothing>

    data object InFlight : OwnerIpcExecutionStart<Nothing>

    data object Canceled : OwnerIpcExecutionStart<Nothing>

    data object Expired : OwnerIpcExecutionStart<Nothing>

    data class TerminalReadback<T>(val response: T) : OwnerIpcExecutionStart<T>

    data object Unknown : OwnerIpcExecutionStart<Nothing>
}

internal sealed interface OwnerIpcCancellation<out T> {
    data object CanceledBeforeExecution : OwnerIpcCancellation<Nothing>

    data object AlreadyCanceled : OwnerIpcCancellation<Nothing>

    data object ExecutionAlreadyStarted : OwnerIpcCancellation<Nothing>

    data class TerminalReadback<T>(val response: T) : OwnerIpcCancellation<T>

    data object Unknown : OwnerIpcCancellation<Nothing>
}

internal sealed interface OwnerIpcCompletion {
    data object TerminalCached : OwnerIpcCompletion

    data object SuppressCanceledCompletion : OwnerIpcCompletion

    data object Unknown : OwnerIpcCompletion
}

/**
 * Bounded per-owner request identity and terminal replay table.
 *
 * Cancellation only changes an admitted request before `begin`; once a request
 * enters `Executing`, the side effect continues and its typed terminal value is
 * retained for duplicate request IDs until the replay window expires.
 */
internal class OwnerIpcRequestAdmission<T>(
    private val capacity: Int = DEFAULT_CAPACITY,
    private val terminalRetentionMillis: Long = DEFAULT_TERMINAL_RETENTION_MILLIS,
) {
    private sealed interface State<out T> {
        data object Queued : State<Nothing>

        data object Executing : State<Nothing>

        data object Canceled : State<Nothing>

        data class Terminal<T>(val response: T) : State<T>
    }

    private data class Entry<T>(
        val fingerprint: String,
        val deadline: VpnOwnerRequestDeadline,
        val state: State<T>,
        val retainedUntilElapsedRealtimeMillis: Long,
    )

    private val entries = linkedMapOf<OwnerIpcRequestKey, Entry<T>>()

    init {
        require(capacity > 0)
        require(terminalRetentionMillis > 0L)
    }

    @Synchronized
    fun admit(
        key: OwnerIpcRequestKey,
        fingerprint: String,
        deadline: VpnOwnerRequestDeadline,
        nowElapsedRealtimeMillis: Long,
    ): OwnerIpcAdmission<T> {
        prune(nowElapsedRealtimeMillis)
        if (deadline.remainingAt(nowElapsedRealtimeMillis) == VpnDeadlineState.Expired) {
            return OwnerIpcAdmission.Expired
        }
        val existing = entries[key]
        if (existing != null) {
            if (existing.fingerprint != fingerprint) return OwnerIpcAdmission.RequestIdConflict
            return when (val state = existing.state) {
                State.Queued,
                State.Executing,
                -> OwnerIpcAdmission.InFlight

                State.Canceled -> OwnerIpcAdmission.CanceledBeforeExecution

                is State.Terminal -> OwnerIpcAdmission.TerminalReadback(state.response)
            }
        }
        if (entries.size >= capacity) return OwnerIpcAdmission.QueueFull
        entries[key] = Entry(
            fingerprint = fingerprint,
            deadline = deadline,
            state = State.Queued,
            retainedUntilElapsedRealtimeMillis = addSaturating(
                deadline.elapsedRealtimeMillis,
                terminalRetentionMillis,
            ),
        )
        return OwnerIpcAdmission.Admitted
    }

    @Synchronized
    fun begin(key: OwnerIpcRequestKey, nowElapsedRealtimeMillis: Long): OwnerIpcExecutionStart<T> = when (val entry = entries[key]) {
        null -> OwnerIpcExecutionStart.Unknown

        else -> when (entry.state) {
            State.Queued -> when (entry.deadline.remainingAt(nowElapsedRealtimeMillis)) {
                VpnDeadlineState.Expired -> OwnerIpcExecutionStart.Expired

                is VpnDeadlineState.Remaining -> {
                    entries[key] = entry.copy(state = State.Executing)
                    OwnerIpcExecutionStart.Started
                }
            }

            State.Canceled -> OwnerIpcExecutionStart.Canceled

            State.Executing -> OwnerIpcExecutionStart.InFlight

            is State.Terminal -> OwnerIpcExecutionStart.TerminalReadback(entry.state.response)
        }
    }

    @Synchronized
    fun cancel(key: OwnerIpcRequestKey): OwnerIpcCancellation<T> = when (val entry = entries[key]) {
        null -> OwnerIpcCancellation.Unknown

        else -> when (val state = entry.state) {
            State.Queued -> {
                entries[key] = entry.copy(state = State.Canceled)
                OwnerIpcCancellation.CanceledBeforeExecution
            }

            State.Canceled -> OwnerIpcCancellation.AlreadyCanceled

            State.Executing -> OwnerIpcCancellation.ExecutionAlreadyStarted

            is State.Terminal -> OwnerIpcCancellation.TerminalReadback(state.response)
        }
    }

    @Synchronized
    fun complete(key: OwnerIpcRequestKey, response: T, nowElapsedRealtimeMillis: Long): OwnerIpcCompletion =
        when (val entry = entries[key]) {
            null -> OwnerIpcCompletion.Unknown

            else -> when (entry.state) {
                State.Canceled -> OwnerIpcCompletion.SuppressCanceledCompletion

                State.Queued,
                State.Executing,
                -> {
                    entries[key] = entry.copy(
                        state = State.Terminal(response),
                        retainedUntilElapsedRealtimeMillis = addSaturating(
                            nowElapsedRealtimeMillis,
                            terminalRetentionMillis,
                        ),
                    )
                    OwnerIpcCompletion.TerminalCached
                }

                is State.Terminal -> OwnerIpcCompletion.Unknown
            }
        }

    @Synchronized
    fun retainedRequestCount(nowElapsedRealtimeMillis: Long): Int {
        prune(nowElapsedRealtimeMillis)
        return entries.size
    }

    @Synchronized
    fun pendingRequestCount(): Int = entries.values.count {
        it.state == State.Queued || it.state == State.Executing
    }

    @Synchronized
    fun activeKeys(): List<OwnerIpcRequestKey> = entries
        .filterValues { it.state == State.Queued || it.state == State.Executing }
        .keys
        .toList()

    private fun prune(nowElapsedRealtimeMillis: Long) {
        entries.entries.removeAll { (_, entry) ->
            entry.state != State.Executing &&
                nowElapsedRealtimeMillis >= entry.retainedUntilElapsedRealtimeMillis
        }
    }

    private fun addSaturating(value: Long, increment: Long): Long =
        if (Long.MAX_VALUE - value < increment) Long.MAX_VALUE else value + increment

    private companion object {
        const val DEFAULT_CAPACITY = 128
        const val DEFAULT_TERMINAL_RETENTION_MILLIS = 60_000L
    }
}
