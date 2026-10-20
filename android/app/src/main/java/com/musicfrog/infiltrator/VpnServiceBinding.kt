package com.musicfrog.infiltrator

import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.ServiceConnection
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.util.Log

/**
 * BANDROID-001/003: shared binding seam between a UI host (Compose or the Bevy
 * host Activity) and the `:vpn` control service.
 *
 * The service is declared with `android:process=":vpn"`, so a UI process binds
 * to observe the connection while the service process owns the Mihomo process
 * and TUN. Messenger requests carry Rust-generated enums; business command
 * sequencing and snapshots remain in the shared Rust application. This type
 * owns only the connection lifecycle and exposes typed connection state.
 */
class VpnServiceBinding(private val context: Context, private val listener: Listener) {
    enum class State {
        /** No bind attempt is active. */
        UNBOUND,

        /** A bind was requested and the connection is not established yet. */
        BINDING,

        /** The service connection is established. */
        CONNECTED,

        /** The bound process disconnected; Android may reconnect this binding. */
        DISCONNECTED,

        /** Android reported that the process binding died and must be released. */
        RECOVERING,

        /** Android supplied no binder; the failed registration is being released. */
        NULL_BINDING,

        /** A release is in progress; the registration remains owned. */
        RELEASE_PENDING,

        /** A release failed; retry is required before another bind is admitted. */
        RELEASE_FAILED,

        /** The last bind call failed, or a missing-binder registration was released. */
        FAILED,
    }

    interface Listener {
        fun onBindingStateChanged(state: State)
    }

    private data class ActiveRegistration(val registrationId: VpnServiceBindingRegistrationId, val connection: ServiceConnection)

    private data class ScheduledCleanup(val runnable: Runnable)

    private val handler = Handler(Looper.getMainLooper())
    private val lifecycle = VpnServiceBindingState()
    private var activeRegistration: ActiveRegistration? = null
    private var scheduledCleanup: ScheduledCleanup? = null

    /** Request the service binding and return its explicit lifecycle state. */
    fun bind(): State {
        val registrationId = when (val reservation = lifecycle.reserveBind()) {
            is VpnServiceBindReservation.Reserved -> reservation.registrationId
            is VpnServiceBindReservation.Denied -> return reservation.state
        }
        val connection = serviceConnection(registrationId)
        activeRegistration = ActiveRegistration(registrationId, connection)
        val intent = Intent(context, MihomoControlService::class.java)
        val attempt = try {
            when (context.bindService(intent, connection, Context.BIND_AUTO_CREATE)) {
                true -> VpnServiceBindAttempt.Accepted
                false -> VpnServiceBindAttempt.Rejected
            }
        } catch (error: Exception) {
            Log.w(TAG, "bindService failed: ${error.message}")
            VpnServiceBindAttempt.Rejected
        }
        val transition = lifecycle.bindResult(registrationId, attempt)
        if (attempt == VpnServiceBindAttempt.Rejected) {
            activeRegistration = null
        }
        notifyIfApplied(transition)
        return lifecycle.state()
    }

    /**
     * Retry a failed cleanup using its original typed intent. A successful
     * binding-death cleanup starts the next registration; other intents finish
     * in their corresponding terminal state.
     */
    fun retryCleanup(): State {
        removeScheduledCleanup()
        return when (val transition = lifecycle.retryCleanup()) {
            is VpnServiceCleanupTransition.Execute -> performCleanup(transition)
            is VpnServiceCleanupTransition.Ignored -> transition.state
        }
    }

    /** Release the service binding. A failed release remains retryable. */
    fun unbind() {
        removeScheduledCleanup()
        val registration = activeRegistration
        if (registration == null) {
            notifyIfApplied(lifecycle.explicitUnbindWithoutRegistration())
            return
        }
        when (
            val transition = lifecycle.beginCleanup(
                registration.registrationId,
                VpnServiceBindingCleanupIntent.ExplicitUnbind,
            )
        ) {
            is VpnServiceCleanupTransition.Execute -> performCleanup(transition)
            is VpnServiceCleanupTransition.Ignored -> Unit
        }
    }

