package com.musicfrog.infiltrator.ui

import com.musicfrog.infiltrator.AndroidVpnAuthorizationUiState
import com.musicfrog.infiltrator.CoreOwnerIdentityWire
import com.musicfrog.infiltrator.CoreOwnerLifecycleWire
import com.musicfrog.infiltrator.CoreOwnerSnapshotWire
import com.musicfrog.infiltrator.VpnOwnerCoreReadbackWire
import com.musicfrog.infiltrator.VpnOwnerDataPlaneState
import com.musicfrog.infiltrator.VpnOwnerLifecycleWire
import com.musicfrog.infiltrator.VpnOwnerReadinessWire
import com.musicfrog.infiltrator.VpnOwnerResourceFactWire
import com.musicfrog.infiltrator.VpnOwnerResourceKindWire
import com.musicfrog.infiltrator.VpnOwnerResourceStateWire
import com.musicfrog.infiltrator.VpnOwnerSnapshotFacts
import com.musicfrog.infiltrator.VpnOwnerSnapshotPublisher
import com.musicfrog.infiltrator.VpnOwnerSnapshotWire
import com.musicfrog.infiltrator.ui.common.ObservationRevision
import com.musicfrog.infiltrator.ui.common.OwnerVpnObservation
import com.musicfrog.infiltrator.ui.overview.ObservationState
import com.musicfrog.infiltrator.ui.overview.canStartVpnSession
import com.musicfrog.infiltrator.ui.overview.vpnSessionIsRunning
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

class FourPagesHierarchyAndSemanticsTest {

    private fun sampleVpnSnapshot(
        state: VpnOwnerLifecycleWire = VpnOwnerLifecycleWire.Running,
        readiness: VpnOwnerReadinessWire = VpnOwnerReadinessWire.Ready,
        resourceState: VpnOwnerResourceStateWire = VpnOwnerResourceStateWire.Held(1L),
    ): VpnOwnerSnapshotWire {
        val facts = VpnOwnerSnapshotFacts(
            ownerSessionId = "test-session",
            vpnGeneration = 1L,
            vpnState = state,
            readiness = readiness,
            resources = VpnOwnerResourceKindWire.entries.map { kind ->
                VpnOwnerResourceFactWire(
                    kind,
                    if (kind == VpnOwnerResourceKindWire.Worker || kind == VpnOwnerResourceKindWire.TunnelDescriptor) {
                        resourceState
                    } else {
                        VpnOwnerResourceStateWire.Absent
                    },
                )
            },
            foreground = true,
            mtu = 1500,
            routeCount = 2,
            dnsServers = listOf("1.1.1.1"),
            ipv6 = false,
            core = VpnOwnerCoreReadbackWire.Available(
                CoreOwnerSnapshotWire(
                    identity = CoreOwnerIdentityWire.Present("mihomo", "core-a", "1.20.0"),
                    lifecycle = CoreOwnerLifecycleWire.Running,
                    generation = 1L,
                    sessionToken = "123",
                    revision = 1L,
                    failure = null,
                ),
            ),
        )
        return VpnOwnerSnapshotWire.parse(VpnOwnerSnapshotPublisher().publish(facts))
            ?: error("failed to parse test snapshot")
    }

