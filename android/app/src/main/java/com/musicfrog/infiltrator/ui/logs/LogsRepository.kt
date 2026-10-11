package com.musicfrog.infiltrator.ui.logs

import com.musicfrog.infiltrator.ui.common.AndroidComposeRepository
import com.musicfrog.infiltrator.ui.common.FailureTextKey
import com.musicfrog.infiltrator.ui.common.ObservationRevision
import com.musicfrog.infiltrator.ui.common.OwnerSurfaceRead
import com.musicfrog.infiltrator.ui.common.UniFfiAndroidComposeRepository
import com.musicfrog.infiltrator.ui.common.localizedFailure
import com.musicfrog.infiltrator.ui.common.toAppFailure
import infiltrator_android.FfiErrorCode
import infiltrator_android.logsClear

internal interface LogsRepository {
    suspend fun readSnapshot(): LogsSnapshotRead
    suspend fun clearLogs(): LogsClearResult
}

/** Reads only the owner-authored surface snapshot and routes clear through its scoped command seam. */
internal class UniFfiLogsRepository(
    private val ownerSurface: AndroidComposeRepository = UniFfiAndroidComposeRepository,
) : LogsRepository {
    override suspend fun readSnapshot(): LogsSnapshotRead = when (val read = ownerSurface.readOwnerSurface()) {
        is OwnerSurfaceRead.Unavailable -> LogsSnapshotRead.Unavailable(read.failure.toAppFailure())
        is OwnerSurfaceRead.Available -> LogsSnapshotRead.OwnerSnapshot(
            revision = ObservationRevision.Owner(
                generation = read.snapshot.generation,
                revision = read.snapshot.revision,
            ),
            page = read.snapshot.logs,
        )
    }

    override suspend fun clearLogs(): LogsClearResult {
        val status = logsClear()
        return when (status.code) {
            FfiErrorCode.OK -> LogsClearResult.Accepted
            else -> LogsClearResult.Failed(status.toAppFailure())
        }
    }
}

internal fun logsNotReadyFailure() = localizedFailure(FailureTextKey.NotReady).toAppFailure()