    private fun serviceConnection(registrationId: VpnServiceBindingRegistrationId): ServiceConnection = object : ServiceConnection {
        override fun onServiceConnected(name: ComponentName?, service: IBinder?) {
            val binder = when (service) {
                null -> VpnServiceBinderAvailability.Missing
                else -> VpnServiceBinderAvailability.Present
            }
            val transition = lifecycle.serviceConnected(registrationId, binder)
            notifyIfApplied(transition)
            if (binder == VpnServiceBinderAvailability.Missing && transition is VpnServiceBindingTransition.Applied) {
                scheduleCleanup(registrationId, VpnServiceBindingCleanupIntent.FinishMissingBinder)
            }
        }

        override fun onServiceDisconnected(name: ComponentName?) {
            notifyIfApplied(lifecycle.serviceDisconnected(registrationId))
        }

        override fun onBindingDied(name: ComponentName?) {
            when (val transition = lifecycle.bindingDied(registrationId)) {
                is VpnServiceBindingTransition.Applied -> {
                    listener.onBindingStateChanged(transition.state)
                    scheduleCleanup(registrationId, VpnServiceBindingCleanupIntent.RebindAfterDeath)
                }

                is VpnServiceBindingTransition.Ignored -> Unit
            }
        }

        override fun onNullBinding(name: ComponentName?) {
            when (val transition = lifecycle.nullBinding(registrationId)) {
                is VpnServiceBindingTransition.Applied -> {
                    listener.onBindingStateChanged(transition.state)
                    scheduleCleanup(registrationId, VpnServiceBindingCleanupIntent.FinishMissingBinder)
                }

                is VpnServiceBindingTransition.Ignored -> Unit
            }
        }
    }

    private fun scheduleCleanup(registrationId: VpnServiceBindingRegistrationId, intent: VpnServiceBindingCleanupIntent) {
        removeScheduledCleanup()
        val runnable = object : Runnable {
            override fun run() {
                if (scheduledCleanup?.runnable !== this) return
                scheduledCleanup = null
                when (val transition = lifecycle.beginCleanup(registrationId, intent)) {
                    is VpnServiceCleanupTransition.Execute -> performCleanup(transition)
                    is VpnServiceCleanupTransition.Ignored -> Unit
                }
            }
        }
        val cleanup = ScheduledCleanup(runnable)
        scheduledCleanup = cleanup
        handler.post(cleanup.runnable)
    }

    private fun removeScheduledCleanup() {
        scheduledCleanup?.let { handler.removeCallbacks(it.runnable) }
        scheduledCleanup = null
    }

    private fun performCleanup(transition: VpnServiceCleanupTransition.Execute): State {
        listener.onBindingStateChanged(transition.state)
        val outcome = releaseRegistration(transition.registrationId)
        if (outcome != VpnServiceReleaseOutcome.Failed) {
            activeRegistration = when (activeRegistration?.registrationId) {
                transition.registrationId -> null
                else -> activeRegistration
            }
        }
        val finish = lifecycle.finishCleanup(transition.registrationId, transition.intent, outcome)
        notifyIfApplied(finish)
        if (finish is VpnServiceBindingTransition.Applied &&
            outcome != VpnServiceReleaseOutcome.Failed &&
            transition.intent == VpnServiceBindingCleanupIntent.RebindAfterDeath
        ) {
            return bind()
        }
        return lifecycle.state()
    }

    private fun releaseRegistration(registrationId: VpnServiceBindingRegistrationId): VpnServiceReleaseOutcome {
        val registration = activeRegistration
        if (registration?.registrationId != registrationId) {
            Log.w(TAG, "unbindService has no matching registration for generation=${registrationId.generation}")
            return VpnServiceReleaseOutcome.Failed
        }
        return try {
            context.unbindService(registration.connection)
            VpnServiceReleaseOutcome.Released
        } catch (error: IllegalArgumentException) {
            // Android reports an unregistered ServiceConnection this way; the
            // local registration is already released and can be discarded.
            Log.i(TAG, "unbindService registration was already released: ${error.message}")
            VpnServiceReleaseOutcome.AlreadyReleased
        } catch (error: Exception) {
            Log.w(TAG, "unbindService failed: ${error.message}")
            VpnServiceReleaseOutcome.Failed
        }
    }

    private fun notifyIfApplied(transition: VpnServiceBindingTransition) {
        when (transition) {
            is VpnServiceBindingTransition.Applied -> listener.onBindingStateChanged(transition.state)
            is VpnServiceBindingTransition.Ignored -> Unit
        }
    }

    private companion object {
        private const val TAG = "VpnServiceBinding"
    }
}

internal object AndroidVpnServiceBindingLog : VpnServiceBinding.Listener {
    override fun onBindingStateChanged(state: VpnServiceBinding.State) {
        Log.i("VpnServiceBinding", "state=$state")
    }
}
