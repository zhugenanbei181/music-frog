package com.musicfrog.infiltrator

import java.util.concurrent.atomic.AtomicReference

internal data class VpnOwnerSnapshotVersion(val ownerSessionId: String, val requestId: Long, val snapshot: VpnOwnerSnapshotWire) {
    init {
        require(ownerSessionId.isNotBlank())
        require(requestId > 0L)
        require(snapshot.ownerSessionId == ownerSessionId)
    }
}

internal enum class VpnSnapshotAcceptance {
    Accepted,
    Stale,
}

/** Fences the owner-authored VPN, Core, and resource facts as one readback. */
internal class AndroidVpnSnapshotFence {
    private val latest = AtomicReference<VpnOwnerSnapshotVersion?>(null)

    fun admit(incoming: VpnOwnerSnapshotVersion): VpnSnapshotAcceptance {
        while (true) {
            val current = latest.get()
            if (current?.snapshot == incoming.snapshot) return VpnSnapshotAcceptance.Accepted
            if (current != null && isOlderThan(incoming, current)) {
                return VpnSnapshotAcceptance.Stale
            }
            if (latest.compareAndSet(current, incoming)) return VpnSnapshotAcceptance.Accepted
        }
    }

    fun reset() {
        latest.set(null)
    }

    private fun isOlderThan(incoming: VpnOwnerSnapshotVersion, current: VpnOwnerSnapshotVersion): Boolean {
        val next = incoming.snapshot
        val previous = current.snapshot
        if (incoming.ownerSessionId != current.ownerSessionId) return true
        if (next.revision < previous.revision) return true
        if (next.revision == previous.revision) return next != previous
        if (next.vpnGeneration < previous.vpnGeneration) return true
        val nextCore = next.core
        val previousCore = previous.core
        if (nextCore is VpnOwnerCoreReadbackWire.Available &&
            previousCore is VpnOwnerCoreReadbackWire.Available
        ) {
            val nextFacts = nextCore.snapshot
            val previousFacts = previousCore.snapshot
            if (nextFacts.generation < previousFacts.generation) return true
            if (nextFacts.generation == previousFacts.generation &&
                (
                    nextFacts.identity != previousFacts.identity ||
                        nextFacts.sessionToken != previousFacts.sessionToken ||
                        nextFacts.revision < previousFacts.revision
                    )
            ) {
                return true
            }
        }
        return false
    }
}
