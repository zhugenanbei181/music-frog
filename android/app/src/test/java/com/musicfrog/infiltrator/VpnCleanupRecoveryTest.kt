package com.musicfrog.infiltrator

import org.junit.Assert.assertEquals
import org.junit.Test

class VpnCleanupRecoveryTest {
    @Test
    fun workerDescriptorAndLeaseFailuresRetainTheirHandlesAndGeneration() {
        val resources = VpnOwnerResourceRecord<String, String, String, String>()
        val generation = 27L
        assertEquals(VpnOwnerResourceMutation.Recorded, resources.recordWorker(generation, "worker-handle"))
        assertEquals(VpnOwnerResourceMutation.Recorded, resources.recordTunnelDescriptor(generation, "tun-fd"))
        assertEquals(VpnOwnerResourceMutation.Recorded, resources.updateRoutingLease(generation, "routing-lease"))

        val workerFailure = VpnCleanupFailure(VpnCleanupResourceKind.Worker, "worker exit is pending")
        val descriptorFailure = VpnCleanupFailure(VpnCleanupResourceKind.TunnelDescriptor, "fd close is pending")
        val leaseFailure = VpnCleanupFailure(VpnCleanupResourceKind.RoutingLease, "lease release is pending")
        resources.markWorkerPending(generation, workerFailure)
        resources.markTunnelDescriptorPending(generation, descriptorFailure)
        resources.markRoutingLeasePending(generation, leaseFailure)

        val retained = resources.snapshot()
        assertEquals(VpnOwnedResource.CleanupPending(generation, "worker-handle", workerFailure), retained.worker)
        assertEquals(VpnOwnedResource.CleanupPending(generation, "tun-fd", descriptorFailure), retained.tunnelDescriptor)
        assertEquals(VpnOwnedResource.CleanupPending(generation, "routing-lease", leaseFailure), retained.routingLease)
        assertEquals(VpnOwnedResourcesPresence.Retained, retained.presence())

        assertEquals(VpnOwnerResourceMutation.StaleGeneration, resources.clearWorker(generation + 1L))
        assertEquals(retained, resources.snapshot())
        assertEquals(VpnOwnerResourceMutation.Cleared, resources.clearWorker(generation))
        assertEquals(VpnOwnerResourceMutation.Cleared, resources.clearTunnelDescriptor(generation))
        assertEquals(VpnOwnerResourceMutation.Cleared, resources.clearRoutingLease(generation))
        assertEquals(VpnOwnedResourcesPresence.Clear, resources.snapshot().presence())
    }
}
