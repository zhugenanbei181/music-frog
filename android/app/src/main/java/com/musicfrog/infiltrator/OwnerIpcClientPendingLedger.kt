package com.musicfrog.infiltrator

internal sealed interface OwnerIpcPendingAdmission {
    data object Admitted : OwnerIpcPendingAdmission

    data object DuplicateRequestId : OwnerIpcPendingAdmission

    data object CapacityExceeded : OwnerIpcPendingAdmission
}

internal enum class OwnerIpcPendingDispatch {
    WaitingForBinding,
    Sent,
    CancellationSent,
}

internal sealed interface OwnerIpcPendingCancellation {
    data object CanceledBeforeDispatch : OwnerIpcPendingCancellation

    data object CancelSentRequest : OwnerIpcPendingCancellation

    data object AlreadyCanceled : OwnerIpcPendingCancellation

    data object Unknown : OwnerIpcPendingCancellation
}

internal data class OwnerIpcPendingFailure(
    val requestId: Long,
    val terminal: OwnerClientTerminal,
)

/** Pure request ledger shared by the Binder client and JVM fake transport tests. */
internal class OwnerIpcClientPendingLedger(
    private val capacity: Int = DEFAULT_CAPACITY,
) {
    private val requests = linkedMapOf<Long, OwnerIpcPendingDispatch>()

    init {
        require(capacity > 0)
    }

    @Synchronized
    fun admit(requestId: Long): OwnerIpcPendingAdmission = when {
        requests.containsKey(requestId) -> OwnerIpcPendingAdmission.DuplicateRequestId
        requests.size >= capacity -> OwnerIpcPendingAdmission.CapacityExceeded
        else -> {
            requests[requestId] = OwnerIpcPendingDispatch.WaitingForBinding
            OwnerIpcPendingAdmission.Admitted
        }
    }

    @Synchronized
    fun markSent(requestId: Long): Boolean = when (requests[requestId]) {
        OwnerIpcPendingDispatch.WaitingForBinding -> {
            requests[requestId] = OwnerIpcPendingDispatch.Sent
            true
        }
        OwnerIpcPendingDispatch.Sent,
        OwnerIpcPendingDispatch.CancellationSent,
        null -> false
    }

    @Synchronized
    fun cancel(requestId: Long): OwnerIpcPendingCancellation = when (requests[requestId]) {
        OwnerIpcPendingDispatch.WaitingForBinding -> {
            requests.remove(requestId)
            OwnerIpcPendingCancellation.CanceledBeforeDispatch
        }
        OwnerIpcPendingDispatch.Sent -> {
            requests[requestId] = OwnerIpcPendingDispatch.CancellationSent
            OwnerIpcPendingCancellation.CancelSentRequest
        }
        OwnerIpcPendingDispatch.CancellationSent -> OwnerIpcPendingCancellation.AlreadyCanceled
        null -> OwnerIpcPendingCancellation.Unknown
    }

    @Synchronized
    fun complete(requestId: Long): Boolean = requests.remove(requestId) != null

    @Synchronized
    fun disconnect(terminal: OwnerClientTerminal): List<OwnerIpcPendingFailure> {
        val failures = requests.keys.map { OwnerIpcPendingFailure(it, terminal) }
        requests.clear()
        return failures
    }

    @Synchronized
    fun pendingCount(): Int = requests.size

    private companion object {
        const val DEFAULT_CAPACITY = 32
    }
}
