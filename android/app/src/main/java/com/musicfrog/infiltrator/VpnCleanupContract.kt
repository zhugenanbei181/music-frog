package com.musicfrog.infiltrator

internal sealed interface VpnCleanupIntent {
    data object Stop : VpnCleanupIntent

    data object Revoke : VpnCleanupIntent

    data class StartFailure(val message: String) : VpnCleanupIntent
}

internal fun VpnCleanupIntent.merge(incoming: VpnCleanupIntent): VpnCleanupIntent = when (this) {
    VpnCleanupIntent.Revoke -> this

    VpnCleanupIntent.Stop -> when (incoming) {
        VpnCleanupIntent.Revoke -> incoming

        VpnCleanupIntent.Stop,
        is VpnCleanupIntent.StartFailure,
        -> this
    }

    is VpnCleanupIntent.StartFailure -> when (incoming) {
        VpnCleanupIntent.Revoke,
        VpnCleanupIntent.Stop,
        -> incoming

        is VpnCleanupIntent.StartFailure -> this
    }
}

internal data class VpnCleanupRequest(val generation: Long, val resourceGeneration: Long, val intent: VpnCleanupIntent) {
    init {
        require(generation > 0L)
        require(resourceGeneration >= 0L)
    }

    fun merge(incoming: VpnCleanupIntent): VpnCleanupRequest = copy(intent = intent.merge(incoming))
}

/** A single owner cleanup transaction, whether it was first dispatched or retried. */
internal sealed interface VpnCleanupPlan {
    data class Dispatch(val request: VpnCleanupRequest) : VpnCleanupPlan

    data class Join(val request: VpnCleanupRequest) : VpnCleanupPlan

    data class Retry(val request: VpnCleanupRequest) : VpnCleanupPlan

    data class AlreadyClean(val generation: Long) : VpnCleanupPlan

    data object Superseded : VpnCleanupPlan
}

internal enum class VpnCleanupResourceKind {
    ExecutorQueue,
    PreparedConfiguration,
    Worker,
    TunnelDescriptor,
    RoutingLease,
    OwnerReadback,
}

internal data class VpnCleanupFailure(val resource: VpnCleanupResourceKind, val message: String)

internal sealed interface VpnCleanupOutcome {
    data object Completed : VpnCleanupOutcome

    data class CleanupPending(val failure: VpnCleanupFailure) : VpnCleanupOutcome
}

internal enum class VpnOwnedResourcesPresence {
    Clear,
    Retained,
}

internal sealed interface VpnPreparedConfigurationGate {
    data object Accepted : VpnPreparedConfigurationGate

    data object CleanupPending : VpnPreparedConfigurationGate

    data object StaleGeneration : VpnPreparedConfigurationGate

    data class UnexpectedState(val state: VpnProcessState) : VpnPreparedConfigurationGate
}

internal data class VpnCleanupOwnerReadback(val generation: Long, val state: VpnProcessState, val resources: VpnOwnedResourcesPresence)

internal sealed interface VpnCleanupPublication {
    data class Published(val state: VpnProcessState) : VpnCleanupPublication

    data class CleanupPending(val failure: VpnCleanupFailure) : VpnCleanupPublication

    data object StaleGeneration : VpnCleanupPublication
}

internal sealed interface VpnOwnedResource<out T> {
    data object Absent : VpnOwnedResource<Nothing>

    data class Held<T>(val generation: Long, val value: T) : VpnOwnedResource<T>

    data class CleanupPending<T>(val generation: Long, val value: T, val failure: VpnCleanupFailure) : VpnOwnedResource<T>
}

internal data class VpnWorkerOwnership(
    val generation: Long,
    val mtu: Int,
    val routeCount: Int,
    val dnsServers: List<String>,
    val ipv6: Boolean,
)

internal data class VpnPreparedOwnership(val preparationId: ULong)

internal data class VpnOwnerResourceRecordSnapshot<InterfaceHandle, WorkerHandle, LeaseHandle, PreparedHandle>(
    val tunnelDescriptor: VpnOwnedResource<InterfaceHandle>,
    val worker: VpnOwnedResource<WorkerHandle>,
    val routingLease: VpnOwnedResource<LeaseHandle>,
    val preparedConfiguration: VpnOwnedResource<PreparedHandle>,
) {
    fun presence(): VpnOwnedResourcesPresence = when {
        tunnelDescriptor is VpnOwnedResource.Absent &&
            worker is VpnOwnedResource.Absent &&
            routingLease is VpnOwnedResource.Absent &&
            preparedConfiguration is VpnOwnedResource.Absent -> VpnOwnedResourcesPresence.Clear

        else -> VpnOwnedResourcesPresence.Retained
    }
}

