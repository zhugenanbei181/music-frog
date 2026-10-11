package com.musicfrog.infiltrator

import infiltrator_android.AndroidOwnerDataDomain
import org.junit.Assert.assertEquals
import org.junit.Test

class AndroidOwnerDataFenceTest {
    @Test
    fun revisionIsBoundToOwnerSessionAndCoreGeneration() {
        val fence = AndroidOwnerDataFence()
        assertEquals(
            OwnerDataAcceptance.Accepted,
            fence.admit("owner-a", 4, 10, null, null),
        )
        assertEquals(
            OwnerDataAcceptance.Stale,
            fence.admit("owner-a", 3, 11, null, null),
        )
        assertEquals(
            OwnerDataAcceptance.Stale,
            fence.admit("owner-a", 4, 9, null, null),
        )
        assertEquals(
            OwnerDataAcceptance.Stale,
            fence.admit("owner-b", 0, 1, null, null),
        )

        fence.reset()
        assertEquals(
            OwnerDataAcceptance.Accepted,
            fence.admit("owner-b", 0, 1, null, null),
        )
    }

    @Test
    fun equalRevisionAllowsSameDomainSnapshotAndRejectsDifferentSnapshot() {
        val fence = AndroidOwnerDataFence()
        assertEquals(
            OwnerDataAcceptance.Accepted,
            fence.admit("owner-a", 0, 3, AndroidOwnerDataDomain.PROFILES, "profiles-a"),
        )
        assertEquals(
            OwnerDataAcceptance.Accepted,
            fence.admit("owner-a", 0, 3, AndroidOwnerDataDomain.PROFILES, "profiles-a"),
        )
        assertEquals(
            OwnerDataAcceptance.Stale,
            fence.admit("owner-a", 0, 3, AndroidOwnerDataDomain.PROFILES, "profiles-b"),
        )
    }

    @Test
    fun dnsAndFakeIpSnapshotsUseTheirOwnTypedDomains() {
        val fence = AndroidOwnerDataFence()
        assertEquals(
            OwnerDataAcceptance.Accepted,
            fence.admit("owner-a", 2, 8, AndroidOwnerDataDomain.DNS_SETTINGS, "dns-a"),
        )
        assertEquals(
            OwnerDataAcceptance.Accepted,
            fence.admit("owner-a", 2, 8, AndroidOwnerDataDomain.FAKE_IP_SETTINGS, "fake-ip-a"),
        )
    }

    @Test
    fun rulesAndProvidersSnapshotsUseDistinctTypedDomains() {
        val fence = AndroidOwnerDataFence()
        assertEquals(
            OwnerDataAcceptance.Accepted,
            fence.admit("owner-a", 3, 12, AndroidOwnerDataDomain.RULES, "rules-a"),
        )
        assertEquals(
            OwnerDataAcceptance.Accepted,
            fence.admit(
                "owner-a",
                3,
                12,
                AndroidOwnerDataDomain.RULE_PROVIDERS,
                "providers-a",
            ),
        )
        assertEquals(
            OwnerDataAcceptance.Stale,
            fence.admit("owner-a", 3, 12, AndroidOwnerDataDomain.RULES, "rules-b"),
        )
    }

    @Test
    fun newerCoreGenerationStartsANewDataRevisionSequence() {
        val fence = AndroidOwnerDataFence()
        fence.admit("owner-a", 4, 9, AndroidOwnerDataDomain.PROFILES, "old")

        assertEquals(
            OwnerDataAcceptance.Accepted,
            fence.admit("owner-a", 5, 1, AndroidOwnerDataDomain.PROFILES, "new"),
        )
    }
}
