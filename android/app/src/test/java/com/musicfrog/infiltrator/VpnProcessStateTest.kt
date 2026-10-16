package com.musicfrog.infiltrator

import infiltrator_android.AndroidAppRoutingMode
import infiltrator_android.AndroidHostReply
import infiltrator_android.AndroidVpnRoutingActivationLease
import infiltrator_android.AndroidVpnRoutingActivationReleaseResult
import infiltrator_android.FfiErrorCode
import infiltrator_android.FfiStatus
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference

class VpnProcessStateTest {
    private fun reserveAcceptedBinding(binding: VpnServiceBindingState): VpnServiceBindingRegistrationId {
        val reservation = binding.reserveBind() as VpnServiceBindReservation.Reserved
        assertEquals(
            VpnServiceBindingTransition.Applied(VpnServiceBinding.State.BINDING),
            binding.bindResult(reservation.registrationId, VpnServiceBindAttempt.Accepted),
        )
        return reservation.registrationId
    }

    @Test
    fun repeatedStartJoinsTheSameGenerationAndStopRejectsLateStartCompletion() {
        val lifecycle = VpnLifecycleCoordinator()
        val start = lifecycle.requestStart() as VpnStartPlan.Dispatch
        assertEquals(VpnStartPlan.Join(start.generation), lifecycle.requestStart())
        assertEquals(VpnTransitionOutcome.Applied, lifecycle.transition(start.generation, VpnProcessState.Starting))
        assertEquals(VpnTransitionOutcome.Applied, lifecycle.transition(start.generation, VpnProcessState.Foreground))

        val stop = lifecycle.requestCleanup(VpnCleanupIntent.Stop) as VpnCleanupPlan.Dispatch
        assertEquals(
            VpnTransitionOutcome.StaleGeneration,
            lifecycle.transition(start.generation, VpnProcessState.Running),
        )
        assertEquals(
            VpnStartResult.Failed("VPN service stopped before proxy became active"),
            lifecycle.awaitStart(start.generation, 1),
        )
        assertEquals(
            VpnCleanupPublication.Published(VpnProcessState.Stopped),
            completeCleanup(lifecycle, stop.request),
        )
        assertEquals(VpnStopResult.Stopped, lifecycle.awaitStop(stop.request.generation, 1))
    }

    @Test
    fun staleNotificationStopCannotStopANewerVpnGeneration() {
        val lifecycle = VpnLifecycleCoordinator()
        val first = lifecycle.requestStart() as VpnStartPlan.Dispatch
        val firstStop = lifecycle.requestCleanup(VpnCleanupIntent.Stop) as VpnCleanupPlan.Dispatch
        assertEquals(
            VpnCleanupPlan.Join(firstStop.request),
            lifecycle.requestCleanup(VpnCleanupIntent.Stop),
        )
        assertEquals(
            VpnCleanupPublication.Published(VpnProcessState.Stopped),
            completeCleanup(lifecycle, firstStop.request),
        )
        assertEquals(
            VpnCleanupPlan.AlreadyClean(firstStop.request.generation),
            lifecycle.requestCleanup(VpnCleanupIntent.Stop),
        )
        val next = lifecycle.requestStart() as VpnStartPlan.Dispatch

        val nextBeforeOldStop = lifecycle.snapshot()
        assertEquals(
            VpnCleanupPlan.Superseded,
            lifecycle.requestCleanup(VpnCleanupIntent.Stop, VpnStopTarget.Generation(first.generation)),
        )
        assertEquals(nextBeforeOldStop, lifecycle.snapshot())
        assertEquals(next.generation, lifecycle.snapshot().generation)
        assertEquals(VpnProcessState.StartRequested, lifecycle.snapshot().state)
    }

    @Test
    fun stopTargetWireRequiresAnExplicitValidGeneration() {
        assertEquals(VpnStopTarget.Current, VpnStopTarget.fromWire(0, 0L))
        assertEquals(VpnStopTarget.Generation(17L), VpnStopTarget.fromWire(1, 17L))
        assertEquals(null, VpnStopTarget.fromWire(0, 17L))
        assertEquals(null, VpnStopTarget.fromWire(1, 0L))
        assertEquals(null, VpnStopTarget.fromWire(9, 17L))
        assertEquals(listOf(0, 1), VpnStopTargetWire.entries.map { it.wireCode })
    }