internal enum class VpnOwnerResourceMutation {
    Recorded,
    Cleared,
    AlreadyAbsent,
    StaleGeneration,
}

/** Holds the actual owner resources so a failed cleanup can be retried without losing handles. */
internal class VpnOwnerResourceRecord<InterfaceHandle, WorkerHandle, LeaseHandle, PreparedHandle> {
    private var tunnelDescriptor: VpnOwnedResource<InterfaceHandle> = VpnOwnedResource.Absent
    private var worker: VpnOwnedResource<WorkerHandle> = VpnOwnedResource.Absent
    private var routingLease: VpnOwnedResource<LeaseHandle> = VpnOwnedResource.Absent
    private var preparedConfiguration: VpnOwnedResource<PreparedHandle> = VpnOwnedResource.Absent

    @Synchronized
    fun snapshot(): VpnOwnerResourceRecordSnapshot<InterfaceHandle, WorkerHandle, LeaseHandle, PreparedHandle> =
        VpnOwnerResourceRecordSnapshot(
            tunnelDescriptor,
            worker,
            routingLease,
            preparedConfiguration,
        )

    @Synchronized
    fun markCleanupPending(generation: Long, failure: VpnCleanupFailure) {
        markPending(generation, failure, tunnelDescriptor) { tunnelDescriptor = it }
        markPending(generation, failure, worker) { worker = it }
        markPending(generation, failure, routingLease) { routingLease = it }
        markPending(generation, failure, preparedConfiguration) { preparedConfiguration = it }
    }

    @Synchronized
    fun recordTunnelDescriptor(generation: Long, descriptor: InterfaceHandle): VpnOwnerResourceMutation =
        record(generation, descriptor, tunnelDescriptor) { tunnelDescriptor = it }

    @Synchronized
    fun tunnelDescriptor(generation: Long): InterfaceHandle? = tunnelDescriptor.valueFor(generation)

    @Synchronized
    fun markTunnelDescriptorPending(generation: Long, failure: VpnCleanupFailure): VpnOwnerResourceMutation =
        markPending(generation, failure, tunnelDescriptor) { tunnelDescriptor = it }

    @Synchronized
    fun clearTunnelDescriptor(generation: Long): VpnOwnerResourceMutation = clear(generation, tunnelDescriptor) { tunnelDescriptor = it }

    @Synchronized
    fun recordWorker(generation: Long, handle: WorkerHandle): VpnOwnerResourceMutation = record(generation, handle, worker) { worker = it }

    @Synchronized
    fun hasWorker(generation: Long): Boolean = worker.valueFor(generation) != null

    @Synchronized
    fun markWorkerPending(generation: Long, failure: VpnCleanupFailure): VpnOwnerResourceMutation =
        markPending(generation, failure, worker) { worker = it }

    @Synchronized
    fun clearWorker(generation: Long): VpnOwnerResourceMutation = clear(generation, worker) { worker = it }

    @Synchronized
    fun routingLeaseState(): LeaseHandle? = when (val current = routingLease) {
        VpnOwnedResource.Absent -> null
        is VpnOwnedResource.Held -> current.value
        is VpnOwnedResource.CleanupPending -> current.value
    }

    @Synchronized
    fun routingLeaseState(generation: Long): LeaseHandle? = routingLease.valueFor(generation)

    @Synchronized
    fun updateRoutingLease(generation: Long, state: LeaseHandle, pendingFailure: VpnCleanupFailure? = null): VpnOwnerResourceMutation =
        when (pendingFailure) {
            null -> record(generation, state, routingLease) { routingLease = it }
            else -> recordPending(generation, state, pendingFailure, routingLease) { routingLease = it }
        }

    @Synchronized
    fun markRoutingLeasePending(generation: Long, failure: VpnCleanupFailure): VpnOwnerResourceMutation =
        markPending(generation, failure, routingLease) { routingLease = it }

    @Synchronized
    fun clearRoutingLease(generation: Long): VpnOwnerResourceMutation = clear(generation, routingLease) { routingLease = it }

    @Synchronized
    fun recordPreparedConfiguration(generation: Long, preparation: PreparedHandle): VpnOwnerResourceMutation = record(
        generation,
        preparation,
        preparedConfiguration,
    ) { preparedConfiguration = it }

