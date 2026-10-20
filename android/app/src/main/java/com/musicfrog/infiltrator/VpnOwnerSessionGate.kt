package com.musicfrog.infiltrator

internal data class VpnOwnerSession<T>(val id: String, val value: T)

internal sealed interface VpnOwnerSessionState<out T> {
    data object Unavailable : VpnOwnerSessionState<Nothing>

    data class Preparing<T>(val id: String, val value: T) : VpnOwnerSessionState<T>

    data class Active<T>(val id: String, val value: T) : VpnOwnerSessionState<T>

    data class Failed<T>(val id: String, val value: T, val reason: String) : VpnOwnerSessionState<T>

    data class Ended(val id: String) : VpnOwnerSessionState<Nothing>
}

internal sealed interface VpnOwnerSessionReadiness {
    data object NotStarted : VpnOwnerSessionReadiness
    data class Preparing(val sessionId: String) : VpnOwnerSessionReadiness
    data class Ready(val sessionId: String) : VpnOwnerSessionReadiness
    data class Failed(val sessionId: String, val reason: String) : VpnOwnerSessionReadiness
    data class Ended(val sessionId: String) : VpnOwnerSessionReadiness
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

        is VpnOwnerSessionState.Failed,
        is VpnOwnerSessionState.Ended,
        VpnOwnerSessionState.Unavailable,
        -> VpnOwnerSessionActivationOutcome.StaleSession
    }

    @Synchronized
    fun fail(id: String, reason: String) {
        when (val current = state) {
            is VpnOwnerSessionState.Preparing -> when (current.id == id) {
                true -> state = VpnOwnerSessionState.Failed(current.id, current.value, reason)
                false -> Unit
            }

            is VpnOwnerSessionState.Active,
            is VpnOwnerSessionState.Failed,
            is VpnOwnerSessionState.Ended,
            VpnOwnerSessionState.Unavailable,
            -> Unit
        }
    }

    @Synchronized
    fun end(id: String) {
        when (val current = state) {
            is VpnOwnerSessionState.Preparing -> when (current.id == id) {
                true -> state = VpnOwnerSessionState.Ended(id)
                false -> Unit
            }

            is VpnOwnerSessionState.Active -> when (current.id == id) {
                true -> state = VpnOwnerSessionState.Ended(id)
                false -> Unit
            }

            is VpnOwnerSessionState.Failed -> when (current.id == id) {
                true -> state = VpnOwnerSessionState.Ended(id)
                false -> Unit
            }

            is VpnOwnerSessionState.Ended,
            VpnOwnerSessionState.Unavailable,
            -> Unit
        }
    }

    @Synchronized
    fun readiness(): VpnOwnerSessionReadiness = when (val current = state) {
        VpnOwnerSessionState.Unavailable -> VpnOwnerSessionReadiness.NotStarted
        is VpnOwnerSessionState.Preparing -> VpnOwnerSessionReadiness.Preparing(current.id)
        is VpnOwnerSessionState.Active -> VpnOwnerSessionReadiness.Ready(current.id)
        is VpnOwnerSessionState.Failed -> VpnOwnerSessionReadiness.Failed(current.id, current.reason)
        is VpnOwnerSessionState.Ended -> VpnOwnerSessionReadiness.Ended(current.id)
    }

    fun snapshotSession(): VpnOwnerSession<T>? = when (val current = state) {
        is VpnOwnerSessionState.Preparing -> VpnOwnerSession(current.id, current.value)

        is VpnOwnerSessionState.Active -> VpnOwnerSession(current.id, current.value)

        is VpnOwnerSessionState.Failed -> VpnOwnerSession(current.id, current.value)

        is VpnOwnerSessionState.Ended,
        VpnOwnerSessionState.Unavailable,
        -> null
    }

    fun activeSession(): VpnOwnerSession<T>? = when (val current = state) {
        is VpnOwnerSessionState.Active -> VpnOwnerSession(current.id, current.value)

        is VpnOwnerSessionState.Preparing,
        is VpnOwnerSessionState.Failed,
        is VpnOwnerSessionState.Ended,
        VpnOwnerSessionState.Unavailable,
        -> null
    }
}
