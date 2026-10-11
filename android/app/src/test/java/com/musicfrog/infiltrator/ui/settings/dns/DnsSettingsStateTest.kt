package com.musicfrog.infiltrator.ui.settings.dns

import com.musicfrog.infiltrator.ui.common.AppFailure
import com.musicfrog.infiltrator.ui.common.AppFailureCode
import com.musicfrog.infiltrator.ui.common.FailureTextKey
import infiltrator_android.DnsFallbackFilterSettings
import infiltrator_android.DnsSettings
import infiltrator_android.DnsSettingsPatch
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class DnsSettingsStateTest {
    @Test
    fun unavailableReadRetainsAcceptedSnapshotAndRevision() {
        val point = DnsSettingsObservationPoint(settings(), revision(7uL))

        val unavailable = unavailableDns(DnsSettingsReadState.Loading(point), failure())

        assertEquals(point, (unavailable as DnsSettingsReadState.Unavailable).lastAccepted)
        assertEquals(failure(), unavailable.failure)
    }

    @Test
    fun initialFailureBecomesUnavailableWithoutInventingSettings() {
        val unavailable = unavailableDns(DnsSettingsReadState.Loading(), failure())

        assertTrue(unavailable is DnsSettingsReadState.Unavailable)
        assertNull((unavailable as DnsSettingsReadState.Unavailable).lastAccepted)
    }

    @Test
    fun oldRequestRevisionCannotReplaceAcceptedSettings() {
        val observed = DnsSettingsReadState.Observed(settings(), revision(8uL))

        assertEquals(DnsSnapshotAdmission.Stale, acceptsDnsRevision(observed, revision(7uL)))
        assertEquals(DnsSnapshotAdmission.Accepted, acceptsDnsRevision(observed, revision(9uL)))
        assertEquals(
            DnsSnapshotAdmission.Accepted,
            acceptsDnsRevision(observed, revision(1uL, generation = 3uL)),
        )
        assertEquals(
            DnsSnapshotAdmission.Accepted,
            acceptsDnsRevision(observed, revision(1uL, session = "owner-b")),
        )
    }

    @Test
    fun absentSettingsRemainUnspecifiedInsteadOfBecomingDisabled() {
        assertEquals(DnsToggleState.Unspecified, null.toDnsToggleState())
        assertNull(DnsToggleState.Unspecified.toPatchValue())
        assertEquals(DnsToggleState.Disabled, false.toDnsToggleState())
        assertEquals(DnsToggleState.Enabled, true.toDnsToggleState())
    }

    @Test
    fun absentEnhancedModeIsNotTreatedAsAnExplicitDisabledMode() {
        assertEquals(DnsEnhancedMode.Unspecified, dnsEnhancedModeFromReadback(null))
        assertEquals(DnsEnhancedMode.Disabled, dnsEnhancedModeFromSelection(""))
        assertNull(DnsEnhancedMode.Unspecified.toPatchValue())
        assertNull(DnsEnhancedMode.Disabled.toPatchValue())
        assertEquals("fake-ip", DnsEnhancedMode.Unspecified.expectedReadback("fake-ip"))
        assertNull(DnsEnhancedMode.Disabled.expectedReadback("fake-ip"))
    }

    @Test
    fun successfulReceiptDoesNotConfirmModeOrFallbackThatCouldNotBeCleared() {
        val actual = settings().copy(
            enhancedMode = "fake-ip",
            fallbackFilter = DnsFallbackFilterSettings(
                geoip = true,
                geoipCode = "CN",
                ipcidr = emptyList(),
                domain = emptyList(),
                domainSuffix = emptyList(),
            ),
        )
        val requestedEmpty = DnsSettingsPatch(
            enable = null,
            ipv6 = null,
            enhancedMode = null,
            nameserver = emptyList(),
            defaultNameserver = emptyList(),
            fallback = emptyList(),
            fallbackFilter = null,
        )

        assertEquals(
            DnsSettingsReadbackResult.NotConfirmed,
            actual.matches(requestedEmpty, expectedEnhancedMode = null, expectedFallbackFilter = null),
        )
    }

    private fun settings() = DnsSettings(
        enable = null,
        ipv6 = null,
        enhancedMode = null,
        nameserverConfigured = false,
        nameserver = emptyList(),
        defaultNameserver = emptyList(),
        fallback = emptyList(),
        fallbackFilter = null,
    )

    private fun revision(
        dataRevision: ULong,
        generation: ULong = 2uL,
        session: String = "owner-a",
    ) = DnsSettingsRevision(session, generation, dataRevision)

    private fun failure() = AppFailure(
        code = AppFailureCode.NotReady,
        localization = FailureTextKey.NotReady,
    )
}
