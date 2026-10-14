package com.musicfrog.infiltrator

import infiltrator_android.AndroidVpnRoutingActivationLease
import infiltrator_android.AndroidVpnRoutingActivationReleaseResult
import infiltrator_android.FfiErrorCode
import infiltrator_android.FfiStatus
import org.junit.Assert.assertEquals
import org.junit.Test

class VpnRoutingLeaseStateTest {
    private val firstLease = AndroidVpnRoutingActivationLease(7uL, 12uL)
    private val otherLease = AndroidVpnRoutingActivationLease(8uL, 13uL)

    @Test
    fun successfulReleaseClearsTheHeldLease() {
        assertEquals(
            VpnRoutingLeaseState.Released,
            VpnRoutingLeaseState.Held(firstLease).afterReleaseAttempt(
                firstLease,
                AndroidVpnRoutingActivationReleaseResult.Released,
            ),
        )
    }

    @Test
    fun everyTypedReleaseFailureRetainsTheExactLeaseForRetry() {
        val outcomes = listOf(
            AndroidVpnRoutingActivationReleaseResult.LeaseMismatch to
                VpnRoutingLeaseReleaseProblem.LeaseMismatch,
            AndroidVpnRoutingActivationReleaseResult.LeaseMissing to
                VpnRoutingLeaseReleaseProblem.LeaseMissing,
            AndroidVpnRoutingActivationReleaseResult.Failed(
                testFfiStatus(FfiErrorCode.NOT_READY, "owner unavailable"),
            ) to VpnRoutingLeaseReleaseProblem.OwnerFailure("owner unavailable"),
        )

        outcomes.forEach { (outcome, problem) ->
            val pending = VpnRoutingLeaseState.Held(firstLease).afterReleaseAttempt(firstLease, outcome)
            assertEquals(
                VpnRoutingLeaseState.ReleasePending(firstLease, problem),
                pending,
            )
            assertEquals(VpnRoutingLeaseCleanupAction.RetryRelease, pending.cleanupAction())
        }
    }

    @Test
    fun aLaterSuccessfulRetryClearsTheRetainedLease() {
        val pending = VpnRoutingLeaseState.Held(firstLease).afterReleaseFailure(
            firstLease,
            "Binder temporarily unavailable",
        )

        assertEquals(
            VpnRoutingLeaseState.Released,
            pending.afterReleaseAttempt(firstLease, AndroidVpnRoutingActivationReleaseResult.Released),
        )
    }

    @Test
    fun repeatedReleaseResultsCannotResurrectAnAlreadyReleasedLease() {
        val released = VpnRoutingLeaseState.Held(firstLease).afterReleaseAttempt(
            firstLease,
            AndroidVpnRoutingActivationReleaseResult.Released,
        )

        assertEquals(
            VpnRoutingLeaseState.Released,
            released.afterReleaseAttempt(
                firstLease,
                AndroidVpnRoutingActivationReleaseResult.LeaseMissing,
            ),
        )
        assertEquals(
            VpnRoutingLeaseState.Released,
            released.afterReleaseFailure(firstLease, "late duplicate release failure"),
        )
        assertEquals(VpnRoutingLeaseCleanupAction.NotRequired, released.cleanupAction())
    }

    @Test
    fun heldAndReleasePendingLeasesRequestCleanupRetry() {
        assertEquals(
            VpnRoutingLeaseCleanupAction.RetryRelease,
            VpnRoutingLeaseState.Held(firstLease).cleanupAction(),
        )
        assertEquals(
            VpnRoutingLeaseCleanupAction.RetryRelease,
            VpnRoutingLeaseState.ReleasePending(
                firstLease,
                VpnRoutingLeaseReleaseProblem.LeaseMissing,
            ).cleanupAction(),
        )
    }

    @Test
    fun staleReleaseCannotDiscardAnotherLease() {
        val current = VpnRoutingLeaseState.Held(otherLease)

        assertEquals(
            current,
            current.afterReleaseAttempt(firstLease, AndroidVpnRoutingActivationReleaseResult.Released),
        )
        assertEquals(current, current.afterReleaseFailure(firstLease, "stale release failure"))
    }
}
