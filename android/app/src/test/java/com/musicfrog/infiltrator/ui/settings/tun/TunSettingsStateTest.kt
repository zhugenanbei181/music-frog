package com.musicfrog.infiltrator.ui.settings.tun

import com.musicfrog.infiltrator.ui.common.AppFailure
import com.musicfrog.infiltrator.ui.common.AppFailureCode
import com.musicfrog.infiltrator.ui.common.FailureTextKey
import infiltrator_android.VpnTunSettings
import infiltrator_android.VpnTunSettingsPatch
import infiltrator_android.VpnTunStack
import infiltrator_android.VpnTunStackUpdate
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class TunSettingsStateTest {
    @Test
    fun unavailableReadRetainsLastAcceptedSettingsAndOwnerRevision() {
        val point = point()

        val unavailable = unavailableTun(TunSettingsReadState.Loading(point), failure())

        assertEquals(point, (unavailable as TunSettingsReadState.Unavailable).lastAccepted)
        assertEquals(failure(), unavailable.failure)
    }

    @Test
    fun firstReadFailureDoesNotInventTunSettings() {
        val unavailable = unavailableTun(TunSettingsReadState.Loading(), failure())

        assertTrue(unavailable is TunSettingsReadState.Unavailable)
        assertNull((unavailable as TunSettingsReadState.Unavailable).lastAccepted)
    }

    @Test
    fun olderRevisionFromTheSameOwnerCannotReplaceAcceptedSettings() {
        val observed = TunSettingsReadState.Observed(point(dataRevision = 9uL))

        assertFalse(acceptsTunRevision(observed, revision(dataRevision = 8uL)))
        assertTrue(acceptsTunRevision(observed, revision(dataRevision = 10uL)))
    }

    @Test
    fun newOwnerSessionOrCoreGenerationStartsANewRevisionDomain() {
        val observed = TunSettingsReadState.Observed(point(dataRevision = 20uL))

        assertTrue(acceptsTunRevision(observed, revision(session = "owner-b", dataRevision = 1uL)))
        assertTrue(acceptsTunRevision(observed, revision(generation = 5uL, dataRevision = 1uL)))
        assertFalse(acceptsTunRevision(observed, revision(generation = 3uL, dataRevision = 30uL)))
    }

    @Test
    fun writeReceiptMustMatchEveryRequestedTunField() {
        val actual = settings()
        val patch = VpnTunSettingsPatch(
            mtu = 1420u,
            autoRoute = false,
            strictRoute = true,
            stackUpdate = VpnTunStackUpdate.Set(stack = VpnTunStack.GVISOR),
            autoDetectInterface = false,
        )

        assertEquals(TunReadbackMatch.Confirmed, actual.matches(patch))
        assertEquals(
            TunReadbackMatch.NotConfirmed,
            actual.copy(mtu = 1500u).matches(patch),
        )
        assertEquals(
            TunReadbackMatch.Confirmed,
            actual.copy(stack = null).matches(patch.copy(stackUpdate = VpnTunStackUpdate.Clear)),
        )
        assertEquals(
            TunReadbackMatch.NotConfirmed,
            actual.matches(patch.copy(stackUpdate = VpnTunStackUpdate.Clear)),
        )
        assertEquals(
            TunReadbackMatch.Confirmed,
            actual.matches(patch.copy(stackUpdate = VpnTunStackUpdate.Keep)),
        )
    }

    @Test
    fun readbackDefaultsAreOnlyPresentationDefaults() {
        assertEquals(
            TunSettingsDraft(mtu = "", stack = "", autoRoute = true),
            TunSettingsObservationPoint(
                value = VpnTunSettings(
                    mtu = null,
                    autoRoute = null,
                    strictRoute = null,
                    stack = null,
                    autoDetectInterface = null,
                ),
                revision = revision(),
            ).toDraft(),
        )
    }

    private fun point(dataRevision: ULong = 7uL) = TunSettingsObservationPoint(
        value = settings(),
        revision = revision(dataRevision = dataRevision),
    )

    private fun settings() = VpnTunSettings(
        mtu = 1420u,
        autoRoute = false,
        strictRoute = true,
        stack = "gvisor",
        autoDetectInterface = false,
    )

    private fun revision(session: String = "owner-a", generation: ULong = 4uL, dataRevision: ULong = 7uL) =
        TunSettingsRevision(session, generation, dataRevision)

    private fun failure() = AppFailure(
        code = AppFailureCode.NotReady,
        localization = FailureTextKey.NotReady,
    )
}
