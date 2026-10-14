package com.musicfrog.infiltrator

import android.util.Log

interface BridgeHost {
    /** Wire value mirrors Rust `AndroidHostProcessRole`; unknown values fail closed. */
    fun processRole(): Int
    /** Dispatches a serde-tagged Rust enum and returns before host work starts. */
    fun dispatchAndroidBridgeCall(requestId: Long, requestJson: String, timeoutMillis: Long): Int
    /** Dispatches Age metadata as JSON and its private key as a one-shot byte array. */
    fun dispatchProfileAgeCredentialOwnerOperation(
        requestId: Long,
        metadataJson: String,
        privateKeyBytes: ByteArray,
        timeoutMillis: Long,
    ): Int
    /** Cancels the matching queued or running host request by correlation ID. */
    fun cancelAndroidBridgeCall(requestId: Long): Int
    fun dataDir(): String?
    fun cacheDir(): String?
}

object RustBridge {
    private var loaded = false

    private external fun nativePing(): String
    private external fun nativeInit(dataDir: String, cacheDir: String): Int
    private external fun nativeRegisterBridge(host: BridgeHost): Int
    private external fun nativeCompleteAndroidBridgeCall(requestId: Long, completionJson: String): Int
    private external fun nativeRegisterNativeHost(host: NativeHostBridge): Int
    private external fun nativeClearNativeHost(): Int
    private external fun nativeOnLifecycle(
        generation: Long,
        phaseCode: Int,
        focusCode: Int,
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

    /** Complete an asynchronous Rust host call using its original request ID. */
    fun completeAndroidBridgeCall(requestId: Long, completionJson: String): Int =
        if (ensureLoaded()) {
            try {
                nativeCompleteAndroidBridgeCall(requestId, completionJson)
            } catch (err: LinkageError) {
                Log.e("RustBridge", "host-call completion failed: ${err.message}", err)
                255
            }
        } else {
            255
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

    /** Push one typed Activity lifecycle/focus observation. */
    internal fun onLifecycle(observation: NativeHostLifecycleObservation): Int {
        return if (ensureLoaded()) {
            try {
                nativeOnLifecycle(
                    observation.generation,
                    observation.phase.wireCode,
                    observation.focus.wireCode,
                )
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
