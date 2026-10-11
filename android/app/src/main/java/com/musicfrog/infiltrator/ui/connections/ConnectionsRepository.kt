package com.musicfrog.infiltrator.ui.connections

import com.musicfrog.infiltrator.ui.common.OwnerSurfaceRead
import com.musicfrog.infiltrator.ui.common.AndroidComposeRepository
import com.musicfrog.infiltrator.ui.common.UniFfiAndroidComposeRepository
import infiltrator_android.FfiStatus
import infiltrator_android.connectionClose
import infiltrator_android.connectionsCloseAll

internal interface ConnectionsRepository {
    suspend fun readSurface(): OwnerSurfaceRead
    suspend fun closeConnection(id: String): FfiStatus
    suspend fun closeAll(): FfiStatus
}

internal class UniFfiConnectionsRepository(
    private val ownerSurface: AndroidComposeRepository = UniFfiAndroidComposeRepository,
) : ConnectionsRepository {
    override suspend fun readSurface(): OwnerSurfaceRead = ownerSurface.readOwnerSurface()

    override suspend fun closeConnection(id: String): FfiStatus = connectionClose(id)

    override suspend fun closeAll(): FfiStatus = connectionsCloseAll()
}
