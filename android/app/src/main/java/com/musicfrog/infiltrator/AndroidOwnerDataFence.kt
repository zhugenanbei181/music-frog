package com.musicfrog.infiltrator

import infiltrator_android.AndroidOwnerDataDomain
import java.util.concurrent.atomic.AtomicReference

internal data class OwnerDataStamp(val ownerSessionId: String, val coreGeneration: Long, val revision: Long)

internal enum class OwnerDataAcceptance {
    Accepted,
    Stale,
}

/** Fences persisted-data replies against owner restarts, Core generations and revisions. */
internal class AndroidOwnerDataFence {
    private val latest = AtomicReference<OwnerDataStamp?>(null)
    private val snapshots = mutableMapOf<AndroidOwnerDataDomain, String>()

    @Synchronized
    fun current(): OwnerDataStamp? = latest.get()

    @Synchronized
    fun reset() {
        latest.set(null)
        snapshots.clear()
    }

    @Synchronized
    fun admit(
        ownerSessionId: String,
        coreGeneration: Long,
        revision: Long,
        domain: AndroidOwnerDataDomain?,
        snapshotJson: String?,
    ): OwnerDataAcceptance {
        val current = latest.get()
        if (current != null) {
            if (current.ownerSessionId != ownerSessionId ||
                coreGeneration < current.coreGeneration ||
                (coreGeneration == current.coreGeneration && revision < current.revision)
            ) {
                return OwnerDataAcceptance.Stale
            }
            if (coreGeneration > current.coreGeneration) snapshots.clear()
        }

        if (domain != null && snapshotJson != null && current != null &&
            current.ownerSessionId == ownerSessionId &&
            current.coreGeneration == coreGeneration && current.revision == revision
        ) {
            val known = snapshots[domain]
            if (known != null && known != snapshotJson) return OwnerDataAcceptance.Stale
        }

        if (domain != null && snapshotJson != null) snapshots[domain] = snapshotJson
        latest.set(
            OwnerDataStamp(
                ownerSessionId = ownerSessionId,
                coreGeneration = coreGeneration,
                revision = revision,
            ),
        )
        return OwnerDataAcceptance.Accepted
    }
}
