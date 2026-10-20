package com.musicfrog.infiltrator

/** Resources acquired by one NativeHostAdapter generation. */
internal enum class NativeHostAttachmentResource {
    ActivityLifecycleCallbacks,
    WindowInsetsListener,
    RustBridgeRegistration,
}

/** Ordered attach operations. A resource is recorded before its operation runs. */
internal enum class NativeHostAttachStep(val resource: NativeHostAttachmentResource?) {
    RegisterLifecycleCallbacks(NativeHostAttachmentResource.ActivityLifecycleCallbacks),
    InstallInsetsListener(NativeHostAttachmentResource.WindowInsetsListener),
    RegisterRustBridge(NativeHostAttachmentResource.RustBridgeRegistration),
    PublishInitialLifecycle(null),
    RequestInitialInsets(null),
}

internal sealed interface NativeHostFailureLocation {
    data class AttachStep(val step: NativeHostAttachStep) : NativeHostFailureLocation
    data class DetachResource(val resource: NativeHostAttachmentResource) : NativeHostFailureLocation
}

internal data class NativeHostOperationFailure(val location: NativeHostFailureLocation, val detail: String)

/** A superseded resource belongs to a newer Activity generation; stale cleanup is complete. */
internal sealed interface NativeHostOperationResult {
    data object Applied : NativeHostOperationResult
    data object Superseded : NativeHostOperationResult
    data class Failed(val detail: String) : NativeHostOperationResult
    data class Unknown(val detail: String) : NativeHostOperationResult
}

internal sealed interface NativeHostAttachmentState {
    data object Detached : NativeHostAttachmentState
    data class Attaching(
        val generation: Long,
        val resources: List<NativeHostAttachmentResource>,
        val completedSteps: List<NativeHostAttachStep>,
    ) : NativeHostAttachmentState
    data class Attached(val generation: Long, val resources: List<NativeHostAttachmentResource>) : NativeHostAttachmentState
    data class Detaching(val generation: Long, val resources: List<NativeHostAttachmentResource>) : NativeHostAttachmentState
    data class CleanupNeeded(
        val generation: Long,
        val resources: List<NativeHostAttachmentResource>,
        val failures: List<NativeHostOperationFailure>,
    ) : NativeHostAttachmentState
}

internal sealed interface NativeHostAttachOutcome {
    data class Attached(val generation: Long) : NativeHostAttachOutcome
    data class AlreadyAttached(val generation: Long) : NativeHostAttachOutcome
    data class Failed(
        val failure: NativeHostOperationFailure,
        val cleanupFailures: List<NativeHostOperationFailure>,
        val state: NativeHostAttachmentState,
    ) : NativeHostAttachOutcome
    data class Rejected(val state: NativeHostAttachmentState) : NativeHostAttachOutcome
}

internal sealed interface NativeHostDetachOutcome {
    data object AlreadyDetached : NativeHostDetachOutcome
    data class Detached(val generation: Long) : NativeHostDetachOutcome
    data class CleanupNeeded(
        val generation: Long,
        val pendingResources: List<NativeHostAttachmentResource>,
        val failures: List<NativeHostOperationFailure>,
    ) : NativeHostDetachOutcome
    data class Rejected(val state: NativeHostAttachmentState) : NativeHostDetachOutcome
}

/**
 * Injectable Android side effects for the attachment transaction.
 *
 * The coordinator records each resource before calling its install operation,
 * because an Android API can throw after partially changing platform state.
 */
internal interface NativeHostAttachmentEffects {
    fun registerLifecycleCallbacks(): NativeHostOperationResult
    fun installInsetsListener(): NativeHostOperationResult
    fun registerRustBridge(): NativeHostOperationResult
    fun publishInitialLifecycle(): NativeHostOperationResult
    fun requestInitialInsets(): NativeHostOperationResult

    fun unregisterLifecycleCallbacks(): NativeHostOperationResult
    fun removeInsetsListener(): NativeHostOperationResult
    fun clearRustBridge(): NativeHostOperationResult
}

/** Atomic attach with reverse rollback and exhaustive, retryable detach. */
internal class NativeHostAttachment(private val generation: Long, private val effects: NativeHostAttachmentEffects) {
    private var currentState: NativeHostAttachmentState = NativeHostAttachmentState.Detached

    val state: NativeHostAttachmentState
        @Synchronized get() = currentState

    /** Callbacks are accepted only after Rust has received the initial lifecycle fact. */
    @Synchronized
    fun acceptsCallbacks(): Boolean = when (val state = currentState) {
        NativeHostAttachmentState.Detached,
        is NativeHostAttachmentState.Detaching,
        is NativeHostAttachmentState.CleanupNeeded,
        -> false

        is NativeHostAttachmentState.Attaching ->
            NativeHostAttachStep.PublishInitialLifecycle in state.completedSteps

        is NativeHostAttachmentState.Attached -> true
    }

