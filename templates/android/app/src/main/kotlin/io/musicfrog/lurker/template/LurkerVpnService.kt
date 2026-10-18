package io.musicfrog.lurker.template

import android.content.Intent
import android.net.VpnService
import android.os.ParcelFileDescriptor

/**
 * Standard VpnService lifecycle topology for Lurker core engines.
 */
class LurkerVpnService : VpnService() {

    private var tunInterface: ParcelFileDescriptor? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == ACTION_START_VPN) {
            startVpnTunnel()
        } else if (intent?.action == ACTION_STOP_VPN) {
            stopVpnTunnel()
        }
        return START_NOT_STICKY
    }

    private fun startVpnTunnel() {
        if (tunInterface != null) return

        val builder = Builder()
            .setSession("LurkerVpn")
            .addAddress("172.19.0.1", 30)
            .addDnsServer("1.1.1.1")
            .addRoute("0.0.0.0", 0)
            .setMtu(1500)

        tunInterface = builder.establish()
    }

    private fun stopVpnTunnel() {
        tunInterface?.close()
        tunInterface = null
        stopSelf()
    }

    override fun onDestroy() {
        stopVpnTunnel()
        super.onDestroy()
    }

    companion object {
        const val ACTION_START_VPN = "io.musicfrog.lurker.template.START_VPN"
        const val ACTION_STOP_VPN = "io.musicfrog.lurker.template.STOP_VPN"
    }
}
