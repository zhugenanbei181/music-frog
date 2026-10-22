package com.musicfrog.infiltrator

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * EFFECTS-047（#191）：手势仲裁的 typed 结果与 Rust 唯一真源的线码一致。
 */
class NativeGestureArbiterTest {
    @Test
    fun everyWireCodeDecodesIntoItsTypedOutcome() {
        assertEquals(NativeGestureOutcome.Undecided, nativeGestureOutcomeFrom(0))
        assertEquals(NativeGestureOutcome.Tap, nativeGestureOutcomeFrom(1))
        assertEquals(NativeGestureOutcome.LongPress, nativeGestureOutcomeFrom(2))
        assertEquals(NativeGestureOutcome.ScrollVertical, nativeGestureOutcomeFrom(3))
        assertEquals(NativeGestureOutcome.ScrollHorizontal, nativeGestureOutcomeFrom(4))
        assertEquals(NativeGestureOutcome.MultiTouch, nativeGestureOutcomeFrom(5))
        assertEquals(NativeGestureOutcome.Canceled, nativeGestureOutcomeFrom(6))
    }

    @Test
    fun anUnknownCodeStaysUndecidedAndNeverBlocksTheRowAction() {
        // The native library reports an unknown code when it is unavailable:
        // the host keeps the platform default instead of swallowing a click.
        assertEquals(NativeGestureOutcome.Undecided, nativeGestureOutcomeFrom(255))
        assertEquals(NativeGestureOutcome.Undecided, nativeGestureOutcomeFrom(-1))
        assertFalse(NativeGestureOutcome.Undecided.blocksRowAction())
    }

    @Test
    fun onlyATapDispatchesTheRowAction() {
        assertFalse(NativeGestureOutcome.Tap.blocksRowAction())
        assertFalse(NativeGestureOutcome.Undecided.blocksRowAction())
        assertTrue(NativeGestureOutcome.LongPress.blocksRowAction())
        assertTrue(NativeGestureOutcome.ScrollVertical.blocksRowAction())
        assertTrue(NativeGestureOutcome.ScrollHorizontal.blocksRowAction())
        assertTrue(NativeGestureOutcome.MultiTouch.blocksRowAction())
        assertTrue(NativeGestureOutcome.Canceled.blocksRowAction())
    }

    @Test
    fun theWireCodesMatchTheRustOwner() {
        val owner = RustOwnerWireCodes.read(
            "../../crates/infiltrator-android/src/native_host/gesture.rs",
        )

        assertEquals(
            NativeTouchPhase.entries.map(NativeTouchPhase::wireCode),
            RustOwnerWireCodes.asCodeCodes(owner, "impl TouchPhase {"),
        )
        assertEquals(
            NativeGestureOutcome.entries.map(NativeGestureOutcome::wireCode),
            RustOwnerWireCodes.asCodeCodes(owner, "impl GestureOutcome {"),
        )
    }
}
