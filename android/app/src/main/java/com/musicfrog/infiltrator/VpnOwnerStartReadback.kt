package com.musicfrog.infiltrator

import infiltrator_android.AndroidControlIntent
import infiltrator_android.FfiErrorCode
import infiltrator_android.androidControl

/**
 * Dispatch the authorized start through the typed Rust command and verify the
 * result against the VPN state read from the `:vpn` owner over Messenger.
 */
internal suspend fun executeVpnStartAndReadOwnerState(ownerClient: MihomoControlClient): VpnProxyStartReadback {
    val command = try {
        androidControl(AndroidControlIntent.START_PROXY)
    } catch (error: Exception) {
        return VpnProxyStartReadback.CommandRejected(
            FfiErrorCode.UNKNOWN,
            error.message ?: "Android VPN start command failed",
        )
    }
    if (command.code != FfiErrorCode.OK) {
        return VpnProxyStartReadback.CommandRejected(
            command.code,
            command.message ?: "Android VPN start command was rejected",
        )
    }

    val snapshot = try {
        ownerClient.readVpnOwnerSnapshot()
    } catch (error: Exception) {
        return VpnProxyStartReadback.OwnerStateUnavailable(
            error.message ?: "The :vpn owner status could not be read",
        )
    }
    val observed = snapshot ?: return VpnProxyStartReadback.OwnerStateUnavailable(
        "The :vpn owner returned no VPN session snapshot",
    )
    return classifyVpnOwnerStartReadback(observed)
}
