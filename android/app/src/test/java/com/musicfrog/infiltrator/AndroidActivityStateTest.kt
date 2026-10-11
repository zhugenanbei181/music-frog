package com.musicfrog.infiltrator

import com.musicfrog.infiltrator.ui.common.FailureTextKey
import com.musicfrog.infiltrator.ui.common.localizedFailure
import infiltrator_android.VpnAuthorizationResult
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Before
import org.junit.Test

/** Tests the Activity-owned projection of Android's system consent result. */
class AndroidActivityStateTest {
    @Before
    fun resetFeedback() {
        AndroidActivityState.clearError()
        AndroidActivityState.applyVpnAuthorizationResult(VpnAuthorizationResult.DENIED)
    }

    @Test
    fun grantedConsentProjectsGrantedAuthorization() {
        AndroidActivityState.applyVpnAuthorizationResult(VpnAuthorizationResult.GRANTED)

        assertEquals(
            AndroidVpnAuthorizationUiState.Granted,
            AndroidActivityState.vpnAuthorization.value,
        )
    }

    @Test
    fun deniedAndRevokedConsentProjectRequiredAuthorization() {
        AndroidActivityState.applyVpnAuthorizationResult(VpnAuthorizationResult.DENIED)
        assertEquals(
            AndroidVpnAuthorizationUiState.Required,
            AndroidActivityState.vpnAuthorization.value,
        )

        AndroidActivityState.applyVpnAuthorizationResult(VpnAuthorizationResult.REVOKED)
        assertEquals(
            AndroidVpnAuthorizationUiState.Required,
            AndroidActivityState.vpnAuthorization.value,
        )
    }

    @Test
    fun failedAuthorizationObservationRemainsUnavailableAndCannotImplyGrant() {
        AndroidActivityState.applyVpnAuthorizationAvailability(
            VpnAuthorizationAvailability.Unavailable("system VPN service unavailable"),
        )

        assertEquals(
            AndroidVpnAuthorizationUiState.Unavailable,
            AndroidActivityState.vpnAuthorization.value,
        )
    }

    @Test
    fun transientActivityFeedbackCanBeClearedWithoutChangingAuthorization() {
        AndroidActivityState.applyVpnAuthorizationResult(VpnAuthorizationResult.GRANTED)
        AndroidActivityState.reportError(
            localizedFailure(FailureTextKey.Internal, "VPN command failed"),
        )

        assertEquals(
            localizedFailure(FailureTextKey.Internal, "VPN command failed"),
            AndroidActivityState.error.value,
        )

        AndroidActivityState.clearError()

        assertNull(AndroidActivityState.error.value)
        assertEquals(
            AndroidVpnAuthorizationUiState.Granted,
            AndroidActivityState.vpnAuthorization.value,
        )
    }
}
