package com.musicfrog.infiltrator

import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.ServiceConnection
import android.os.IBinder
import android.util.Log

/**
 * BANDROID-001/003: shared binding seam between a UI host (Compose or the Bevy
 * host Activity) and the `:vpn` process service.
 *
 * The service is declared with `android:process=":vpn"`, so a UI process binds
 * to observe the connection while the service process owns the TUN, the core
 * lifecycle and the credentials. The binder is intentionally *not* the
 * application protocol: commands/results/snapshots travel through the Rust
 * application boundary (see docs/android/BEVY_ANDROID_PRODUCT.md §2). This type
 * owns only the connection lifecycle and exposes typed connection state; it is
 * not a cross-process state store.
 */
class VpnServiceBinding(
    private val context: Context,
    private val listener: Listener,
) {
    enum class State {
        /** No bind attempt is active. */
        UNBOUND,

        /** A bind was requested and the connection is not established yet. */
        BINDING,

        /** The service connection is established. */
        CONNECTED,

        /** The bind failed or the service died. */
        DISCONNECTED,
    }

    interface Listener {
        fun onBindingStateChanged(state: State)
    }

    private var bound = false
    private var bindAttempted = false

    private val connection = object : ServiceConnection {
        override fun onServiceConnected(name: ComponentName?, service: IBinder?) {
            bound = true
            listener.onBindingStateChanged(State.CONNECTED)
        }

        override fun onServiceDisconnected(name: ComponentName?) {
            bound = false
            listener.onBindingStateChanged(State.DISCONNECTED)
        }
    }

    /** Request the service binding. Returns `false` when the bind call fails. */
    fun bind(): Boolean {
        if (bindAttempted) {
            return bound
        }
        val intent = Intent(context, MihomoVpnService::class.java)
        val accepted = try {
            context.bindService(intent, connection, Context.BIND_AUTO_CREATE)
        } catch (error: Exception) {
            Log.w(TAG, "bindService failed: ${error.message}")
            false
        }
        bindAttempted = accepted
        listener.onBindingStateChanged(if (accepted) State.BINDING else State.DISCONNECTED)
        return accepted
    }

    /** Release the service binding. Safe to call when no bind is active. */
    fun unbind() {
        if (!bindAttempted) {
            return
        }
        try {
            context.unbindService(connection)
        } catch (error: Exception) {
            Log.w(TAG, "unbindService failed: ${error.message}")
        }
        bound = false
        bindAttempted = false
        listener.onBindingStateChanged(State.UNBOUND)
    }

    private companion object {
        private const val TAG = "VpnServiceBinding"
    }
}