    @Test
    fun restartWithoutAnExplicitCommandFailsClosedUntilTheNextUserStart() {
        val lifecycle = VpnLifecycleCoordinator()
        val restored = lifecycle.failUnrequestedRestart(
            "VPN service was recreated without an explicit start command",
        )
        assertEquals(
            VpnProcessState.Failed("VPN service was recreated without an explicit start command"),
            restored.state,
        )
        assertFalse(restored.state == VpnProcessState.Running)

        val retry = lifecycle.requestStart() as VpnStartPlan.Dispatch
        assertTrue(retry.generation > restored.generation)
        assertEquals(VpnProcessState.StartRequested, lifecycle.snapshot().state)
    }

    @Test
    fun proxySelectedWithNoValidPackageFailsClosed() {
        val plan = buildVpnPackageRoutingPlan(
            VpnRoutingConfiguration.Ready(
                AndroidAppRoutingMode.PROXY_SELECTED,
                setOf("com.musicfrog.infiltrator"),
                routingRevision = 7uL,
            ),
            "com.musicfrog.infiltrator",
        )

        assertEquals(
            VpnPackageRoutingPlan.Failed("Select at least one app to use selected-app routing"),
            plan,
        )
    }

    @Test
    fun proxyAllExcludesTheVpnOwnerPackageFromItsOwnTunnel() {
        assertEquals(
            VpnPackageRoutingPlan.Ready(
                listOf(VpnPackageRule.Disallow("com.musicfrog.infiltrator")),
            ),
            buildVpnPackageRoutingPlan(
                VpnRoutingConfiguration.Ready(
                    AndroidAppRoutingMode.PROXY_ALL,
                    emptySet(),
                    routingRevision = 8uL,
                ),
                "com.musicfrog.infiltrator",
            ),
        )
    }

    @Test
    fun appRoutingReadFailureIsNotConvertedToProxyAll() {
        assertEquals(
            VpnPackageRoutingPlan.Failed("routing store unavailable"),
            buildVpnPackageRoutingPlan(
                VpnRoutingConfiguration.Failed("routing store unavailable"),
                "com.musicfrog.infiltrator",
            ),
        )
    }

    @Test
    fun blankAppRoutingPackageFailsInsteadOfDroppingTheEntry() {
        assertEquals(
            VpnPackageRoutingPlan.Failed("VPN app-routing configuration contains an empty package name"),
            buildVpnPackageRoutingPlan(
                VpnRoutingConfiguration.Ready(
                    AndroidAppRoutingMode.BYPASS_SELECTED,
                    setOf("com.example", " "),
                    routingRevision = 7uL,
                ),
                "com.musicfrog.infiltrator",
            ),
        )
    }

    @Test
    fun selectedAndBypassRulesNeverSilentlyDropAnotherSelectedPackage() {
        val selected = buildVpnPackageRoutingPlan(
            VpnRoutingConfiguration.Ready(
                AndroidAppRoutingMode.PROXY_SELECTED,
                setOf("com.zeta", "com.alpha", "com.musicfrog.infiltrator"),
                routingRevision = 7uL,
            ),
            "com.musicfrog.infiltrator",
        )
        assertEquals(
            VpnPackageRoutingPlan.Ready(
                listOf(VpnPackageRule.Allow("com.alpha"), VpnPackageRule.Allow("com.zeta")),
            ),
            selected,
        )

        val bypass = buildVpnPackageRoutingPlan(
            VpnRoutingConfiguration.Ready(
                AndroidAppRoutingMode.BYPASS_SELECTED,
                setOf("com.example"),
                routingRevision = 7uL,
            ),
            "com.musicfrog.infiltrator",
        )
        assertEquals(
            VpnPackageRoutingPlan.Ready(
                listOf(
                    VpnPackageRule.Disallow("com.musicfrog.infiltrator"),
                    VpnPackageRule.Disallow("com.example"),
                ),
            ),
            bypass,
        )
    }

