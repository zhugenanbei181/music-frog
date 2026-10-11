package com.musicfrog.infiltrator.ui.profiles

import com.musicfrog.infiltrator.ui.common.AppFailure
import com.musicfrog.infiltrator.ui.common.AndroidOwnerOperationExchange
import com.musicfrog.infiltrator.ui.common.AndroidOwnerOperationGateway
import com.musicfrog.infiltrator.ui.common.FailureTextKey
import com.musicfrog.infiltrator.ui.common.UniFfiAndroidOwnerOperationGateway
import com.musicfrog.infiltrator.ui.common.localizedFailure
import com.musicfrog.infiltrator.ui.common.toAppFailure
import infiltrator_android.AndroidOwnerDataDomain
import infiltrator_android.AndroidOwnerDataSnapshotData
import infiltrator_android.AndroidOwnerOperation
import infiltrator_android.AndroidOwnerOperationProtocolVersion
import infiltrator_android.AndroidOwnerOperationReadback
import infiltrator_android.AndroidOwnerOperationReply
import infiltrator_android.FfiErrorCode
import infiltrator_android.FfiStatus

internal sealed interface ProfilesRepositoryObservation {
    data class Profiles(val snapshot: ProfilesSnapshot) : ProfilesRepositoryObservation
    data class Detail(
        val detail: ProfileDetailSnapshot,
        val profiles: ProfilesSnapshot,
    ) : ProfilesRepositoryObservation
}

internal sealed interface ProfilesRepositoryResult {
    data class Completed(val observation: ProfilesRepositoryObservation) : ProfilesRepositoryResult
    data class Failed(
        val failure: AppFailure,
        val latest: ProfilesSnapshot?,
    ) : ProfilesRepositoryResult
    data class Cancelled(
        val failure: AppFailure,
        val latest: ProfilesSnapshot?,
    ) : ProfilesRepositoryResult
    data class TimedOut(val failure: AppFailure) : ProfilesRepositoryResult
    data class Unconfirmed(
        val failure: AppFailure,
        val latest: ProfilesSnapshot?,
    ) : ProfilesRepositoryResult
}

internal interface ProfilesRepository {
    suspend fun execute(
        operation: AndroidOwnerOperation,
        timeoutMs: Long,
    ): ProfilesRepositoryResult
}

internal class UniFfiProfilesRepository(
    private val gateway: AndroidOwnerOperationGateway = UniFfiAndroidOwnerOperationGateway,
) : ProfilesRepository {
    override suspend fun execute(
        operation: AndroidOwnerOperation,
        timeoutMs: Long,
    ): ProfilesRepositoryResult = when (val result = gateway.execute(operation, timeoutMs)) {
        is AndroidOwnerOperationExchange.Completed -> result.reply.toProfilesRepositoryResult(operation)
        is AndroidOwnerOperationExchange.Failed -> ProfilesRepositoryResult.Failed(result.failure, null)
        is AndroidOwnerOperationExchange.Cancelled -> ProfilesRepositoryResult.Cancelled(result.failure, null)
        is AndroidOwnerOperationExchange.TimedOut -> ProfilesRepositoryResult.TimedOut(result.failure)
    }
}

private fun AndroidOwnerOperationReply.toProfilesRepositoryResult(
    operation: AndroidOwnerOperation,
): ProfilesRepositoryResult {
    val kind = operation.profilesRequestKind()
    if (kind == ProfilesRequestKind.Unsupported || !matchesOwnerFence(kind)) {
        return unconfirmedProfiles(null)
    }

    val expectedRevision = when (kind.revisionAdvance()) {
        ProfilesRevisionAdvance.None -> context.expectedDataRevision
        ProfilesRevisionAdvance.One -> context.expectedDataRevision
            .takeIf { it < ULong.MAX_VALUE }
            ?.plus(1u)
            ?: return unconfirmedProfiles(null)
        ProfilesRevisionAdvance.Unsupported -> return unconfirmedProfiles(null)
    }
    if (dataRevision != expectedRevision) return unconfirmedProfiles(null)

    val latest = latestProfilesSnapshot() ?: return unconfirmedProfiles(null)

    val status = when (val receipt = readbackStatus(kind)) {
        is ProfilesReadbackReceipt.Observed -> receipt.status
        ProfilesReadbackReceipt.Mismatch -> return unconfirmedProfiles(latest)
    }
    return when (status.code) {
        FfiErrorCode.OK -> operation.toCompletedObservation(this, latest)
            ?: unconfirmedProfiles(latest)
        FfiErrorCode.CANCELED -> ProfilesRepositoryResult.Cancelled(status.toAppFailure(), latest)
        else -> ProfilesRepositoryResult.Failed(status.toAppFailure(), latest)
    }
}

private fun AndroidOwnerOperationReply.matchesOwnerFence(kind: ProfilesRequestKind): Boolean {
    if (kind == ProfilesRequestKind.Unsupported) return false
    val ownerSnapshot = snapshot ?: return false
    return protocol == AndroidOwnerOperationProtocolVersion.V3 &&
        context.clientSessionId.isNotBlank() &&
        context.ownerSessionId.isNotBlank() &&
        context.expectedOwnerSessionId == context.ownerSessionId &&
        context.expectedCoreGeneration == observedCoreGeneration &&
        dataDomain == AndroidOwnerDataDomain.PROFILES &&
        ownerSnapshot.domain == AndroidOwnerDataDomain.PROFILES &&
        ownerSnapshot.data is AndroidOwnerDataSnapshotData.Profiles
}

