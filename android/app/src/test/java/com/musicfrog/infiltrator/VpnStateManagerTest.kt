package com.musicfrog.infiltrator

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

/**
 * BANDROID-021: JVM unit tests for the pure VPN state machine in [VpnStateManager].
 *
 * No device and no Robolectric: `testOptions.unitTests.isReturnDefaultValues`
 * turns the `android.util.Log` calls into no-ops. Context-bound paths
 * (SharedPreferences persistence and the broadcast receiver) are deliberately
 * not exercised here — they need an instrumented test on a real Android runtime.
 */
class VpnStateManagerTest {
    @Before
    fun resetState() {
        // VpnStateManager is a process singleton; normalise the observable state
        // before each case so tests never depend on execution order.
        VpnStateManager.onVpnStopped()
        VpnStateManager.clearError()
        VpnStateManager.updateCoreState(false)
    }

    @Test
    fun startsStoppedWithNoError() {
        assertEquals(VpnStateManager.VpnState.STOPPED, VpnStateManager.vpnState.value)
        assertNull(VpnStateManager.errorMessage.value)
    }

    @Test
    fun startingClearsAPreviousError() {
        VpnStateManager.onVpnError("boom")
        VpnStateManager.onVpnStarting()
        assertEquals(VpnStateManager.VpnState.STARTING, VpnStateManager.vpnState.value)
        assertNull(VpnStateManager.errorMessage.value)
    }

    @Test
    fun runningThenStoppingThenStopped() {
        VpnStateManager.onVpnStarted()
        assertEquals(VpnStateManager.VpnState.RUNNING, VpnStateManager.vpnState.value)
        VpnStateManager.onVpnStopping()
        assertEquals(VpnStateManager.VpnState.STOPPING, VpnStateManager.vpnState.value)
        VpnStateManager.onVpnStopped()
        assertEquals(VpnStateManager.VpnState.STOPPED, VpnStateManager.vpnState.value)
    }

    @Test
    fun errorRecordsMessageAndClearErrorResetsToStopped() {
        VpnStateManager.onVpnError("tun failed")
        assertEquals(VpnStateManager.VpnState.ERROR, VpnStateManager.vpnState.value)
        assertEquals("tun failed", VpnStateManager.errorMessage.value)

        VpnStateManager.clearError()
        assertNull(VpnStateManager.errorMessage.value)
        assertEquals(VpnStateManager.VpnState.STOPPED, VpnStateManager.vpnState.value)
    }

    @Test
    fun clearErrorLeavesANonErrorStateUntouched() {
        VpnStateManager.onVpnStarted()
        VpnStateManager.clearError()
        assertEquals(VpnStateManager.VpnState.RUNNING, VpnStateManager.vpnState.value)
    }

    @Test
    fun coreRunningTracksTheLastUpdate() {
        VpnStateManager.updateCoreState(true)
        assertTrue(VpnStateManager.coreRunning.value)
        VpnStateManager.updateCoreState(false)
        assertFalse(VpnStateManager.coreRunning.value)
    }

    @Test
    fun permissionStateFollowsGrantAndDenyCallbacks() {
        VpnStateManager.onPermissionGranted()
        assertEquals(
            VpnStateManager.PermissionState.GRANTED,
            VpnStateManager.permissionState.value,
        )
        VpnStateManager.onPermissionDenied()
        assertEquals(
            VpnStateManager.PermissionState.DENIED,
            VpnStateManager.permissionState.value,
        )
    }

    @Test
    fun broadcastActionAndExtraNamesAreStable() {
        assertEquals(
            "com.musicfrog.infiltrator.VPN_STATE_CHANGED",
            VpnStateManager.ACTION_VPN_STATE_CHANGED,
        )
        assertEquals("vpn_state", VpnStateManager.EXTRA_VPN_STATE)
        assertEquals("error_message", VpnStateManager.EXTRA_ERROR_MESSAGE)
    }
}