    @Test
    fun deadServiceBindingIsReleasedBeforeRebinding() {
        val binding = VpnServiceBindingState()
        val registration = reserveAcceptedBinding(binding)

        assertEquals(VpnServiceBindAdmission.AlreadyActive, binding.bindAdmission())
        assertEquals(
            VpnServiceBindingTransition.Applied(VpnServiceBinding.State.CONNECTED),
            binding.serviceConnected(registration, VpnServiceBinderAvailability.Present),
        )
        assertEquals(
            VpnServiceBindingTransition.Applied(VpnServiceBinding.State.RECOVERING),
            binding.bindingDied(registration),
        )
        assertEquals(
            VpnServiceBindingTransition.Ignored(VpnServiceBinding.State.RECOVERING),
            binding.serviceConnected(registration, VpnServiceBinderAvailability.Present),
        )
        assertEquals(
            VpnServiceBindingTransition.Ignored(VpnServiceBinding.State.RECOVERING),
            binding.bindingDied(registration),
        )
        assertEquals(VpnServiceBindAdmission.CleanupInProgress, binding.bindAdmission())

        val cleanup = binding.beginCleanup(registration, VpnServiceBindingCleanupIntent.RebindAfterDeath)
        assertEquals(
            VpnServiceCleanupTransition.Execute(
                registration,
                VpnServiceBindingCleanupIntent.RebindAfterDeath,
                VpnServiceBinding.State.RELEASE_PENDING,
            ),
            cleanup,
        )
        assertEquals(
            VpnServiceBindingTransition.Applied(VpnServiceBinding.State.UNBOUND),
            binding.finishCleanup(
                registration,
                VpnServiceBindingCleanupIntent.RebindAfterDeath,
                VpnServiceReleaseOutcome.Released,
            ),
        )
        val nextRegistration = reserveAcceptedBinding(binding)
        assertTrue(nextRegistration.generation > registration.generation)
        assertEquals(VpnServiceBinding.State.BINDING, binding.state())
    }

    @Test
    fun disconnectedBindingCanReconnectAndNullBindingIsReleased() {
        val binding = VpnServiceBindingState()
        val registration = reserveAcceptedBinding(binding)
        assertEquals(
            VpnServiceBindingTransition.Applied(VpnServiceBinding.State.DISCONNECTED),
            binding.serviceDisconnected(registration),
        )
        assertEquals(
            VpnServiceBindingTransition.Applied(VpnServiceBinding.State.CONNECTED),
            binding.serviceConnected(registration, VpnServiceBinderAvailability.Present),
        )

        assertEquals(
            VpnServiceBindingTransition.Applied(VpnServiceBinding.State.NULL_BINDING),
            binding.nullBinding(registration),
        )
        assertEquals(VpnServiceBindAdmission.CleanupInProgress, binding.bindAdmission())
        val cleanup = binding.beginCleanup(registration, VpnServiceBindingCleanupIntent.FinishMissingBinder)
        assertTrue(cleanup is VpnServiceCleanupTransition.Execute)
        assertEquals(
            VpnServiceBindingTransition.Applied(VpnServiceBinding.State.FAILED),
            binding.finishCleanup(
                registration,
                VpnServiceBindingCleanupIntent.FinishMissingBinder,
                VpnServiceReleaseOutcome.Released,
            ),
        )
        assertEquals(VpnServiceBindAdmission.Allowed, binding.bindAdmission())
        assertFalse(binding.hasActiveRegistration())
    }

    @Test
    fun rejectedBindAttemptIsTypedAndCanBeRetried() {
        val binding = VpnServiceBindingState()
        val reservation = binding.reserveBind() as VpnServiceBindReservation.Reserved

        assertEquals(
            VpnServiceBindingTransition.Applied(VpnServiceBinding.State.FAILED),
            binding.bindResult(reservation.registrationId, VpnServiceBindAttempt.Rejected),
        )
        assertEquals(VpnServiceBindAdmission.Allowed, binding.bindAdmission())
        assertEquals(
            VpnServiceBindingTransition.Ignored(VpnServiceBinding.State.FAILED),
            binding.bindingDied(reservation.registrationId),
        )
    }

    @Test
    fun callbacksFromUnboundOrOldRegistrationAreIgnored() {
        val binding = VpnServiceBindingState()
        assertEquals(
            VpnServiceBindingTransition.Ignored(VpnServiceBinding.State.UNBOUND),
            binding.bindingDied(VpnServiceBindingRegistrationId(Long.MAX_VALUE)),
        )
        assertEquals(
            VpnServiceBindingTransition.Ignored(VpnServiceBinding.State.UNBOUND),
            binding.nullBinding(VpnServiceBindingRegistrationId(Long.MAX_VALUE)),
        )

        val first = reserveAcceptedBinding(binding)
        assertEquals(
            VpnServiceBindingTransition.Applied(VpnServiceBinding.State.RECOVERING),
            binding.bindingDied(first),
        )
        val cleanup = binding.beginCleanup(first, VpnServiceBindingCleanupIntent.RebindAfterDeath)
            as VpnServiceCleanupTransition.Execute
        binding.finishCleanup(first, cleanup.intent, VpnServiceReleaseOutcome.Released)
        val second = reserveAcceptedBinding(binding)

        assertEquals(
            VpnServiceBindingTransition.Ignored(VpnServiceBinding.State.BINDING),
            binding.serviceConnected(first, VpnServiceBinderAvailability.Present),
        )
        assertEquals(
            VpnServiceBindingTransition.Ignored(VpnServiceBinding.State.BINDING),
            binding.serviceDisconnected(first),
        )
        assertEquals(
            VpnServiceBindingTransition.Ignored(VpnServiceBinding.State.BINDING),
            binding.bindingDied(first),
        )
        assertEquals(
            VpnServiceBindingTransition.Ignored(VpnServiceBinding.State.BINDING),
            binding.nullBinding(first),
        )
        assertTrue(first != second)
    }

