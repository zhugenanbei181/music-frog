package com.musicfrog.infiltrator

internal data class VpnServiceBindingRegistrationId(val generation: Long)

internal enum class VpnServiceBindAttempt {
    Accepted,
    Rejected,
}

internal enum class VpnServiceBinderAvailability {
    Present,
    Missing,
}

internal enum class VpnServiceReleaseOutcome {
    Released,
    AlreadyReleased,
    Failed,
}

internal enum class VpnServiceBindingCleanupIntent {
    RebindAfterDeath,
    FinishMissingBinder,
    ExplicitUnbind,
}

internal enum class VpnServiceBindAdmission {
    Allowed,
    AlreadyActive,
    CleanupInProgress,
}

internal sealed interface VpnServiceBindReservation {
    data class Reserved(val registrationId: VpnServiceBindingRegistrationId) : VpnServiceBindReservation

    data class Denied(
        val admission: VpnServiceBindAdmission,
        val state: VpnServiceBinding.State,
    ) : VpnServiceBindReservation
}

internal sealed interface VpnServiceBindingTransition {
    data class Applied(val state: VpnServiceBinding.State) : VpnServiceBindingTransition
    data class Ignored(val state: VpnServiceBinding.State) : VpnServiceBindingTransition
}

internal sealed interface VpnServiceCleanupTransition {
    data class Execute(
        val registrationId: VpnServiceBindingRegistrationId,
        val intent: VpnServiceBindingCleanupIntent,
        val state: VpnServiceBinding.State,
    ) : VpnServiceCleanupTransition

    data class Ignored(val state: VpnServiceBinding.State) : VpnServiceCleanupTransition
}

/**
 * Typed lifecycle for an Android service binding. Every callback is fenced by
 * the registration generation that created its ServiceConnection.
 */
internal class VpnServiceBindingState {
    private sealed interface Phase {
        data object Unbound : Phase
        data object Failed : Phase
        data class Binding(val registrationId: VpnServiceBindingRegistrationId) : Phase
        data class Connected(val registrationId: VpnServiceBindingRegistrationId) : Phase
        data class Disconnected(val registrationId: VpnServiceBindingRegistrationId) : Phase
        data class Recovering(val registrationId: VpnServiceBindingRegistrationId) : Phase
        data class MissingBinder(val registrationId: VpnServiceBindingRegistrationId) : Phase
        data class Releasing(
            val registrationId: VpnServiceBindingRegistrationId,
            val intent: VpnServiceBindingCleanupIntent,
        ) : Phase
        data class ReleaseFailed(
            val registrationId: VpnServiceBindingRegistrationId,
            val intent: VpnServiceBindingCleanupIntent,
        ) : Phase
    }

    private var phase: Phase = Phase.Unbound
    private var lastGeneration = 0L

    fun state(): VpnServiceBinding.State = when (phase) {
        Phase.Unbound -> VpnServiceBinding.State.UNBOUND
        Phase.Failed -> VpnServiceBinding.State.FAILED
        is Phase.Binding -> VpnServiceBinding.State.BINDING
        is Phase.Connected -> VpnServiceBinding.State.CONNECTED
        is Phase.Disconnected -> VpnServiceBinding.State.DISCONNECTED
        is Phase.Recovering -> VpnServiceBinding.State.RECOVERING
        is Phase.MissingBinder -> VpnServiceBinding.State.NULL_BINDING
        is Phase.Releasing -> VpnServiceBinding.State.RELEASE_PENDING
        is Phase.ReleaseFailed -> VpnServiceBinding.State.RELEASE_FAILED
    }

    fun reserveBind(): VpnServiceBindReservation = when (bindAdmission()) {
        VpnServiceBindAdmission.Allowed -> {
            lastGeneration += 1L
            val registrationId = VpnServiceBindingRegistrationId(lastGeneration)
            phase = Phase.Binding(registrationId)
            VpnServiceBindReservation.Reserved(registrationId)
        }
        VpnServiceBindAdmission.AlreadyActive,
        VpnServiceBindAdmission.CleanupInProgress -> VpnServiceBindReservation.Denied(
            bindAdmission(),
            state(),
        )
    }

