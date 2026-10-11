package com.musicfrog.infiltrator

import infiltrator_android.AndroidHostReply

/** JNI wire adapter for the shared Rust VpnConfigurationApplyOutcome enum. */
internal enum class VpnConfigurationApplyOutcomeWire(val wireCode: Int) {
    Accepted(0),
    Rejected(1),
    OwnerUnavailable(2),
}

internal fun AndroidHostReply.toVpnConfigurationApplyOutcomeWire(): VpnConfigurationApplyOutcomeWire =
    when (this) {
        AndroidHostReply.VPN_CONFIGURATION_ACCEPTED -> VpnConfigurationApplyOutcomeWire.Accepted
        AndroidHostReply.SERVICE_UNAVAILABLE -> VpnConfigurationApplyOutcomeWire.OwnerUnavailable
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
        AndroidHostReply.VPN_AUTHORIZED,
        AndroidHostReply.VPN_RUNNING,
        AndroidHostReply.VPN_STOPPED,
        AndroidHostReply.VPN_PERMISSION_REQUIRED,
        AndroidHostReply.VPN_START_TIMED_OUT,
        AndroidHostReply.FAILED -> VpnConfigurationApplyOutcomeWire.Rejected
    }