    @Test
    fun missingBinderCannotBeReplacedByLateConnectedCallbackAndReleaseFailureRemainsRetryable() {
        val binding = VpnServiceBindingState()
        val registration = reserveAcceptedBinding(binding)

        assertEquals(
            VpnServiceBindingTransition.Applied(VpnServiceBinding.State.NULL_BINDING),
            binding.serviceConnected(registration, VpnServiceBinderAvailability.Missing),
        )
        assertEquals(
            VpnServiceBindingTransition.Ignored(VpnServiceBinding.State.NULL_BINDING),
            binding.serviceConnected(registration, VpnServiceBinderAvailability.Present),
        )
        val cleanup = binding.beginCleanup(registration, VpnServiceBindingCleanupIntent.FinishMissingBinder)
            as VpnServiceCleanupTransition.Execute
        assertEquals(VpnServiceBinding.State.RELEASE_PENDING, binding.state())
        assertEquals(
            VpnServiceBindingTransition.Applied(VpnServiceBinding.State.RELEASE_FAILED),
            binding.finishCleanup(registration, cleanup.intent, VpnServiceReleaseOutcome.Failed),
        )
        assertTrue(binding.hasActiveRegistration())
        assertEquals(VpnServiceBindAdmission.CleanupInProgress, binding.bindAdmission())

        val retry = binding.retryCleanup()
        assertEquals(
            VpnServiceCleanupTransition.Execute(
                registration,
                VpnServiceBindingCleanupIntent.FinishMissingBinder,
                VpnServiceBinding.State.RELEASE_PENDING,
            ),
            retry,
        )
        assertEquals(
            VpnServiceBindingTransition.Applied(VpnServiceBinding.State.FAILED),
            binding.finishCleanup(
                registration,
                VpnServiceBindingCleanupIntent.FinishMissingBinder,
                VpnServiceReleaseOutcome.AlreadyReleased,
            ),
        )
        assertFalse(binding.hasActiveRegistration())
        assertEquals(VpnServiceBindAdmission.Allowed, binding.bindAdmission())
    }

    @Test
    fun explicitReleaseFailureDoesNotClaimUnboundAndCanBeRetried() {
        val binding = VpnServiceBindingState()
        val registration = reserveAcceptedBinding(binding)
        binding.serviceConnected(registration, VpnServiceBinderAvailability.Present)

        val cleanup = binding.beginCleanup(registration, VpnServiceBindingCleanupIntent.ExplicitUnbind)
            as VpnServiceCleanupTransition.Execute
        assertEquals(
            VpnServiceBindingTransition.Applied(VpnServiceBinding.State.RELEASE_FAILED),
            binding.finishCleanup(registration, cleanup.intent, VpnServiceReleaseOutcome.Failed),
        )
        assertEquals(VpnServiceBinding.State.RELEASE_FAILED, binding.state())
        assertEquals(VpnServiceBindAdmission.CleanupInProgress, binding.bindAdmission())

        val retry = binding.beginCleanup(registration, VpnServiceBindingCleanupIntent.ExplicitUnbind)
            as VpnServiceCleanupTransition.Execute
        assertEquals(
            VpnServiceBindingTransition.Applied(VpnServiceBinding.State.UNBOUND),
            binding.finishCleanup(registration, retry.intent, VpnServiceReleaseOutcome.Released),
        )
        assertEquals(VpnServiceBindAdmission.Allowed, binding.bindAdmission())
    }

