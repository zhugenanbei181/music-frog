package com.musicfrog.infiltrator

import android.util.Log

interface BridgeHost {
    /** Wire value mirrors Rust `AndroidHostProcessRole`; unknown values fail closed. */
    fun processRole(): Int
    /** CoreProcessStartReceipt wire code; decoded exhaustively by Rust. */
    fun coreStart(startRequestJson: String): Int
    /** Serde-tagged validation receipt produced by the stopped :vpn owner. */
    fun coreValidateConfiguration(requestJson: String): String
    /** CoreProcessStopReceipt wire code; decoded exhaustively by Rust. */
    fun coreStop(): Int
    fun coreProcessObservationJson(): String
    /** JSON-encoded typed lifecycle snapshot from the `:vpn` owner. */
    fun coreLifecycleSnapshot(): String?
    /** Execute shared typed Rust command JSON through the `:vpn` owner. */
    fun executeProductCommandJson(commandJson: String): String?
    /** Read the owner-authored shared product snapshot through typed IPC. */
    fun surfaceSnapshotJson(): String?
    /** Execute a Rust-serde typed persisted-data operation through :vpn. */
    fun executeOwnerOperationJson(operationJson: String): String?
    fun credentialGet(service: String, key: String): String?
    fun credentialSet(service: String, key: String, value: String): Boolean
    fun credentialDelete(service: String, key: String): Boolean
    fun dataDir(): String?
    fun cacheDir(): String?
    /** VpnStartOutcome wire code, decoded exhaustively by the Rust bridge. */
    fun vpnStartOutcome(): Int
    /** Rust wire codes: 0 = granted, 1 = system consent required, other = unavailable. */
    fun vpnAuthorizationState(): Int
    /** VpnConfigurationApplyOutcome wire code, decoded exhaustively by Rust. */
    fun vpnApplyConfiguration(configJson: String): Int
    /** Serde JSON containing one dedicated Stop terminal and its owner snapshot. */
    fun vpnStopResultJson(targetCode: Int, generation: Long): String?
    /** Serde JSON for one owner-authored VPN/Core/resource snapshot. */
    fun vpnOwnerSnapshotJson(): String?
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
            try {
                nativeInit(dataDir, cacheDir)
            } catch (err: LinkageError) {
                Log.e("RustBridge", "nativeInit failed: ${err.message}", err)
                255
            }
        } else {
            255
        }
    }

    fun registerBridge(host: BridgeHost): Int {
        return if (ensureLoaded()) {
            try {
                nativeRegisterBridge(host)
            } catch (err: LinkageError) {
                Log.e("RustBridge", "nativeRegisterBridge failed: ${err.message}", err)
                255
            }
        } else {
            255
        }
    }

    /** BANDROID-005/006/008: register the Activity's native host adapter. */
    fun registerNativeHost(host: NativeHostBridge): Int {
        return if (ensureLoaded()) {
            try {
                nativeRegisterNativeHost(host)
            } catch (err: LinkageError) {
                Log.e("RustBridge", "nativeRegisterNativeHost failed: ${err.message}", err)
                255
            }
        } else {
            255
        }
    }

    /** Release the retired Activity's native host global ref. */
    fun clearNativeHost(): Int {
        return if (ensureLoaded()) {
            try {
                nativeClearNativeHost()
            } catch (err: LinkageError) {
                Log.e("RustBridge", "nativeClearNativeHost failed: ${err.message}", err)
                255
            }
        } else {
            255
        }
    }

    /** Push one Activity lifecycle/focus observation. */
    fun onLifecycle(generation: Long, phase: Int, focused: Boolean, visible: Boolean): Int {
        return if (ensureLoaded()) {
            try {
                nativeOnLifecycle(generation, phase, focused, visible)
            } catch (err: LinkageError) {
                Log.e("RustBridge", "nativeOnLifecycle failed: ${err.message}", err)
                255
            }
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
            try {
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
            } catch (err: LinkageError) {
                Log.e("RustBridge", "nativeOnInsets failed: ${err.message}", err)
                255
            }
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
