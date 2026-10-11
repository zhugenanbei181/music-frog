package com.musicfrog.infiltrator.ui.common

/** Source identity for a UI observation; local request order is explicit too. */
internal sealed interface ObservationRevision {
    data class Owner(val generation: ULong, val revision: ULong) : ObservationRevision
    data class VpnService(
        val ownerSessionId: String,
        val vpnGeneration: Long,
        val revision: Long,
    ) : ObservationRevision
    data class Request(val sequence: ULong) : ObservationRevision
}