    @Test
    fun vpnStartOutcomesKeepPermissionFailureTimeoutAndOwnerAvailabilityDistinct() {
        assertEquals(VpnStartOutcomeWire.Running, VpnStartResult.Running.toWireOutcome())
        assertEquals(
            VpnStartOutcomeWire.PermissionRequired,
            VpnStartResult.PermissionRequired.toWireOutcome(),
        )
        assertEquals(
            VpnStartOutcomeWire.Failed,
            VpnStartResult.Failed("Builder rejected tunnel").toWireOutcome(),
        )
        assertEquals(VpnStartOutcomeWire.TimedOut, VpnStartResult.TimedOut.toWireOutcome())

        assertEquals(
            VpnStartOutcomeWire.Running,
            AndroidHostReply.VPN_RUNNING.toVpnStartOutcomeWire(),
        )
        assertEquals(
            VpnStartOutcomeWire.PermissionRequired,
            AndroidHostReply.VPN_PERMISSION_REQUIRED.toVpnStartOutcomeWire(),
        )
        assertEquals(
            VpnStartOutcomeWire.OwnerUnavailable,
            AndroidHostReply.SERVICE_UNAVAILABLE.toVpnStartOutcomeWire(),
        )
        assertEquals(
            VpnStartOutcomeWire.TimedOut,
            AndroidHostReply.VPN_START_TIMED_OUT.toVpnStartOutcomeWire(),
        )
        assertEquals(
            VpnStartOutcomeWire.Failed,
            AndroidHostReply.FAILED.toVpnStartOutcomeWire(),
        )
        assertEquals(listOf(0, 1, 2, 3, 4), VpnStartOutcomeWire.entries.map { it.wireCode })
    }

    @Test
    fun vpnConfigurationApplyOutcomesPreserveAcceptanceRejectionAndOwnerAvailability() {
        assertEquals(
            VpnConfigurationApplyOutcomeWire.Accepted,
            AndroidHostReply.VPN_CONFIGURATION_ACCEPTED.toVpnConfigurationApplyOutcomeWire(),
        )
        assertEquals(
            VpnConfigurationApplyOutcomeWire.OwnerUnavailable,
            AndroidHostReply.SERVICE_UNAVAILABLE.toVpnConfigurationApplyOutcomeWire(),
        )
        assertEquals(
            VpnConfigurationApplyOutcomeWire.Rejected,
            AndroidHostReply.FAILED.toVpnConfigurationApplyOutcomeWire(),
        )
        assertEquals(
            listOf(0, 1, 2),
            VpnConfigurationApplyOutcomeWire.entries.map { it.wireCode },
        )
    }

    @Test
    fun interruptedOwnerStopWaitReturnsTypedCanceledTerminal() {
        val lifecycle = VpnLifecycleCoordinator()
        lifecycle.requestStart()
        val stop = lifecycle.requestCleanup(VpnCleanupIntent.Stop) as VpnCleanupPlan.Dispatch
        val enteredWait = CountDownLatch(1)
        val result = AtomicReference<VpnStopResult?>()
        val waiter = Thread {
            enteredWait.countDown()
            result.set(lifecycle.awaitStop(stop.request.generation, 60_000))
        }

        waiter.start()
        assertTrue(enteredWait.await(1, TimeUnit.SECONDS))
        waiter.interrupt()
        waiter.join(1_000)

        assertFalse(waiter.isAlive)
        assertEquals(VpnStopResult.Canceled, result.get())
    }

    @Test
    fun terminalRunningWaitsForTypedRustStartReceiptAfterTunnelEstablishment() {
        val lifecycle = VpnLifecycleCoordinator()
        val start = lifecycle.requestStart() as VpnStartPlan.Dispatch
        val lease = AndroidVpnRoutingActivationLease(7uL, 12uL)
        assertEquals(VpnTransitionOutcome.Applied, lifecycle.transition(start.generation, VpnProcessState.Starting))
        assertEquals(VpnTransitionOutcome.Applied, lifecycle.transition(start.generation, VpnProcessState.Foreground))
        assertEquals(VpnTransitionOutcome.Applied, lifecycle.transition(start.generation, VpnProcessState.TunnelEstablished))
        assertEquals(
            VpnTransitionOutcome.RunningRequiresDataPlaneReceipt,
            lifecycle.transition(start.generation, VpnProcessState.Running),
        )

        val waiterStarted = CountDownLatch(1)
        val waiter = Executors.newSingleThreadExecutor()
        try {
            val result = waiter.submit<VpnStartResult> {
                waiterStarted.countDown()
                lifecycle.awaitStart(start.generation, 2_000)
            }
            assertTrue(waiterStarted.await(1, TimeUnit.SECONDS))
            assertEquals(VpnProcessState.TunnelEstablished, lifecycle.snapshot().state)
            assertFalse(result.isDone)

            assertEquals(
                VpnDataPlaneStartPublication.Published,
                lifecycle.completeDataPlaneStart(
                    start.generation,
                    testFfiStatus(FfiErrorCode.OK).toVpnDataPlaneStartReceipt(),
                    VpnRoutingLeaseState.Held(lease),
                ),
            )
            assertEquals(VpnStartResult.Running, result.get(1, TimeUnit.SECONDS))
        } finally {
            waiter.shutdownNow()
        }
    }

