package com.musicfrog.infiltrator

import infiltrator_android.AndroidVpnConfigurationActivationLease
import infiltrator_android.AndroidVpnConfigurationActivationReleaseResult
import infiltrator_android.AndroidVpnConfigurationActivationResult
import java.io.IOException

internal sealed interface VpnConfigurationActivationAdmission {
    data class Admitted(val lease: AndroidVpnConfigurationActivationLease) : VpnConfigurationActivationAdmission

    data class StaleOwnerData(val expectedRevision: VpnOwnerDataRevision, val observedRevision: VpnOwnerDataRevision) :
        VpnConfigurationActivationAdmission

    data class PreparedRevisionMismatch(val preparedRevision: VpnOwnerDataRevision, val requestedRevision: VpnOwnerDataRevision) :
        VpnConfigurationActivationAdmission

    data object Busy : VpnConfigurationActivationAdmission
    data object Missing : VpnConfigurationActivationAdmission
    data object NotReady : VpnConfigurationActivationAdmission
    data object TransactionMismatch : VpnConfigurationActivationAdmission
    data object StaleGeneration : VpnConfigurationActivationAdmission
    data class Failed(val message: String) : VpnConfigurationActivationAdmission
}

internal fun AndroidVpnConfigurationActivationResult.toAdmission(): VpnConfigurationActivationAdmission = when (this) {
    is AndroidVpnConfigurationActivationResult.Acquired ->
        VpnConfigurationActivationAdmission.Admitted(lease)

    is AndroidVpnConfigurationActivationResult.StaleOwnerData -> {
        val expected = VpnOwnerDataRevision.fromWire(expectedRevision)
        val observed = VpnOwnerDataRevision.fromWire(observedRevision)
        if (expected == null || observed == null) {
            VpnConfigurationActivationAdmission.Failed(
                "VPN owner returned an invalid configuration revision",
            )
        } else {
            VpnConfigurationActivationAdmission.StaleOwnerData(expected, observed)
        }
    }

    is AndroidVpnConfigurationActivationResult.PreparedRevisionMismatch -> {
        val prepared = VpnOwnerDataRevision.fromWire(preparedRevision)
        val requested = VpnOwnerDataRevision.fromWire(requestedRevision)
        if (prepared == null || requested == null) {
            VpnConfigurationActivationAdmission.Failed(
                "VPN owner returned an invalid prepared configuration revision",
            )
        } else {
            VpnConfigurationActivationAdmission.PreparedRevisionMismatch(prepared, requested)
        }
    }

    AndroidVpnConfigurationActivationResult.Busy -> VpnConfigurationActivationAdmission.Busy

    AndroidVpnConfigurationActivationResult.Missing -> VpnConfigurationActivationAdmission.Missing

    AndroidVpnConfigurationActivationResult.NotReady -> VpnConfigurationActivationAdmission.NotReady

    AndroidVpnConfigurationActivationResult.TransactionMismatch ->
        VpnConfigurationActivationAdmission.TransactionMismatch

    AndroidVpnConfigurationActivationResult.StaleGeneration ->
        VpnConfigurationActivationAdmission.StaleGeneration

    is AndroidVpnConfigurationActivationResult.Failed ->
        VpnConfigurationActivationAdmission.Failed(
            status.message ?: "VPN configuration activation admission failed",
        )
}

internal sealed interface VpnConfigurationActivationLeaseState {
    data class Held(val lease: AndroidVpnConfigurationActivationLease) : VpnConfigurationActivationLeaseState

    data class ReleasePending(val lease: AndroidVpnConfigurationActivationLease, val problem: VpnConfigurationActivationReleaseProblem) :
        VpnConfigurationActivationLeaseState

    data object Released : VpnConfigurationActivationLeaseState
}

internal sealed interface VpnConfigurationActivationReleaseProblem {
    data object LeaseMissing : VpnConfigurationActivationReleaseProblem
    data object LeaseMismatch : VpnConfigurationActivationReleaseProblem
    data class OwnerFailure(val message: String) : VpnConfigurationActivationReleaseProblem
}

