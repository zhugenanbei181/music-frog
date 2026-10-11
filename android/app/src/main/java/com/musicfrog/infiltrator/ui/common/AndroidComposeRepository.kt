package com.musicfrog.infiltrator.ui.common

import infiltrator_android.AndroidCoreLifecycleResult
import infiltrator_android.IpResult
import infiltrator_android.androidCoreLifecycleStatus
import infiltrator_android.androidExecuteProductCommandJson
import infiltrator_android.ipCheck

/** Injectable boundary for the Overview and Proxies Compose surfaces. */
internal interface AndroidComposeRepository {
    suspend fun readOwnerSurface(): OwnerSurfaceRead
    suspend fun executeProductCommand(commandJson: String): String
    suspend fun readCoreLifecycle(): AndroidCoreLifecycleResult
    suspend fun checkPublicIp(): IpResult
}

/** Production adapter; global UniFFI functions are kept behind this owner. */
internal object UniFfiAndroidComposeRepository : AndroidComposeRepository {
    override suspend fun readOwnerSurface(): OwnerSurfaceRead = readOwnerSurfaceSnapshot()

    override suspend fun executeProductCommand(commandJson: String): String =
        androidExecuteProductCommandJson(commandJson)

    override suspend fun readCoreLifecycle(): AndroidCoreLifecycleResult =
        androidCoreLifecycleStatus()

    override suspend fun checkPublicIp(): IpResult = ipCheck()
}
