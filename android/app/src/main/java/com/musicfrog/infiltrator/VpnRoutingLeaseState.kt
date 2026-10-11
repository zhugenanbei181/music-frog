package com.musicfrog.infiltrator

import infiltrator_android.AndroidVpnRoutingActivationLease
import infiltrator_android.AndroidVpnRoutingActivationReleaseResult

internal sealed interface VpnRoutingLeaseState {
    data object Released : VpnRoutingLeaseState

    data class Held(val lease: AndroidVpnRoutingActivationLease) : VpnRoutingLeaseState

    data class ReleasePending(
        val lease: AndroidVpnRoutingActivationLease,
        val problem: VpnRoutingLeaseReleaseProblem,
    ) : VpnRoutingLeaseState
}

internal sealed interface VpnRoutingLeaseReleaseProblem {
    data object LeaseMismatch : VpnRoutingLeaseReleaseProblem

    data object LeaseMissing : VpnRoutingLeaseReleaseProblem

    data class OwnerFailure(val message: String) : VpnRoutingLeaseReleaseProblem
}

internal enum class VpnRoutingLeaseCleanupAction {
    NotRequired,
    RetryRelease,
}

internal fun VpnRoutingLeaseState.afterReleaseAttempt(
    lease: AndroidVpnRoutingActivationLease,
    result: AndroidVpnRoutingActivationReleaseResult,
): VpnRoutingLeaseState = when (this) {
    VpnRoutingLeaseState.Released -> this
    is VpnRoutingLeaseState.Held -> when (this.lease) {
        lease -> result.toLeaseState(this.lease)
        else -> this
    }
    is VpnRoutingLeaseState.ReleasePending -> when (this.lease) {
        lease -> result.toLeaseState(this.lease)
        else -> this
    }
}

private fun AndroidVpnRoutingActivationReleaseResult.toLeaseState(
    lease: AndroidVpnRoutingActivationLease,
): VpnRoutingLeaseState = when (this) {
    AndroidVpnRoutingActivationReleaseResult.Released -> VpnRoutingLeaseState.Released
    AndroidVpnRoutingActivationReleaseResult.LeaseMismatch -> VpnRoutingLeaseState.ReleasePending(
        lease,
        VpnRoutingLeaseReleaseProblem.LeaseMismatch,
    )
    AndroidVpnRoutingActivationReleaseResult.LeaseMissing -> VpnRoutingLeaseState.ReleasePending(
        lease,
        VpnRoutingLeaseReleaseProblem.LeaseMissing,
    )
    is AndroidVpnRoutingActivationReleaseResult.Failed -> VpnRoutingLeaseState.ReleasePending(
        lease,
        VpnRoutingLeaseReleaseProblem.OwnerFailure(
            status.message ?: "VPN routing activation release failed",
        ),
    )
}

internal fun VpnRoutingLeaseState.pendingLease(): AndroidVpnRoutingActivationLease? = when (this) {
    VpnRoutingLeaseState.Released -> null
    is VpnRoutingLeaseState.Held -> lease
    is VpnRoutingLeaseState.ReleasePending -> lease
}

internal fun VpnRoutingLeaseState.afterReleaseFailure(
    lease: AndroidVpnRoutingActivationLease,
    message: String,
): VpnRoutingLeaseState = when (this) {
    VpnRoutingLeaseState.Released -> this
    is VpnRoutingLeaseState.Held -> when (this.lease) {
        lease -> VpnRoutingLeaseState.ReleasePending(
            this.lease,
            VpnRoutingLeaseReleaseProblem.OwnerFailure(message),
        )
        else -> this
    }
    is VpnRoutingLeaseState.ReleasePending -> when (this.lease) {
        lease -> VpnRoutingLeaseState.ReleasePending(
            this.lease,
            VpnRoutingLeaseReleaseProblem.OwnerFailure(message),
        )
        else -> this
    }
}

internal fun VpnRoutingLeaseState.cleanupAction(): VpnRoutingLeaseCleanupAction = when (this) {
    VpnRoutingLeaseState.Released -> VpnRoutingLeaseCleanupAction.NotRequired
    is VpnRoutingLeaseState.Held,
    is VpnRoutingLeaseState.ReleasePending -> VpnRoutingLeaseCleanupAction.RetryRelease
}

internal fun VpnRoutingLeaseState.releaseFailureMessage(): String? = when (this) {
    VpnRoutingLeaseState.Released -> null
    is VpnRoutingLeaseState.Held -> "VPN routing activation lease has not been released"
    is VpnRoutingLeaseState.ReleasePending -> when (problem) {
        VpnRoutingLeaseReleaseProblem.LeaseMismatch -> "VPN routing activation lease did not match during release"
        VpnRoutingLeaseReleaseProblem.LeaseMissing -> "VPN routing activation lease was missing during release"
        is VpnRoutingLeaseReleaseProblem.OwnerFailure -> problem.message
    }
}