    fun bindAdmission(): VpnServiceBindAdmission = when (phase) {
        Phase.Unbound,
        Phase.Failed -> VpnServiceBindAdmission.Allowed
        is Phase.Binding,
        is Phase.Connected,
        is Phase.Disconnected -> VpnServiceBindAdmission.AlreadyActive
        is Phase.Recovering,
        is Phase.MissingBinder,
        is Phase.Releasing,
        is Phase.ReleaseFailed -> VpnServiceBindAdmission.CleanupInProgress
    }

    fun bindResult(
        registrationId: VpnServiceBindingRegistrationId,
        attempt: VpnServiceBindAttempt,
    ): VpnServiceBindingTransition = when (val current = phase) {
        is Phase.Binding -> when {
            current.registrationId != registrationId -> ignored()
            attempt == VpnServiceBindAttempt.Accepted -> applied()
            else -> {
                phase = Phase.Failed
                applied()
            }
        }
        else -> ignored()
    }

    fun serviceConnected(
        registrationId: VpnServiceBindingRegistrationId,
        binder: VpnServiceBinderAvailability,
    ): VpnServiceBindingTransition = when (val current = phase) {
        is Phase.Binding -> when {
            current.registrationId != registrationId -> ignored()
            binder == VpnServiceBinderAvailability.Present -> {
                phase = Phase.Connected(registrationId)
                applied()
            }
            else -> missingBinder(registrationId)
        }
        is Phase.Connected -> when {
            current.registrationId != registrationId -> ignored()
            binder == VpnServiceBinderAvailability.Present -> ignored()
            else -> missingBinder(registrationId)
        }
        is Phase.Disconnected -> when {
            current.registrationId != registrationId -> ignored()
            binder == VpnServiceBinderAvailability.Present -> {
                phase = Phase.Connected(registrationId)
                applied()
            }
            else -> missingBinder(registrationId)
        }
        else -> ignored()
    }

    fun serviceDisconnected(
        registrationId: VpnServiceBindingRegistrationId,
    ): VpnServiceBindingTransition = when (val current = phase) {
        is Phase.Binding -> when (current.registrationId == registrationId) {
            true -> {
                phase = Phase.Disconnected(registrationId)
                applied()
            }
            false -> ignored()
        }
        is Phase.Connected -> when (current.registrationId == registrationId) {
            true -> {
                phase = Phase.Disconnected(registrationId)
                applied()
            }
            false -> ignored()
        }
        else -> ignored()
    }

    fun bindingDied(
        registrationId: VpnServiceBindingRegistrationId,
    ): VpnServiceBindingTransition = when (val current = phase) {
        is Phase.Binding -> when (current.registrationId == registrationId) {
            true -> {
                phase = Phase.Recovering(registrationId)
                applied()
            }
            false -> ignored()
        }
        is Phase.Connected -> when (current.registrationId == registrationId) {
            true -> {
                phase = Phase.Recovering(registrationId)
                applied()
            }
            false -> ignored()
        }
        is Phase.Disconnected -> when (current.registrationId == registrationId) {
            true -> {
                phase = Phase.Recovering(registrationId)
                applied()
            }
            false -> ignored()
        }
        else -> ignored()
    }

    fun nullBinding(
        registrationId: VpnServiceBindingRegistrationId,
    ): VpnServiceBindingTransition = when (val current = phase) {
        is Phase.Binding -> when (current.registrationId == registrationId) {
            true -> missingBinder(registrationId)
            false -> ignored()
        }
        is Phase.Connected -> when (current.registrationId == registrationId) {
            true -> missingBinder(registrationId)
            false -> ignored()
        }
        is Phase.Disconnected -> when (current.registrationId == registrationId) {
            true -> missingBinder(registrationId)
            false -> ignored()
        }
        else -> ignored()
    }

