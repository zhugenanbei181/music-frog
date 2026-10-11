package com.musicfrog.infiltrator

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class OwnerIpcClientPendingLedgerTest {
    @Test
    fun platformBindResultIsImmediatelyConvertedToTypedAcceptance() {
        assertEquals(OwnerBindAcceptance.Accepted, OwnerBindAcceptance.fromPlatformResult(true))
        assertEquals(OwnerBindAcceptance.Rejected, OwnerBindAcceptance.fromPlatformResult(false))
    }

    @Test
    fun oneBindingQueueAdmitsUpToCapacityAndRejectsOverflow() {
        val ledger = OwnerIpcClientPendingLedger(capacity = 2)

        assertEquals(OwnerIpcPendingAdmission.Admitted, ledger.admit(1L))
        assertEquals(OwnerIpcPendingAdmission.Admitted, ledger.admit(2L))
        assertEquals(OwnerIpcPendingAdmission.DuplicateRequestId, ledger.admit(2L))
        assertEquals(OwnerIpcPendingAdmission.CapacityExceeded, ledger.admit(3L))
        assertEquals(2, ledger.pendingCount())
    }

    @Test
    fun timeoutBeforeDispatchRemovesRequestWithoutRemoteCancellation() {
        val ledger = OwnerIpcClientPendingLedger()
        assertEquals(OwnerIpcPendingAdmission.Admitted, ledger.admit(7L))

        assertEquals(OwnerIpcPendingCancellation.CanceledBeforeDispatch, ledger.cancel(7L))
        assertFalse(ledger.markSent(7L))
        assertEquals(0, ledger.pendingCount())
    }

    @Test
    fun timeoutAfterDispatchSendsCancellationAndClearsLocalPendingEntry() {
        val ledger = OwnerIpcClientPendingLedger()
        assertEquals(OwnerIpcPendingAdmission.Admitted, ledger.admit(8L))
        assertTrue(ledger.markSent(8L))

        assertEquals(OwnerIpcPendingCancellation.CancelSentRequest, ledger.cancel(8L))
        assertEquals(OwnerIpcPendingCancellation.AlreadyCanceled, ledger.cancel(8L))
        assertEquals(0, ledger.pendingCount())
        assertFalse(ledger.complete(8L))
    }

    @Test
    fun disconnectReturnsTypedFailureForEveryPendingRequestAndClearsTheTimeoutTable() {
        val ledger = OwnerIpcClientPendingLedger()
        assertEquals(OwnerIpcPendingAdmission.Admitted, ledger.admit(10L))
        assertEquals(OwnerIpcPendingAdmission.Admitted, ledger.admit(11L))
        assertTrue(ledger.markSent(11L))

        assertEquals(
            listOf(
                OwnerIpcPendingFailure(10L, OwnerClientTerminal.BinderDied),
                OwnerIpcPendingFailure(11L, OwnerClientTerminal.BinderDied),
            ),
            ledger.disconnect(OwnerClientTerminal.BinderDied),
        )
        assertEquals(0, ledger.pendingCount())
        assertEquals(OwnerIpcPendingAdmission.Admitted, ledger.admit(12L))
    }
}