    @Test
    fun dataPlaneReceiptCannotSkipTunnelEstablishment() {
        val lifecycle = VpnLifecycleCoordinator()
        val start = lifecycle.requestStart() as VpnStartPlan.Dispatch
        val lease = AndroidVpnRoutingActivationLease(7uL, 12uL)

        assertEquals(
            VpnDataPlaneStartPublication.UnexpectedState(VpnProcessState.StartRequested),
            lifecycle.completeDataPlaneStart(
                start.generation,
                VpnDataPlaneStartReceipt.Started,
                VpnRoutingLeaseState.Held(lease),
            ),
        )
        assertEquals(VpnProcessState.StartRequested, lifecycle.snapshot().state)
    }

    @Test
    fun runningCannotPublishAfterTheRoutingLeaseHasBeenReleased() {
        val lifecycle = VpnLifecycleCoordinator()
        val start = lifecycle.requestStart() as VpnStartPlan.Dispatch
        assertEquals(VpnTransitionOutcome.Applied, lifecycle.transition(start.generation, VpnProcessState.Starting))
        assertEquals(VpnTransitionOutcome.Applied, lifecycle.transition(start.generation, VpnProcessState.Foreground))
        assertEquals(VpnTransitionOutcome.Applied, lifecycle.transition(start.generation, VpnProcessState.TunnelEstablished))
        val lease = AndroidVpnRoutingActivationLease(7uL, 12uL)
        val released = VpnRoutingLeaseState.Held(lease).afterReleaseAttempt(
            lease,
            AndroidVpnRoutingActivationReleaseResult.Released,
        )

        assertEquals(
            VpnDataPlaneStartPublication.RoutingLeaseNotHeld,
            lifecycle.completeDataPlaneStart(start.generation, VpnDataPlaneStartReceipt.Started, released),
        )
        assertEquals(
            VpnProcessState.Failed("VPN Running requires a held routing activation lease"),
            lifecycle.snapshot().state,
        )
    }

    @Test
    fun runningCannotPublishWhileTheRoutingLeaseReleaseIsPending() {
        val lifecycle = VpnLifecycleCoordinator()
        val start = lifecycle.requestStart() as VpnStartPlan.Dispatch
        assertEquals(VpnTransitionOutcome.Applied, lifecycle.transition(start.generation, VpnProcessState.Starting))
        assertEquals(VpnTransitionOutcome.Applied, lifecycle.transition(start.generation, VpnProcessState.Foreground))
        assertEquals(VpnTransitionOutcome.Applied, lifecycle.transition(start.generation, VpnProcessState.TunnelEstablished))
        val lease = AndroidVpnRoutingActivationLease(7uL, 12uL)
        val pending = VpnRoutingLeaseState.Held(lease).afterReleaseAttempt(
            lease,
            AndroidVpnRoutingActivationReleaseResult.Failed(
                testFfiStatus(FfiErrorCode.NOT_READY, "owner unavailable"),
            ),
        )

        assertEquals(
            VpnDataPlaneStartPublication.RoutingLeaseNotHeld,
            lifecycle.completeDataPlaneStart(start.generation, VpnDataPlaneStartReceipt.Started, pending),
        )
        assertEquals(
            VpnProcessState.Failed("VPN Running requires a held routing activation lease"),
            lifecycle.snapshot().state,
        )
    }

