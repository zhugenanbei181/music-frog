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
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import java.io.IOException

class MihomoVpnService : VpnService() {
    private var vpnInterface: ParcelFileDescriptor? = null
    private val TAG = "MihomoVpnService"
    private val CHANNEL_ID = "vpn_service_channel"
    private val NOTIFICATION_ID = 1
    private val PREFS_NAME = "app_routing"
    private val PREF_SELECTED_PACKAGES = "selected_packages"
    private val PREF_ROUTING_MODE = "routing_mode"
    private val ROUTING_MODE_PROXY_ALL = "proxy_all"
    private val ROUTING_MODE_PROXY_SELECTED = "proxy_selected"
    private val ROUTING_MODE_BYPASS_SELECTED = "bypass_selected"
    private val DEFAULT_MTU = 1500
    private val IPV6_ADDRESS = "fd00:fd00:fd00::1"

    private val serviceScope = CoroutineScope(Dispatchers.Main + Job())
    private var trafficJob: Job? = null

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
        startForeground(NOTIFICATION_ID, createNotification(getString(R.string.notification_starting)))
        establishVpn()
        startTrafficMonitoring()
        return START_STICKY
    }

    private fun startTrafficMonitoring() {
        trafficJob?.cancel()
        trafficJob = serviceScope.launch {
            while (true) {
                delay(2000)
                if (vpnInterface != null) {
                    val notification = createNotification(getString(R.string.notification_active))
                    val notificationManager = getSystemService(NotificationManager::class.java)
                    notificationManager.notify(NOTIFICATION_ID, notification)
                }
            }
        }
    }

    override fun onDestroy() {
        super.onDestroy()
        stopVpn()
    }

    private fun establishVpn() {
        if (vpnInterface != null) {
            Log.w(TAG, "VPN already running")
            VpnStateManager.onVpnStarted(this)
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

            builder.setSession("MusicFrog Infiltrator")

            // Apply Per-App Routing
            val prefs = getSharedPreferences(PREFS_NAME, android.content.Context.MODE_PRIVATE)
            val routingMode = prefs.getString(PREF_ROUTING_MODE, ROUTING_MODE_PROXY_ALL)
            val selectedPackages = prefs.getStringSet(PREF_SELECTED_PACKAGES, emptySet()) ?: emptySet()

            Log.i(TAG, "Routing mode: ${'$'}routingMode, apps: ${'$'}{selectedPackages.size}")

            when (routingMode) {
                ROUTING_MODE_PROXY_SELECTED -> {
                    if (selectedPackages.isNotEmpty()) {
                        for (pkg in selectedPackages) {
                            try {
                                builder.addAllowedApplication(pkg)
                            } catch (e: Exception) {
                                Log.w(TAG, "Failed to allow app: ${'$'}pkg", e)
                            }
                        }
                    }
                }
                ROUTING_MODE_BYPASS_SELECTED -> {
                    if (selectedPackages.isNotEmpty()) {
                        for (pkg in selectedPackages) {
                            try {
                                builder.addDisallowedApplication(pkg)
                            } catch (e: Exception) {
                                Log.w(TAG, "Failed to disallow app: ${'$'}pkg", e)
                            }
                        }
                    }
                }
                else -> Log.i(TAG, "Proxy All mode active")
            }
            
            vpnInterface = builder.establish()

            if (vpnInterface != null) {
                val fd = vpnInterface!!.fd
                startVpn(fd)
                VpnStateManager.onVpnStarted(this)
                VpnStateManager.broadcastState(this, VpnStateManager.VpnState.RUNNING)
            } else {
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
        trafficJob?.cancel()
        try {
            val status = stopTun2Proxy()
            if (status.code != FfiErrorCode.OK) {
                Log.w(TAG, "tun2proxy stop failed: ${status.message}")
            }
            vpnInterface?.close()
            vpnInterface = null
            stopForeground(STOP_FOREGROUND_REMOVE)
            
            VpnStateManager.onVpnStopped(this)
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
                    null
                }
            }
        } catch (e: Exception) {
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
        if (parts.size != 4) return false
        return parts.all { it.toIntOrNull() in 0..255 }
    }

    private fun isIpv6(value: String): Boolean {
        if (!value.contains(":")) return false
        return value.all { it == ':' || it in '0'..'9' || it in 'a'..'f' || it in 'A'..'F' }
    }

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val serviceChannel = NotificationChannel(
                CHANNEL_ID,
                getString(R.string.notification_channel_name),
                NotificationManager.IMPORTANCE_LOW // Low priority to avoid annoying sounds
            )
            val manager = getSystemService(NotificationManager::class.java)
            manager.createNotificationChannel(serviceChannel)
        }
    }

    private fun createNotification(statusText: String): Notification {
        val stopIntent = Intent(this, MihomoVpnService::class.java).apply {
            action = ACTION_STOP
        }
        val pendingStopIntent = PendingIntent.getService(
            this, 0, stopIntent, PendingIntent.FLAG_IMMUTABLE
        )

        val mainIntent = Intent(this, MainActivity::class.java)
        val pendingMainIntent = PendingIntent.getActivity(
            this, 0, mainIntent, PendingIntent.FLAG_IMMUTABLE
        )

        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle(getString(R.string.notification_title))
            .setContentText(statusText)
            .setSmallIcon(R.drawable.ic_app_icon)
            .setContentIntent(pendingMainIntent)
            .addAction(android.R.drawable.ic_menu_close_clear_cancel, getString(R.string.action_stop), pendingStopIntent)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .build()
    }

    companion object {
        const val ACTION_START = "com.musicfrog.despicableinfiltrator.START_VPN"
        const val ACTION_STOP = "com.musicfrog.despicableinfiltrator.STOP_VPN"

        fun start(context: android.content.Context): Boolean {
            val intent = Intent(context, MihomoVpnService::class.java).apply {
                action = ACTION_START
            }
            try {
                VpnStateManager.onVpnStarting()
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                    context.startForegroundService(intent)
                } else {
                    context.startService(intent)
                }
                return true
            } catch (e: Exception) {
                VpnStateManager.onVpnError(e.message ?: "Failed to start VPN")
                return false
            }
        }

        fun stop(context: android.content.Context): Boolean {
            val intent = Intent(context, MihomoVpnService::class.java).apply {
                action = ACTION_STOP
            }
            try {
                VpnStateManager.onVpnStopping()
                context.startService(intent)
                return true
            } catch (e: Exception) {
                VpnStateManager.onVpnError(e.message ?: "Failed to stop VPN")
                return false
            }
        }
    }
}