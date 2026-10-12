package com.musicfrog.infiltrator

import org.junit.Assert.assertEquals
import org.junit.Test

class OwnerIpcClientPendingLedgerTypedStateTest {
    @Test
    fun sendAndCompletionUseDisjointReceipts() {
        val ledger = OwnerIpcClientPendingLedger()
        assertEquals(OwnerIpcPendingAdmission.Admitted, ledger.admit(1L))
        assertEquals(OwnerIpcPendingSendAdmission.Sent, ledger.markSent(1L))
        assertEquals(OwnerIpcPendingSendAdmission.AlreadySent, ledger.markSent(1L))
        assertEquals(OwnerIpcPendingCancellation.CancelSentRequest, ledger.cancel(1L))
        assertEquals(OwnerIpcPendingSendAdmission.CancellationPending, ledger.markSent(1L))
        assertEquals(OwnerIpcPendingCompletion.Completed, ledger.complete(1L))
        assertEquals(OwnerIpcPendingCompletion.Unknown, ledger.complete(1L))
    }
}
