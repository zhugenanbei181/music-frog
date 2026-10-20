package com.musicfrog.infiltrator.ui.connections

import com.musicfrog.infiltrator.ui.common.AndroidApplicationCommand
import com.musicfrog.infiltrator.ui.common.AndroidComposeRepository
import com.musicfrog.infiltrator.ui.common.CoreScopeObservation
import com.musicfrog.infiltrator.ui.common.OwnerSurfaceRead
import com.musicfrog.infiltrator.ui.common.ScopedProductCommandResult
import com.musicfrog.infiltrator.ui.common.UniFfiAndroidComposeRepository
import com.musicfrog.infiltrator.ui.common.buildApplicationCommandJson
import com.musicfrog.infiltrator.ui.common.parseScopedProductCommandResult

internal interface ConnectionsRepository {
    suspend fun readSurface(): OwnerSurfaceRead
    suspend fun closeConnection(scope: CoreScopeObservation, id: String): ScopedProductCommandResult
    suspend fun closeAll(scope: CoreScopeObservation): ScopedProductCommandResult
}

internal class UniFfiConnectionsRepository(private val ownerSurface: AndroidComposeRepository = UniFfiAndroidComposeRepository) :
    ConnectionsRepository {
    override suspend fun readSurface(): OwnerSurfaceRead = ownerSurface.readOwnerSurface()

    override suspend fun closeConnection(scope: CoreScopeObservation, id: String): ScopedProductCommandResult {
        val command = buildApplicationCommandJson(scope, AndroidApplicationCommand.CloseConnection(id))
        return parseScopedProductCommandResult(ownerSurface.executeProductCommand(command), scope)
    }

    override suspend fun closeAll(scope: CoreScopeObservation): ScopedProductCommandResult {
        val command = buildApplicationCommandJson(scope, AndroidApplicationCommand.CloseAllConnections)
        return parseScopedProductCommandResult(ownerSurface.executeProductCommand(command), scope)
    }
}
