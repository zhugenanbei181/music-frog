package com.musicfrog.infiltrator

internal enum class OwnerSessionReadiness {
    ReadbackRequired,
    Ready,
}

internal enum class OwnerSessionEvidence {
    CoreLifecycleReadback,
    VpnStateReadback,
    VpnStopReadback,
    VpnOwnerSnapshotReadback,
    OperationReply,
}

internal enum class OwnerSessionAcceptance {
    FirstObserved,
    Current,
    Changed,
    ReadbackRequired,
    StaleResponse,
}

internal enum class OwnerSessionInvalidation {
    ReadbackRequired,
    RetiredBinding,
}

internal enum class OwnerSessionReadbackProgress {
    AwaitingCoreAndVpn,
    AwaitingCore,
    AwaitingVpn,
    Complete,
}

/** Terminal meaning carried by the Android owner client instead of a nullable fallback. */
internal enum class OwnerClientTerminal {
    ReplyReceived,
    OwnerUnavailable,
    BindRejected,
    NullBinding,
    BinderDied,
    BindingDied,
    SendFailed,
    ReplyTimedOut,
    Interrupted,
    PendingCapacityExceeded,
    InvalidReply,
    OwnerSessionStale,
    OwnerSessionReadbackRequired,
    CoreSnapshotStale,
    VpnSnapshotStale,
    OwnerDataStale,
}

internal sealed interface OwnerSessionFenceState {
    val requestId: Long

    data class ReadbackRequired(
        val ownerSessionId: String?,
        override val requestId: Long,
        val terminal: OwnerClientTerminal,
        val progress: OwnerSessionReadbackProgress,
    ) : OwnerSessionFenceState

    data class Ready(
        val ownerSessionId: String,
        override val requestId: Long,
    ) : OwnerSessionFenceState
}

/** Tracks :vpn process identity, forces post-death readback, and rejects retired sessions. */
internal class AndroidOwnerSessionFence {
    private var state: OwnerSessionFenceState = OwnerSessionFenceState.ReadbackRequired(
        ownerSessionId = null,
        requestId = 0,
        terminal = OwnerClientTerminal.OwnerSessionReadbackRequired,
        progress = OwnerSessionReadbackProgress.AwaitingCoreAndVpn,
    )
    private val retiredSessions = mutableSetOf<String>()

    @Synchronized
    fun current(): String? = when (val current = state) {
        is OwnerSessionFenceState.ReadbackRequired -> current.ownerSessionId
        is OwnerSessionFenceState.Ready -> current.ownerSessionId
    }

    @Synchronized
    fun readiness(): OwnerSessionReadiness = when (state) {
        is OwnerSessionFenceState.ReadbackRequired -> OwnerSessionReadiness.ReadbackRequired
        is OwnerSessionFenceState.Ready -> OwnerSessionReadiness.Ready
    }

    @Synchronized
    fun state(): OwnerSessionFenceState = state

    @Synchronized
    fun requireReadback(
        requestId: Long,
        expectedSessionId: String?,
        terminal: OwnerClientTerminal,
    ): OwnerSessionInvalidation {
        val currentSessionId = current()
        if (currentSessionId != null && expectedSessionId != null &&
            expectedSessionId != currentSessionId
        ) {
            return OwnerSessionInvalidation.RetiredBinding
        }
        if (requestId < state.requestId) return OwnerSessionInvalidation.RetiredBinding

        state = OwnerSessionFenceState.ReadbackRequired(
            ownerSessionId = currentSessionId,
            requestId = maxOf(state.requestId, requestId),
            terminal = terminal,
            progress = OwnerSessionReadbackProgress.AwaitingCoreAndVpn,
        )
        return OwnerSessionInvalidation.ReadbackRequired
    }

