package com.musicfrog.infiltrator

import java.util.concurrent.atomic.AtomicReference

internal data class CoreSnapshotVersion(
    val generation: Long,
    val revision: Long,
    val sessionToken: String?,
    val identity: CoreOwnerIdentityWire = CoreOwnerIdentityWire.Absent,
    val lifecycle: CoreOwnerLifecycleWire = CoreOwnerLifecycleWire.Stopped,
    val failure: CoreOwnerFailureWire? = null,
)

internal enum class CoreSnapshotAcceptance {
    Accepted,
    Stale,
}

/** Rejects delayed owner snapshots that move backward across a Core session. */
internal class AndroidCoreSnapshotFence {
    private val latest = AtomicReference<CoreSnapshotVersion?>(null)

    fun admit(incoming: CoreSnapshotVersion): CoreSnapshotAcceptance {
        while (true) {
            val current = latest.get()
            if (current == incoming) return CoreSnapshotAcceptance.Accepted
            if (current != null && isOlderThan(incoming, current)) {
                return CoreSnapshotAcceptance.Stale
            }
            if (latest.compareAndSet(current, incoming)) {
                return CoreSnapshotAcceptance.Accepted
            }
        }
    }

    fun reset() {
        latest.set(null)
    }

    private fun isOlderThan(incoming: CoreSnapshotVersion, current: CoreSnapshotVersion): Boolean {
        if (incoming.generation != current.generation) {
            return incoming.generation < current.generation
        }
        if (incoming.identity != current.identity || incoming.sessionToken != current.sessionToken) {
            return true
        }
        return incoming.revision <= current.revision
    }
}
