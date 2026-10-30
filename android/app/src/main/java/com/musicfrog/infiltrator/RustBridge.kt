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
    private external fun nativeOnLifecycle(generation: Long, phaseCode: Int, focusCode: Int): Int
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
    private external fun nativeOnSurface(generation: Long, stateCode: Int, widthPx: Int, heightPx: Int): Int
    private external fun nativeOnConfiguration(
        generation: Long,
        orientationCode: Int,
        widthPx: Int,
        heightPx: Int,
        density: Float,
        fontScale: Float,
    ): Int
    private external fun nativeOnGesture(pointerId: Long, phaseCode: Int, x: Float, y: Float, timeMillis: Long): Int

    fun ensureLoaded(): Boolean {
        if (loaded) {
            return true
        }
        return try {
            System.loadLibrary("infiltrator_android")
            loaded = true
            true
        } catch (err: UnsatisfiedLinkError) {
            Log.e("RustBridge", "infiltrator_android native library is unavailable: ${err.message}", err)
            false
        }
    }

    fun init(dataDir: String, cacheDir: String): Int = if (ensureLoaded()) {
        try {
            nativeInit(dataDir, cacheDir)
        } catch (err: LinkageError) {
            Log.e("RustBridge", "nativeInit failed: ${err.message}", err)
            255
        }
    } else {
        255
    }

    fun registerBridge(host: BridgeHost): Int = if (ensureLoaded()) {
        try {
            nativeRegisterBridge(host)
        } catch (err: LinkageError) {
            Log.e("RustBridge", "nativeRegisterBridge failed: ${err.message}", err)
            255
        }
    } else {
        255
    }

    /** Complete an asynchronous Rust host call using its original request ID. */
    fun completeAndroidBridgeCall(requestId: Long, completionJson: String): Int = if (ensureLoaded()) {
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
    fun registerNativeHost(host: NativeHostBridge): Int = if (ensureLoaded()) {
        try {
            nativeRegisterNativeHost(host)
        } catch (err: LinkageError) {
            Log.e("RustBridge", "nativeRegisterNativeHost failed: ${err.message}", err)
            255
        }
    } else {
        255
    }

    /** Release the retired Activity's native host global ref. */
    fun clearNativeHost(): Int = if (ensureLoaded()) {
        try {
            nativeClearNativeHost()
        } catch (err: LinkageError) {
            Log.e("RustBridge", "nativeClearNativeHost failed: ${err.message}", err)
            255
        }
    } else {
        255
    }

    /** Push one typed Activity lifecycle/focus observation. */
    internal fun onLifecycle(observation: NativeHostLifecycleObservation): Int = if (ensureLoaded()) {
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
    ): Int = if (ensureLoaded()) {
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

    /**
     * EFFECTS-047 (#190): push one surface fact of the Activity's render
     * surface. The Rust owner answers with a typed lifecycle update.
     */
    internal fun onSurface(generation: Long, stateCode: Int, widthPx: Int, heightPx: Int): Int = if (ensureLoaded()) {
        try {
            nativeOnSurface(generation, stateCode, widthPx, heightPx)
        } catch (err: LinkageError) {
            Log.e("RustBridge", "nativeOnSurface failed: ${err.message}", err)
            255
        }
    } else {
        255
    }

    /**
     * EFFECTS-047 (#192): push one window configuration fact so the owner can
     * relayout after a rotation.
     */
    internal fun onConfiguration(
        generation: Long,
        orientationCode: Int,
        widthPx: Int,
        heightPx: Int,
        density: Float,
        fontScale: Float,
    ): Int = if (ensureLoaded()) {
        try {
            nativeOnConfiguration(
                generation,
                orientationCode,
                widthPx,
                heightPx,
                density,
                fontScale,
            )
        } catch (err: LinkageError) {
            Log.e("RustBridge", "nativeOnConfiguration failed: ${err.message}", err)
            255
        }
    } else {
        255
    }

    /**
     * EFFECTS-047 (#191): push one normalized pointer sample and read the typed
     * arbitration verdict. The returned code is a verdict, not an error code.
     */
    internal fun onGestureSample(pointerId: Long, phaseCode: Int, x: Float, y: Float, timeMillis: Long): Int = if (ensureLoaded()) {
        try {
            nativeOnGesture(pointerId, phaseCode, x, y, timeMillis)
        } catch (err: LinkageError) {
            Log.e("RustBridge", "nativeOnGesture failed: ${err.message}", err)
            255
        }
    } else {
        255
    }

    fun ping(): String = if (ensureLoaded()) {
        nativePing()
    } else {
        "unavailable"
    }
}
