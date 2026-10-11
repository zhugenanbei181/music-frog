package com.musicfrog.infiltrator.ui.overview

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class RoutingIntentObservationFenceTest {
    @Test
    fun readStartedBeforeSuccessfulMutationCannotReplaceItsReadback() {
        val fence = RoutingIntentObservationFence()
        val pollingRead = fence.beginRead()

        fence.beginMutation()
        fence.finishMutation()

        assertFalse(fence.accepts(pollingRead))
        assertTrue(fence.accepts(fence.beginRead()))
    }

    @Test
    fun newerReadInvalidatesAnOlderDelayedRead() {
        val fence = RoutingIntentObservationFence()
        val delayed = fence.beginRead()
        val fresh = fence.beginRead()

        assertFalse(fence.accepts(delayed))
        assertTrue(fence.accepts(fresh))
    }

    @Test
    fun readDuringMutationIsInvalidatedWhenMutationSettles() {
        val fence = RoutingIntentObservationFence()
        fence.beginMutation()
        val duringMutation = fence.beginRead()

        fence.finishMutation()

        assertFalse(fence.accepts(duringMutation))
    }
}
