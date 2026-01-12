package com.musicfrog.despicableinfiltrator

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Intent
import android.net.VpnService
import android.os.Build
import android.os.ParcelFileDescriptor
import android.util.Log
import androidx.core.app.NotificationCompat
import infiltrator_android.FfiErrorCode
import infiltrator_android.VpnTunSettings
import infiltrator_android.startVpn
import infiltrator_android.stopVpn as stopTun2Proxy
import infiltrator_android.vpnTunSettings
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.runBlocking

class MihomoVpnService : VpnService() {
    private var vpnInterface: ParcelFileDescriptor? = null
    private val TAG = "MihomoVpnService"
    private val CHANNEL_ID = "vpn_service_channel"
    private val NOTIFICATION_ID = 1
    private val PREFS_NAME = "app_routing"
    private val PREF_SELECTED_PACKAGES = "selected_packages"
    private val PREF_ROUTING_MODE = "routing_mode"
    private val ROUTING_MODE_PROXY_SELECTED = "proxy_selected"
    private val DEFAULT_MTU = 1500
    private val IPV6_ADDRESS = "fd00:fd00:fd00::1"

    override fun onCreate() {
        super.onCreate()
        createNotificationChannel()
        VpnStateManager.register(this)
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val action = intent?.action
        if (action == ACTION_STOP) {
            VpnStateManager.onVpnStopping()
            VpnStateManager.broadcastState(this, VpnStateManager.VpnState.STOPPING)
            stopVpn()
            return START_NOT_STICKY
        }

        VpnStateManager.onVpnStarting()
        VpnStateManager.broadcastState(this, VpnStateManager.VpnState.STARTING)
        startForeground(NOTIFICATION_ID, createNotification())
        establishVpn()
        return START_STICKY
    }

    override fun onDestroy() {
        super.onDestroy()
        stopVpn()
    }

    private fun establishVpn() {
        if (vpnInterface != null) {
            Log.w(TAG, "VPN already running")
            VpnStateManager.onVpnStarted()
            VpnStateManager.broadcastState(this, VpnStateManager.VpnState.RUNNING)
            return
        }

        try {
            val builder = Builder()
            val settings = loadTunSettings()
            val mtu = resolveMtu(settings)
            builder.setMtu(mtu)
            builder.addAddress("172.19.0.1", 30)

            val autoRoute = settings?.autoRoute ?: true
            val strictRoute = settings?.strictRoute ?: false
            val ipv6Enabled = settings?.ipv6 ?: false
            if (autoRoute) {
                builder.addRoute("0.0.0.0", 0)
                if (ipv6Enabled) {
                    builder.addAddress(IPV6_ADDRESS, 126)
                    builder.addRoute("::", 0)
                }
            } else if (strictRoute) {
                Log.i(TAG, "strict-route enabled without auto-route; routing is limited")
            }

            val dnsServers = filterDnsServers(settings?.dnsServers ?: emptyList())
            if (dnsServers.isNotEmpty()) {
                for (server in dnsServers) {
                    try {
                        builder.addDnsServer(server)
                    } catch (e: Exception) {
                        Log.w(TAG, "Failed to add DNS server: $server", e)
                    }
                }
            }

            // On Android 10+, we might want to set a metered status or HTTP proxy
            // builder.setMetered(false)

            builder.setSession("MusicFrog Infiltrator")

            // Apply Per-App Routing
            val prefs = getSharedPreferences(PREFS_NAME, android.content.Context.MODE_PRIVATE)
            val routingMode = prefs.getString(PREF_ROUTING_MODE, null)
            val selectedPackages = prefs.getStringSet(PREF_SELECTED_PACKAGES, emptySet()) ?: emptySet()

            if (routingMode == ROUTING_MODE_PROXY_SELECTED) {
                if (selectedPackages.isNotEmpty()) {
                    Log.i(TAG, "Applying VPN rules for ${selectedPackages.size} apps")
                    for (pkg in selectedPackages) {
                        try {
                            builder.addAllowedApplication(pkg)
                        } catch (e: Exception) {
                            Log.w(TAG, "Failed to allow app: $pkg", e)
                        }
                    }
                } else {
                    Log.i(TAG, "Proxy selected enabled but no apps selected, fallback to proxy all")
                }
            } else {
                Log.i(TAG, "Proxy all mode enabled")
            }
            
            // Create the interface
            vpnInterface = builder.establish()

            if (vpnInterface != null) {
                val fd = vpnInterface!!.fd
                Log.i(TAG, "VPN established, FD: $fd. Passing to Rust...")
                
                // Pass the File Descriptor to Rust
                // Rust will take it from here (or just log it for now)
                startVpn(fd)
                
                VpnStateManager.onVpnStarted()
                VpnStateManager.broadcastState(this, VpnStateManager.VpnState.RUNNING)
            } else {
                Log.e(TAG, "Failed to establish VPN interface (null result)")
                VpnStateManager.onVpnError("Failed to establish VPN interface")
                VpnStateManager.broadcastState(this, VpnStateManager.VpnState.ERROR, "Failed to establish VPN interface")
                stopSelf()
            }
        } catch (e: Exception) {
            Log.e(TAG, "Error establishing VPN", e)
            VpnStateManager.onVpnError(e.message ?: "Unknown error")
            VpnStateManager.broadcastState(this, VpnStateManager.VpnState.ERROR, e.message)
            stopSelf()
        }
    }

