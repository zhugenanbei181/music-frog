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
import infiltrator_android.startVpn

class MihomoVpnService : VpnService() {
    private var vpnInterface: ParcelFileDescriptor? = null
    private val TAG = "MihomoVpnService"
    private val CHANNEL_ID = "vpn_service_channel"
    private val NOTIFICATION_ID = 1

    override fun onCreate() {
        super.onCreate()
        createNotificationChannel()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val action = intent?.action
        if (action == ACTION_STOP) {
            stopVpn()
            return START_NOT_STICKY
        }

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
            return
        }

        try {
            val builder = Builder()
            
            // Configure the TUN interface
            // These settings should eventually come from Rust configuration
            builder.setMtu(9000)
            builder.addAddress("172.19.0.1", 30)
            builder.addRoute("0.0.0.0", 0)
            
            // On Android 10+, we might want to set a metered status or HTTP proxy
            // builder.setMetered(false)

            builder.setSession("MusicFrog Infiltrator")

            // Apply Per-App Routing
            val prefs = getSharedPreferences("app_routing", android.content.Context.MODE_PRIVATE)
            val selectedPackages = prefs.getStringSet("selected_packages", emptySet()) ?: emptySet()
            
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
                Log.i(TAG, "No apps selected for VPN routing (Proxy All mode or empty)")
                // If you want "Proxy All" by default when nothing is selected, do nothing here.
                // If you want "Block All" until selected, that's a different logic.
                // Assuming "Proxy All" for now if list is empty for convenience, 
                // OR we can force user to select.
                // Let's assume: Empty list = Proxy All (Default VPN behavior).
            }
            
            // Create the interface
            vpnInterface = builder.establish()

            if (vpnInterface != null) {
                val fd = vpnInterface!!.fd
                Log.i(TAG, "VPN established, FD: $fd. Passing to Rust...")
                
                // Pass the File Descriptor to Rust
                // Rust will take it from here (or just log it for now)
                startVpn(fd)
            } else {
                Log.e(TAG, "Failed to establish VPN interface (null result)")
                stopSelf()
            }
        } catch (e: Exception) {
            Log.e(TAG, "Error establishing VPN", e)
            stopSelf()
        }
    }

    private fun stopVpn() {
        try {
            vpnInterface?.close()
            vpnInterface = null
            stopForeground(STOP_FOREGROUND_REMOVE)
            Log.i(TAG, "VPN stopped")
        } catch (e: Exception) {
            Log.e(TAG, "Error stopping VPN", e)
        }
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

        @Volatile
        private var isRunning = false

        fun start(context: android.content.Context): Boolean {
            val intent = Intent(context, MihomoVpnService::class.java)
            intent.action = ACTION_START
            try {
                if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.O) {
                    context.startForegroundService(intent)
                } else {
                    context.startService(intent)
                }
                isRunning = true
                return true
            } catch (e: Exception) {
                android.util.Log.e("MihomoVpnService", "Failed to start VPN service", e)
                return false
            }
        }

        fun stop(context: android.content.Context): Boolean {
            val intent = Intent(context, MihomoVpnService::class.java)
            intent.action = ACTION_STOP
            try {
                context.startService(intent)
                isRunning = false // Optimistic update
                return true
            } catch (e: Exception) {
                return false
            }
        }

        fun isRunning(): Boolean {
            return isRunning
        }
    }
}
