package com.musicfrog.infiltrator

import org.junit.Assert.assertEquals
import org.junit.Test

class NativeHostLifecycleTest {
    @Test
    fun activityPhaseWireCodesMatchTheRustLifecycleContract() {
        assertEquals(
            listOf(0, 1, 2, 3, 4, 5),
            NativeActivityPhase.values().map(NativeActivityPhase::wireCode),
        )
    }

    @Test
    fun phaseTransitionsAcceptAndroidLifecycleEdgesAndRejectIllegalJumps() {
        assertEquals(
            NativeActivityPhaseTransition.Advanced,
            NativeActivityPhase.Created.transitionTo(NativeActivityPhase.Started),
        )
        assertEquals(
            NativeActivityPhaseTransition.Advanced,
            NativeActivityPhase.Resumed.transitionTo(NativeActivityPhase.Paused),
        )
        assertEquals(
            NativeActivityPhaseTransition.Advanced,
            NativeActivityPhase.Stopped.transitionTo(NativeActivityPhase.Started),
        )
        assertEquals(
            NativeActivityPhaseTransition.Advanced,
            NativeActivityPhase.Paused.transitionTo(NativeActivityPhase.Destroyed),
        )
        assertEquals(
            NativeActivityPhaseTransition.Unchanged,
            NativeActivityPhase.Resumed.transitionTo(NativeActivityPhase.Resumed),
        )
        assertEquals(
            NativeActivityPhaseTransition.Rejected,
            NativeActivityPhase.Created.transitionTo(NativeActivityPhase.Paused),
        )
        assertEquals(
            NativeActivityPhaseTransition.Rejected,
            NativeActivityPhase.Destroyed.transitionTo(NativeActivityPhase.Started),
        )
    }

    @Test
    fun windowFocusWireCodesPreserveUnknownAndBothObservedStates() {
        assertEquals(
            listOf(0, 1, 2),
            NativeWindowFocus.values().map(NativeWindowFocus::wireCode),
        )
    }
}
