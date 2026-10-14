package com.musicfrog.infiltrator

internal sealed interface OwnerIpcSensitivePayloadLookup {
    data object Missing : OwnerIpcSensitivePayloadLookup

    data class Present(val bytes: ByteArray) : OwnerIpcSensitivePayloadLookup
}

internal enum class OwnerIpcSensitivePayloadRemoval {
    Wiped,
    Missing,
}

/** Owner-process storage for one-shot IPC bytes; every removal scrubs the owned array. */
internal class OwnerIpcSensitivePayloadStore {
    private val lock = Any()
    private val payloads = linkedMapOf<OwnerIpcRequestKey, ByteArray>()

    fun retain(key: OwnerIpcRequestKey, bytes: ByteArray) {
        synchronized(lock) {
            payloads.put(key, bytes)?.fill(0)
        }
    }

    fun lookup(key: OwnerIpcRequestKey): OwnerIpcSensitivePayloadLookup = synchronized(lock) {
        when (val bytes = payloads[key]) {
            null -> OwnerIpcSensitivePayloadLookup.Missing
            else -> OwnerIpcSensitivePayloadLookup.Present(bytes)
        }
    }

    fun removeAndWipe(key: OwnerIpcRequestKey): OwnerIpcSensitivePayloadRemoval =
        synchronized(lock) {
            when (val bytes = payloads.remove(key)) {
                null -> OwnerIpcSensitivePayloadRemoval.Missing
                else -> {
                    bytes.fill(0)
                    OwnerIpcSensitivePayloadRemoval.Wiped
                }
            }
        }
}