internal sealed interface VpnConfigurationActivationCleanupResult {
    data object Cleared : VpnConfigurationActivationCleanupResult
    data class Failed(val message: String) : VpnConfigurationActivationCleanupResult
}

internal sealed interface VpnPreparedConfigurationLeaseMatch {
    data object Matched : VpnPreparedConfigurationLeaseMatch
    data object TransactionMismatch : VpnPreparedConfigurationLeaseMatch
    data object StaleGeneration : VpnPreparedConfigurationLeaseMatch
    data object InvalidLeaseRevision : VpnPreparedConfigurationLeaseMatch
    data class RevisionMismatch(val preparedRevision: VpnOwnerDataRevision, val leaseRevision: VpnOwnerDataRevision) :
        VpnPreparedConfigurationLeaseMatch
}

internal fun NativePreparedVpnConfiguration.matchActivationLease(
    lease: AndroidVpnConfigurationActivationLease,
): VpnPreparedConfigurationLeaseMatch = when {
    lease.preparationId != preparationId -> VpnPreparedConfigurationLeaseMatch.TransactionMismatch

    lease.vpnGeneration != vpnGeneration -> VpnPreparedConfigurationLeaseMatch.StaleGeneration

    else -> when (val leaseRevision = VpnOwnerDataRevision.fromWire(lease.ownerDataRevision)) {
        null -> VpnPreparedConfigurationLeaseMatch.InvalidLeaseRevision
        ownerDataRevision -> VpnPreparedConfigurationLeaseMatch.Matched
        else -> VpnPreparedConfigurationLeaseMatch.RevisionMismatch(ownerDataRevision, leaseRevision)
    }
}

internal fun VpnConfigurationActivationLeaseState.afterReleaseAttempt(
    lease: AndroidVpnConfigurationActivationLease,
    result: AndroidVpnConfigurationActivationReleaseResult,
): VpnConfigurationActivationLeaseState = when (this) {
    VpnConfigurationActivationLeaseState.Released -> this

    is VpnConfigurationActivationLeaseState.Held -> when (this.lease) {
        lease -> result.toLeaseState(lease)
        else -> this
    }

    is VpnConfigurationActivationLeaseState.ReleasePending -> when (this.lease) {
        lease -> result.toLeaseState(lease)
        else -> this
    }
}

private fun AndroidVpnConfigurationActivationReleaseResult.toLeaseState(
    lease: AndroidVpnConfigurationActivationLease,
): VpnConfigurationActivationLeaseState = when (this) {
    AndroidVpnConfigurationActivationReleaseResult.RELEASED_BEFORE_CONSUME,
    AndroidVpnConfigurationActivationReleaseResult.RELEASED_AFTER_CONSUME,
    ->
        VpnConfigurationActivationLeaseState.Released

    AndroidVpnConfigurationActivationReleaseResult.LEASE_MISSING ->
        VpnConfigurationActivationLeaseState.ReleasePending(
            lease,
            VpnConfigurationActivationReleaseProblem.LeaseMissing,
        )

    AndroidVpnConfigurationActivationReleaseResult.LEASE_MISMATCH ->
        VpnConfigurationActivationLeaseState.ReleasePending(
            lease,
            VpnConfigurationActivationReleaseProblem.LeaseMismatch,
        )
}

internal fun VpnConfigurationActivationLeaseState.afterReleaseFailure(
    lease: AndroidVpnConfigurationActivationLease,
    message: String,
): VpnConfigurationActivationLeaseState = when (this) {
    VpnConfigurationActivationLeaseState.Released -> this

    is VpnConfigurationActivationLeaseState.Held -> when (this.lease) {
        lease -> VpnConfigurationActivationLeaseState.ReleasePending(
            lease,
            VpnConfigurationActivationReleaseProblem.OwnerFailure(message),
        )

        else -> this
    }

    is VpnConfigurationActivationLeaseState.ReleasePending -> when (this.lease) {
        lease -> VpnConfigurationActivationLeaseState.ReleasePending(
            lease,
            VpnConfigurationActivationReleaseProblem.OwnerFailure(message),
        )

        else -> this
    }
}

