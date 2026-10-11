package com.musicfrog.infiltrator

import android.content.Context
import android.content.Intent
import infiltrator_android.AndroidControlIntent

/** Android-only notification that the visible Activity must launch VPN consent. */
internal object VpnAuthorizationBroadcast {
    const val ACTION = "com.musicfrog.infiltrator.REQUEST_VPN_AUTHORIZATION"

    fun requestForExplicitStart(context: Context) {
        context.sendBroadcast(
            Intent(ACTION)
                .setPackage(context.packageName)
                .putExtra(AndroidControlIntentWire.EXTRA, AndroidControlIntent.START_PROXY),
        )
    }
}
