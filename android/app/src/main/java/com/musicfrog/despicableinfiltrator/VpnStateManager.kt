package com.musicfrog.despicableinfiltrator

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.net.VpnService
import android.util.Log
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * Centralized VPN state management using StateFlow.
 * Provides reactive state updates for UI components.
 */
object VpnStateManager {
    private const val TAG = "VpnStateManager"

    /**
     * VPN running states
     */
    enum class VpnState {
        /** VPN is stopped */
        STOPPED,
        /** VPN is starting */
        STARTING,
        /** VPN is running */
        RUNNING,
        /** VPN is stopping */
        STOPPING,
        /** VPN encountered an error */
        ERROR
    }

    /**
     * Permission states
     */
    enum class PermissionState {
        /** Permission not yet checked */
        UNKNOWN,
        /** VPN permission granted */
        GRANTED,
        /** VPN permission denied or revoked */
        DENIED
    }

    private val _vpnState = MutableStateFlow(VpnState.STOPPED)
    val vpnState: StateFlow<VpnState> = _vpnState.asStateFlow()

    private val _permissionState = MutableStateFlow(PermissionState.UNKNOWN)
    val permissionState: StateFlow<PermissionState> = _permissionState.asStateFlow()

    private val _coreState = MutableStateFlow(false)
    val coreRunning: StateFlow<Boolean> = _coreState.asStateFlow()

    private val _errorMessage = MutableStateFlow<String?>(null)
    val errorMessage: StateFlow<String?> = _errorMessage.asStateFlow()

    private var receiver: VpnStateReceiver? = null

    /**
     * Check and update VPN permission state
     */
    fun checkPermission(context: Context): Boolean {
        val intent = VpnService.prepare(context)
        val granted = intent == null
        _permissionState.value = if (granted) PermissionState.GRANTED else PermissionState.DENIED
        return granted
    }

    /**
     * Update permission state after user grants permission
     */
    fun onPermissionGranted() {
        _permissionState.value = PermissionState.GRANTED
    }

    /**
     * Update permission state after user denies permission
     */
    fun onPermissionDenied() {
        _permissionState.value = PermissionState.DENIED
    }

    /**
     * Called when VPN service is starting
     */
    fun onVpnStarting() {
        _vpnState.value = VpnState.STARTING
        _errorMessage.value = null
    }

    /**
     * Called when VPN service has started successfully
     */
    fun onVpnStarted() {
        _vpnState.value = VpnState.RUNNING
        _errorMessage.value = null
    }

    /**
     * Called when VPN service is stopping
     */
    fun onVpnStopping() {
        _vpnState.value = VpnState.STOPPING
    }

    /**
     * Called when VPN service has stopped
     */
    fun onVpnStopped() {
        _vpnState.value = VpnState.STOPPED
    }

    /**
     * Called when VPN service encounters an error
     */
    fun onVpnError(message: String) {
        _vpnState.value = VpnState.ERROR
        _errorMessage.value = message
        Log.e(TAG, "VPN error: $message")
    }

    /**
     * Update core running state
     */
    fun updateCoreState(running: Boolean) {
        _coreState.value = running
    }

    /**
     * Clear error message
     */
    fun clearError() {
        _errorMessage.value = null
        if (_vpnState.value == VpnState.ERROR) {
            _vpnState.value = VpnState.STOPPED
        }
    }

    /**
     * Register broadcast receiver for VPN state changes
     */
    fun register(context: Context) {
        if (receiver != null) return
        receiver = VpnStateReceiver()
        val filter = IntentFilter().apply {
            addAction(ACTION_VPN_STATE_CHANGED)
        }
        context.applicationContext.registerReceiver(receiver, filter, Context.RECEIVER_NOT_EXPORTED)
        Log.d(TAG, "VPN state receiver registered")
    }

    /**
     * Unregister broadcast receiver
     */
    fun unregister(context: Context) {
        receiver?.let {
            try {
                context.applicationContext.unregisterReceiver(it)
            } catch (e: Exception) {
                Log.w(TAG, "Failed to unregister receiver: ${e.message}")
            }
            receiver = null
        }
    }

    /**
     * Broadcast VPN state change
     */
    fun broadcastState(context: Context, state: VpnState, errorMessage: String? = null) {
        val intent = Intent(ACTION_VPN_STATE_CHANGED).apply {
            putExtra(EXTRA_VPN_STATE, state.name)
            errorMessage?.let { putExtra(EXTRA_ERROR_MESSAGE, it) }
            setPackage(context.packageName)
        }
        context.sendBroadcast(intent)
    }

    private class VpnStateReceiver : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            if (intent.action != ACTION_VPN_STATE_CHANGED) return
            
            val stateName = intent.getStringExtra(EXTRA_VPN_STATE) ?: return
            val state = try {
                VpnState.valueOf(stateName)
            } catch (e: Exception) {
                Log.w(TAG, "Unknown VPN state: $stateName")
                return
            }
            
            when (state) {
                VpnState.STARTING -> onVpnStarting()
                VpnState.RUNNING -> onVpnStarted()
                VpnState.STOPPING -> onVpnStopping()
                VpnState.STOPPED -> onVpnStopped()
                VpnState.ERROR -> {
                    val error = intent.getStringExtra(EXTRA_ERROR_MESSAGE) ?: "Unknown error"
                    onVpnError(error)
                }
            }
        }
    }

    // Intent action and extras
    const val ACTION_VPN_STATE_CHANGED = "com.musicfrog.despicableinfiltrator.VPN_STATE_CHANGED"
    const val EXTRA_VPN_STATE = "vpn_state"
    const val EXTRA_ERROR_MESSAGE = "error_message"
}
