package com.musicfrog.infiltrator

import android.app.Application
import android.util.Log
import infiltrator_android.bridgeShutdown

class MihomoApplication : Application() {
    override fun onCreate() {
        super.onCreate()
        Log.i("MihomoApp", "Initializing application...")

        try {
            System.loadLibrary("infiltrator_android")
            Log.i("MihomoApp", "Native library loaded.")
        } catch (e: UnsatisfiedLinkError) {
            Log.e("MihomoApp", "Failed to load native library", e)
        }

        // BANDROID-001/002: the `:vpn` process is created by the system without
        // any Activity, so it must initialize the native runtime and register
        // the bridge itself. The UI process keeps registering in MainActivity
        // (which also auto-starts the core); this path only covers the service
        // process so VPN start works from boot / always-on with no Activity.
        if (isServiceProcess()) {
            registerServiceProcessBridge()
        }
    }

    private fun isServiceProcess(): Boolean {
        // `Application.getProcessName()` is the static API (API 28+) that
        // reports the *current* process, unlike `applicationInfo.processName`
        // which keeps the manifest-declared default.
        return Application.getProcessName() != packageName
    }

    private fun registerServiceProcessBridge() {
        val initCode = RustBridge.init(filesDir.absolutePath, cacheDir.absolutePath)
        if (initCode != 0) {
            Log.w("MihomoApp", "service process native init failed: $initCode")
            return
        }
        val host = MihomoHost(this)
        val registerCode = RustBridge.registerBridge(host)
        if (registerCode != 0) {
            Log.w("MihomoApp", "service process bridge registration failed: $registerCode")
        } else {
            Log.i("MihomoApp", "service process bridge registered")
        }
    }

    override fun onTerminate() {
        super.onTerminate()
        bridgeShutdown()
    }
}