    fun beginCleanup(
        registrationId: VpnServiceBindingRegistrationId,
        intent: VpnServiceBindingCleanupIntent,
    ): VpnServiceCleanupTransition = when (val current = phase) {
        is Phase.Recovering -> when {
            current.registrationId == registrationId && intent == VpnServiceBindingCleanupIntent.RebindAfterDeath ->
                beginRelease(registrationId, intent)
            current.registrationId == registrationId && intent == VpnServiceBindingCleanupIntent.ExplicitUnbind ->
                beginRelease(registrationId, intent)
            else -> cleanupIgnored()
        }
        is Phase.MissingBinder -> when {
            current.registrationId == registrationId && intent == VpnServiceBindingCleanupIntent.FinishMissingBinder ->
                beginRelease(registrationId, intent)
            current.registrationId == registrationId && intent == VpnServiceBindingCleanupIntent.ExplicitUnbind ->
                beginRelease(registrationId, intent)
            else -> cleanupIgnored()
        }
        is Phase.Binding -> when (current.registrationId == registrationId && intent == VpnServiceBindingCleanupIntent.ExplicitUnbind) {
            true -> beginRelease(registrationId, intent)
            false -> cleanupIgnored()
        }
        is Phase.Connected -> when (current.registrationId == registrationId && intent == VpnServiceBindingCleanupIntent.ExplicitUnbind) {
            true -> beginRelease(registrationId, intent)
            false -> cleanupIgnored()
        }
        is Phase.Disconnected -> when (current.registrationId == registrationId && intent == VpnServiceBindingCleanupIntent.ExplicitUnbind) {
            true -> beginRelease(registrationId, intent)
            false -> cleanupIgnored()
        }
        is Phase.Releasing -> when (current.registrationId == registrationId && intent == VpnServiceBindingCleanupIntent.ExplicitUnbind) {
            true -> beginRelease(registrationId, intent)
            false -> cleanupIgnored()
        }
        is Phase.ReleaseFailed -> when (current.registrationId == registrationId && intent == VpnServiceBindingCleanupIntent.ExplicitUnbind) {
            true -> beginRelease(registrationId, intent)
            false -> cleanupIgnored()
        }
        Phase.Unbound,
        Phase.Failed -> cleanupIgnored()
    }

    fun retryCleanup(): VpnServiceCleanupTransition = when (val current = phase) {
        is Phase.ReleaseFailed -> beginRelease(current.registrationId, current.intent)
        else -> cleanupIgnored()
    }

    fun finishCleanup(
        registrationId: VpnServiceBindingRegistrationId,
        intent: VpnServiceBindingCleanupIntent,
        outcome: VpnServiceReleaseOutcome,
    ): VpnServiceBindingTransition = when (val current = phase) {
        is Phase.Releasing -> when {
            current.registrationId != registrationId || current.intent != intent -> ignored()
            outcome == VpnServiceReleaseOutcome.Failed -> {
                phase = Phase.ReleaseFailed(registrationId, intent)
                applied()
            }
            else -> {
                phase = when (intent) {
                    VpnServiceBindingCleanupIntent.RebindAfterDeath,
                    VpnServiceBindingCleanupIntent.ExplicitUnbind -> Phase.Unbound
                    VpnServiceBindingCleanupIntent.FinishMissingBinder -> Phase.Failed
                }
                applied()
            }
        }
        else -> ignored()
    }

    fun explicitUnbindWithoutRegistration(): VpnServiceBindingTransition = when (phase) {
        Phase.Unbound -> ignored()
        Phase.Failed -> {
            phase = Phase.Unbound
            applied()
        }
        else -> ignored()
    }

    fun hasActiveRegistration(): Boolean = when (phase) {
        is Phase.Binding,
        is Phase.Connected,
        is Phase.Disconnected,
        is Phase.Recovering,
        is Phase.MissingBinder,
        is Phase.Releasing,
        is Phase.ReleaseFailed -> true
        Phase.Unbound,
        Phase.Failed -> false
    }

    private fun missingBinder(registrationId: VpnServiceBindingRegistrationId): VpnServiceBindingTransition {
        phase = Phase.MissingBinder(registrationId)
        return applied()
    }

    private fun beginRelease(
        registrationId: VpnServiceBindingRegistrationId,
        intent: VpnServiceBindingCleanupIntent,
    ): VpnServiceCleanupTransition {
        phase = Phase.Releasing(registrationId, intent)
        return VpnServiceCleanupTransition.Execute(registrationId, intent, state())
    }

    private fun applied(): VpnServiceBindingTransition = VpnServiceBindingTransition.Applied(state())

    private fun ignored(): VpnServiceBindingTransition = VpnServiceBindingTransition.Ignored(state())

    private fun cleanupIgnored(): VpnServiceCleanupTransition = VpnServiceCleanupTransition.Ignored(state())
}
