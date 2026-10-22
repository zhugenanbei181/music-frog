package com.musicfrog.infiltrator

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * EFFECTS-047（#190 / #192）：Surface 与窗口配置事实的 typed 线码。
 */
class NativeHostSurfaceTest {
    @Test
    fun theOrientationFollowsTheObservedPixelSize() {
        assertEquals(NativeWindowOrientation.Portrait, nativeOrientationFor(1080, 1920))
        assertEquals(NativeWindowOrientation.Landscape, nativeOrientationFor(1920, 1080))
        assertEquals(NativeWindowOrientation.Square, nativeOrientationFor(1080, 1080))
    }

    @Test
    fun theSurfaceReceiptDistinguishesAcceptedStaleAndFailed() {
        assertEquals(NativeSurfaceReceipt.Accepted, nativeSurfaceReceiptFrom(0))
        assertEquals(NativeSurfaceReceipt.Stale, nativeSurfaceReceiptFrom(1))
        assertEquals(NativeSurfaceReceipt.Failed, nativeSurfaceReceiptFrom(2))
        // The native library reports an unknown code when it is unavailable.
        assertEquals(NativeSurfaceReceipt.Failed, nativeSurfaceReceiptFrom(255))
    }

    @Test
    fun theWireCodesMatchTheRustOwner() {
        val surfaceOwner = RustOwnerWireCodes.read(
            "../../crates/infiltrator-android/src/native_host/surface.rs",
        )
        val configurationOwner = RustOwnerWireCodes.read(
            "../../crates/infiltrator-android/src/native_host/configuration.rs",
        )

        assertEquals(
            NativeSurfaceState.entries.map(NativeSurfaceState::wireCode),
            RustOwnerWireCodes.asCodeCodes(surfaceOwner, "impl HostSurfaceState {"),
        )
        assertEquals(
            NativeWindowOrientation.entries.map(NativeWindowOrientation::wireCode),
            RustOwnerWireCodes.asCodeCodes(configurationOwner, "impl HostOrientation {"),
        )
    }
}
