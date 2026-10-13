package com.musicfrog.infiltrator

import android.app.Application

/** Minimal Application fixture for compiling the production typed IPC client. */
internal class MihomoApplication : Application() {
    lateinit var ownerIpcConnection: MihomoOwnerIpcConnection
    lateinit var ownerIpcClient: MihomoControlClient
    val bridgeCallDispatcher = AndroidBridgeCallDispatcher()

    fun vpnOwnerSnapshotJson(): String? = null
}
