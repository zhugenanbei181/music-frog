package com.musicfrog.infiltrator

import org.junit.Assert.assertEquals
import org.junit.Test

class VpnProcessStateCleanupTest {
    @Test
    fun workerDescriptorAndLeaseFailuresRetainTheirResourcesUntilAReadbackConfirmedRetry() {
        val lifecycle = VpnLifecycleCoordinator()
        val start = lifecycle.requestStart() as VpnStartPlan.Dispatch
        val resources = VpnOwnerResourceRecord<String, String, String, String>()
        assertEquals(VpnOwnerResourceMutation.Recorded, resources.recordWorker(start.generation, "worker-handle"))
        assertEquals(VpnOwnerResourceMutation.Recorded, resources.recordTunnelDescriptor(start.generation, "tun-fd"))
        assertEquals(VpnOwnerResourceMutation.Recorded, resources.updateRoutingLease(start.generation, "lease"))
        assertEquals(
            VpnOwnerResourceMutation.Recorded,
            resources.recordPreparedConfiguration(start.generation, "prepared-transaction"),
        )
        val request = (
            lifecycle.requestCleanup(
                VpnCleanupIntent.Stop,
                resources = resources.snapshot().presence(),
            ) as VpnCleanupPlan.Dispatch
            ).request

        val failures = listOf(
            Triple(
                VpnCleanupFailure(VpnCleanupResourceKind.Worker, "worker stop failed"),
                { failure: VpnCleanupFailure -> resources.markWorkerPending(start.generation, failure) },
                { resources.clearWorker(start.generation) },
            ),
            Triple(
                VpnCleanupFailure(VpnCleanupResourceKind.TunnelDescriptor, "fd close failed"),
                { failure: VpnCleanupFailure -> resources.markTunnelDescriptorPending(start.generation, failure) },
                { resources.clearTunnelDescriptor(start.generation) },
            ),
            Triple(
                VpnCleanupFailure(VpnCleanupResourceKind.RoutingLease, "lease release failed"),
                { failure: VpnCleanupFailure -> resources.markRoutingLeasePending(start.generation, failure) },
                { resources.clearRoutingLease(start.generation) },
            ),
        )

        failures.forEach { (failure, markPending, clearAfterRetry) ->
            assertEquals(VpnOwnerResourceMutation.Recorded, markPending(failure))
            val retained = resources.snapshot()
            assertEquals(VpnOwnedResourcesPresence.Retained, retained.presence())
            assertEquals(
                VpnCleanupPublication.CleanupPending(failure),
                completeCleanup(
                    lifecycle,
                    request,
                    VpnCleanupOutcome.CleanupPending(failure),
                    retained.presence(),
                ),
            )
            assertEquals(VpnStartPlan.StopInProgress, lifecycle.requestStart(retained.presence()))
            val retry = lifecycle.retryPendingCleanup() as VpnCleanupPlan.Retry
            assertEquals(request.generation, retry.request.generation)
            assertEquals(VpnOwnerResourceMutation.Cleared, clearAfterRetry())
        }

        assertEquals(VpnOwnerResourceMutation.Cleared, resources.clearPreparedConfiguration(start.generation))
        assertEquals(VpnOwnedResourcesPresence.Clear, resources.snapshot().presence())
        assertEquals(
            VpnCleanupPublication.Published(VpnProcessState.Stopped),
            completeCleanup(lifecycle, request, resources = resources.snapshot().presence()),
        )
    }

    @Test
    fun oldCleanupCannotClearOrPublishOverANewerGeneration() {
        val lifecycle = VpnLifecycleCoordinator()
        val oldStart = lifecycle.requestStart() as VpnStartPlan.Dispatch
        val oldRequest = (
            lifecycle.requestCleanup(
                VpnCleanupIntent.Stop,
                resources = VpnOwnedResourcesPresence.Retained,
            ) as VpnCleanupPlan.Dispatch
            ).request
        assertEquals(
            VpnCleanupPublication.Published(VpnProcessState.Stopped),
            completeCleanup(lifecycle, oldRequest),
        )
        val newStart = lifecycle.requestStart() as VpnStartPlan.Dispatch
        val resources = VpnOwnerResourceRecord<String, String, String, String>()
        assertEquals(VpnOwnerResourceMutation.Recorded, resources.recordWorker(newStart.generation, "new-worker"))
        val newRequest = lifecycle.requestCleanup(
            VpnCleanupIntent.Stop,
            resources = resources.snapshot().presence(),
        ) as VpnCleanupPlan.Dispatch

        assertEquals(VpnOwnerResourceMutation.StaleGeneration, resources.clearWorker(oldStart.generation))
        assertEquals(
            VpnOwnedResource.Held(newStart.generation, "new-worker"),
            resources.snapshot().worker,
        )
        assertEquals(
            VpnCleanupPublication.StaleGeneration,
            lifecycle.completeCleanup(
                oldRequest,
                VpnCleanupOutcome.Completed,
                VpnCleanupOwnerReadback(
                    lifecycle.snapshot().generation,
                    VpnProcessState.Stopping,
                    VpnOwnedResourcesPresence.Clear,
                ),
            ),
        )
        assertEquals(newRequest.request.generation, lifecycle.snapshot().generation)
        assertEquals(VpnProcessState.Stopping, lifecycle.snapshot().state)
    }

    private fun completeCleanup(
        lifecycle: VpnLifecycleCoordinator,
        request: VpnCleanupRequest,
        outcome: VpnCleanupOutcome = VpnCleanupOutcome.Completed,
        resources: VpnOwnedResourcesPresence = VpnOwnedResourcesPresence.Clear,
    ): VpnCleanupPublication {
        val snapshot = lifecycle.snapshot()
        return lifecycle.completeCleanup(
            request,
            outcome,
            VpnCleanupOwnerReadback(snapshot.generation, snapshot.state, resources),
        )
    }
}