internal fun VpnConfigurationActivationLeaseState.afterCancellation(
    result: VpnConfigurationActivationCleanupResult,
): VpnConfigurationActivationLeaseState = when (this) {
    VpnConfigurationActivationLeaseState.Released -> this

    is VpnConfigurationActivationLeaseState.Held -> when (result) {
        VpnConfigurationActivationCleanupResult.Cleared -> VpnConfigurationActivationLeaseState.Released

        is VpnConfigurationActivationCleanupResult.Failed -> VpnConfigurationActivationLeaseState.ReleasePending(
            lease,
            VpnConfigurationActivationReleaseProblem.OwnerFailure(result.message),
        )
    }

    is VpnConfigurationActivationLeaseState.ReleasePending -> when (result) {
        VpnConfigurationActivationCleanupResult.Cleared -> VpnConfigurationActivationLeaseState.Released

        is VpnConfigurationActivationCleanupResult.Failed -> VpnConfigurationActivationLeaseState.ReleasePending(
            lease,
            VpnConfigurationActivationReleaseProblem.OwnerFailure(result.message),
        )
    }
}

internal inline fun <T> withVpnConfigurationActivation(
    prepared: NativePreparedVpnConfiguration,
    admission: VpnConfigurationActivationAdmission,
    release: (AndroidVpnConfigurationActivationLease) -> Unit,
    execute: (NativeVpnConfiguration, AndroidVpnConfigurationActivationLease) -> T,
): T = when (admission) {
    is VpnConfigurationActivationAdmission.Admitted -> try {
        when (val leaseMatch = prepared.matchActivationLease(admission.lease)) {
            VpnPreparedConfigurationLeaseMatch.Matched ->
                execute(prepared.configuration, admission.lease)

            VpnPreparedConfigurationLeaseMatch.TransactionMismatch -> throw IOException(
                "VPN activation lease does not match the frozen preparation transaction",
            )

            VpnPreparedConfigurationLeaseMatch.StaleGeneration -> throw IOException(
                "VPN activation lease belongs to a different VPN generation",
            )

            VpnPreparedConfigurationLeaseMatch.InvalidLeaseRevision -> throw IOException(
                "VPN activation lease returned an invalid owner-data revision",
            )

            is VpnPreparedConfigurationLeaseMatch.RevisionMismatch -> throw IOException(
                "VPN activation lease does not match the frozen owner-data revision " +
                    "(${prepared.ownerDataRevision.value} != ${leaseMatch.leaseRevision.value})",
            )
        }
    } finally {
        release(admission.lease)
    }

    is VpnConfigurationActivationAdmission.StaleOwnerData -> throw IOException(
        "Prepared VPN owner data is stale (expected ${admission.expectedRevision.value}, observed ${admission.observedRevision.value})",
    )

    is VpnConfigurationActivationAdmission.PreparedRevisionMismatch -> throw IOException(
        "Prepared VPN revision does not match activation request " +
            "(${admission.preparedRevision.value} != ${admission.requestedRevision.value})",
    )

    VpnConfigurationActivationAdmission.Busy -> throw IOException(
        "Another VPN configuration activation is in progress",
    )

    VpnConfigurationActivationAdmission.Missing -> throw IOException(
        "Prepared VPN configuration is no longer owned by this service",
    )

    VpnConfigurationActivationAdmission.NotReady -> throw IOException(
        "Prepared VPN configuration has not completed Builder readback",
    )

    VpnConfigurationActivationAdmission.TransactionMismatch -> throw IOException(
        "VPN activation request does not match the prepared transaction",
    )

    VpnConfigurationActivationAdmission.StaleGeneration -> throw IOException(
        "Prepared VPN configuration belongs to an expired generation",
    )

    is VpnConfigurationActivationAdmission.Failed -> throw IOException(admission.message)
}