    private fun stopVpn() {
        try {
            val status = stopTun2Proxy()
            if (status.code != FfiErrorCode.OK) {
                Log.w(TAG, "tun2proxy stop failed: ${status.message}")
            }
            vpnInterface?.close()
            vpnInterface = null
            stopForeground(STOP_FOREGROUND_REMOVE)
            Log.i(TAG, "VPN stopped")
            
            VpnStateManager.onVpnStopped()
            VpnStateManager.broadcastState(this, VpnStateManager.VpnState.STOPPED)
        } catch (e: Exception) {
            Log.e(TAG, "Error stopping VPN", e)
            VpnStateManager.onVpnError(e.message ?: "Error stopping VPN")
        }
    }

    private fun loadTunSettings(): VpnTunSettings? {
        return try {
            runBlocking(Dispatchers.IO) {
                val result = vpnTunSettings()
                if (result.status.code == FfiErrorCode.OK) {
                    result.settings
                } else {
                    Log.w(TAG, "load tun settings failed: ${result.status.message}")
                    null
                }
            }
        } catch (e: Exception) {
            Log.w(TAG, "load tun settings failed", e)
            null
        }
    }

    private fun resolveMtu(settings: VpnTunSettings?): Int {
        val mtu = settings?.mtu?.toLong()
        if (mtu != null && mtu > 0 && mtu <= Int.MAX_VALUE.toLong()) {
            return mtu.toInt()
        }
        return DEFAULT_MTU
    }

    private fun filterDnsServers(servers: List<String>): List<String> {
        return servers.mapNotNull { raw ->
            val value = raw.trim()
            if (value.isEmpty() || value.contains("://")) {
                return@mapNotNull null
            }
            if (isIpv4(value) || isIpv6(value)) value else null
        }
    }

    private fun isIpv4(value: String): Boolean {
        val parts = value.split(".")
        if (parts.size != 4) {
            return false
        }
        for (part in parts) {
            val octet = part.toIntOrNull() ?: return false
            if (octet < 0 || octet > 255) {
                return false
            }
        }
        return true
    }

    private fun isIpv6(value: String): Boolean {
        if (!value.contains(":")) {
            return false
        }
        for (ch in value) {
            val ok = ch == ':' || (ch in '0'..'9') || (ch in 'a'..'f') || (ch in 'A'..'F')
            if (!ok) {
                return false
            }
        }
        return true
    }

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val serviceChannel = NotificationChannel(
                CHANNEL_ID,
                "VPN Service",
                NotificationManager.IMPORTANCE_DEFAULT
            )
            val manager = getSystemService(NotificationManager::class.java)
            manager.createNotificationChannel(serviceChannel)
        }
    }

    private fun createNotification(): Notification {
        val stopIntent = Intent(this, MihomoVpnService::class.java)
        stopIntent.action = ACTION_STOP
        val pendingStopIntent = PendingIntent.getService(
            this, 0, stopIntent, PendingIntent.FLAG_IMMUTABLE
        )

        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle("Infiltrator VPN Running")
            .setContentText("Traffic is being protected")
            .setSmallIcon(R.drawable.ic_launcher_foreground)
            .addAction(android.R.drawable.ic_menu_close_clear_cancel, "Stop", pendingStopIntent)
            .setOngoing(true)
            .build()
    }

    companion object {
        const val ACTION_START = "com.musicfrog.despicableinfiltrator.START_VPN"
        const val ACTION_STOP = "com.musicfrog.despicableinfiltrator.STOP_VPN"

        fun start(context: android.content.Context): Boolean {
            val intent = Intent(context, MihomoVpnService::class.java)
            intent.action = ACTION_START
            try {
                VpnStateManager.onVpnStarting()
                if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.O) {
                    context.startForegroundService(intent)
                } else {
                    context.startService(intent)
                }
                return true
            } catch (e: Exception) {
                android.util.Log.e("MihomoVpnService", "Failed to start VPN service", e)
                VpnStateManager.onVpnError(e.message ?: "Failed to start VPN service")
                return false
            }
        }

        fun stop(context: android.content.Context): Boolean {
            val intent = Intent(context, MihomoVpnService::class.java)
            intent.action = ACTION_STOP
            try {
                VpnStateManager.onVpnStopping()
                context.startService(intent)
                return true
            } catch (e: Exception) {
                VpnStateManager.onVpnError(e.message ?: "Failed to stop VPN service")
                return false
            }
        }

        fun isRunning(): Boolean {
            return VpnStateManager.vpnState.value == VpnStateManager.VpnState.RUNNING
        }
    }
}
