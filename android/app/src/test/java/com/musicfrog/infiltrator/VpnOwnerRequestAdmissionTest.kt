package com.musicfrog.infiltrator

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class VpnOwnerRequestAdmissionTest {
    @Test
    fun oneAbsoluteDeadlineCoversQueueAdmissionAndExecution() {
        val requests = VpnOwnerStartRequestAdmission()
        val key = VpnOwnerRequestKey("client-session", 17L)
        val deadline = VpnOwnerRequestDeadline.after(1_000L, 200L)

        assertEquals(
            VpnOwnerStartAdmission.Admitted,
            requests.admit(key, deadline, nowElapsedRealtimeMillis = 1_000L),
        )
        assertEquals(
            VpnOwnerStartExecution.Expired,
            requests.begin(key, nowElapsedRealtimeMillis = 1_200L),
        )
        assertEquals(VpnOwnerStartCompletion.Publish, requests.complete(key))
        assertEquals(VpnOwnerStartAcknowledgment.Acknowledged, requests.acknowledgeTerminal(key))
        assertEquals(0, requests.retainedRequestCount())
    }

    @Test
    fun saturatedOwnerQueueReturnsTypedResultWithoutRetainingExtraRequests() {
        val requests = VpnOwnerStartRequestAdmission()
        val deadline = VpnOwnerRequestDeadline.after(1_000L, 5_000L)

        assertEquals(
            VpnOwnerStartAdmission.Admitted,
            requests.admit(VpnOwnerRequestKey("client-session", 1L), deadline, 1_000L),
        )
        assertEquals(
            VpnOwnerStartAdmission.Admitted,
            requests.admit(VpnOwnerRequestKey("client-session", 2L), deadline, 1_000L),
        )
        assertEquals(
            VpnOwnerStartAdmission.QueueFull,
            requests.admit(VpnOwnerRequestKey("client-session", 3L), deadline, 1_000L),
        )
        assertEquals(2, requests.retainedRequestCount())
    }

    @Test
    fun cancellationBeforeOwnerExecutionSuppressesStartAndLateReceipt() {
        val requests = VpnOwnerStartRequestAdmission()
        val key = VpnOwnerRequestKey("client-session", 31L)
        val deadline = VpnOwnerRequestDeadline.after(1_000L, 5_000L)
        assertEquals(VpnOwnerStartAdmission.Admitted, requests.admit(key, deadline, 1_000L))

        assertEquals(VpnOwnerStartCancellation.CanceledBeforeGeneration, requests.cancel(key))
        assertEquals(VpnOwnerStartExecution.Canceled, requests.begin(key, 1_001L))
        assertEquals(VpnOwnerStartCompletion.SuppressLateReceipt, requests.complete(key))
        assertEquals(0, requests.retainedRequestCount())
    }

    @Test
    fun generationBoundCancellationIsIdempotentAndFencesLateStartReceipt() {
        val requests = VpnOwnerStartRequestAdmission()
        val key = VpnOwnerRequestKey("client-session", 32L)
        val deadline = VpnOwnerRequestDeadline.after(1_000L, 5_000L)
        assertEquals(VpnOwnerStartAdmission.Admitted, requests.admit(key, deadline, 1_000L))
        assertEquals(VpnOwnerStartExecution.Started(deadline), requests.begin(key, 1_001L))
        assertEquals(VpnOwnerStartGenerationBinding.Bound, requests.associateGeneration(key, 9L))
        assertEquals(
            VpnOwnerStartDispatchOutcome.Unknown,
            requests.markServiceDispatchPending(key, 8L),
        )

        assertEquals(
            VpnOwnerStartCancellation.CancelGeneration(9L),
            requests.cancel(key),
        )
        assertEquals(
            VpnOwnerStartDispatchOutcome.Canceled,
            requests.markServiceDispatchPending(key, 9L),
        )
        assertEquals(
            VpnOwnerStartDispatchOutcome.Canceled,
            requests.markServiceDispatched(key, 9L),
        )
        assertEquals(VpnOwnerStartCancellation.AlreadyCanceled, requests.cancel(key))
        assertEquals(VpnOwnerStartCompletion.SuppressLateReceipt, requests.complete(key))
        assertEquals(VpnOwnerStartCancellation.Unknown, requests.cancel(key))
    }

    @Test
    fun acknowledgedTerminalReplyReleasesRequestAndLateCancelCannotStopIt() {
        val requests = VpnOwnerStartRequestAdmission()
        val key = VpnOwnerRequestKey("client-session", 37L)
        val deadline = VpnOwnerRequestDeadline.after(1_000L, 5_000L)
        assertEquals(VpnOwnerStartAdmission.Admitted, requests.admit(key, deadline, 1_000L))
        assertEquals(VpnOwnerStartExecution.Started(deadline), requests.begin(key, 1_001L))
        assertEquals(VpnOwnerStartGenerationBinding.Bound, requests.associateGeneration(key, 14L))
        assertEquals(VpnOwnerStartDispatchOutcome.Admitted, requests.markServiceDispatchPending(key, 14L))
        assertEquals(VpnOwnerStartDispatchOutcome.ExecutionStarted, requests.markServiceDispatchStarted(key, 14L))
        assertEquals(VpnOwnerStartDispatchOutcome.Dispatched, requests.markServiceDispatched(key, 14L))
        assertEquals(VpnOwnerStartCompletion.Publish, requests.complete(key))

        assertEquals(VpnOwnerStartAcknowledgment.Acknowledged, requests.acknowledgeTerminal(key))
        assertEquals(VpnOwnerStartCancellation.Unknown, requests.cancel(key))
        assertEquals(0, requests.retainedRequestCount())
    }

    @Test
    fun callerCancellationAfterAcceptedTerminalIsReadbackOnly() {
        val requests = VpnOwnerStartRequestAdmission()
        val key = VpnOwnerRequestKey("client-session", 38L)
        val deadline = VpnOwnerRequestDeadline.after(1_000L, 200L)
        assertEquals(VpnOwnerStartAdmission.Admitted, requests.admit(key, deadline, 1_000L))
        assertEquals(VpnOwnerStartExecution.Started(deadline), requests.begin(key, 1_001L))
        assertEquals(VpnOwnerStartGenerationBinding.Bound, requests.associateGeneration(key, 15L))
        assertEquals(VpnOwnerStartDispatchOutcome.Admitted, requests.markServiceDispatchPending(key, 15L))
        assertEquals(VpnOwnerStartDispatchOutcome.ExecutionStarted, requests.markServiceDispatchStarted(key, 15L))
        assertEquals(VpnOwnerStartDispatchOutcome.Dispatched, requests.markServiceDispatched(key, 15L))
        assertEquals(VpnOwnerStartCompletion.Publish, requests.complete(key))

        assertEquals(
            VpnOwnerStartCancellation.ExecutionAlreadyStarted,
            requests.cancel(key),
        )
        assertEquals(1, requests.retainedRequestCount())
        assertEquals(VpnOwnerStartCompletion.Unknown, requests.complete(key))
        assertEquals(VpnOwnerStartAcknowledgment.Acknowledged, requests.acknowledgeTerminal(key))
        assertEquals(0, requests.retainedRequestCount())
    }

    @Test
    fun cancelAfterNoGenerationTerminalReadbackDoesNotScheduleVpnCleanup() {
        val requests = VpnOwnerStartRequestAdmission()
        val key = VpnOwnerRequestKey("client-session", 40L)
        val deadline = VpnOwnerRequestDeadline.after(1_000L, 200L)
        assertEquals(VpnOwnerStartAdmission.Admitted, requests.admit(key, deadline, 1_000L))
        assertEquals(VpnOwnerStartExecution.Started(deadline), requests.begin(key, 1_001L))
        assertEquals(VpnOwnerStartCompletion.Publish, requests.complete(key))

        assertEquals(VpnOwnerStartCancellation.TerminalReadback, requests.cancel(key))
        assertEquals(1, requests.retainedRequestCount())
        assertEquals(VpnOwnerStartAcknowledgment.Acknowledged, requests.acknowledgeTerminal(key))
        assertEquals(0, requests.retainedRequestCount())
    }

    @Test
    fun dispatchCommitAndAcceptedStartAreNotRolledBackByCallerCancellation() {
        val requests = VpnOwnerStartRequestAdmission()
        val key = VpnOwnerRequestKey("client-session", 41L)
        val deadline = VpnOwnerRequestDeadline.after(1_000L, 5_000L)
        assertEquals(VpnOwnerStartAdmission.Admitted, requests.admit(key, deadline, 1_000L))
        assertEquals(VpnOwnerStartExecution.Started(deadline), requests.begin(key, 1_001L))
        assertEquals(VpnOwnerStartGenerationBinding.Bound, requests.associateGeneration(key, 21L))
        assertEquals(VpnOwnerStartDispatchOutcome.Admitted, requests.markServiceDispatchPending(key, 21L))
        assertEquals(
            VpnOwnerStartDispatchOutcome.ExecutionStarted,
            requests.markServiceDispatchStarted(key, 21L),
        )

        assertEquals(VpnOwnerStartCancellation.ExecutionAlreadyStarted, requests.cancel(key))
        assertEquals(1, requests.retainedRequestCount())
        assertEquals(VpnOwnerStartDispatchOutcome.Dispatched, requests.markServiceDispatched(key, 21L))
        assertEquals(VpnOwnerStartCompletion.Publish, requests.complete(key))
        assertEquals(VpnOwnerStartCancellation.ExecutionAlreadyStarted, requests.cancel(key))
        assertEquals(VpnOwnerStartAdmission.Duplicate, requests.admit(key, deadline, 1_002L))
        assertEquals(VpnOwnerStartAcknowledgment.Acknowledged, requests.acknowledgeTerminal(key))
        assertEquals(0, requests.retainedRequestCount())
    }

    @Test
    fun cancelDuringDispatchPendingFencesLateServiceStart() {
        val requests = VpnOwnerStartRequestAdmission()
        val key = VpnOwnerRequestKey("client-session", 42L)
        val deadline = VpnOwnerRequestDeadline.after(1_000L, 5_000L)
        assertEquals(VpnOwnerStartAdmission.Admitted, requests.admit(key, deadline, 1_000L))
        assertEquals(VpnOwnerStartExecution.Started(deadline), requests.begin(key, 1_001L))
        assertEquals(VpnOwnerStartGenerationBinding.Bound, requests.associateGeneration(key, 22L))
        assertEquals(VpnOwnerStartDispatchOutcome.Admitted, requests.markServiceDispatchPending(key, 22L))

        assertEquals(VpnOwnerStartCancellation.CancelGeneration(22L), requests.cancel(key))
        assertEquals(VpnOwnerStartDispatchOutcome.Canceled, requests.markServiceDispatchStarted(key, 22L))
        assertEquals(VpnOwnerStartDispatchOutcome.Canceled, requests.markServiceDispatched(key, 22L))
        assertEquals(VpnOwnerStartCompletion.SuppressLateReceipt, requests.complete(key))
    }

    @Test
    fun joiningAnExistingStartIsAlreadyExecutingAndCannotBeCanceledAsItsOwner() {
        val requests = VpnOwnerStartRequestAdmission()
        val key = VpnOwnerRequestKey("client-session", 43L)
        val deadline = VpnOwnerRequestDeadline.after(1_000L, 5_000L)
        assertEquals(VpnOwnerStartAdmission.Admitted, requests.admit(key, deadline, 1_000L))
        assertEquals(VpnOwnerStartExecution.Started(deadline), requests.begin(key, 1_001L))

        assertEquals(
            VpnOwnerStartGenerationBinding.ExecutionAlreadyStarted,
            requests.associateAlreadyExecutingGeneration(key, 23L),
        )
        assertEquals(VpnOwnerStartCancellation.ExecutionAlreadyStarted, requests.cancel(key))
        assertEquals(VpnOwnerStartCompletion.Publish, requests.complete(key))
        assertEquals(VpnOwnerStartCancellation.ExecutionAlreadyStarted, requests.cancel(key))
        assertEquals(VpnOwnerStartAcknowledgment.Acknowledged, requests.acknowledgeTerminal(key))
    }

    @Test
    fun unacknowledgedTerminalExpiryReleasesReplayWithoutStoppingAcceptedGeneration() {
        val requests = VpnOwnerStartRequestAdmission()
        val key = VpnOwnerRequestKey("client-session", 39L)
        val deadline = VpnOwnerRequestDeadline.after(1_000L, 200L)
        assertEquals(VpnOwnerStartAdmission.Admitted, requests.admit(key, deadline, 1_000L))
        assertEquals(VpnOwnerStartExecution.Started(deadline), requests.begin(key, 1_001L))
        assertEquals(VpnOwnerStartGenerationBinding.Bound, requests.associateGeneration(key, 16L))
        assertEquals(VpnOwnerStartDispatchOutcome.Admitted, requests.markServiceDispatchPending(key, 16L))
        assertEquals(VpnOwnerStartDispatchOutcome.ExecutionStarted, requests.markServiceDispatchStarted(key, 16L))
        assertEquals(VpnOwnerStartDispatchOutcome.Dispatched, requests.markServiceDispatched(key, 16L))
        assertEquals(VpnOwnerStartCompletion.Publish, requests.complete(key))

        assertEquals(
            VpnOwnerStartExpiry.NotExpired,
            requests.expireTerminal(key, nowElapsedRealtimeMillis = 1_199L),
        )
        assertEquals(
            VpnOwnerStartExpiry.ExpiredTerminal,
            requests.expireTerminal(key, nowElapsedRealtimeMillis = 1_200L),
        )
        assertEquals(0, requests.retainedRequestCount())
    }

    @Test
    fun cancellationDistinguishesGenerationAdmissionFromDispatchedStart() {
        val deadline = VpnOwnerRequestDeadline.after(1_000L, 5_000L)
        val beforeDispatch = VpnOwnerStartRequestAdmission()
        val firstKey = VpnOwnerRequestKey("client-session", 34L)
        assertEquals(VpnOwnerStartAdmission.Admitted, beforeDispatch.admit(firstKey, deadline, 1_000L))
        assertEquals(VpnOwnerStartExecution.Started(deadline), beforeDispatch.begin(firstKey, 1_001L))
        assertEquals(VpnOwnerStartGenerationBinding.Bound, beforeDispatch.associateGeneration(firstKey, 11L))
        assertEquals(
            VpnOwnerStartCancellation.CancelGeneration(11L),
            beforeDispatch.cancel(firstKey),
        )

        val afterDispatch = VpnOwnerStartRequestAdmission()
        val secondKey = VpnOwnerRequestKey("client-session", 35L)
        assertEquals(VpnOwnerStartAdmission.Admitted, afterDispatch.admit(secondKey, deadline, 1_000L))
        assertEquals(VpnOwnerStartExecution.Started(deadline), afterDispatch.begin(secondKey, 1_001L))
        assertEquals(VpnOwnerStartGenerationBinding.Bound, afterDispatch.associateGeneration(secondKey, 12L))
        assertEquals(
            VpnOwnerStartDispatchOutcome.Admitted,
            afterDispatch.markServiceDispatchPending(secondKey, 12L),
        )
        assertEquals(
            VpnOwnerStartDispatchOutcome.ExecutionStarted,
            afterDispatch.markServiceDispatchStarted(secondKey, 12L),
        )
        assertEquals(
            VpnOwnerStartDispatchOutcome.Dispatched,
            afterDispatch.markServiceDispatched(secondKey, 12L),
        )
        assertEquals(
            VpnOwnerStartCancellation.ExecutionAlreadyStarted,
            afterDispatch.cancel(secondKey),
        )
        assertEquals(1, afterDispatch.retainedRequestCount())

        val pending = VpnOwnerStartRequestAdmission()
        val pendingKey = VpnOwnerRequestKey("client-session", 36L)
        assertEquals(VpnOwnerStartAdmission.Admitted, pending.admit(pendingKey, deadline, 1_000L))
        assertEquals(VpnOwnerStartExecution.Started(deadline), pending.begin(pendingKey, 1_001L))
        assertEquals(VpnOwnerStartGenerationBinding.Bound, pending.associateGeneration(pendingKey, 13L))
        assertEquals(
            VpnOwnerStartDispatchOutcome.Admitted,
            pending.markServiceDispatchPending(pendingKey, 13L),
        )
        assertEquals(
            VpnOwnerStartCancellation.CancelGeneration(13L),
            pending.cancel(pendingKey),
        )
    }

    @Test
    fun requestIdentityIncludesClientSessionAndCannotCancelAnotherClient() {
        val requests = VpnOwnerStartRequestAdmission()
        val deadline = VpnOwnerRequestDeadline.after(1_000L, 5_000L)
        val first = VpnOwnerRequestKey("client-a", 1L)
        val second = VpnOwnerRequestKey("client-b", 1L)
        assertEquals(VpnOwnerStartAdmission.Admitted, requests.admit(first, deadline, 1_000L))
        assertEquals(VpnOwnerStartAdmission.Admitted, requests.admit(second, deadline, 1_000L))

        assertEquals(VpnOwnerStartCancellation.CanceledBeforeGeneration, requests.cancel(first))
        assertEquals(VpnOwnerStartExecution.Started(deadline), requests.begin(second, 1_001L))
        assertEquals(VpnOwnerStartCompletion.SuppressLateReceipt, requests.complete(first))
        assertEquals(VpnOwnerStartCompletion.Publish, requests.complete(second))
        assertEquals(VpnOwnerStartAcknowledgment.Acknowledged, requests.acknowledgeTerminal(second))
    }

    @Test
    fun cancellationRacingGenerationAssociationStillReturnsGenerationForCleanup() {
        val requests = VpnOwnerStartRequestAdmission()
        val key = VpnOwnerRequestKey("client-session", 33L)
        val deadline = VpnOwnerRequestDeadline.after(1_000L, 5_000L)
        assertEquals(VpnOwnerStartAdmission.Admitted, requests.admit(key, deadline, 1_000L))
        assertEquals(VpnOwnerStartExecution.Started(deadline), requests.begin(key, 1_001L))
        assertEquals(VpnOwnerStartCancellation.CanceledBeforeGeneration, requests.cancel(key))

        assertEquals(VpnOwnerStartGenerationBinding.Canceled, requests.associateGeneration(key, 10L))
        assertEquals(VpnOwnerStartCancellation.AlreadyCanceled, requests.cancel(key))
        assertEquals(VpnOwnerStartCompletion.SuppressLateReceipt, requests.complete(key))
    }

    @Test
    fun serviceQueueHasOnePendingSlotAndCleanupReplacesQueuedStart() {
        val admission = VpnServiceExecutorAdmission()
        val start = VpnCleanupRequest(
            generation = 5L,
            resourceGeneration = 4L,
            intent = VpnCleanupIntent.Stop,
        )

        assertEquals(VpnServiceTaskAdmission.Accepted, admission.admitStart(4L))
        assertEquals(VpnServiceTaskAdmission.Accepted, admission.beginStart(4L))
        assertEquals(VpnServiceTaskAdmission.Accepted, admission.admitStart(5L))
        assertEquals(VpnServiceTaskAdmission.QueueFull, admission.admitStart(6L))
        assertEquals(VpnServiceTaskAdmission.ReplacedQueuedStart, admission.admitCleanup(start))
        assertEquals(VpnServiceTaskAdmission.StaleGeneration, admission.beginStart(5L))
        assertEquals(VpnServiceTaskAdmission.Accepted, admission.beginCleanup(start))
        admission.finishStart(4L)
        admission.finishCleanup(start)
        assertEquals(VpnServiceTaskSlot.Empty, admission.pendingTask())
    }

    @Test
    fun ordinaryStartAndCleanupCompletionKeepServiceAdmissionOpen() {
        val admission = VpnServiceExecutorAdmission()
        assertEquals(VpnServiceTaskAdmission.Accepted, admission.admitStart(20L))
        assertEquals(VpnServiceTaskAdmission.Accepted, admission.beginStart(20L))
        admission.finishStart(20L)
        assertEquals(VpnServiceAdmissionPhase.Accepting, admission.finishDestroy())

        val cleanup = VpnCleanupRequest(
            generation = 22L,
            resourceGeneration = 21L,
            intent = VpnCleanupIntent.Stop,
        )
        assertEquals(VpnServiceTaskAdmission.Accepted, admission.admitCleanup(cleanup))
        assertEquals(VpnServiceTaskAdmission.Accepted, admission.beginCleanup(cleanup))
        admission.finishCleanup(cleanup)
        assertEquals(VpnServiceAdmissionPhase.Accepting, admission.finishDestroy())
        assertEquals(VpnServiceTaskAdmission.Accepted, admission.admitStart(23L))
    }

    @Test
    fun serviceDestroyRejectsNewStartsButRetainsAndDrainsCleanup() {
        val admission = VpnServiceExecutorAdmission()
        val cleanup = VpnCleanupRequest(
            generation = 8L,
            resourceGeneration = 7L,
            intent = VpnCleanupIntent.Revoke,
        )
        assertEquals(VpnServiceAdmissionPhase.Destroying, admission.beginDestroy())
        assertEquals(VpnServiceTaskAdmission.Destroying, admission.admitStart(8L))
        assertEquals(VpnServiceTaskAdmission.Accepted, admission.admitCleanup(cleanup))
        assertEquals(VpnServiceAdmissionPhase.Destroying, admission.finishDestroy())
        assertEquals(VpnServiceTaskAdmission.Accepted, admission.beginCleanup(cleanup))
        admission.finishCleanup(cleanup)
        assertEquals(VpnServiceAdmissionPhase.Destroyed, admission.finishDestroy())
        assertEquals(VpnServiceTaskAdmission.Destroyed, admission.admitCleanup(cleanup))
    }

    @Test
    fun cleanupJoinDoesNotConsumeAnotherQueueSlot() {
        val admission = VpnServiceExecutorAdmission()
        val cleanup = VpnCleanupRequest(
            generation = 12L,
            resourceGeneration = 11L,
            intent = VpnCleanupIntent.Stop,
        )
        assertEquals(VpnServiceTaskAdmission.Accepted, admission.admitCleanup(cleanup))
        assertEquals(VpnServiceTaskAdmission.Joined, admission.admitCleanup(cleanup.copy(intent = VpnCleanupIntent.Revoke)))
        assertTrue(admission.pendingTask() is VpnServiceTaskSlot.Cleanup)
        assertFalse(admission.pendingTask() is VpnServiceTaskSlot.Start)
    }
}