    @Synchronized
    fun attach(): NativeHostAttachOutcome {
        when (val state = currentState) {
            NativeHostAttachmentState.Detached -> Unit

            is NativeHostAttachmentState.Attached ->
                return NativeHostAttachOutcome.AlreadyAttached(state.generation)

            is NativeHostAttachmentState.Attaching,
            is NativeHostAttachmentState.Detaching,
            is NativeHostAttachmentState.CleanupNeeded,
            ->
                return NativeHostAttachOutcome.Rejected(state)
        }

        val resources = mutableListOf<NativeHostAttachmentResource>()
        val completedSteps = mutableListOf<NativeHostAttachStep>()
        currentState = NativeHostAttachmentState.Attaching(
            generation,
            resources.toList(),
            completedSteps.toList(),
        )

        for (step in NativeHostAttachStep.entries) {
            step.resource?.let { resource ->
                if (resource !in resources) resources += resource
                currentState = NativeHostAttachmentState.Attaching(
                    generation,
                    resources.toList(),
                    completedSteps.toList(),
                )
            }
            val result = performAttachStep(step)
            when (result) {
                NativeHostOperationResult.Applied -> Unit

                NativeHostOperationResult.Superseded -> {
                    val failureDetail = "operation was superseded by a newer Activity generation"
                    val cleanupFailures = release(resources)
                    return NativeHostAttachOutcome.Failed(
                        NativeHostOperationFailure(
                            NativeHostFailureLocation.AttachStep(step),
                            failureDetail,
                        ),
                        cleanupFailures,
                        currentState,
                    )
                }

                is NativeHostOperationResult.Failed -> {
                    val cleanupFailures = release(resources)
                    return NativeHostAttachOutcome.Failed(
                        NativeHostOperationFailure(
                            NativeHostFailureLocation.AttachStep(step),
                            result.detail,
                        ),
                        cleanupFailures,
                        currentState,
                    )
                }

                is NativeHostOperationResult.Unknown -> {
                    val cleanupFailures = release(resources)
                    return NativeHostAttachOutcome.Failed(
                        NativeHostOperationFailure(
                            NativeHostFailureLocation.AttachStep(step),
                            result.detail,
                        ),
                        cleanupFailures,
                        currentState,
                    )
                }
            }
            completedSteps += step
            currentState = NativeHostAttachmentState.Attaching(
                generation,
                resources.toList(),
                completedSteps.toList(),
            )
        }

        currentState = NativeHostAttachmentState.Attached(generation, resources.toList())
        return NativeHostAttachOutcome.Attached(generation)
    }

    @Synchronized
    fun detach(): NativeHostDetachOutcome {
        val resources = when (val state = currentState) {
            NativeHostAttachmentState.Detached -> return NativeHostDetachOutcome.AlreadyDetached

            is NativeHostAttachmentState.Attached -> state.resources

            is NativeHostAttachmentState.CleanupNeeded -> state.resources

            is NativeHostAttachmentState.Attaching,
            is NativeHostAttachmentState.Detaching,
            ->
                return NativeHostDetachOutcome.Rejected(state)
        }
        val failures = release(resources)
        return when (val state = currentState) {
            NativeHostAttachmentState.Detached ->
                NativeHostDetachOutcome.Detached(generation)

            is NativeHostAttachmentState.CleanupNeeded ->
                NativeHostDetachOutcome.CleanupNeeded(
                    state.generation,
                    state.resources,
                    failures.ifEmpty { state.failures },
                )

            is NativeHostAttachmentState.Attaching,
            is NativeHostAttachmentState.Attached,
            is NativeHostAttachmentState.Detaching,
            ->
                NativeHostDetachOutcome.Rejected(state)
        }
    }

    private fun performAttachStep(step: NativeHostAttachStep): NativeHostOperationResult = safely {
        when (step) {
            NativeHostAttachStep.RegisterLifecycleCallbacks ->
                effects.registerLifecycleCallbacks()

            NativeHostAttachStep.InstallInsetsListener ->
                effects.installInsetsListener()

            NativeHostAttachStep.RegisterRustBridge -> effects.registerRustBridge()

            NativeHostAttachStep.PublishInitialLifecycle ->
                effects.publishInitialLifecycle()

            NativeHostAttachStep.RequestInitialInsets -> effects.requestInitialInsets()
        }
    }

