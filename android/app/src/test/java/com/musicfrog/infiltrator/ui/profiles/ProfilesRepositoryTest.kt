package com.musicfrog.infiltrator.ui.profiles

import com.musicfrog.infiltrator.testFfiStatus
import com.musicfrog.infiltrator.ui.common.AndroidOwnerOperationExchange
import com.musicfrog.infiltrator.ui.common.AndroidOwnerOperationGateway
import com.musicfrog.infiltrator.ui.common.AppFailure
import com.musicfrog.infiltrator.ui.common.AppFailureCode
import com.musicfrog.infiltrator.ui.common.FailureTextKey
import infiltrator_android.AndroidOwnerDataDomain
import infiltrator_android.AndroidOwnerDataSnapshot
import infiltrator_android.AndroidOwnerDataSnapshotData
import infiltrator_android.AndroidActiveProfileReferenceObservation
import infiltrator_android.AndroidOwnerOperation
import infiltrator_android.AndroidOwnerOperationContext
import infiltrator_android.AndroidOwnerOperationProtocolVersion
import infiltrator_android.AndroidOwnerOperationReadback
import infiltrator_android.AndroidOwnerOperationReply
import infiltrator_android.AndroidProfileSelectionReadback
import infiltrator_android.AndroidProfileSelectionTarget
import infiltrator_android.FfiErrorCode
import infiltrator_android.FfiStatus
import infiltrator_android.ProfileDetail
import infiltrator_android.ProfileDetailResult
import infiltrator_android.ProfileSummary
import infiltrator_android.ProfilesResult
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class ProfilesRepositoryTest {
    @Test
    fun listUsesAcceptedOwnerSnapshotAndRevision() = runBlocking {
        val ownerProfiles = listOf(profile("alpha", active = true))
        val repository = repository { operation -> clientCompleted(reply(operation, ownerProfiles)) }

        val result = repository.execute(AndroidOwnerOperation.ProfilesList, 1000L)

        val completed = result as ProfilesRepositoryResult.Completed
        val observation = completed.observation as ProfilesRepositoryObservation.Profiles
        assertEquals(ownerProfiles, observation.snapshot.profiles)
        assertEquals(ProfilesRevision("owner-session-1", 7uL, 10uL), observation.snapshot.revision)
    }

    @Test
    fun selectionRequiresExactEngineScopedReadbackAtAdvancedRevision() = runBlocking {
        val ownerProfiles = listOf(profile("alpha", active = true))
        val target = AndroidProfileSelectionTarget.Mihomo("alpha")
        val operation = AndroidOwnerOperation.ProfileSelect(target)
        val repository = repository { requested -> clientCompleted(reply(requested, ownerProfiles)) }

        val result = repository.execute(operation, 1000L)

        val completed = result as ProfilesRepositoryResult.Completed
        val selection = completed.observation as ProfilesRepositoryObservation.Selection
        assertEquals(ProfilesRevision("owner-session-1", 7uL, 11uL), selection.snapshot.revision)
        assertEquals(target, selection.target)
        assertEquals(4uL, selection.activeReferenceRevision)
    }

    @Test
    fun selectionRejectsReadbackForAnotherEngine() = runBlocking {
        val target = AndroidProfileSelectionTarget.Mihomo("alpha")
        val requested = AndroidOwnerOperation.ProfileSelect(target)
        val mismatching = reply(requested, listOf(profile("alpha", active = false))).copy(
            readback = AndroidOwnerOperationReadback.ProfileSelection(
                AndroidProfileSelectionReadback(
                    operationStatus = testFfiStatus(FfiErrorCode.OK),
                    activeReference = AndroidActiveProfileReferenceObservation.Selected(
                        target = AndroidProfileSelectionTarget.SingBox("alpha"),
                        revision = 4uL,
                    ),
                ),
            ),
        )
        val repository = repository { clientCompleted(mismatching) }

        assertTrue(repository.execute(requested, 1000L) is ProfilesRepositoryResult.Unconfirmed)
    }

    @Test
    fun staleRevisionAndForeignOwnerSessionAreUnconfirmedWithoutAdoptingSnapshot() = runBlocking {
        val profiles = listOf(profile("alpha"))
        val operation = AndroidOwnerOperation.ProfileCreate("alpha", "https://example.invalid")
        val staleRevision = reply(operation, profiles).copy(dataRevision = 10uL)
        val staleRepository = repository { clientCompleted(staleRevision) }
        val foreignSession = reply(operation, profiles).copy(
            context = context().copy(
                ownerSessionId = "owner-session-old",
                expectedOwnerSessionId = "owner-session-current",
            ),
        )
        val foreignRepository = repository { clientCompleted(foreignSession) }

        val staleResult = staleRepository.execute(operation, 1000L)
        val foreignResult = foreignRepository.execute(operation, 1000L)

        assertTrue(staleResult is ProfilesRepositoryResult.Unconfirmed)
        assertEquals(null, (staleResult as ProfilesRepositoryResult.Unconfirmed).latest)
        assertTrue(foreignResult is ProfilesRepositoryResult.Unconfirmed)
        assertEquals(null, (foreignResult as ProfilesRepositoryResult.Unconfirmed).latest)
    }

    @Test
    fun mismatchedTypedReadbackCannotClaimAValidProfilesSnapshot() = runBlocking {
        val ownerProfiles = listOf(profile("alpha"))
        val mismatched = reply(AndroidOwnerOperation.ProfilesList, ownerProfiles).copy(
            readback = AndroidOwnerOperationReadback.ProfilesList(
                ProfilesResult(testFfiStatus(FfiErrorCode.OK), listOf(profile("other"))),
            ),
        )
        val repository = repository { clientCompleted(mismatched) }

        val result = repository.execute(AndroidOwnerOperation.ProfilesList, 1000L)

        assertTrue(result is ProfilesRepositoryResult.Unconfirmed)
        assertEquals(ownerProfiles, (result as ProfilesRepositoryResult.Unconfirmed).latest?.profiles)
    }

    @Test
    fun ownerCancellationAndTimeoutHaveDistinctTypedTerminals() = runBlocking {
        val operation = AndroidOwnerOperation.ProfileUpdate("alpha")
        val canceledRepository = repository { requested ->
            clientCompleted(
                reply(
                    requested,
                    listOf(profile("alpha")),
                    status = testFfiStatus(FfiErrorCode.CANCELED, "canceled by owner"),
                ),
            )
        }
        val timeoutRepository = repository {
            AndroidOwnerOperationExchange.TimedOut(failure())
        }

        assertTrue(canceledRepository.execute(operation, 1000L) is ProfilesRepositoryResult.Cancelled)
        assertTrue(timeoutRepository.execute(operation, 1L) is ProfilesRepositoryResult.TimedOut)
    }

    @Test
    fun callerCancellationIsRethrown() = runBlocking {
        val repository = repository { throw CancellationException("caller cancelled") }

        try {
            repository.execute(AndroidOwnerOperation.ProfilesList, 1000L)
            throw AssertionError("caller cancellation was converted into a repository result")
        } catch (error: CancellationException) {
            assertEquals("caller cancelled", error.message)
        }
    }

    @Test
    fun detailReadbackMustMatchRequestedProfileAndCarryOwnerRevision() = runBlocking {
        val detail = detail("alpha", active = false, content = "mixed-port: 7890")
        val operation = AndroidOwnerOperation.ProfileDetail("alpha")
        val repository = repository { requested ->
            clientCompleted(reply(requested, listOf(profile("alpha")), profileDetail = detail))
        }

        val result = repository.execute(operation, 1000L)

        val observation = (result as ProfilesRepositoryResult.Completed).observation
            as ProfilesRepositoryObservation.Detail
        assertEquals(detail, observation.detail.profile)
        assertEquals(observation.profiles.revision, observation.detail.revision)

        val wrongTarget = repository { requested ->
            clientCompleted(
                reply(
                    requested,
                    listOf(profile("alpha")),
                    profileDetail = detail("other", active = false, content = "mixed-port: 7890"),
                ),
            )
        }
        assertTrue(
            wrongTarget.execute(operation, 1000L) is ProfilesRepositoryResult.Unconfirmed,
        )
    }

    private fun repository(
        handle: suspend (AndroidOwnerOperation) -> AndroidOwnerOperationExchange,
    ) = UniFfiProfilesRepository(object : AndroidOwnerOperationGateway {
        override suspend fun execute(
            operation: AndroidOwnerOperation,
            timeoutMs: Long,
        ): AndroidOwnerOperationExchange = handle(operation)
    })

    private fun clientCompleted(reply: AndroidOwnerOperationReply) =
        AndroidOwnerOperationExchange.Completed(reply)

    private fun reply(
        operation: AndroidOwnerOperation,
        profiles: List<ProfileSummary>,
        status: FfiStatus = testFfiStatus(FfiErrorCode.OK),
        profileDetail: ProfileDetail = detail("alpha", active = false, content = "mixed-port: 7890"),
    ): AndroidOwnerOperationReply {
        val profilesResult = ProfilesResult(testFfiStatus(FfiErrorCode.OK), profiles)
        val readback = when (operation) {
            AndroidOwnerOperation.ProfilesList -> AndroidOwnerOperationReadback.ProfilesList(profilesResult)
            is AndroidOwnerOperation.ProfileDetail -> AndroidOwnerOperationReadback.ProfileDetail(
                ProfileDetailResult(status, profileDetail),
            )
            is AndroidOwnerOperation.ProfileSelect -> AndroidOwnerOperationReadback.ProfileSelection(
                AndroidProfileSelectionReadback(
                    operationStatus = status,
                    activeReference = AndroidActiveProfileReferenceObservation.Selected(
                        target = operation.target,
                        revision = 4uL,
                    ),
                ),
            )
            is AndroidOwnerOperation.ProfileCreate,
            is AndroidOwnerOperation.ProfileUpdate,
            is AndroidOwnerOperation.ProfileSave,
            is AndroidOwnerOperation.ProfileDelete,
            is AndroidOwnerOperation.ProfileSubscriptionSave,
            is AndroidOwnerOperation.ProfileSubscriptionClear -> AndroidOwnerOperationReadback.ProfileMutation(status)
            else -> throw IllegalArgumentException("Not a Profiles operation: $operation")
        }
        val operationRevision = when (operation) {
            AndroidOwnerOperation.ProfilesList,
            is AndroidOwnerOperation.ProfileDetail -> 0uL
            else -> 1uL
        }
        return AndroidOwnerOperationReply(
            protocol = AndroidOwnerOperationProtocolVersion.V6,
            context = context(),
            observedCoreGeneration = 7uL,
            dataRevision = 10uL + operationRevision,
            dataDomain = AndroidOwnerDataDomain.PROFILES,
            snapshot = AndroidOwnerDataSnapshot(
                domain = AndroidOwnerDataDomain.PROFILES,
                data = AndroidOwnerDataSnapshotData.Profiles(profilesResult),
            ),
            readback = readback,
        )
    }

    private fun context() = AndroidOwnerOperationContext(
        clientSessionId = "client-session-1",
        expectedOwnerSessionId = "owner-session-1",
        ownerSessionId = "owner-session-1",
        expectedCoreGeneration = 7uL,
        expectedDataRevision = 10uL,
    )

    private fun profile(name: String, active: Boolean = false) = ProfileSummary(
        name = name,
        active = active,
        subscriptionUrl = null,
        autoUpdateEnabled = false,
        updateIntervalHours = null,
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