    @Test
    fun failedReleaseAfterRunningKeepsTheActiveDataPlaneAndRequestsCleanupRetry() {
        val lifecycle = VpnLifecycleCoordinator()
        val start = lifecycle.requestStart() as VpnStartPlan.Dispatch
        assertEquals(VpnTransitionOutcome.Applied, lifecycle.transition(start.generation, VpnProcessState.Starting))
        assertEquals(VpnTransitionOutcome.Applied, lifecycle.transition(start.generation, VpnProcessState.Foreground))
        assertEquals(VpnTransitionOutcome.Applied, lifecycle.transition(start.generation, VpnProcessState.TunnelEstablished))
        val lease = AndroidVpnRoutingActivationLease(7uL, 12uL)
        val held = VpnRoutingLeaseState.Held(lease)

        assertEquals(
            VpnDataPlaneStartPublication.Published,
            lifecycle.completeDataPlaneStart(start.generation, VpnDataPlaneStartReceipt.Started, held),
        )
        val pending = held.afterReleaseAttempt(
            lease,
            AndroidVpnRoutingActivationReleaseResult.Failed(
                testFfiStatus(FfiErrorCode.NOT_READY, "owner unavailable"),
            ),
        )

        assertEquals(VpnProcessState.Running, lifecycle.snapshot().state)
        assertEquals(
            VpnRoutingLeaseState.ReleasePending(
                lease,
                VpnRoutingLeaseReleaseProblem.OwnerFailure("owner unavailable"),
            ),
            pending,
        )
        assertEquals(VpnRoutingLeaseCleanupAction.RetryRelease, pending.cleanupAction())
    }

    @Test
    fun staleOrFailedDataPlaneReceiptCannotPublishRunning() {
        val lifecycle = VpnLifecycleCoordinator()
        val first = lifecycle.requestStart() as VpnStartPlan.Dispatch
        assertEquals(VpnTransitionOutcome.Applied, lifecycle.transition(first.generation, VpnProcessState.Starting))
        assertEquals(VpnTransitionOutcome.Applied, lifecycle.transition(first.generation, VpnProcessState.Foreground))
        assertEquals(VpnTransitionOutcome.Applied, lifecycle.transition(first.generation, VpnProcessState.TunnelEstablished))
        val stop = lifecycle.requestCleanup(VpnCleanupIntent.Stop) as VpnCleanupPlan.Dispatch

        assertEquals(
            VpnDataPlaneStartPublication.StaleGeneration,
            lifecycle.completeDataPlaneStart(
                first.generation,
                VpnDataPlaneStartReceipt.Started,
                VpnRoutingLeaseState.Held(AndroidVpnRoutingActivationLease(7uL, 12uL)),
            ),
        )
        assertEquals(VpnProcessState.Stopping, lifecycle.snapshot().state)
        assertEquals(stop.request.generation, lifecycle.snapshot().generation)

        assertEquals(
            VpnCleanupPublication.Published(VpnProcessState.Stopped),
            completeCleanup(lifecycle, stop.request),
        )

        val retry = lifecycle.requestStart() as VpnStartPlan.Dispatch
        assertEquals(VpnTransitionOutcome.Applied, lifecycle.transition(retry.generation, VpnProcessState.Starting))
        assertEquals(VpnTransitionOutcome.Applied, lifecycle.transition(retry.generation, VpnProcessState.Foreground))
        assertEquals(VpnTransitionOutcome.Applied, lifecycle.transition(retry.generation, VpnProcessState.TunnelEstablished))
        assertEquals(
            VpnDataPlaneStartPublication.Published,
            lifecycle.completeDataPlaneStart(
                retry.generation,
                testFfiStatus(FfiErrorCode.INVALID_STATE, "worker failed")
                    .toVpnDataPlaneStartReceipt(),
                VpnRoutingLeaseState.Held(AndroidVpnRoutingActivationLease(8uL, 13uL)),
            ),
        )
        assertEquals(VpnProcessState.Failed("worker failed"), lifecycle.snapshot().state)
        assertEquals(
            VpnStartResult.Failed("worker failed"),
            lifecycle.awaitStart(retry.generation, 1),
        )
    }

    @Test
    fun stopThenRevokeAndRevokeThenStopShareOnePlanAndPublishRevoked() {
        listOf(
            listOf(VpnCleanupIntent.Stop, VpnCleanupIntent.Revoke),
            listOf(VpnCleanupIntent.Revoke, VpnCleanupIntent.Stop),
        ).forEach { order ->
            val lifecycle = VpnLifecycleCoordinator()
            lifecycle.requestStart()

            val first = lifecycle.requestCleanup(
                order.first(),
                resources = VpnOwnedResourcesPresence.Retained,
            ) as VpnCleanupPlan.Dispatch
            val second = lifecycle.requestCleanup(order.last()) as VpnCleanupPlan.Join

            assertEquals(first.request.generation, second.request.generation)
            assertEquals(VpnCleanupIntent.Revoke, second.request.intent)
            assertEquals(
                VpnCleanupPublication.Published(VpnProcessState.Revoked),
                completeCleanup(lifecycle, first.request),
            )
        }
    }

