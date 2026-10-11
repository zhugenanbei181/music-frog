package com.musicfrog.infiltrator

import org.junit.Assert.assertEquals
import org.junit.Test

class OwnerIpcRequestAdmissionTest {
    @Test
    fun admissionIsBoundedAndDuplicateKeyCannotChangeItsRequest() {
        val admission = OwnerIpcRequestAdmission<String>(capacity = 1)
        val firstKey = OwnerIpcRequestKey("client-a", 1L)
        val deadline = VpnOwnerRequestDeadline.after(1_000L, 5_000L)

        assertEquals(
            OwnerIpcAdmission.Admitted,
            admission.admit(firstKey, "read-snapshot", deadline, 1_000L),
        )
        assertEquals(
            OwnerIpcAdmission.InFlight,
            admission.admit(firstKey, "read-snapshot", deadline, 1_001L),
        )
        assertEquals(
            OwnerIpcAdmission.RequestIdConflict,
            admission.admit(firstKey, "mutate-profile", deadline, 1_001L),
        )
        assertEquals(
            OwnerIpcAdmission.QueueFull,
            admission.admit(
                OwnerIpcRequestKey("client-b", 1L),
                "read-snapshot",
                deadline,
                1_001L,
            ),
        )
        assertEquals(1, admission.pendingRequestCount())
    }

    @Test
    fun executingRequestHasAnExplicitInFlightState() {
        val admission = OwnerIpcRequestAdmission<String>()
        val key = OwnerIpcRequestKey("client", 12L)
        val deadline = VpnOwnerRequestDeadline.after(1_000L, 5_000L)
        assertEquals(OwnerIpcAdmission.Admitted, admission.admit(key, "read", deadline, 1_000L))
        assertEquals(OwnerIpcExecutionStart.Started, admission.begin(key, 1_001L))
        assertEquals(OwnerIpcExecutionStart.InFlight, admission.begin(key, 1_002L))
    }

    @Test
    fun expiredQueuedRequestCachesItsTypedTerminalWithoutStartingWork() {
        val admission = OwnerIpcRequestAdmission<String>()
        val key = OwnerIpcRequestKey("client", 13L)
        val deadline = VpnOwnerRequestDeadline.after(1_000L, 50L)
        assertEquals(OwnerIpcAdmission.Admitted, admission.admit(key, "start", deadline, 1_000L))
        assertEquals(OwnerIpcExecutionStart.Expired, admission.begin(key, 1_050L))
        assertEquals(
            OwnerIpcCompletion.TerminalCached,
            admission.complete(key, "expired", 1_050L),
        )
        assertEquals(
            OwnerIpcAdmission.TerminalReadback("expired"),
            admission.admit(
                key,
                "start",
                VpnOwnerRequestDeadline.after(1_051L, 5_000L),
                1_051L,
            ),
        )
    }

    @Test
    fun onlyQueuedRequestCanBeCanceledAndNoLateExecutionStarts() {
        val admission = OwnerIpcRequestAdmission<String>()
        val key = OwnerIpcRequestKey("client", 2L)
        val deadline = VpnOwnerRequestDeadline.after(1_000L, 5_000L)
        assertEquals(OwnerIpcAdmission.Admitted, admission.admit(key, "start", deadline, 1_000L))

        assertEquals(OwnerIpcCancellation.CanceledBeforeExecution, admission.cancel(key))
        assertEquals(OwnerIpcExecutionStart.Canceled, admission.begin(key, 1_001L))
        assertEquals(
            OwnerIpcCompletion.SuppressCanceledCompletion,
            admission.complete(key, "late-start", 1_002L),
        )
        assertEquals(OwnerIpcAdmission.CanceledBeforeExecution, admission.admit(key, "start", deadline, 1_003L))
        assertEquals(0, admission.pendingRequestCount())
    }

    @Test
    fun startedSideEffectContinuesAndSameRequestIdReadsBackOneTerminalResult() {
        val admission = OwnerIpcRequestAdmission<String>()
        val key = OwnerIpcRequestKey("client", 3L)
        val deadline = VpnOwnerRequestDeadline.after(1_000L, 5_000L)
        var sideEffectExecutions = 0
        assertEquals(OwnerIpcAdmission.Admitted, admission.admit(key, "restart-core", deadline, 1_000L))
        assertEquals(OwnerIpcExecutionStart.Started, admission.begin(key, 1_001L))
        sideEffectExecutions += 1

        assertEquals(OwnerIpcCancellation.ExecutionAlreadyStarted, admission.cancel(key))
        assertEquals(1, admission.pendingRequestCount())
        assertEquals(
            OwnerIpcCompletion.TerminalCached,
            admission.complete(key, "generation=9:restarted", 1_200L),
        )
        assertEquals(
            OwnerIpcAdmission.TerminalReadback("generation=9:restarted"),
            admission.admit(key, "restart-core", deadline, 1_201L),
        )
        assertEquals(1, sideEffectExecutions)
        assertEquals(0, admission.pendingRequestCount())
    }

    @Test
    fun terminalReplayExpiresAfterBoundedRetentionWindow() {
        val admission = OwnerIpcRequestAdmission<String>(terminalRetentionMillis = 100L)
        val key = OwnerIpcRequestKey("client", 4L)
        val deadline = VpnOwnerRequestDeadline.after(1_000L, 5_000L)
        assertEquals(OwnerIpcAdmission.Admitted, admission.admit(key, "read", deadline, 1_000L))
        assertEquals(OwnerIpcExecutionStart.Started, admission.begin(key, 1_001L))
        assertEquals(OwnerIpcCompletion.TerminalCached, admission.complete(key, "done", 1_010L))
        assertEquals(1, admission.retainedRequestCount(1_109L))
        assertEquals(0, admission.retainedRequestCount(1_110L))
    }
}
