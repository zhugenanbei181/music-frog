package com.musicfrog.infiltrator

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
    fun vpnApplyConfiguration(configJson: String): Boolean
    fun vpnStop(): Boolean
    fun vpnIsRunning(): Boolean
    fun vpnIsForeground(): Boolean
    fun tunSetEnabled(enabled: Boolean): Boolean
    fun tunIsEnabled(): Boolean
}

object RustBridge {
    private var loaded = false

    private external fun nativePing(): String
    private external fun nativeInit(dataDir: String, cacheDir: String): Int
    private external fun nativeRegisterBridge(host: BridgeHost): Int
    private external fun nativeRegisterNativeHost(host: NativeHostBridge): Int
    private external fun nativeClearNativeHost(): Int
    private external fun nativeOnLifecycle(
        generation: Long,
        phase: Int,
        focused: Boolean,
        visible: Boolean,
    ): Int
    private external fun nativeOnInsets(
        density: Float,
        systemTop: Int,
        systemRight: Int,
        systemBottom: Int,
        systemLeft: Int,
        imeTop: Int,
        imeRight: Int,
        imeBottom: Int,
        imeLeft: Int,
    ): Int

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

    /** BANDROID-005/006/008: register the Activity's native host adapter. */
    fun registerNativeHost(host: NativeHostBridge): Int {
        return if (ensureLoaded()) {
            nativeRegisterNativeHost(host)
        } else {
            255
        }
    }

    /** Release the retired Activity's native host global ref. */
    fun clearNativeHost(): Int {
        return if (ensureLoaded()) {
            nativeClearNativeHost()
        } else {
            255
        }
    }

    /** Push one Activity lifecycle/focus observation. */
    fun onLifecycle(generation: Long, phase: Int, focused: Boolean, visible: Boolean): Int {
        return if (ensureLoaded()) {
            nativeOnLifecycle(generation, phase, focused, visible)
        } else {
            255
        }
    }

    /** Push one `WindowInsetsCompat` observation in physical pixels. */
    fun onInsets(
        density: Float,
        systemTop: Int,
        systemRight: Int,
        systemBottom: Int,
        systemLeft: Int,
        imeTop: Int,
        imeRight: Int,
        imeBottom: Int,
        imeLeft: Int,
    ): Int {
        return if (ensureLoaded()) {
            nativeOnInsets(
                density,
                systemTop,
                systemRight,
                systemBottom,
                systemLeft,
                imeTop,
                imeRight,
                imeBottom,
                imeLeft,
            )
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
