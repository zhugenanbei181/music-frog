package com.musicfrog.infiltrator.ui.settings.fakeip

import com.musicfrog.infiltrator.ui.common.AppFailure
import com.musicfrog.infiltrator.ui.common.AppFailureCode
import com.musicfrog.infiltrator.ui.common.FailureTextKey
import com.musicfrog.infiltrator.ui.common.ObservationRevision
import infiltrator_android.FakeIpSettings
import infiltrator_android.FakeIpSettingsPatch
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class FakeIpSettingsStateTest {
    @Test
    fun unavailableReadRetainsAcceptedSnapshotAndOwnerRevision() {
        val point = FakeIpSettingsObservationPoint(settings(), revision(dataRevision = 4uL))

        val unavailable = unavailableFakeIp(FakeIpSettingsReadState.Loading(point), failure())

        assertEquals(point, (unavailable as FakeIpSettingsReadState.Unavailable).lastAccepted)
        assertEquals(failure(), unavailable.failure)
    }

    @Test
    fun initialFailureBecomesUnavailableWithoutStaleSettings() {
        val unavailable = unavailableFakeIp(FakeIpSettingsReadState.Loading(), failure())

        assertTrue(unavailable is FakeIpSettingsReadState.Unavailable)
        assertNull((unavailable as FakeIpSettingsReadState.Unavailable).lastAccepted)
    }

    @Test
    fun ownerSessionChangeStartsAnewRevisionDomain() {
        val observed = observed(revision(dataRevision = 100uL))

        assertEquals(
            FakeIpSnapshotAdmission.Accepted,
            acceptsFakeIpRevision(observed, revision(ownerSessionId = "owner-b", coreGeneration = 1uL, dataRevision = 1uL)),
        )
    }

    @Test
    fun coreGenerationAndDataRevisionRejectOlderOwnerSnapshots() {
        val observed = observed(revision(coreGeneration = 7uL, dataRevision = 5uL))

        assertEquals(
            FakeIpSnapshotAdmission.Stale,
            acceptsFakeIpRevision(observed, revision(coreGeneration = 6uL, dataRevision = 999uL)),
        )
        assertEquals(
            FakeIpSnapshotAdmission.Stale,
            acceptsFakeIpRevision(observed, revision(coreGeneration = 7uL, dataRevision = 4uL)),
        )
        assertEquals(
            FakeIpSnapshotAdmission.Accepted,
            acceptsFakeIpRevision(observed, revision(coreGeneration = 7uL, dataRevision = 6uL)),
        )
        assertEquals(
            FakeIpSnapshotAdmission.Accepted,
            acceptsFakeIpRevision(observed, revision(coreGeneration = 8uL, dataRevision = 1uL)),
        )
    }

    @Test
    fun olderLocalRequestCannotSupersedeCurrentRequest() {
        assertEquals(
            FakeIpRequestAdmission.Superseded,
            admitsFakeIpRequest(ObservationRevision.Request(9uL), ObservationRevision.Request(8uL)),
        )
        assertEquals(
            FakeIpRequestAdmission.Current,
            admitsFakeIpRequest(ObservationRevision.Request(9uL), ObservationRevision.Request(9uL)),
        )
    }

    @Test
    fun omittedStoreSettingRemainsUnspecified() {
        assertEquals(FakeIpBooleanSetting.Unspecified, null.toFakeIpBooleanSetting())
        assertNull(FakeIpBooleanSetting.Unspecified.toPatchValue())
        assertEquals(FakeIpBooleanSetting.Disabled, false.toFakeIpBooleanSetting())
        assertEquals(FakeIpBooleanSetting.Enabled, true.toFakeIpBooleanSetting())
    }

    @Test
    fun clearRangeReadbackMustMatchTheRequestedValue() {
        val current = settings().copy(fakeIpRange = "198.18.0.1/16")
        val patch = FakeIpSettingsPatch(
            fakeIpRange = null,
            fakeIpFilter = emptyList(),
            storeFakeIp = null,
        )

        assertEquals(
            FakeIpSettingsReadbackResult.NotConfirmed,
            current.matches(patch, expectedRange = null),
        )
    }

    @Test
    fun hostCacheReceiptStatesOnlyWhatTheHostOperationReported() {
        assertEquals(HostFakeIpFileCacheState.Cleared, hostFakeIpFileCacheReceipt(removedFile = true))
        assertEquals(HostFakeIpFileCacheState.AlreadyEmpty, hostFakeIpFileCacheReceipt(removedFile = false))
    }

    private fun observed(revision: FakeIpSettingsRevision) =
        FakeIpSettingsReadState.Observed(settings(), revision)

    private fun revision(
        ownerSessionId: String = "owner-a",
        coreGeneration: ULong = 7uL,
        dataRevision: ULong,
    ) = FakeIpSettingsRevision(ownerSessionId, coreGeneration, dataRevision)

    private fun settings() = FakeIpSettings(
        fakeIpRange = null,
        fakeIpFilter = emptyList(),
        storeFakeIp = null,
    )

    private fun failure() = AppFailure(
        code = AppFailureCode.NotReady,
        localization = FailureTextKey.NotReady,
    )
}
