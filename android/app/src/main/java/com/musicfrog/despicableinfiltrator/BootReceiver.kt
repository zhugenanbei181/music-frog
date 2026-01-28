package com.musicfrog.despicableinfiltrator

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log

class BootReceiver : BroadcastReceiver() {
    private val TAG = "BootReceiver"

    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action == Intent.ACTION_BOOT_COMPLETED) {
            Log.i(TAG, "Device boot completed")
            
            if (VpnStateManager.shouldBeRunning(context)) {
                Log.i(TAG, "VPN was running before shutdown, restarting...")
                if (VpnStateManager.checkPermission(context)) {
                    MihomoVpnService.start(context)
                } else {
                    Log.w(TAG, "Cannot restart VPN: Permission missing")
                }
            } else {
                Log.i(TAG, "VPN was not running, skipping auto-start")
            }
        }
    }
}
