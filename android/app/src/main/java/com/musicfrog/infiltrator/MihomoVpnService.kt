package com.musicfrog.infiltrator

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Intent
import android.net.IpPrefix
import android.net.VpnService
import android.os.Build
import android.os.ParcelFileDescriptor
import android.util.Log
import androidx.core.app.NotificationCompat
import infiltrator_android.AppRoutingMode
import infiltrator_android.FfiErrorCode
import infiltrator_android.appRoutingLoad
import infiltrator_android.prepareVpn
import infiltrator_android.revokeVpn
import infiltrator_android.startVpn
import infiltrator_android.stopVpn as stopTun2Proxy
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import org.json.JSONObject
import java.net.InetAddress
import java.io.IOException

private data class NativeVpnRoute(
    val address: String,
    val prefix: Int,
    val exclude: Boolean,
)

private data class NativeVpnConfiguration(
    val proxyEndpoint: String,
    val mtu: Int,
    val routes: List<NativeVpnRoute>,
    val dnsServers: List<String>,
    val ipv6: Boolean,
    val foregroundRequested: Boolean,
) {
    companion object {
        fun parse(raw: String): NativeVpnConfiguration? {
            val json = try {
                JSONObject(raw)
            } catch (_: Exception) {
                return null
            }
            val proxyEndpoint = json.optString("proxy_endpoint", "").trim()
            val mtu = json.optInt("mtu", -1)
            val routesJson = json.optJSONArray("routes") ?: return null
            val routes = mutableListOf<NativeVpnRoute>()
            for (index in 0 until routesJson.length()) {
                val route = routesJson.optJSONObject(index) ?: return null
                val address = route.optString("address", "").trim()
                val prefix = route.optInt("prefix", -1)
                if (address.isEmpty() || prefix !in 0..128) return null
                routes += NativeVpnRoute(
                    address = address,
                    prefix = prefix,
                    exclude = route.optBoolean("exclude", false),
                )
            }
            val dnsServersJson = json.optJSONArray("dns_servers")
            val dnsServers = mutableListOf<String>()
            if (dnsServersJson != null) {
                for (index in 0 until dnsServersJson.length()) {
                    val server = dnsServersJson.optString(index, "").trim()
                    if (server.isEmpty()) return null
                    dnsServers += server
                }
            }
            if (
                proxyEndpoint.isEmpty() ||
                mtu !in 1280..9000 ||
                routes.isEmpty() ||
                !json.optBoolean("foreground_requested", false)
            ) {
                return null
            }
            return NativeVpnConfiguration(
                proxyEndpoint = proxyEndpoint,
                mtu = mtu,
                routes = routes,
                dnsServers = dnsServers,
                ipv6 = json.optBoolean("ipv6", true),
                foregroundRequested = true,
            )
        }
    }
}

class MihomoVpnService : VpnService() {
    private var vpnInterface: ParcelFileDescriptor? = null
    private val TAG = "MihomoVpnService"
    private val CHANNEL_ID = "vpn_service_channel"
    private val NOTIFICATION_ID = 1
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
        foregroundActive = true
        val prepareStatus = prepareVpn()
        if (prepareStatus.code != FfiErrorCode.OK) {
            Log.e(TAG, "Rust VPN configuration preparation failed: ${prepareStatus.message}")
            foregroundActive = false
            stopForeground(STOP_FOREGROUND_REMOVE)
            VpnStateManager.onVpnError(prepareStatus.message ?: "Failed to prepare VPN")
            stopSelf()
            return START_NOT_STICKY
        }
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
        VpnStateManager.onVpnStopping()
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
            val configuration = takePendingConfiguration()
                ?: throw IOException("Rust VPN configuration was not prepared")
            val builder = Builder()
            builder.setMtu(configuration.mtu)
            builder.addAddress("172.19.0.1", 30)

            if (configuration.ipv6) {
                builder.addAddress(IPV6_ADDRESS, 126)
            }
            for (route in configuration.routes) {
                if (!configuration.ipv6 && route.address.contains(":")) {
                    continue
                }
                addRoute(builder, route)
            }

            for (server in configuration.dnsServers) {
                builder.addDnsServer(server)
            }

            builder.setSession("MusicFrog Infiltrator")

            // Apply Per-App Routing
            val (routingMode, selectedPackages) = loadRoutingConfig()

            Log.i(TAG, "Routing mode: ${'$'}routingMode, apps: ${'$'}{selectedPackages.size}")

