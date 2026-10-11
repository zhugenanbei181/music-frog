package com.musicfrog.infiltrator

import org.junit.Assert.assertEquals
import org.junit.Test

class VpnOwnerStartTerminalCacheTest {
    @Test
    fun callerRetryWithSameRequestIdentityReadsCachedTerminal() {
        val cache = VpnOwnerStartTerminalCache<String>(retentionMillis = 100L)
        val key = VpnOwnerRequestKey("client-session", 51L)

        assertEquals(VpnOwnerStartReplayLookup.Miss, cache.read(key, 1_000L))
        cache.store(key, "running:generation-8", 1_010L)
        assertEquals(
            VpnOwnerStartReplayLookup.Hit("running:generation-8"),
            cache.read(key, 1_109L),
        )
        assertEquals(VpnOwnerStartReplayLookup.Miss, cache.read(key, 1_110L))
    }

    @Test
    fun replayCacheEvictsOldestTerminalWhenBoundedCapacityIsReached() {
        val cache = VpnOwnerStartTerminalCache<String>(capacity = 1, retentionMillis = 100L)
        val first = VpnOwnerRequestKey("client-session", 52L)
        val second = VpnOwnerRequestKey("client-session", 53L)

        cache.store(first, "timed-out", 2_000L)
        cache.store(second, "running:generation-9", 2_001L)

        assertEquals(VpnOwnerStartReplayLookup.Miss, cache.read(first, 2_002L))
        assertEquals(VpnOwnerStartReplayLookup.Hit("running:generation-9"), cache.read(second, 2_002L))
        assertEquals(1, cache.size(2_002L))
    }
}
