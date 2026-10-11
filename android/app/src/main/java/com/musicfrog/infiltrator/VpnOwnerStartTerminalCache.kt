package com.musicfrog.infiltrator

internal sealed interface VpnOwnerStartReplayLookup<out T> {
    data object Miss : VpnOwnerStartReplayLookup<Nothing>

    data class Hit<T>(val response: T) : VpnOwnerStartReplayLookup<T>
}

/** Bounded same-request-ID terminal replay window for starts after caller timeout. */
internal class VpnOwnerStartTerminalCache<T>(
    private val capacity: Int = 128,
    private val retentionMillis: Long = 60_000L,
) {
    init {
        require(capacity > 0)
        require(retentionMillis > 0L)
    }

    private val terminals = linkedMapOf<VpnOwnerRequestKey, Entry<T>>()

    @Synchronized
    fun read(
        key: VpnOwnerRequestKey,
        nowElapsedRealtimeMillis: Long,
    ): VpnOwnerStartReplayLookup<T> {
        pruneExpired(nowElapsedRealtimeMillis)
        val entry = terminals[key] ?: return VpnOwnerStartReplayLookup.Miss
        return VpnOwnerStartReplayLookup.Hit(entry.response)
    }

    @Synchronized
    fun store(
        key: VpnOwnerRequestKey,
        response: T,
        nowElapsedRealtimeMillis: Long,
    ) {
        require(nowElapsedRealtimeMillis > 0L)
        pruneExpired(nowElapsedRealtimeMillis)
        terminals.remove(key)
        terminals[key] = Entry(
            response = response,
            expiresAtElapsedRealtimeMillis = saturatingAdd(nowElapsedRealtimeMillis, retentionMillis),
        )
        while (terminals.size > capacity) {
            terminals.remove(terminals.keys.first())
        }
    }

    @Synchronized
    fun size(nowElapsedRealtimeMillis: Long): Int {
        pruneExpired(nowElapsedRealtimeMillis)
        return terminals.size
    }

    private fun pruneExpired(nowElapsedRealtimeMillis: Long) {
        val expired = terminals.filterValues {
            nowElapsedRealtimeMillis >= it.expiresAtElapsedRealtimeMillis
        }.keys
        expired.forEach { terminals.remove(it) }
    }

    private fun saturatingAdd(first: Long, second: Long): Long =
        if (Long.MAX_VALUE - first < second) Long.MAX_VALUE else first + second

    private data class Entry<T>(
        val response: T,
        val expiresAtElapsedRealtimeMillis: Long,
    )
}
