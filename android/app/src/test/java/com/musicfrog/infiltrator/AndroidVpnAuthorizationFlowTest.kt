package com.musicfrog.infiltrator

import infiltrator_android.FfiErrorCode
import infiltrator_android.FfiStatus
import infiltrator_android.VpnAuthorizationResult
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class AndroidVpnAuthorizationFlowTest {
    @Test
    fun consentIsRequestedFromTheVisibleHostAndStartWaitsForTheResult() {
        val host = FakeVpnAuthorizationHost()
        val flow = AndroidVpnAuthorizationFlow(host)

        flow.requestStart()
        flow.requestStart()

        assertEquals(AndroidVpnAuthorizationFlowState.AwaitingVpnConsent, flow.state())
        assertEquals(1, host.vpnConsentLaunches)
        assertEquals(0, host.startDispatches)

        host.authorization = VpnAuthorizationAvailability.Granted
        flow.onVpnConsentPromptReturned()

        assertEquals(listOf(VpnAuthorizationResult.GRANTED), host.authorizationReports)
        assertEquals(1, host.startDispatches)
        assertEquals(AndroidVpnAuthorizationFlowState.DispatchingStart, flow.state())

        host.completeStart(VpnProxyStartReadback.Running)

        assertEquals(AndroidVpnAuthorizationFlowState.Idle, flow.state())
        assertTrue(host.failures.isEmpty())
    }

    @Test
    fun aPromptReturnWithoutSystemAuthorizationDoesNotDispatchStart() {
        val host = FakeVpnAuthorizationHost()
        val flow = AndroidVpnAuthorizationFlow(host)

        flow.requestStart()
        flow.onVpnConsentPromptReturned()

        assertEquals(listOf(VpnAuthorizationResult.DENIED), host.authorizationReports)
        assertEquals(1, host.authorizationNotGrantedCount)
        assertEquals(0, host.startDispatches)
        assertEquals(AndroidVpnAuthorizationFlowState.Idle, flow.state())
    }

    @Test
    fun notificationPromptLaunchFailureIsWarnedAndDoesNotBlockStart() {
        val host = FakeVpnAuthorizationHost().apply {
            authorization = VpnAuthorizationAvailability.Granted
            notificationStatus = NotificationPermissionStatus.NotGranted
            notificationPromptLaunch = NotificationPromptLaunch.Failed("activity is not available")
        }
        val flow = AndroidVpnAuthorizationFlow(host)

        flow.requestStart()

        assertEquals(1, host.notificationPermissionLaunches)
        assertEquals(
            listOf("activity is not available"),
            host.notificationPromptWarnings,
        )
        assertEquals(1, host.startDispatches)
        assertEquals(AndroidVpnAuthorizationFlowState.DispatchingStart, flow.state())
        assertTrue(host.failures.isEmpty())

        host.completeStart(VpnProxyStartReadback.Running)

        assertEquals(AndroidVpnAuthorizationFlowState.Idle, flow.state())
        assertTrue(host.failures.isEmpty())
    }

    @Test
    fun vpnConsentPromptLaunchFailureBlocksStart() {
        val host = FakeVpnAuthorizationHost().apply {
            vpnConsentLaunch = VpnConsentPromptLaunch.Failed("system consent activity unavailable")
        }
        val flow = AndroidVpnAuthorizationFlow(host)

        flow.requestStart()

        assertEquals(1, host.vpnConsentLaunches)
        assertEquals(0, host.startDispatches)
        assertEquals(
            listOf("system consent activity unavailable"),
            host.failures,
        )
        assertEquals(AndroidVpnAuthorizationFlowState.Idle, flow.state())
    }

    @Test
    fun notificationDenialDoesNotBecomeVpnFailureAndOwnerReadbackIsRequired() {
        val host = FakeVpnAuthorizationHost().apply {
            authorization = VpnAuthorizationAvailability.Granted
            notificationStatus = NotificationPermissionStatus.NotGranted
        }
        val flow = AndroidVpnAuthorizationFlow(host)

        flow.requestStart()

        assertEquals(AndroidVpnAuthorizationFlowState.AwaitingNotificationPermission, flow.state())
        assertEquals(0, host.startDispatches)

        flow.onNotificationPermissionPromptReturned(NotificationPermissionPromptResult.NotGranted)

        assertEquals(1, host.notificationPermissionNotGrantedCount)
        assertEquals(1, host.startDispatches)
        val starting = ownerSnapshot(VpnOwnerLifecycleWire.Starting, foreground = true)
        host.completeStart(classifyVpnOwnerStartReadback(starting))

        assertEquals(AndroidVpnAuthorizationFlowState.Idle, flow.state())
        assertEquals(
            listOf(
                "The :vpn owner did not confirm a foreground running VPN session " +
                    "(state=Starting, generation=5, revision=3)",
            ),
            host.failures,
        )
    }

    @Test
    fun failedAuthorizationRecordingCannotStartTheService() {
        val host = FakeVpnAuthorizationHost().apply {
            authorization = VpnAuthorizationAvailability.Granted
            authorizationReport = testFfiStatus(FfiErrorCode.INVALID_STATE, "owner unavailable")
        }
        val flow = AndroidVpnAuthorizationFlow(host)

        flow.requestStart()

        assertEquals(0, host.startDispatches)
        assertEquals(AndroidVpnAuthorizationFlowState.Idle, flow.state())
        assertEquals(listOf("owner unavailable"), host.failures)
    }

    @Test
    fun unavailableAuthorizationReadIsNotTreatedAsPermissionRequiredOrGranted() {
        val host = FakeVpnAuthorizationHost().apply {
            authorization = VpnAuthorizationAvailability.Unavailable("prepare failed")
        }
        val flow = AndroidVpnAuthorizationFlow(host)

        flow.requestStart()

        assertEquals(0, host.vpnConsentLaunches)
        assertEquals(0, host.authorizationReports.size)
        assertEquals(0, host.startDispatches)
        assertEquals(listOf("prepare failed"), host.failures)
        assertEquals(AndroidVpnAuthorizationFlowState.Idle, flow.state())
    }

    @Test
    fun activityRecreationRetainsOnlyOutstandingSystemPrompts() {
        assertEquals(
            AndroidVpnAuthorizationFlowState.AwaitingVpnConsent,
            AndroidVpnAuthorizationFlow.stateAfterRecreation(
                AndroidVpnAuthorizationFlowState.AwaitingVpnConsent,
            ),
        )
        assertEquals(
            AndroidVpnAuthorizationFlowState.AwaitingNotificationPermission,
            AndroidVpnAuthorizationFlow.stateAfterRecreation(
                AndroidVpnAuthorizationFlowState.AwaitingNotificationPermission,
            ),
        )
        assertEquals(
            AndroidVpnAuthorizationFlowState.Idle,
            AndroidVpnAuthorizationFlow.stateAfterRecreation(
                AndroidVpnAuthorizationFlowState.DispatchingStart,
            ),
        )
    }

    @Test
    fun ownerReadbackRequiresRunningReadinessForegroundAndResourceFactsTogether() {
        val running = ownerSnapshot(VpnOwnerLifecycleWire.Running, foreground = true)
        assertEquals(
            VpnProxyStartReadback.Running,
            classifyVpnOwnerStartReadback(running),
        )
        val background = ownerSnapshot(VpnOwnerLifecycleWire.Running, foreground = false)
        assertEquals(
            VpnProxyStartReadback.OwnerFailed(
                "VPN Running readback lacks foreground protection",
            ),
            classifyVpnOwnerStartReadback(background),
        )
        val starting = ownerSnapshot(VpnOwnerLifecycleWire.Starting, foreground = true)
        assertEquals(
            VpnProxyStartReadback.OwnerNotRunning(starting, VpnOwnerDataPlaneState.Starting),
            classifyVpnOwnerStartReadback(starting),
        )
        val failed = ownerSnapshot(
            VpnOwnerLifecycleWire.Failed("tun establish failed"),
            foreground = false,
        )
        assertEquals(
            VpnProxyStartReadback.OwnerFailed("tun establish failed"),
            classifyVpnOwnerStartReadback(failed),
        )
    }

    private fun ownerSnapshot(
        lifecycle: VpnOwnerLifecycleWire,
        foreground: Boolean,
    ): VpnOwnerSnapshotWire {
        val isRunning = lifecycle == VpnOwnerLifecycleWire.Running
        val readiness = when (lifecycle) {
            VpnOwnerLifecycleWire.Starting -> VpnOwnerReadinessWire.AwaitingForeground
            VpnOwnerLifecycleWire.Running -> VpnOwnerReadinessWire.Ready
            is VpnOwnerLifecycleWire.Failed -> VpnOwnerReadinessWire.Failed
            else -> error("test fixture only supports start, running, and failure")
        }
        return VpnOwnerSnapshotWire(
            ownerSessionId = "owner-a",
            revision = 3,
            vpnGeneration = 5,
            vpnState = lifecycle,
            readiness = readiness,
            resources = VpnOwnerResourceKindWire.entries.map { kind ->
                VpnOwnerResourceFactWire(
                    kind,
                    if (isRunning && kind in setOf(
                            VpnOwnerResourceKindWire.Worker,
                            VpnOwnerResourceKindWire.TunnelDescriptor,
                        )
                    ) {
                        VpnOwnerResourceStateWire.Held(5)
                    } else {
                        VpnOwnerResourceStateWire.Absent
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

    private class FakeVpnAuthorizationHost : AndroidVpnAuthorizationHost {
        var authorization: VpnAuthorizationAvailability = VpnAuthorizationAvailability.Required
        var notificationStatus: NotificationPermissionStatus = NotificationPermissionStatus.NotRequired
        var vpnConsentLaunch: VpnConsentPromptLaunch = VpnConsentPromptLaunch.Launched
        var notificationPromptLaunch: NotificationPromptLaunch = NotificationPromptLaunch.Launched
        var authorizationReport: FfiStatus = testFfiStatus(FfiErrorCode.OK)
        val authorizationReports = mutableListOf<VpnAuthorizationResult>()
        var vpnConsentLaunches = 0
        var notificationPermissionLaunches = 0
        var authorizationNotGrantedCount = 0
        var notificationPermissionNotGrantedCount = 0
        var startDispatches = 0
        val failures = mutableListOf<String>()
        val notificationPromptWarnings = mutableListOf<String>()
        private var completion: ((VpnProxyStartReadback) -> Unit)? = null

        override fun authorizationAvailability(): VpnAuthorizationAvailability = authorization

        override fun launchVpnConsentPrompt(): VpnConsentPromptLaunch {
            vpnConsentLaunches += 1
            return vpnConsentLaunch
        }

        override fun notificationPermissionStatus(): NotificationPermissionStatus = notificationStatus

        override fun launchNotificationPermissionPrompt(): NotificationPromptLaunch {
            notificationPermissionLaunches += 1
            return notificationPromptLaunch
        }

        override fun reportVpnAuthorization(result: VpnAuthorizationResult): FfiStatus {
            authorizationReports += result
            return authorizationReport
        }

        override fun onVpnAuthorizationNotGranted(status: FfiStatus) {
            authorizationNotGrantedCount += 1
        }

        override fun onNotificationPermissionNotGranted(result: NotificationPermissionPromptResult) {
            notificationPermissionNotGrantedCount += 1
        }

        override fun onNotificationPermissionPromptFailed(message: String) {
            notificationPromptWarnings += message
        }

        override fun dispatchProxyStart(completion: (VpnProxyStartReadback) -> Unit) {
            startDispatches += 1
            this.completion = completion
        }

        override fun reportFailure(message: String) {
            failures += message
        }

        fun completeStart(result: VpnProxyStartReadback) {
            requireNotNull(completion).invoke(result)
        }
    }
}