private fun AndroidOwnerOperationReply.readbackStatus(
    kind: ProfilesRequestKind,
): ProfilesReadbackReceipt = when (kind) {
    ProfilesRequestKind.List -> when (val receipt = readback) {
        is AndroidOwnerOperationReadback.ProfilesList -> ProfilesReadbackReceipt.Observed(receipt.v1.status)
        else -> ProfilesReadbackReceipt.Mismatch
    }
    ProfilesRequestKind.Detail -> when (val receipt = readback) {
        is AndroidOwnerOperationReadback.ProfileDetail -> ProfilesReadbackReceipt.Observed(receipt.v1.status)
        else -> ProfilesReadbackReceipt.Mismatch
    }
    ProfilesRequestKind.Mutation -> when (val receipt = readback) {
        is AndroidOwnerOperationReadback.ProfileMutation -> ProfilesReadbackReceipt.Observed(receipt.v1)
        else -> ProfilesReadbackReceipt.Mismatch
    }
    ProfilesRequestKind.Unsupported -> ProfilesReadbackReceipt.Mismatch
}

private fun AndroidOwnerOperation.toCompletedObservation(
    reply: AndroidOwnerOperationReply,
    profiles: ProfilesSnapshot,
): ProfilesRepositoryObservation? {
    return when (this) {
        AndroidOwnerOperation.ProfilesList -> {
            val receipt = reply.readback as? AndroidOwnerOperationReadback.ProfilesList ?: return null
            val snapshotData = reply.snapshot?.data as? AndroidOwnerDataSnapshotData.Profiles ?: return null
            when (receipt.v1 == snapshotData.v1) {
                true -> ProfilesRepositoryObservation.Profiles(profiles)
                false -> null
            }
        }
        is AndroidOwnerOperation.ProfileDetail -> {
            val receipt = reply.readback as? AndroidOwnerOperationReadback.ProfileDetail ?: return null
            val detail = receipt.v1.profile ?: return null
            if (detail.name != name) return null
            ProfilesRepositoryObservation.Detail(
                detail = ProfileDetailSnapshot(detail, profiles.revision),
                profiles = profiles,
            )
        }
        is AndroidOwnerOperation.ProfileCreate,
        is AndroidOwnerOperation.ProfileSelect,
        is AndroidOwnerOperation.ProfileUpdate,
        is AndroidOwnerOperation.ProfileSave,
        is AndroidOwnerOperation.ProfileDelete,
        is AndroidOwnerOperation.ProfileSubscriptionSave,
        is AndroidOwnerOperation.ProfileSubscriptionClear -> when (reply.readback) {
            is AndroidOwnerOperationReadback.ProfileMutation -> ProfilesRepositoryObservation.Profiles(profiles)
            else -> null
        }
        else -> null
    }
}

private fun AndroidOwnerOperationReply.latestProfilesSnapshot(): ProfilesSnapshot? {
    if (dataDomain != AndroidOwnerDataDomain.PROFILES) return null
    val ownerSnapshot = snapshot ?: return null
    val result = when (val data = ownerSnapshot.data) {
        is AndroidOwnerDataSnapshotData.Profiles -> data.v1
        else -> return null
    }
    if (ownerSnapshot.domain != AndroidOwnerDataDomain.PROFILES || result.status.code != FfiErrorCode.OK) {
        return null
    }
    return ProfilesSnapshot(
        profiles = result.profiles,
        revision = ProfilesRevision(
            ownerSessionId = context.ownerSessionId,
            coreGeneration = observedCoreGeneration,
            dataRevision = dataRevision,
        ),
    )
}

private fun AndroidOwnerOperation.profilesRequestKind(): ProfilesRequestKind = when (this) {
    AndroidOwnerOperation.ProfilesList -> ProfilesRequestKind.List
    is AndroidOwnerOperation.ProfileDetail -> ProfilesRequestKind.Detail
    is AndroidOwnerOperation.ProfileCreate,
    is AndroidOwnerOperation.ProfileSelect,
    is AndroidOwnerOperation.ProfileUpdate,
    is AndroidOwnerOperation.ProfileSave,
    is AndroidOwnerOperation.ProfileDelete,
    is AndroidOwnerOperation.ProfileSubscriptionSave,
    is AndroidOwnerOperation.ProfileSubscriptionClear -> ProfilesRequestKind.Mutation
    else -> ProfilesRequestKind.Unsupported
}

private fun ProfilesRequestKind.revisionAdvance(): ProfilesRevisionAdvance = when (this) {
    ProfilesRequestKind.List,
    ProfilesRequestKind.Detail -> ProfilesRevisionAdvance.None
    ProfilesRequestKind.Mutation -> ProfilesRevisionAdvance.One
    ProfilesRequestKind.Unsupported -> ProfilesRevisionAdvance.Unsupported
}

private enum class ProfilesRequestKind {
    List,
    Detail,
    Mutation,
    Unsupported,
}

private enum class ProfilesRevisionAdvance {
    None,
    One,
    Unsupported,
}

private sealed interface ProfilesReadbackReceipt {
    data class Observed(val status: FfiStatus) : ProfilesReadbackReceipt
    data object Mismatch : ProfilesReadbackReceipt
}

private fun notReadyFailure(): AppFailure = localizedFailure(FailureTextKey.NotReady).toAppFailure()

private fun unconfirmedProfiles(
    latest: ProfilesSnapshot?,
) = ProfilesRepositoryResult.Unconfirmed(notReadyFailure(), latest)