    @Test
    fun queueRejectionRetainsResourcesBlocksStartAndDuplicateStopRetriesSameGeneration() {
        val lifecycle = VpnLifecycleCoordinator()
        val start = lifecycle.requestStart() as VpnStartPlan.Dispatch
        val resources = VpnOwnerResourceRecord<String, String, String, String>()
        assertEquals(VpnOwnerResourceMutation.Recorded, resources.recordWorker(start.generation, "worker"))
        val queueFailure = VpnCleanupFailure(
            VpnCleanupResourceKind.ExecutorQueue,
            "executor rejected cleanup",
        )
        val plan = lifecycle.requestCleanup(
            VpnCleanupIntent.Stop,
            resources = resources.snapshot().presence(),
        ) as VpnCleanupPlan.Dispatch

        resources.markCleanupPending(start.generation, queueFailure)
        assertEquals(
            VpnCleanupPublication.CleanupPending(
                queueFailure,
            ),
            lifecycle.cleanupQueueRejected(plan.request, "executor rejected cleanup"),
        )
        assertEquals(
            VpnOwnedResource.CleanupPending(start.generation, "worker", queueFailure),
            resources.snapshot().worker,
        )
        assertEquals(VpnProcessState.Failed("VPN cleanup pending: executor rejected cleanup"), lifecycle.snapshot().state)
        assertEquals(VpnStartPlan.StopInProgress, lifecycle.requestStart(resources.snapshot().presence()))

        val retry = lifecycle.requestCleanup(
            VpnCleanupIntent.Stop,
            resources = resources.snapshot().presence(),
        ) as VpnCleanupPlan.Retry
        assertEquals(plan.request.generation, retry.request.generation)
        assertEquals(plan.request.resourceGeneration, retry.request.resourceGeneration)
        assertEquals(VpnCleanupPlan.Join(retry.request), lifecycle.requestCleanup(VpnCleanupIntent.Stop))
    }

    @Test
    fun terminalStateRequiresClearResourceRecordAndMatchingOwnerReadback() {
        val lifecycle = VpnLifecycleCoordinator()
        lifecycle.requestStart()
        val request = (lifecycle.requestCleanup(
            VpnCleanupIntent.Stop,
            resources = VpnOwnedResourcesPresence.Retained,
        ) as VpnCleanupPlan.Dispatch).request

        assertEquals(
            VpnCleanupPublication.CleanupPending(
                VpnCleanupFailure(
                    VpnCleanupResourceKind.OwnerReadback,
                    "VPN owner still retains resources after cleanup",
                ),
            ),
            completeCleanup(
                lifecycle,
                request,
                resources = VpnOwnedResourcesPresence.Retained,
            ),
        )
        assertEquals(VpnStopResult.Failed("VPN cleanup pending: VPN owner still retains resources after cleanup"), lifecycle.awaitStop(request.generation, 1))

        assertEquals(VpnCleanupPlan.Retry(request), lifecycle.retryPendingCleanup())
        val mismatchedReadback = lifecycle.snapshot()
        assertEquals(
            VpnCleanupPublication.CleanupPending(
                VpnCleanupFailure(
                    VpnCleanupResourceKind.OwnerReadback,
                    "VPN cleanup owner readback did not match the active cleanup generation",
                ),
            ),
            lifecycle.completeCleanup(
                request,
                VpnCleanupOutcome.Completed,
                VpnCleanupOwnerReadback(
                    mismatchedReadback.generation - 1,
                    VpnProcessState.Stopping,
                    VpnOwnedResourcesPresence.Clear,
                ),
            ),
        )
        assertEquals(VpnCleanupPlan.Retry(request), lifecycle.retryPendingCleanup())
        assertEquals(
            VpnCleanupPublication.Published(VpnProcessState.Stopped),
            completeCleanup(lifecycle, request),
        )
    }

    private fun completeCleanup(
        lifecycle: VpnLifecycleCoordinator,
        request: VpnCleanupRequest,
        outcome: VpnCleanupOutcome = VpnCleanupOutcome.Completed,
        resources: VpnOwnedResourcesPresence = VpnOwnedResourcesPresence.Clear,
    ): VpnCleanupPublication {
        val snapshot = lifecycle.snapshot()
        return lifecycle.completeCleanup(
            request,
            outcome,
            VpnCleanupOwnerReadback(snapshot.generation, snapshot.state, resources),
        )
    }
}
