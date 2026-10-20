package com.musicfrog.infiltrator

internal sealed interface FakeNativeHostEffect {
    data class Attach(val step: NativeHostAttachStep) : FakeNativeHostEffect
    data class Detach(val resource: NativeHostAttachmentResource) : FakeNativeHostEffect
}

internal class FakeNativeHostAttachmentEffects : NativeHostAttachmentEffects {
    val effects = mutableListOf<FakeNativeHostEffect>()
    val installedResources = linkedSetOf<NativeHostAttachmentResource>()
    var failingAttachStep: NativeHostAttachStep? = null
    var throwingAttachStep: NativeHostAttachStep? = null
    var rustRegistrationResult: NativeHostOperationResult? = null
    var rustCleanupResult: NativeHostOperationResult? = null
    val throwingDetachResources = mutableSetOf<NativeHostAttachmentResource>()
    val detachFailuresRemaining =
        mutableMapOf<NativeHostAttachmentResource, Int>()

    override fun registerLifecycleCallbacks() = attach(NativeHostAttachStep.RegisterLifecycleCallbacks)

    override fun installInsetsListener() = attach(NativeHostAttachStep.InstallInsetsListener)

    override fun registerRustBridge() = attach(NativeHostAttachStep.RegisterRustBridge, rustRegistrationResult)

    override fun publishInitialLifecycle() = attach(NativeHostAttachStep.PublishInitialLifecycle)

    override fun requestInitialInsets() = attach(NativeHostAttachStep.RequestInitialInsets)

    override fun unregisterLifecycleCallbacks() = detach(NativeHostAttachmentResource.ActivityLifecycleCallbacks)

    override fun removeInsetsListener() = detach(NativeHostAttachmentResource.WindowInsetsListener)

    override fun clearRustBridge() = detach(NativeHostAttachmentResource.RustBridgeRegistration, rustCleanupResult)

    private fun attach(step: NativeHostAttachStep, forcedResult: NativeHostOperationResult? = null): NativeHostOperationResult {
        effects += FakeNativeHostEffect.Attach(step)
        // Simulate an Android API that mutates state and then reports failure.
        step.resource?.let(installedResources::add)
        if (throwingAttachStep == step) {
            throw IllegalStateException("injected exception at $step")
        }
        if (forcedResult != null) return forcedResult
        return if (failingAttachStep == step) {
            NativeHostOperationResult.Failed("injected failure at $step")
        } else {
            NativeHostOperationResult.Applied
        }
    }

    private fun detach(
        resource: NativeHostAttachmentResource,
        forcedResult: NativeHostOperationResult? = null,
    ): NativeHostOperationResult {
        effects += FakeNativeHostEffect.Detach(resource)
        if (forcedResult != null) {
            if (forcedResult == NativeHostOperationResult.Applied ||
                forcedResult == NativeHostOperationResult.Superseded
            ) {
                installedResources.remove(resource)
            }
            return forcedResult
        }
        if (throwingDetachResources.remove(resource)) {
            throw IllegalStateException("injected exception while releasing $resource")
        }
        val remainingFailures = detachFailuresRemaining[resource] ?: 0
        if (remainingFailures > 0) {
            detachFailuresRemaining[resource] = remainingFailures - 1
            return NativeHostOperationResult.Failed("injected cleanup failure for $resource")
        }
        installedResources.remove(resource)
        return NativeHostOperationResult.Applied
    }
}
