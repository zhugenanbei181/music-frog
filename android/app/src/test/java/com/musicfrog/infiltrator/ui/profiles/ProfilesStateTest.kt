package com.musicfrog.infiltrator.ui.profiles

import com.musicfrog.infiltrator.ui.common.AppFailure
import com.musicfrog.infiltrator.ui.common.AppFailureCode
import com.musicfrog.infiltrator.ui.common.FailureTextKey
import infiltrator_android.ProfileDetail
import infiltrator_android.ProfileSummary
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class ProfilesStateTest {
    @Test
    fun unavailableReadRetainsLastAcceptedProfilesAndOwnerRevision() {
        val point = ProfilesObservationPoint(
            value = snapshot(
                listOf(profile("active", active = true)),
                revision = revision(dataRevision = 8uL),
            ),
            requestSequence = 12uL,
        )

        val unavailable = ProfilesReadState.Unavailable(point, failure())

        assertEquals(point, acceptedProfiles(unavailable))
        assertEquals(failure(), unavailable.failure)
    }

    @Test
    fun initialReadFailureHasNoAcceptedSnapshot() {
        val unavailable = ProfilesReadState.Unavailable(lastAccepted = null, failure = failure())

        assertTrue(unavailable is ProfilesReadState.Unavailable)
        assertNull(acceptedProfiles(unavailable))
    }

    @Test
    fun lowerGenerationAndDataRevisionAreRejected() {
        val current = point(revision = revision(generation = 4uL, dataRevision = 9uL), requestSequence = 9uL)

        assertEquals(
            ProfilesSnapshotAdmission.Stale,
            admitsProfilesSnapshot(
                current,
                point(revision = revision(generation = 3uL, dataRevision = 99uL), requestSequence = 10uL),
            ),
        )
        assertEquals(
            ProfilesSnapshotAdmission.Stale,
            admitsProfilesSnapshot(
                current,
                point(revision = revision(dataRevision = 8uL), requestSequence = 10uL),
            ),
        )
    }

    @Test
    fun newerOwnerSessionIsAdoptedAndDelayedOldSessionReplyIsRejected() {
        val current = point(revision = revision(ownerSessionId = "owner-old"), requestSequence = 4uL)
        val adopted = point(
            revision = revision(ownerSessionId = "owner-new", generation = 1uL, dataRevision = 1uL),
            requestSequence = 5uL,
        )
        val delayedOldReply = point(
            revision = revision(ownerSessionId = "owner-old", dataRevision = 20uL),
            requestSequence = 3uL,
        )

        assertEquals(ProfilesSnapshotAdmission.Accepted, admitsProfilesSnapshot(current, adopted))
        assertEquals(ProfilesSnapshotAdmission.Stale, admitsProfilesSnapshot(adopted, delayedOldReply))
    }

    @Test
    fun sameOwnerRevisionCannotDescribeDifferentProfileData() {
        val current = point(
            profiles = listOf(profile("active", active = true)),
            revision = revision(dataRevision = 3uL),
            requestSequence = 2uL,
        )
        val conflicting = point(
            profiles = listOf(profile("other", active = true)),
            revision = revision(dataRevision = 3uL),
            requestSequence = 3uL,
        )

        assertEquals(
            ProfilesSnapshotAdmission.Conflicting,
            admitsProfilesSnapshot(current, conflicting),
        )
    }

    @Test
    fun profileMutationReadbackChecksTheRequestedOwnerState() {
        val state = snapshot(
            listOf(
                profile(
                    name = "active",
                    active = true,
                    subscriptionUrl = "https://example.invalid/config.yaml",
                    autoUpdateEnabled = true,
                    updateIntervalHours = 12u,
                ),
            ),
        )

        assertEquals(
            ProfileReadbackResult.Confirmed,
            state.satisfies(ProfileReadbackExpectation.Active("active")),
        )
        assertEquals(
            ProfileReadbackResult.Confirmed,
            state.satisfies(
                ProfileReadbackExpectation.Subscription(
                    name = "active",
                    url = "https://example.invalid/config.yaml",
                    updatePolicy = SubscriptionUpdatePolicy.Enabled,
                    intervalHours = 12,
                ),
            ),
        )
        assertEquals(
            ProfileReadbackResult.NotConfirmed,
            state.satisfies(ProfileReadbackExpectation.Absent("active")),
        )
        assertEquals(
            ProfileReadbackResult.NotConfirmed,
            state.satisfies(ProfileReadbackExpectation.Active("missing")),
        )
    }

    @Test
    fun profileSaveReadbackChecksExactContentAndActivation() {
        val expected = ProfileReadbackExpectation.SavedContent(
            name = "active",
            content = "mixed-port: 7890",
            activation = ProfileActivation.Active,
        )
        val detail = ProfileDetailSnapshot(
            profile = detail(
                name = "active",
                active = true,
                content = "mixed-port: 7890",
            ),
            revision = revision(),
        )

        assertEquals(ProfileReadbackResult.Confirmed, detail.satisfies(expected))
        assertEquals(
            ProfileReadbackResult.NotConfirmed,
            detail.copy(profile = detail.profile.copy(content = "mixed-port: 7891")).satisfies(expected),
        )
    }

    private fun point(
        profiles: List<ProfileSummary> = listOf(profile("active", active = true)),
        revision: ProfilesRevision = revision(),
        requestSequence: ULong = 1uL,
    ) = ProfilesObservationPoint(snapshot(profiles, revision), requestSequence)

    private fun snapshot(profiles: List<ProfileSummary>, revision: ProfilesRevision = revision()) = ProfilesSnapshot(profiles, revision)

    private fun revision(ownerSessionId: String = "owner-session-1", generation: ULong = 2uL, dataRevision: ULong = 1uL) =
        ProfilesRevision(ownerSessionId, generation, dataRevision)

    private fun profile(
        name: String,
        active: Boolean = false,
        subscriptionUrl: String? = null,
        autoUpdateEnabled: Boolean = false,
        updateIntervalHours: UInt? = null,
    ) = ProfileSummary(
        name = name,
        active = active,
        subscriptionUrl = subscriptionUrl,
        autoUpdateEnabled = autoUpdateEnabled,
        updateIntervalHours = updateIntervalHours,
        lastUpdated = null,
        nextUpdate = null,
    )

    private fun detail(name: String, active: Boolean, content: String) = ProfileDetail(
        name = name,
        active = active,
        content = content,
        subscriptionUrl = null,
        autoUpdateEnabled = false,
        updateIntervalHours = null,
        lastUpdated = null,
        nextUpdate = null,
    )

    private fun failure() = AppFailure(
        code = AppFailureCode.NotReady,
        localization = FailureTextKey.NotReady,
    )
}
