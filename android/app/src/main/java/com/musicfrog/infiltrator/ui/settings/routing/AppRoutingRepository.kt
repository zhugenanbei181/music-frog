package com.musicfrog.infiltrator.ui.settings.routing

import android.content.pm.ApplicationInfo
import android.content.pm.PackageManager
import com.musicfrog.infiltrator.ui.common.AppFailure
import com.musicfrog.infiltrator.ui.common.FailureTextKey
import com.musicfrog.infiltrator.ui.common.ObservationRevision
import com.musicfrog.infiltrator.ui.common.localizedFailure
import com.musicfrog.infiltrator.ui.common.toAppFailure
import infiltrator_android.AndroidAppRoutingSnapshot
import infiltrator_android.AndroidOwnerDataSnapshotData
import infiltrator_android.AndroidOwnerOperation
import infiltrator_android.AndroidOwnerOperationClientResult
import infiltrator_android.AndroidOwnerOperationReadback
import infiltrator_android.AndroidOwnerOperationReply
import infiltrator_android.FfiErrorCode
import infiltrator_android.androidExecuteOwnerOperation
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeoutOrNull

internal sealed interface AppRoutingRepositoryResult {
    data class Completed(val observation: AppRoutingObservation) : AppRoutingRepositoryResult
    data class Failed(
        val failure: AppFailure,
        val latestSnapshot: AcceptedAppRoutingSnapshot,
    ) : AppRoutingRepositoryResult
    data class Cancelled(
        val failure: AppFailure,
        val latestSnapshot: AcceptedAppRoutingSnapshot,
    ) : AppRoutingRepositoryResult
    data class TimedOut(val failure: AppFailure) : AppRoutingRepositoryResult
}

internal interface AppRoutingRepository {
    suspend fun execute(
        operation: AndroidOwnerOperation,
        timeoutMs: Long,
    ): AppRoutingRepositoryResult
}

internal object UniFfiAppRoutingRepository : AppRoutingRepository {
    override suspend fun execute(
        operation: AndroidOwnerOperation,
        timeoutMs: Long,
    ): AppRoutingRepositoryResult = try {
        when (val result = withTimeoutOrNull(timeoutMs) { androidExecuteOwnerOperation(operation) }) {
            null -> AppRoutingRepositoryResult.TimedOut(localizedFailure(FailureTextKey.NotReady).toAppFailure())
            is AndroidOwnerOperationClientResult.Completed -> result.reply.toAppRoutingRepositoryResult()
            is AndroidOwnerOperationClientResult.Failed -> result.status.toRepositoryFailure(
                AcceptedAppRoutingSnapshot.Missing,
            )
        }
    } catch (error: CancellationException) {
        throw error
    } catch (error: Exception) {
        AppRoutingRepositoryResult.Failed(error.toAppFailure(), AcceptedAppRoutingSnapshot.Missing)
    }
}

private fun AndroidOwnerOperationReply.toAppRoutingRepositoryResult(): AppRoutingRepositoryResult {
    val ownerSnapshot = latestAppRoutingSnapshot()
    return when (val readback = readback) {
        is AndroidOwnerOperationReadback.AppRouting -> when (readback.v1.status.code) {
            FfiErrorCode.OK -> when (val snapshot = readback.v1.snapshot) {
                null -> AppRoutingRepositoryResult.Failed(
                    localizedFailure(FailureTextKey.NotReady).toAppFailure(),
                    ownerSnapshot,
                )
                else -> AppRoutingRepositoryResult.Completed(snapshot.toObservation(this))
            }
            FfiErrorCode.CANCELED -> AppRoutingRepositoryResult.Cancelled(
                readback.v1.status.toAppFailure(),
                ownerSnapshot,
            )
            else -> AppRoutingRepositoryResult.Failed(readback.v1.status.toAppFailure(), ownerSnapshot)
        }
        else -> AppRoutingRepositoryResult.Failed(
            localizedFailure(FailureTextKey.Internal).toAppFailure(),
            ownerSnapshot,
        )
    }
}

private fun AndroidOwnerOperationReply.latestAppRoutingSnapshot(): AcceptedAppRoutingSnapshot = when (
    val data = snapshot?.data
) {
    is AndroidOwnerDataSnapshotData.AppRouting -> data.v1.let { result ->
        when (result.status.code) {
            FfiErrorCode.OK -> when (val value = result.snapshot) {
                null -> AcceptedAppRoutingSnapshot.Missing
                else -> AcceptedAppRoutingSnapshot.Present(value.toObservation(this))
            }
            else -> AcceptedAppRoutingSnapshot.Missing
        }
    }
    else -> AcceptedAppRoutingSnapshot.Missing
}

private fun AndroidAppRoutingSnapshot.toObservation(
    reply: AndroidOwnerOperationReply,
): AppRoutingObservation = AppRoutingObservation(
    mode = mode,
    selectedPackages = packages.toSet(),
    ownerSessionId = reply.context.ownerSessionId,
    revision = ObservationRevision.Owner(
        generation = reply.observedCoreGeneration,
        revision = reply.dataRevision,
    ),
)

private fun infiltrator_android.FfiStatus.toRepositoryFailure(
    snapshot: AcceptedAppRoutingSnapshot,
): AppRoutingRepositoryResult = when (code) {
    FfiErrorCode.CANCELED -> AppRoutingRepositoryResult.Cancelled(toAppFailure(), snapshot)
    FfiErrorCode.OK -> AppRoutingRepositoryResult.Failed(
        localizedFailure(FailureTextKey.Internal).toAppFailure(),
        snapshot,
    )
    else -> AppRoutingRepositoryResult.Failed(toAppFailure(), snapshot)
}

internal interface InstalledAppsProvider {
    suspend fun loadInstalledApps(): List<AppItem>
}

/** Android PackageManager access stays separate from the owner operation repository. */
internal class PackageManagerInstalledAppsProvider(
    private val packageManager: PackageManager,
) : InstalledAppsProvider {
    override suspend fun loadInstalledApps(): List<AppItem> = withContext(Dispatchers.IO) {
        packageManager.getInstalledApplications(PackageManager.GET_META_DATA)
            .map(::toAppItem)
            .sortedBy { app -> app.name.lowercase() }
    }

    private fun ApplicationInfo.toAppItem(): AppItem = AppItem(
        name = packageManager.getApplicationLabel(this).toString(),
        packageName = packageName,
        icon = packageManager.getApplicationIcon(this),
        classification = if (flags and ApplicationInfo.FLAG_SYSTEM != 0) {
            AppClassification.System
        } else {
            AppClassification.User
        },
    )
}
