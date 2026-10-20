package com.musicfrog.infiltrator.ui.logs

import com.musicfrog.infiltrator.ui.common.AndroidApplicationCommand
import com.musicfrog.infiltrator.ui.common.AndroidComposeRepository
import com.musicfrog.infiltrator.ui.common.CoreScopeObservation
import com.musicfrog.infiltrator.ui.common.FailureTextKey
import com.musicfrog.infiltrator.ui.common.ObservationRevision
import com.musicfrog.infiltrator.ui.common.OwnerSurfaceRead
import com.musicfrog.infiltrator.ui.common.ScopedProductCommandResult
import com.musicfrog.infiltrator.ui.common.UniFfiAndroidComposeRepository
import com.musicfrog.infiltrator.ui.common.buildApplicationCommandJson
import com.musicfrog.infiltrator.ui.common.localizedFailure
import com.musicfrog.infiltrator.ui.common.parseScopedProductCommandResult
import com.musicfrog.infiltrator.ui.common.toAppFailure

internal interface LogsRepository {
    suspend fun readSnapshot(): LogsSnapshotRead
    suspend fun clearLogs(scope: CoreScopeObservation): LogsClearResult
}

/** Reads only the owner-authored surface snapshot and routes clear through its scoped command seam. */
internal class UniFfiLogsRepository(private val ownerSurface: AndroidComposeRepository = UniFfiAndroidComposeRepository) : LogsRepository {
    override suspend fun readSnapshot(): LogsSnapshotRead = when (val read = ownerSurface.readOwnerSurface()) {
        is OwnerSurfaceRead.Unavailable -> LogsSnapshotRead.Unavailable(read.failure.toAppFailure())

        is OwnerSurfaceRead.Available -> LogsSnapshotRead.OwnerSnapshot(
            coreScope = read.snapshot.coreScope,
            revision = ObservationRevision.Owner(
                generation = read.snapshot.generation,
                revision = read.snapshot.revision,
            ),
            page = read.snapshot.logs,
        )
    }

    override suspend fun clearLogs(scope: CoreScopeObservation): LogsClearResult {
        val command = buildApplicationCommandJson(scope, AndroidApplicationCommand.ClearLogs)
        return when (
            val result = parseScopedProductCommandResult(
                ownerSurface.executeProductCommand(command),
                scope,
            )
        ) {
            is ScopedProductCommandResult.Completed -> LogsClearResult.Accepted

            is ScopedProductCommandResult.Failed -> LogsClearResult.Failed(result.failure.toAppFailure())

            ScopedProductCommandResult.ScopeMismatch,
            ScopedProductCommandResult.NonTerminal,
            ScopedProductCommandResult.Invalid,
            -> LogsClearResult.Failed(logsNotReadyFailure())
        }
    }
}

internal fun logsNotReadyFailure() = localizedFailure(FailureTextKey.NotReady).toAppFailure()
