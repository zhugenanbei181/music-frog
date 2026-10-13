package com.musicfrog.infiltrator

import android.content.Context

/** Compile-only stand-in for Android Service entry points used by CoreHost. */
internal object MihomoVpnService {
    fun startAndAwaitRunning(
        context: Context,
        deadline: VpnOwnerRequestDeadline,
    ): VpnStartResult = VpnStartResult.Failed("fixture")

    fun stopAndAwaitStopped(
        context: Context,
        target: VpnStopTarget,
        deadline: VpnOwnerRequestDeadline,
    ): VpnStopResult = VpnStopResult.Failed("fixture")

    fun setPendingConfiguration(
        context: Context,
        configJson: String,
    ): VpnConfigurationApplyOutcomeWire = VpnConfigurationApplyOutcomeWire.OwnerUnavailable
}