    @Synchronized
    fun markPreparedConfigurationPending(generation: Long, failure: VpnCleanupFailure): VpnOwnerResourceMutation = markPending(
        generation,
        failure,
        preparedConfiguration,
    ) { preparedConfiguration = it }

    @Synchronized
    fun clearPreparedConfiguration(generation: Long): VpnOwnerResourceMutation =
        clear(generation, preparedConfiguration) { preparedConfiguration = it }

    private fun <T> record(
        generation: Long,
        value: T,
        current: VpnOwnedResource<T>,
        assign: (VpnOwnedResource<T>) -> Unit,
    ): VpnOwnerResourceMutation = when (current) {
        VpnOwnedResource.Absent -> {
            assign(VpnOwnedResource.Held(generation, value))
            VpnOwnerResourceMutation.Recorded
        }

        is VpnOwnedResource.Held -> when (current.generation) {
            generation -> {
                assign(VpnOwnedResource.Held(generation, value))
                VpnOwnerResourceMutation.Recorded
            }

            else -> VpnOwnerResourceMutation.StaleGeneration
        }

        is VpnOwnedResource.CleanupPending -> when (current.generation) {
            generation -> {
                assign(VpnOwnedResource.Held(generation, value))
                VpnOwnerResourceMutation.Recorded
            }

            else -> VpnOwnerResourceMutation.StaleGeneration
        }
    }

    private fun <T> recordPending(
        generation: Long,
        value: T,
        failure: VpnCleanupFailure,
        current: VpnOwnedResource<T>,
        assign: (VpnOwnedResource<T>) -> Unit,
    ): VpnOwnerResourceMutation = when (current) {
        VpnOwnedResource.Absent -> VpnOwnerResourceMutation.StaleGeneration

        is VpnOwnedResource.Held -> when (current.generation) {
            generation -> {
                assign(VpnOwnedResource.CleanupPending(generation, value, failure))
                VpnOwnerResourceMutation.Recorded
            }

            else -> VpnOwnerResourceMutation.StaleGeneration
        }

        is VpnOwnedResource.CleanupPending -> when (current.generation) {
            generation -> {
                assign(VpnOwnedResource.CleanupPending(generation, value, failure))
                VpnOwnerResourceMutation.Recorded
            }

            else -> VpnOwnerResourceMutation.StaleGeneration
        }
    }

    private fun <T> markPending(
        generation: Long,
        failure: VpnCleanupFailure,
        current: VpnOwnedResource<T>,
        assign: (VpnOwnedResource<T>) -> Unit,
    ): VpnOwnerResourceMutation = when (current) {
        VpnOwnedResource.Absent -> VpnOwnerResourceMutation.AlreadyAbsent

        is VpnOwnedResource.Held -> when (current.generation) {
            generation -> {
                assign(VpnOwnedResource.CleanupPending(generation, current.value, failure))
                VpnOwnerResourceMutation.Recorded
            }

            else -> VpnOwnerResourceMutation.StaleGeneration
        }

        is VpnOwnedResource.CleanupPending -> when (current.generation) {
            generation -> {
                assign(VpnOwnedResource.CleanupPending(generation, current.value, failure))
                VpnOwnerResourceMutation.Recorded
            }

            else -> VpnOwnerResourceMutation.StaleGeneration
        }
    }

    private fun <T> clear(
        generation: Long,
        current: VpnOwnedResource<T>,
        assign: (VpnOwnedResource<T>) -> Unit,
    ): VpnOwnerResourceMutation = when (current) {
        VpnOwnedResource.Absent -> VpnOwnerResourceMutation.AlreadyAbsent

        is VpnOwnedResource.Held -> when (current.generation) {
            generation -> {
                assign(VpnOwnedResource.Absent)
                VpnOwnerResourceMutation.Cleared
            }

            else -> VpnOwnerResourceMutation.StaleGeneration
        }

        is VpnOwnedResource.CleanupPending -> when (current.generation) {
            generation -> {
                assign(VpnOwnedResource.Absent)
                VpnOwnerResourceMutation.Cleared
            }

            else -> VpnOwnerResourceMutation.StaleGeneration
        }
    }
}

private fun <T> VpnOwnedResource<T>.valueFor(generation: Long): T? = when (this) {
    VpnOwnedResource.Absent -> null
    is VpnOwnedResource.Held -> value.takeIf { this.generation == generation }
    is VpnOwnedResource.CleanupPending -> value.takeIf { this.generation == generation }
}
