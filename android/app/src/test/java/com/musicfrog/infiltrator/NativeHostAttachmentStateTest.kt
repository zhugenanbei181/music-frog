package com.musicfrog.infiltrator

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class NativeHostAttachmentStateTest {
    @Test
    fun everyAttachFailureRollsBackAttemptedResourcesInReverseOrder() {
        for (failedStep in NativeHostAttachStep.entries) {
            val effects = FakeNativeHostAttachmentEffects().apply {
                throwingAttachStep = failedStep
            }
            val attachment = NativeHostAttachment(4, effects)

            val outcome = attachment.attach()

            assertTrue(
                "expected typed failure for $failedStep",
                outcome is NativeHostAttachOutcome.Failed,
            )
            assertEquals(
                NativeHostFailureLocation.AttachStep(failedStep),
                (outcome as NativeHostAttachOutcome.Failed).failure.location,
            )
            val expectedRollback = NativeHostAttachStep.entries
                .takeWhile { it != failedStep }
                .plus(failedStep)
                .mapNotNull(NativeHostAttachStep::resource)
                .distinct()
                .asReversed()
            assertEquals(
                expectedRollback.map { FakeNativeHostEffect.Detach(it) },
                effects.effects.filterIsInstance<FakeNativeHostEffect.Detach>(),
            )
            assertEquals(NativeHostAttachmentState.Detached, attachment.state)
            assertTrue(effects.installedResources.isEmpty())
        }
    }

    @Test
    fun failedRollbackRemainsTypedAndCanBeRetried() {
        val effects = FakeNativeHostAttachmentEffects().apply {
            failingAttachStep = NativeHostAttachStep.PublishInitialLifecycle
            detachFailuresRemaining[NativeHostAttachmentResource.WindowInsetsListener] = 1
        }
        val attachment = NativeHostAttachment(8, effects)

        val attach = attachment.attach() as NativeHostAttachOutcome.Failed

        assertTrue(attach.state is NativeHostAttachmentState.CleanupNeeded)
        assertEquals(
            listOf(NativeHostAttachmentResource.WindowInsetsListener),
            (attach.state as NativeHostAttachmentState.CleanupNeeded).resources,
        )
        assertEquals(
            NativeHostDetachOutcome.Detached(8),
            attachment.detach(),
        )
        assertEquals(NativeHostAttachmentState.Detached, attachment.state)
        assertTrue(effects.installedResources.isEmpty())
    }

    @Test
    fun unknownRustRegistrationRetainsCleanupNeededOwnership() {
        val effects = FakeNativeHostAttachmentEffects().apply {
            rustRegistrationResult =
                NativeHostOperationResult.Unknown("native call lost its completion result")
            rustCleanupResult =
                NativeHostOperationResult.Failed("clear has no generation-scoped API")
        }
        val attachment = NativeHostAttachment(9, effects)

        val result = attachment.attach() as NativeHostAttachOutcome.Failed

        assertTrue(result.state is NativeHostAttachmentState.CleanupNeeded)
        assertEquals(
            listOf(NativeHostAttachmentResource.RustBridgeRegistration),
            (result.state as NativeHostAttachmentState.CleanupNeeded).resources,
        )
        assertTrue(
            effects.installedResources.contains(
                NativeHostAttachmentResource.RustBridgeRegistration,
            ),
        )
        assertTrue(
            attachment.attach() is NativeHostAttachOutcome.Rejected,
        )
    }

    @Test
    fun eachDetachFailureDoesNotPreventOtherResourcesFromBeingReleased() {
        for (failedResource in NativeHostAttachmentResource.entries) {
            val effects = FakeNativeHostAttachmentEffects()
            val attachment = NativeHostAttachment(11, effects)
            assertEquals(NativeHostAttachOutcome.Attached(11), attachment.attach())
            effects.throwingDetachResources += failedResource
            effects.effects.clear()

            val detached = attachment.detach()

            assertTrue(
                "expected cleanup-needed for $failedResource",
                detached is NativeHostDetachOutcome.CleanupNeeded,
            )
            assertEquals(
                listOf(failedResource),
                (detached as NativeHostDetachOutcome.CleanupNeeded).pendingResources,
            )
            assertEquals(
                listOf(
                    NativeHostAttachmentResource.RustBridgeRegistration,
                    NativeHostAttachmentResource.WindowInsetsListener,
                    NativeHostAttachmentResource.ActivityLifecycleCallbacks,
                ).map { FakeNativeHostEffect.Detach(it) },
                effects.effects,
            )
            assertEquals(
                NativeHostDetachOutcome.Detached(11),
                attachment.detach(),
            )
            assertTrue(effects.installedResources.isEmpty())
        }
    }

    @Test
    fun repeatedAttachAndDetachAreIdempotent() {
        val effects = FakeNativeHostAttachmentEffects()
        val attachment = NativeHostAttachment(13, effects)
        assertEquals(NativeHostAttachOutcome.Attached(13), attachment.attach())
        val attachEffectCount = effects.effects.size

        assertEquals(NativeHostAttachOutcome.AlreadyAttached(13), attachment.attach())
        assertEquals(attachEffectCount, effects.effects.size)

        assertEquals(NativeHostDetachOutcome.Detached(13), attachment.detach())
        val detachEffectCount = effects.effects.size
        assertEquals(NativeHostDetachOutcome.AlreadyDetached, attachment.detach())
        assertEquals(detachEffectCount, effects.effects.size)
    }

    @Test
    fun staleGenerationCannotClearNewRustBridgeRegistration() {
        val registry = NativeHostGenerationRegistry()
        val clears = mutableListOf<Long>()
        assertEquals(
            NativeHostOperationResult.Applied,
            registry.register(20) { NativeHostOperationResult.Applied },
        )
        assertEquals(
            NativeHostOperationResult.Applied,
            registry.register(21) { NativeHostOperationResult.Applied },
        )

        assertEquals(
            NativeHostOperationResult.Superseded,
            registry.clearIfOwned(20) {
                clears += 20
                NativeHostOperationResult.Applied
            },
        )
        assertTrue(clears.isEmpty())
        assertTrue(registry.isOwnedBy(21))
        assertEquals(
            NativeHostOperationResult.Applied,
            registry.clearIfOwned(21) {
                clears += 21
                NativeHostOperationResult.Applied
            },
        )
        assertEquals(listOf(21L), clears)
        assertFalse(registry.isOwnedBy(21))
    }

    @Test
    fun uncertainRustRegistrationCannotBeClearedOrSupersededImplicitly() {
        val registry = NativeHostGenerationRegistry()
        var clearWasCalled = false
        var laterRegistrationWasCalled = false

        val registered = registry.register(30) {
            throw IllegalStateException("native registration acknowledgement lost")
        }

        assertTrue(registered is NativeHostOperationResult.Unknown)
        assertTrue(registry.isUncertainFor(30))
        assertEquals(
            NativeHostOperationResult.Failed(
                "Rust bridge registration outcome is unknown; no generation-scoped clear API exists",
            ),
            registry.clearIfOwned(30) {
                clearWasCalled = true
                NativeHostOperationResult.Applied
            },
        )
        assertFalse(clearWasCalled)
        assertEquals(
            NativeHostOperationResult.Failed(
                "Rust bridge registration is unresolved for generation 30; " +
                    "new generation 31 cannot safely replace it",
            ),
            registry.register(31) {
                laterRegistrationWasCalled = true
                NativeHostOperationResult.Applied
            },
        )
        assertFalse(laterRegistrationWasCalled)
    }
}
