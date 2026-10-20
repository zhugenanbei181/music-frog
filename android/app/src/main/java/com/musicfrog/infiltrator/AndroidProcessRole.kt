package com.musicfrog.infiltrator

import android.app.Application
import android.content.Context

/** The only Android processes allowed to host product work. */
internal sealed interface AndroidProcessRole {
    data object Ui : AndroidProcessRole

    data object VpnOwner : AndroidProcessRole

    data class Unsupported(val processName: String) : AndroidProcessRole

    companion object {
        private const val VPN_PROCESS_SUFFIX = ":vpn"

        fun current(context: Context): AndroidProcessRole = identify(context.packageName, Application.getProcessName())

        fun identify(packageName: String, processName: String): AndroidProcessRole = when (processName) {
            packageName -> Ui
            "$packageName$VPN_PROCESS_SUFFIX" -> VpnOwner
            else -> Unsupported(processName)
        }
    }
}

internal fun AndroidProcessRole.toWireCode(): Int = when (this) {
    AndroidProcessRole.Ui -> 0
    AndroidProcessRole.VpnOwner -> 1
    is AndroidProcessRole.Unsupported -> 2
}
