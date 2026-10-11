package com.musicfrog.infiltrator.ui.overview

import com.musicfrog.infiltrator.AndroidVpnAuthorizationUiState
import com.musicfrog.infiltrator.ui.common.AppFailure
import com.musicfrog.infiltrator.ui.common.AppFailureCode
import com.musicfrog.infiltrator.ui.common.FailureTextKey
import com.musicfrog.infiltrator.ui.common.ObservationRevision
import com.musicfrog.infiltrator.ui.common.OwnerVpnObservation
import com.musicfrog.infiltrator.VpnOwnerCoreReadbackWire
import com.musicfrog.infiltrator.VpnOwnerDataPlaneState
import com.musicfrog.infiltrator.VpnOwnerLifecycleWire
import com.musicfrog.infiltrator.VpnOwnerReadinessWire
import com.musicfrog.infiltrator.VpnOwnerResourceFactWire
import com.musicfrog.infiltrator.VpnOwnerResourceKindWire
import com.musicfrog.infiltrator.VpnOwnerResourceStateWire
import com.musicfrog.infiltrator.VpnOwnerSnapshotWire
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class ObservationStateTest {
    @Test
    fun aFailedRefreshRetainsTheLastObservationAndMarksItUnavailable() {
        val revision = ObservationRevision.Owner(4uL, 12uL)
        val failure = AppFailure(AppFailureCode.NotReady, FailureTextKey.NotReady)
        val previous = ObservationState.Observed("observed value", revision)

        assertEquals(
            ObservationState.Unavailable("observed value", revision, failure),
            unavailableObservation(previous, failure),
        )
    }

    @Test
    fun aFirstReadFailureIsUnavailableInsteadOfRemainingInLoading() {
        val previous = ObservationState.Loading<String>()
        val failure = AppFailure(AppFailureCode.NotReady, FailureTextKey.NotReady)

        assertEquals(
            ObservationState.Unavailable<String>(null, null, failure),
            unavailableObservation(previous, failure),
        )
    }

    @Test
    fun aSuccessfulEmptyReadAndAFailedReadRemainDistinct() {
        val empty: ObservationState<String> = ObservationState.Empty
        val failure = AppFailure(AppFailureCode.NotReady, FailureTextKey.NotReady)
        val unavailable = unavailableObservation(empty, failure)

        assertEquals(ObservationState.Empty, empty)
        assertEquals(ObservationState.Unavailable<String>(null, null, failure), unavailable)
    }

    @Test
    fun anUnavailableOrUnauthorizedVpnReadNeverProjectsAsActive() {
        val running = ObservationState.Observed(ownerObservation(runningOwner()), vpnRevision(runningOwner()))
        val unavailable = ObservationState.Unavailable(
            ownerObservation(runningOwner()),
            vpnRevision(runningOwner()),
            AppFailure(AppFailureCode.NotReady, FailureTextKey.NotReady),
        )

        assertTrue(vpnSessionIsRunning(running, AndroidVpnAuthorizationUiState.Granted))
        assertFalse(vpnSessionIsRunning(running, AndroidVpnAuthorizationUiState.Required))
        assertFalse(vpnSessionIsRunning(unavailable, AndroidVpnAuthorizationUiState.Granted))
    }

    @Test
    fun startIsDisabledForUnknownOrInFlightVpnStates() {
        val unknown = ObservationState.Unavailable<OwnerVpnObservation>(
            lastValue = null,
            revision = null,
            failure = AppFailure(AppFailureCode.NotReady, FailureTextKey.NotReady),
        )
        val startingOwner = ownerSnapshot(
            VpnOwnerLifecycleWire.Starting,
            VpnOwnerReadinessWire.AwaitingForeground,
            resourcesReleased = true,
            foreground = false,
        )
        val starting = ObservationState.Observed(ownerObservation(startingOwner), vpnRevision(startingOwner))
        val stoppedOwner = ownerSnapshot(
            VpnOwnerLifecycleWire.Stopped,
            VpnOwnerReadinessWire.Stopped,
            resourcesReleased = true,
            foreground = false,
        )
        val unavailableAuthorization = AndroidVpnAuthorizationUiState.Unavailable

        assertFalse(canStartVpnSession(unknown, AndroidVpnAuthorizationUiState.Granted))
        assertFalse(
            canStartVpnSession(
                ObservationState.Observed(ownerObservation(stoppedOwner), vpnRevision(stoppedOwner)),
                unavailableAuthorization,
            ),
        )
        assertFalse(canStartVpnSession(starting, AndroidVpnAuthorizationUiState.Required))
        assertTrue(canStartVpnSession(ObservationState.Loading(), AndroidVpnAuthorizationUiState.Required))
        assertTrue(
            canStartVpnSession(
                ObservationState.Observed(ownerObservation(stoppedOwner), vpnRevision(stoppedOwner)),
                AndroidVpnAuthorizationUiState.Granted,
            ),
        )
    }

    @Test
    fun vpnReadinessRequiresTheSameOwnerGenerationWorkerAndTunnelFacts() {
        val valid = runningOwner()
        assertEquals(VpnOwnerDataPlaneState.Running, valid.dataPlaneState())
        val missingTunnel = valid.copy(
            resources = valid.resources.map { fact ->
                if (fact.kind == VpnOwnerResourceKindWire.TunnelDescriptor) {
                    fact.copy(state = VpnOwnerResourceStateWire.Absent)
                } else fact
            },
        )
        assertTrue(missingTunnel.dataPlaneState() is VpnOwnerDataPlaneState.Failed)
        assertFalse(
            canStartVpnSession(
                ObservationState.Observed(ownerObservation(missingTunnel), vpnRevision(missingTunnel)),
                AndroidVpnAuthorizationUiState.Granted,
            ),
        )
    }

    private fun ownerObservation(snapshot: VpnOwnerSnapshotWire): OwnerVpnObservation =
        OwnerVpnObservation.AndroidOwner(snapshot)

    private fun vpnRevision(snapshot: VpnOwnerSnapshotWire) = ObservationRevision.VpnService(
        snapshot.ownerSessionId,
        snapshot.vpnGeneration,
        snapshot.revision,
    )

    private fun runningOwner() = ownerSnapshot(
        VpnOwnerLifecycleWire.Running,
        VpnOwnerReadinessWire.Ready,
        resourcesReleased = false,
        foreground = true,
    )

    private fun ownerSnapshot(
        lifecycle: VpnOwnerLifecycleWire,
        readiness: VpnOwnerReadinessWire,
        resourcesReleased: Boolean,
        foreground: Boolean,
    ) = VpnOwnerSnapshotWire(
        ownerSessionId = "owner-a",
        revision = 3,
        vpnGeneration = 5,
        vpnState = lifecycle,
        readiness = readiness,
        resources = VpnOwnerResourceKindWire.entries.map { kind ->
            VpnOwnerResourceFactWire(
                kind = kind,
                state = when {
                    resourcesReleased -> VpnOwnerResourceStateWire.Absent
                    lifecycle == VpnOwnerLifecycleWire.Running &&
                        kind in setOf(VpnOwnerResourceKindWire.Worker, VpnOwnerResourceKindWire.TunnelDescriptor) ->
                        VpnOwnerResourceStateWire.Held(5)
                    else -> VpnOwnerResourceStateWire.Absent
                },
            )
        },
        foreground = foreground,
        mtu = 1500,
        routeCount = 1,
        dnsServers = listOf("1.1.1.1"),
        ipv6 = false,
        core = VpnOwnerCoreReadbackWire.Unavailable("test Core is not composed"),
        canonicalJson = "{}",
    )
}
