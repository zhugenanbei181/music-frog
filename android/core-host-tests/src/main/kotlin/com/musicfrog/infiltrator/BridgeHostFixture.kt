package com.musicfrog.infiltrator

interface BridgeHost {
    fun processRole(): Int
    fun dispatchAndroidBridgeCall(requestId: Long, requestJson: String, timeoutMillis: Long): Int
    fun dispatchProfileAgeCredentialOwnerOperation(
        requestId: Long,
        metadataJson: String,
        privateKeyBytes: ByteArray,
        timeoutMillis: Long,
    ): Int
    fun cancelAndroidBridgeCall(requestId: Long): Int
    fun dataDir(): String?
    fun cacheDir(): String?
}

internal object RustBridge {
    fun completeAndroidBridgeCall(requestId: Long, completionJson: String): Int = 0
}