            when (routingMode) {
                AppRoutingMode.PROXY_SELECTED -> {
                    if (selectedPackages.isNotEmpty()) {
                        for (pkg in selectedPackages) {
                            // Prevent adding self to allowlist (would cause loop)
                            if (pkg == packageName) continue
                            try {
                                builder.addAllowedApplication(pkg)
                            } catch (e: Exception) {
                                Log.w(TAG, "Failed to allow app: ${'$'}pkg", e)
                            }
                        }
                    }
                }
                AppRoutingMode.BYPASS_SELECTED -> {
                    // Always exclude self to prevent loop
                    try { builder.addDisallowedApplication(packageName) } catch (e: Exception) { Log.w(TAG, "Failed to exclude self", e) }

                    if (selectedPackages.isNotEmpty()) {
                        for (pkg in selectedPackages) {
                            if (pkg == packageName) continue // Already handled
                            try {
                                builder.addDisallowedApplication(pkg)
                            } catch (e: Exception) {
                                Log.w(TAG, "Failed to disallow app: ${'$'}pkg", e)
                            }
                        }
                    }
                }
                AppRoutingMode.PROXY_ALL -> {
                    Log.i(TAG, "Proxy All mode active")
                    // Always exclude self to prevent loop
                    try {
                        builder.addDisallowedApplication(packageName)
                    } catch (e: Exception) {
                        Log.e(TAG, "Failed to disallow self in Proxy All mode", e)
                    }
                }
            }
            
            vpnInterface = builder.establish()

            if (vpnInterface != null) {
                val fd = vpnInterface!!.fd
                interfaceActive = true
                val status = startVpn(fd)
                if (status.code != FfiErrorCode.OK) {
                    interfaceActive = false
                    vpnInterface?.close()
                    vpnInterface = null
                    throw IOException(status.message ?: "Rust failed to start tun2proxy")
                }
                VpnStateManager.onVpnStarted(this)
                VpnStateManager.broadcastState(this, VpnStateManager.VpnState.RUNNING)
            } else {
                VpnStateManager.onVpnError("Failed to establish VPN interface")
                VpnStateManager.broadcastState(this, VpnStateManager.VpnState.ERROR, "Failed to establish VPN interface")
                stopSelf()
            }
        } catch (e: Exception) {
            Log.e(TAG, "Error establishing VPN", e)
            interfaceActive = false
            vpnInterface?.close()
            vpnInterface = null
            foregroundActive = false
            stopForeground(STOP_FOREGROUND_REMOVE)
            VpnStateManager.onVpnError(e.message ?: "Unknown error")
            VpnStateManager.broadcastState(this, VpnStateManager.VpnState.ERROR, e.message)
            stopSelf()
        }
    }

    @Suppress("NewApi")
    private fun addRoute(builder: Builder, route: NativeVpnRoute) {
        val address = InetAddress.getByName(route.address)
        if (route.exclude) {
            if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) {
                throw IOException("VPN excluded routes require Android API 33 or newer")
            }
            builder.excludeRoute(IpPrefix(address, route.prefix))
        } else {
            builder.addRoute(address, route.prefix)
        }
    }

    private fun stopVpn(revoked: Boolean = false) {
        trafficJob?.cancel()
        interfaceActive = false
        foregroundActive = false
        try {
            val status = if (revoked) revokeVpn() else stopTun2Proxy()
            if (status.code != FfiErrorCode.OK) {
                Log.w(TAG, "Rust VPN stop failed: ${status.message}")
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

    override fun onRevoke() {
        VpnStateManager.onVpnStopping()
        stopVpn(revoked = true)
        super.onRevoke()
    }

    private fun loadRoutingConfig(): Pair<AppRoutingMode, Set<String>> {
        return try {
            val result = appRoutingLoad()
            val config = result.config
            if (result.status.code == FfiErrorCode.OK && config != null) {
                config.mode to config.packages.toSet()
            } else {
                Log.w(
                    TAG,
                    "Failed to load app routing config: ${result.status.code} ${result.status.message.orEmpty()}",
                )
                AppRoutingMode.PROXY_ALL to emptySet()
            }
        } catch (err: Exception) {
            Log.w(TAG, "Failed to load app routing config: ${err.message}", err)
            AppRoutingMode.PROXY_ALL to emptySet()
        }
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
        const val ACTION_START = "com.musicfrog.infiltrator.START_VPN"
        const val ACTION_STOP = "com.musicfrog.infiltrator.STOP_VPN"

        @Volatile
        private var foregroundActive = false

        @Volatile
        private var interfaceActive = false

        @Volatile
        private var pendingConfiguration: NativeVpnConfiguration? = null

        fun isRunning(): Boolean = interfaceActive

        fun isForegroundActive(): Boolean = foregroundActive

        @Synchronized
        fun setPendingConfiguration(configJson: String): Boolean {
            val configuration = NativeVpnConfiguration.parse(configJson) ?: return false
            pendingConfiguration = configuration
            return true
        }

        @Synchronized
        private fun takePendingConfiguration(): NativeVpnConfiguration? {
            val configuration = pendingConfiguration
            pendingConfiguration = null
            return configuration
        }

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