    @Test
    fun overviewStateMatrixDistinguishesRunningFromUnauthorizedAndStopped() {
        // 1. Loading state
        val loadingVpn = ObservationState.Loading<OwnerVpnObservation>()
        assertFalse(vpnSessionIsRunning(loadingVpn, AndroidVpnAuthorizationUiState.Granted))

        // 2. Stopped state
        val stoppedSnapshot = sampleVpnSnapshot(
            state = VpnOwnerLifecycleWire.Stopped,
            readiness = VpnOwnerReadinessWire.Stopped,
            resourceState = VpnOwnerResourceStateWire.Absent,
        )
        val stoppedObs = ObservationState.Observed(
            OwnerVpnObservation.AndroidOwner(stoppedSnapshot),
            ObservationRevision.Owner(1uL, 1uL),
        )
        assertFalse(vpnSessionIsRunning(stoppedObs, AndroidVpnAuthorizationUiState.Granted))
        assertTrue(canStartVpnSession(stoppedObs, AndroidVpnAuthorizationUiState.Granted))

        // 3. Unauthorized / Permission Required state
        val permSnapshot = sampleVpnSnapshot(
            state = VpnOwnerLifecycleWire.PermissionRequired,
            readiness = VpnOwnerReadinessWire.PermissionRequired,
            resourceState = VpnOwnerResourceStateWire.Absent,
        )
        val permObs = ObservationState.Observed(
            OwnerVpnObservation.AndroidOwner(permSnapshot),
            ObservationRevision.Owner(1uL, 1uL),
        )
        assertFalse(vpnSessionIsRunning(permObs, AndroidVpnAuthorizationUiState.Required))
        assertTrue(canStartVpnSession(permObs, AndroidVpnAuthorizationUiState.Required))

        // 4. Real Running state
        val runningSnapshot = sampleVpnSnapshot(
            state = VpnOwnerLifecycleWire.Running,
            readiness = VpnOwnerReadinessWire.Ready,
            resourceState = VpnOwnerResourceStateWire.Held(1L),
        )
        val runningObs = ObservationState.Observed(
            OwnerVpnObservation.AndroidOwner(runningSnapshot),
            ObservationRevision.Owner(1uL, 1uL),
        )
        assertTrue(vpnSessionIsRunning(runningObs, AndroidVpnAuthorizationUiState.Granted))
        assertFalse(canStartVpnSession(runningObs, AndroidVpnAuthorizationUiState.Granted))
    }

    @Test
    fun appSectionsCoverTheRequiredFourCorePages() {
        val sections = AppSection.values()
        assertTrue(sections.contains(AppSection.Overview))
        assertTrue(sections.contains(AppSection.Profiles))
        assertTrue(sections.contains(AppSection.Proxies))
        assertTrue(sections.contains(AppSection.AppRouting))
        assertTrue(sections.contains(AppSection.Sync))
    }

    @Test
    fun settingsDestinationsProvideCategorizedHierarchicalSubpages() {
        val destinations = SettingsDestination.values().map { it.name }
        assertTrue("Routing subpage present", destinations.contains("Routing"))
        assertTrue("Tun subpage present", destinations.contains("Tun"))
        assertTrue("Dns subpage present", destinations.contains("Dns"))
        assertTrue("FakeIp subpage present", destinations.contains("FakeIp"))
        assertTrue("Rules subpage present", destinations.contains("Rules"))
        assertTrue("Connections subpage present", destinations.contains("Connections"))
        assertTrue("Logs subpage present", destinations.contains("Logs"))
    }

    @Test
    fun bilingualStringsParityAcrossValuesAndChineseResources() {
        val rootDir = File("src/main/res")
        val baseStrings = File(rootDir, "values/strings.xml").readText()
        val zhCnStrings = File(rootDir, "values-zh-rCN/strings.xml").readText()
        val zhTwStrings = File(rootDir, "values-zh-rTW/strings.xml").readText()

        val extractKeys = { text: String ->
            Regex("""<string\s+name="([^"]+)"""")
                .findAll(text)
                .map { it.groupValues[1] }
                .toSet()
        }

        val baseKeys = extractKeys(baseStrings)
        val zhCnKeys = extractKeys(zhCnStrings)
        val zhTwKeys = extractKeys(zhTwStrings)

        // Core four pages navigation titles must exist in all three locales
        val coreNavKeys = listOf(
            "nav_overview", "nav_profiles", "nav_proxies", "nav_settings",
            "setting_routing_title", "setting_tun_title", "setting_dns_title",
            "setting_rules_title", "status_running", "status_stopped",
        )

        for (key in coreNavKeys) {
            assertTrue("Base strings must contain $key", baseKeys.contains(key))
            assertTrue("zh-CN strings must contain $key", zhCnKeys.contains(key))
            assertTrue("zh-TW strings must contain $key", zhTwKeys.contains(key))
        }
    }
}