    @Synchronized
    fun accept(
        expectedSessionId: String?,
        responseSessionId: String,
        requestId: Long,
        evidence: OwnerSessionEvidence,
    ): OwnerSessionAcceptance {
        val current = state
        val currentSessionId = when (current) {
            is OwnerSessionFenceState.ReadbackRequired -> current.ownerSessionId
            is OwnerSessionFenceState.Ready -> current.ownerSessionId
        }
        if (retiredSessions.contains(responseSessionId)) {
            return OwnerSessionAcceptance.StaleResponse
        }
        if (requestId <= current.requestId) {
            return OwnerSessionAcceptance.StaleResponse
        }

        if (currentSessionId == null) {
            if (expectedSessionId != null) return OwnerSessionAcceptance.StaleResponse
            if (current is OwnerSessionFenceState.ReadbackRequired) {
                return acceptReadbackRequired(
                    current,
                    responseSessionId,
                    requestId,
                    evidence,
                    OwnerSessionAcceptance.FirstObserved,
                )
            }
            if (!evidence.isReadback()) {
                state = OwnerSessionFenceState.ReadbackRequired(
                    responseSessionId,
                    requestId,
                    OwnerClientTerminal.OwnerSessionReadbackRequired,
                    OwnerSessionReadbackProgress.AwaitingCoreAndVpn,
                )
                return OwnerSessionAcceptance.ReadbackRequired
            }
            state = OwnerSessionFenceState.ReadbackRequired(
                responseSessionId,
                requestId,
                OwnerClientTerminal.OwnerSessionReadbackRequired,
                readbackProgress(OwnerSessionReadbackProgress.AwaitingCoreAndVpn, evidence),
            )
            return OwnerSessionAcceptance.FirstObserved
        }

        if (currentSessionId == responseSessionId) {
            return when (current) {
                is OwnerSessionFenceState.ReadbackRequired -> acceptReadbackRequired(
                    current,
                    responseSessionId,
                    requestId,
                    evidence,
                    OwnerSessionAcceptance.Current,
                )
                is OwnerSessionFenceState.Ready -> {
                    state = OwnerSessionFenceState.Ready(
                        ownerSessionId = responseSessionId,
                        requestId = maxOf(current.requestId, requestId),
                    )
                    OwnerSessionAcceptance.Current
                }
            }
        }

        if (expectedSessionId != currentSessionId || requestId <= current.requestId) {
            return OwnerSessionAcceptance.StaleResponse
        }
        if (!evidence.isReadback()) {
            state = OwnerSessionFenceState.ReadbackRequired(
                ownerSessionId = currentSessionId,
                requestId = requestId,
                terminal = OwnerClientTerminal.OwnerSessionReadbackRequired,
                progress = OwnerSessionReadbackProgress.AwaitingCoreAndVpn,
            )
            return OwnerSessionAcceptance.ReadbackRequired
        }

        retiredSessions += currentSessionId
        state = OwnerSessionFenceState.ReadbackRequired(
            ownerSessionId = responseSessionId,
            requestId = requestId,
            terminal = OwnerClientTerminal.OwnerSessionReadbackRequired,
            progress = readbackProgress(OwnerSessionReadbackProgress.AwaitingCoreAndVpn, evidence),
        )
        return OwnerSessionAcceptance.Changed
    }

    private fun acceptReadbackRequired(
        current: OwnerSessionFenceState.ReadbackRequired,
        responseSessionId: String,
        requestId: Long,
        evidence: OwnerSessionEvidence,
        accepted: OwnerSessionAcceptance,
    ): OwnerSessionAcceptance {
        if (!evidence.isReadback()) {
            state = current.copy(
                requestId = maxOf(current.requestId, requestId),
                terminal = OwnerClientTerminal.OwnerSessionReadbackRequired,
            )
            return OwnerSessionAcceptance.ReadbackRequired
        }
        val progress = readbackProgress(current.progress, evidence)
        state = when (progress) {
            OwnerSessionReadbackProgress.AwaitingCoreAndVpn,
            OwnerSessionReadbackProgress.AwaitingCore,
            OwnerSessionReadbackProgress.AwaitingVpn -> OwnerSessionFenceState.ReadbackRequired(
                ownerSessionId = responseSessionId,
                requestId = maxOf(current.requestId, requestId),
                terminal = OwnerClientTerminal.OwnerSessionReadbackRequired,
                progress = progress,
            )
            OwnerSessionReadbackProgress.Complete -> OwnerSessionFenceState.Ready(
                ownerSessionId = responseSessionId,
                requestId = maxOf(current.requestId, requestId),
            )
        }
        return accepted
    }

    private fun OwnerSessionEvidence.isReadback(): Boolean = when (this) {
        OwnerSessionEvidence.CoreLifecycleReadback,
        OwnerSessionEvidence.VpnStateReadback,
        OwnerSessionEvidence.VpnStopReadback,
        OwnerSessionEvidence.VpnOwnerSnapshotReadback -> true
        OwnerSessionEvidence.OperationReply -> false
    }

    private fun readbackProgress(
        current: OwnerSessionReadbackProgress,
        evidence: OwnerSessionEvidence,
    ): OwnerSessionReadbackProgress = when (evidence) {
        OwnerSessionEvidence.CoreLifecycleReadback -> when (current) {
            OwnerSessionReadbackProgress.AwaitingCoreAndVpn -> OwnerSessionReadbackProgress.AwaitingVpn
            OwnerSessionReadbackProgress.AwaitingCore -> OwnerSessionReadbackProgress.Complete
            OwnerSessionReadbackProgress.AwaitingVpn,
            OwnerSessionReadbackProgress.Complete -> current
        }
        OwnerSessionEvidence.VpnStateReadback,
        OwnerSessionEvidence.VpnStopReadback -> when (current) {
            OwnerSessionReadbackProgress.AwaitingCoreAndVpn -> OwnerSessionReadbackProgress.AwaitingCore
            OwnerSessionReadbackProgress.AwaitingVpn -> OwnerSessionReadbackProgress.Complete
            OwnerSessionReadbackProgress.AwaitingCore,
            OwnerSessionReadbackProgress.Complete -> current
        }
        OwnerSessionEvidence.VpnOwnerSnapshotReadback -> OwnerSessionReadbackProgress.Complete
        OwnerSessionEvidence.OperationReply -> current
    }
}
