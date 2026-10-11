package com.musicfrog.infiltrator

import infiltrator_android.AndroidHostReply

/** JNI adapter codes; Rust decodes these into the shared VpnStartOutcome enum. */
internal enum class VpnStartOutcomeWire(val wireCode: Int) {
    Running(0),
    PermissionRequired(1),
    OwnerUnavailable(2),
    Failed(3),
    TimedOut(4),
}

internal fun VpnStartResult.toWireOutcome(): VpnStartOutcomeWire = when (this) {
    VpnStartResult.Running -> VpnStartOutcomeWire.Running
    VpnStartResult.PermissionRequired -> VpnStartOutcomeWire.PermissionRequired
    is VpnStartResult.Failed -> VpnStartOutcomeWire.Failed
    VpnStartResult.TimedOut -> VpnStartOutcomeWire.TimedOut
}

/** Exhaustively convert the Messenger reply used by the UI process to the same start outcome. */
internal fun AndroidHostReply.toVpnStartOutcomeWire(): VpnStartOutcomeWire = when (this) {
    AndroidHostReply.VPN_RUNNING -> VpnStartOutcomeWire.Running
    AndroidHostReply.VPN_PERMISSION_REQUIRED -> VpnStartOutcomeWire.PermissionRequired
    AndroidHostReply.SERVICE_UNAVAILABLE -> VpnStartOutcomeWire.OwnerUnavailable
    AndroidHostReply.VPN_START_TIMED_OUT -> VpnStartOutcomeWire.TimedOut
    AndroidHostReply.CORE_LIFECYCLE_AVAILABLE,
    AndroidHostReply.PRODUCT_COMMAND_COMPLETED,
    AndroidHostReply.SURFACE_SNAPSHOT_AVAILABLE,
    AndroidHostReply.OWNER_OPERATION_COMPLETED,
    AndroidHostReply.VPN_CONFIGURATION_BUSY,
    AndroidHostReply.OWNER_DATA_REVISION_AVAILABLE,
    AndroidHostReply.STALE_OWNER_DATA,
    AndroidHostReply.OWNER_SESSION_CHANGED,
    AndroidHostReply.STALE_SNAPSHOT,
    AndroidHostReply.VPN_OWNER_SNAPSHOT_AVAILABLE,
    AndroidHostReply.VPN_STOP_TIMED_OUT,
    AndroidHostReply.VPN_STOP_SUPERSEDED,
    AndroidHostReply.VPN_STOP_CANCELED,
    AndroidHostReply.VPN_CONFIGURATION_ACCEPTED,
    AndroidHostReply.VPN_AUTHORIZED,
    AndroidHostReply.VPN_STOPPED,
    AndroidHostReply.FAILED -> VpnStartOutcomeWire.Failed
}
