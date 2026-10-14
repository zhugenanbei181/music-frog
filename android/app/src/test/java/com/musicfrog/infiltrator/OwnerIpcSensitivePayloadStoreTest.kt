package com.musicfrog.infiltrator

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertSame
import org.junit.Test

class OwnerIpcSensitivePayloadStoreTest {
    @Test
    fun removalZeroesTheRetainedPayload() {
        val store = OwnerIpcSensitivePayloadStore()
        val key = OwnerIpcRequestKey("client", 7L)
        val bytes = byteArrayOf(9, 8, 7)
        store.retain(key, bytes)

        val retained = when (val lookup = store.lookup(key)) {
            OwnerIpcSensitivePayloadLookup.Missing -> error("retained owner payload is missing")
            is OwnerIpcSensitivePayloadLookup.Present -> lookup.bytes
        }
        assertSame(bytes, retained)
        assertEquals(OwnerIpcSensitivePayloadRemoval.Wiped, store.removeAndWipe(key))
        assertArrayEquals(byteArrayOf(0, 0, 0), retained)
        assertEquals(OwnerIpcSensitivePayloadLookup.Missing, store.lookup(key))
        assertEquals(OwnerIpcSensitivePayloadRemoval.Missing, store.removeAndWipe(key))
    }

    @Test
    fun aReplacementScrubsTheSupersededBuffer() {
        val store = OwnerIpcSensitivePayloadStore()
        val key = OwnerIpcRequestKey("client", 8L)
        val first = byteArrayOf(1, 2)
        val second = byteArrayOf(3, 4)
        store.retain(key, first)
        store.retain(key, second)

        assertArrayEquals(byteArrayOf(0, 0), first)
        when (val lookup = store.lookup(key)) {
            OwnerIpcSensitivePayloadLookup.Missing -> error("replacement payload is missing")
            is OwnerIpcSensitivePayloadLookup.Present -> assertSame(second, lookup.bytes)
        }
        store.removeAndWipe(key)
        assertArrayEquals(byteArrayOf(0, 0), second)
    }
}
