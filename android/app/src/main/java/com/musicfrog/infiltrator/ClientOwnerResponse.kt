package com.musicfrog.infiltrator

import infiltrator_android.AndroidHostReply
import infiltrator_android.FfiErrorCode

internal data class ClientOwnerResponse(
    val reply: AndroidHostReply,
    val ownerSessionId: String?,
    val statusCode: Int,
    val statusMessage: String?,
    val payload: OwnerResponsePayload = OwnerResponsePayload.Failure,
    val terminal: OwnerClientTerminal = OwnerClientTerminal.ReplyReceived,
) {
    companion object {
        fun unavailable(terminal: OwnerClientTerminal = OwnerClientTerminal.OwnerUnavailable) = ClientOwnerResponse(
            reply = AndroidHostReply.SERVICE_UNAVAILABLE,
            ownerSessionId = null,
            statusCode = FfiErrorCode.NOT_READY.value.toInt(),
            statusMessage = "Android :vpn owner is unavailable",
            terminal = terminal,
        )

        fun staleOwnerSession(sessionId: String) = ClientOwnerResponse(
            reply = AndroidHostReply.STALE_SNAPSHOT,
            ownerSessionId = sessionId,
            statusCode = FfiErrorCode.NOT_READY.value.toInt(),
            statusMessage = "stale Android owner session reply was rejected",
            terminal = OwnerClientTerminal.OwnerSessionStale,
        )

        fun staleCoreSnapshot(sessionId: String) = ClientOwnerResponse(
            reply = AndroidHostReply.STALE_SNAPSHOT,
            ownerSessionId = sessionId,
            statusCode = FfiErrorCode.NOT_READY.value.toInt(),
            statusMessage = "stale Core snapshot was rejected",
            terminal = OwnerClientTerminal.CoreSnapshotStale,
        )

        fun staleVpnSnapshot(sessionId: String) = ClientOwnerResponse(
            reply = AndroidHostReply.STALE_SNAPSHOT,
            ownerSessionId = sessionId,
            statusCode = FfiErrorCode.NOT_READY.value.toInt(),
            statusMessage = "stale VPN generation readback was rejected",
            terminal = OwnerClientTerminal.VpnSnapshotStale,
        )

        fun staleOwnerData(sessionId: String) = ClientOwnerResponse(
            reply = AndroidHostReply.STALE_OWNER_DATA,
            ownerSessionId = sessionId,
            statusCode = FfiErrorCode.NOT_READY.value.toInt(),
            statusMessage = "stale Android owner data was rejected",
            terminal = OwnerClientTerminal.OwnerDataStale,
        )
    }
}
