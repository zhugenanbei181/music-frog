package com.musicfrog.infiltrator

import org.junit.Assert.assertEquals
import org.junit.Test

class VpnOwnerRequestDeadlineTest {
    @Test
    fun ownerStartWaitUsesTheCallerAbsoluteDeadline() {
        val lifecycle = VpnLifecycleCoordinator()
        val start = lifecycle.requestStart() as VpnStartPlan.Dispatch
        assertEquals(VpnTransitionOutcome.Applied, lifecycle.transition(start.generation, VpnProcessState.Starting))

        val deadline = VpnOwnerRequestDeadline.after(1_000L, 200L)
        assertEquals(
            VpnStartResult.TimedOut,
            lifecycle.awaitStartUntil(start.generation, deadline) { 1_200L },
        )
        assertEquals(VpnProcessState.Starting, lifecycle.snapshot().state)
    }

    @Test
    fun ownerStopWaitUsesTheCallerAbsoluteDeadline() {
        val lifecycle = VpnLifecycleCoordinator()
        lifecycle.requestStart()
        val stop = lifecycle.requestCleanup(VpnCleanupIntent.Stop) as VpnCleanupPlan.Dispatch
        val deadline = VpnOwnerRequestDeadline.after(2_000L, 300L)

        assertEquals(
            VpnStopResult.TimedOut,
            lifecycle.awaitStopUntil(stop.request.generation, deadline) { 2_300L },
        )
        assertEquals(VpnProcessState.Stopping, lifecycle.snapshot().state)
    }
}