    private fun release(resources: List<NativeHostAttachmentResource>): List<NativeHostOperationFailure> {
        currentState = NativeHostAttachmentState.Detaching(generation, resources.toList())
        val pending = resources.toMutableList()
        val failures = mutableListOf<NativeHostOperationFailure>()
        for (resource in resources.asReversed()) {
            val result = safely {
                when (resource) {
                    NativeHostAttachmentResource.ActivityLifecycleCallbacks ->
                        effects.unregisterLifecycleCallbacks()

                    NativeHostAttachmentResource.WindowInsetsListener ->
                        effects.removeInsetsListener()

                    NativeHostAttachmentResource.RustBridgeRegistration ->
                        effects.clearRustBridge()
                }
            }
            when (result) {
                NativeHostOperationResult.Applied,
                NativeHostOperationResult.Superseded,
                -> pending.remove(resource)

                is NativeHostOperationResult.Failed -> failures += NativeHostOperationFailure(
                    NativeHostFailureLocation.DetachResource(resource),
                    result.detail,
                )

                is NativeHostOperationResult.Unknown -> failures += NativeHostOperationFailure(
                    NativeHostFailureLocation.DetachResource(resource),
                    result.detail,
                )
            }
            currentState = NativeHostAttachmentState.Detaching(generation, pending.toList())
        }
        currentState = if (pending.isEmpty()) {
            NativeHostAttachmentState.Detached
        } else {
            NativeHostAttachmentState.CleanupNeeded(
                generation,
                pending.toList(),
                failures.toList(),
            )
        }
        return failures
    }

    private inline fun safely(operation: () -> NativeHostOperationResult): NativeHostOperationResult = try {
        operation()
    } catch (error: Exception) {
        NativeHostOperationResult.Failed(
            error.message ?: error::class.simpleName ?: "Android host operation failed",
        )
    }
}

/** Serializes process-global JNI host registration and generation-fences cleanup. */
internal class NativeHostGenerationRegistry {
    private sealed interface Owner {
        data class Registered(val generation: Long) : Owner
        data class Uncertain(val generation: Long, val previousGeneration: Long?) : Owner
    }

    private var owner: Owner? = null

    @Synchronized
    fun register(generation: Long, operation: () -> NativeHostOperationResult): NativeHostOperationResult {
        val previous = owner
        if (previous is Owner.Uncertain) {
            return NativeHostOperationResult.Failed(
                "Rust bridge registration is unresolved for generation ${previous.generation}; " +
                    "new generation $generation cannot safely replace it",
            )
        }
        return try {
            when (val result = operation()) {
                NativeHostOperationResult.Applied -> {
                    owner = Owner.Registered(generation)
                    result
                }

                is NativeHostOperationResult.Unknown -> {
                    owner = Owner.Uncertain(
                        generation,
                        (previous as? Owner.Registered)?.generation,
                    )
                    result
                }

                NativeHostOperationResult.Superseded,
                is NativeHostOperationResult.Failed,
                -> result
            }
        } catch (error: Exception) {
            owner = Owner.Uncertain(
                generation,
                (previous as? Owner.Registered)?.generation,
            )
            NativeHostOperationResult.Unknown(
                error.message ?: error::class.simpleName ?: "Rust bridge registration outcome is unknown",
            )
        }
    }

    @Synchronized
    fun clearIfOwned(generation: Long, operation: () -> NativeHostOperationResult): NativeHostOperationResult {
        when (val current = owner) {
            null -> return NativeHostOperationResult.Superseded

            is Owner.Registered -> if (current.generation != generation) {
                return NativeHostOperationResult.Superseded
            }

            is Owner.Uncertain -> if (current.generation == generation) {
                val previous = current.previousGeneration
                    ?.let { " (previous owner was $it)" }
                    ?: ""
                return NativeHostOperationResult.Failed(
                    "Rust bridge registration outcome is unknown$previous; " +
                        "no generation-scoped clear API exists",
                )
            } else {
                return NativeHostOperationResult.Superseded
            }
        }
        return try {
            when (val result = operation()) {
                NativeHostOperationResult.Applied -> {
                    owner = null
                    result
                }

                NativeHostOperationResult.Superseded,
                is NativeHostOperationResult.Failed,
                is NativeHostOperationResult.Unknown,
                -> result
            }
        } catch (error: Exception) {
            NativeHostOperationResult.Failed(
                error.message ?: error::class.simpleName ?: "Rust bridge cleanup failed",
            )
        }
    }

    @Synchronized
    fun isOwnedBy(generation: Long): Boolean = when (val current = owner) {
        is Owner.Registered -> current.generation == generation
        is Owner.Uncertain -> current.generation == generation
        null -> false
    }

    @Synchronized
    fun isUncertainFor(generation: Long): Boolean = (owner as? Owner.Uncertain)?.generation == generation
}
