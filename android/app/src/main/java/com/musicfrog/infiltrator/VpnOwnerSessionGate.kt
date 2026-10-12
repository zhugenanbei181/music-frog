package com.musicfrog.infiltrator

internal data class VpnOwnerSession<T>(
    val id: String,
    val value: T,
)

internal sealed interface VpnOwnerSessionState<out T> {
    data object Unavailable : VpnOwnerSessionState<Nothing>

    data class Preparing<T>(
        val id: String,
        val value: T,
    ) : VpnOwnerSessionState<T>

    data class Active<T>(
        val id: String,
        val value: T,
    ) : VpnOwnerSessionState<T>
}

internal enum class VpnOwnerSessionActivationOutcome {
    Activated,
    AlreadyActive,
    StaleSession,
}

/** Keeps VPN snapshots readable during recovery without allowing Core readback recursion. */
internal class VpnOwnerSessionGate<T> {
    @Volatile
    private var state: VpnOwnerSessionState<T> = VpnOwnerSessionState.Unavailable

    @Synchronized
    fun prepare(id: String, value: T) {
        state = VpnOwnerSessionState.Preparing(id, value)
    }

    @Synchronized
    fun activate(id: String): VpnOwnerSessionActivationOutcome = when (val current = state) {
        is VpnOwnerSessionState.Preparing -> when (current.id == id) {
            true -> {
                state = VpnOwnerSessionState.Active(current.id, current.value)
                VpnOwnerSessionActivationOutcome.Activated
            }
            false -> VpnOwnerSessionActivationOutcome.StaleSession
        }
        is VpnOwnerSessionState.Active -> when (current.id == id) {
            true -> VpnOwnerSessionActivationOutcome.AlreadyActive
            false -> VpnOwnerSessionActivationOutcome.StaleSession
        }
        VpnOwnerSessionState.Unavailable -> VpnOwnerSessionActivationOutcome.StaleSession
    }

    @Synchronized
    fun end(id: String) {
        when (val current = state) {
            is VpnOwnerSessionState.Preparing -> when (current.id == id) {
                true -> state = VpnOwnerSessionState.Unavailable
                false -> Unit
            }
            is VpnOwnerSessionState.Active -> when (current.id == id) {
                true -> state = VpnOwnerSessionState.Unavailable
                false -> Unit
            }
            VpnOwnerSessionState.Unavailable -> Unit
        }
    }

    fun snapshotSession(): VpnOwnerSession<T>? = when (val current = state) {
        is VpnOwnerSessionState.Preparing -> VpnOwnerSession(current.id, current.value)
        is VpnOwnerSessionState.Active -> VpnOwnerSession(current.id, current.value)
        VpnOwnerSessionState.Unavailable -> null
    }

    fun activeSession(): VpnOwnerSession<T>? = when (val current = state) {
        is VpnOwnerSessionState.Active -> VpnOwnerSession(current.id, current.value)
        is VpnOwnerSessionState.Preparing,
        VpnOwnerSessionState.Unavailable -> null
    }
}
