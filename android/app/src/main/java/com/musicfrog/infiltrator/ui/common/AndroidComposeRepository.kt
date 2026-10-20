package com.musicfrog.infiltrator.ui.common

import infiltrator_android.AndroidCoreLifecycleResult
import infiltrator_android.IpResult
import infiltrator_android.androidCoreLifecycleStatus
import infiltrator_android.androidExecuteProductCommandJson
import infiltrator_android.ipCheck
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.isActive

/** Injectable boundary for the Overview and Proxies Compose surfaces. */
internal interface AndroidComposeRepository {
    suspend fun readOwnerSurface(): OwnerSurfaceRead
    suspend fun executeProductCommand(commandJson: String): String
    suspend fun readCoreLifecycle(): AndroidCoreLifecycleResult
    suspend fun checkPublicIp(): IpResult

    fun ownerSurfaceFlow(pollIntervalMs: Long = 2000L): Flow<OwnerSurfaceRead> = flow {
        while (currentCoroutineContext().isActive) {
            emit(readOwnerSurface())
            delay(pollIntervalMs)
        }
    }

    fun coreLifecycleFlow(pollIntervalMs: Long = 1500L): Flow<AndroidCoreLifecycleResult> = flow {
        while (currentCoroutineContext().isActive) {
            emit(readCoreLifecycle())
            delay(pollIntervalMs)
        }
    }
}

/** Production adapter; global UniFFI functions are kept behind this owner. */
internal object UniFfiAndroidComposeRepository : AndroidComposeRepository {
    override suspend fun readOwnerSurface(): OwnerSurfaceRead = readOwnerSurfaceSnapshot()

    override suspend fun executeProductCommand(commandJson: String): String = androidExecuteProductCommandJson(commandJson)

    override suspend fun readCoreLifecycle(): AndroidCoreLifecycleResult = androidCoreLifecycleStatus()

    override suspend fun checkPublicIp(): IpResult = ipCheck()
}
