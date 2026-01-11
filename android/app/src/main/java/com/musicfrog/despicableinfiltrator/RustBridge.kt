package com.musicfrog.despicableinfiltrator

interface BridgeHost {
    fun coreStart(): Boolean
    fun coreStop(): Boolean
    fun coreIsRunning(): Boolean
    fun coreControllerUrl(): String?
    fun credentialGet(service: String, key: String): String?
    fun credentialSet(service: String, key: String, value: String): Boolean
    fun credentialDelete(service: String, key: String): Boolean
    fun dataDir(): String?
    fun cacheDir(): String?
    fun vpnStart(): Boolean
    fun vpnStop(): Boolean
    fun vpnIsRunning(): Boolean
    fun tunSetEnabled(enabled: Boolean): Boolean
    fun tunIsEnabled(): Boolean
}

object RustBridge {
    private var loaded = false

    private external fun nativePing(): String
    private external fun nativeInit(dataDir: String, cacheDir: String): Int
    private external fun nativeRegisterBridge(host: BridgeHost): Int

    fun ensureLoaded(): Boolean {
        if (loaded) {
            return true
        }
        return try {
            System.loadLibrary("infiltrator_android")
            loaded = true
            true
        } catch (err: UnsatisfiedLinkError) {
            false
        }
    }

    fun init(dataDir: String, cacheDir: String): Int {
        return if (ensureLoaded()) {
            nativeInit(dataDir, cacheDir)
        } else {
            255
        }
    }

    fun registerBridge(host: BridgeHost): Int {
        return if (ensureLoaded()) {
            nativeRegisterBridge(host)
        } else {
            255
        }
    }

    fun ping(): String {
        return if (ensureLoaded()) {
            nativePing()
        } else {
            "unavailable"
        }
    }
}
