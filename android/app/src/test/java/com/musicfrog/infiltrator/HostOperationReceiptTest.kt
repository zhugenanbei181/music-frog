package com.musicfrog.infiltrator

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class HostOperationReceiptTest {
    @Test
    fun coreStartReceiptCodesRoundTripAndRejectUnknownValues() {
        val receipts = listOf(
            CoreProcessStartReceipt.Started,
            CoreProcessStartReceipt.AlreadyRunning,
            CoreProcessStartReceipt.BinaryUnavailable,
            CoreProcessStartReceipt.LaunchSpecRejected,
            CoreProcessStartReceipt.SpawnFailed,
            CoreProcessStartReceipt.NotVpnOwner,
        )
        receipts.forEach { receipt ->
            assertEquals(receipt, CoreProcessStartReceipt.fromWireCode(receipt.wireCode))
        }
        assertEquals(listOf(0, 1, 2, 3, 4, 5), receipts.map { it.wireCode })
        assertNull(CoreProcessStartReceipt.fromWireCode(-1))
        assertNull(CoreProcessStartReceipt.fromWireCode(6))
    }

    @Test
    fun coreStopReceiptCodesRoundTripAndRejectUnknownValues() {
        val receipts = listOf(
            CoreProcessStopReceipt.StoppedGracefully,
            CoreProcessStopReceipt.StoppedForcibly,
            CoreProcessStopReceipt.AlreadyStopped,
            CoreProcessStopReceipt.TimedOut,
            CoreProcessStopReceipt.Interrupted,
            CoreProcessStopReceipt.NotVpnOwner,
        )
        receipts.forEach { receipt ->
            assertEquals(receipt, CoreProcessStopReceipt.fromWireCode(receipt.wireCode))
        }
        assertEquals(listOf(0, 1, 2, 3, 4, 5), receipts.map { it.wireCode })
        assertNull(CoreProcessStopReceipt.fromWireCode(-1))
        assertNull(CoreProcessStopReceipt.fromWireCode(6))
    }

    @Test
    fun clipboardWriteReceiptDistinguishesAppliedUnavailableAndFailure() {
        var writes = 0
        val written = performNativeClipboardWrite(NativeClipboardWriter.Available { writes += 1 })

        assertEquals(NativeClipboardWriteReceipt.Written, written)
        assertEquals(1, writes)
        assertEquals(0, written.wireCode)

        val unavailable = performNativeClipboardWrite(NativeClipboardWriter.Unavailable)
        assertEquals(1, writes)
        assertEquals(1, unavailable.wireCode)

        val failed = performNativeClipboardWrite(
            NativeClipboardWriter.Available { throw IllegalStateException("denied") },
        )
        assertEquals(NativeClipboardWriteReceipt.Failed("denied"), failed)
        assertEquals(3, failed.wireCode)

        val permissionDenied = performNativeClipboardWrite(
            NativeClipboardWriter.Available { throw SecurityException("clipboard denied") },
        )
        assertEquals(NativeClipboardWriteReceipt.PermissionDenied("clipboard denied"), permissionDenied)
        assertEquals(2, permissionDenied.wireCode)

        listOf(0, 1, 2, 3).forEach { code ->
            assertEquals(code, NativeClipboardWriteReceipt.fromWireCode(code)?.wireCode)
        }
        assertNull(NativeClipboardWriteReceipt.fromWireCode(-1))
        assertNull(NativeClipboardWriteReceipt.fromWireCode(4))
    }
}
