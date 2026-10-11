package com.musicfrog.infiltrator

import infiltrator_android.AndroidVpnConfigurationActivationLease
import infiltrator_android.AndroidVpnConfigurationActivationReleaseResult
import java.util.concurrent.CancellationException
import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test

class VpnConfigurationActivationTest {
    private val lease = AndroidVpnConfigurationActivationLease(
        leaseId = 7uL,
        preparationId = 19uL,
        vpnGeneration = 23uL,
        ownerDataRevision = 41uL,
    )

    @Test
    fun staleRevisionOrGenerationRunsNoBuilderTunnelOrWorkerSideEffects() {
        var builderCalls = 0
        var tunnelDescriptors = 0
        var workerStarts = 0
        var releases = 0

        val rejectedAdmissions = listOf(
            VpnConfigurationActivationAdmission.StaleOwnerData(41uL, 42uL),
            VpnConfigurationActivationAdmission.StaleGeneration,
        )
        rejectedAdmissions.forEach { admission ->
            assertThrows(java.io.IOException::class.java) {
                withVpnConfigurationActivation(
                    admission = admission,
                    release = { releases += 1 },
                    execute = {
                        builderCalls += 1
                        tunnelDescriptors += 1
                        workerStarts += 1
                    },
                )
            }
        }

        assertEquals(0, builderCalls)
        assertEquals(0, tunnelDescriptors)
        assertEquals(0, workerStarts)
        assertEquals(0, releases)
    }

    @Test
    fun builderFailureAndCancellationAlwaysReleaseTheAdmittedLease() {
        var releases = 0

        assertThrows(IllegalStateException::class.java) {
            withVpnConfigurationActivation(
                admission = VpnConfigurationActivationAdmission.Admitted(lease),
                release = { released ->
                    assertEquals(lease, released)
                    releases += 1
                },
                execute = { throw IllegalStateException("Builder failed") },
            )
        }
        assertEquals(1, releases)

        assertThrows(CancellationException::class.java) {
            withVpnConfigurationActivation(
                admission = VpnConfigurationActivationAdmission.Admitted(lease),
                release = { released ->
                    assertEquals(lease, released)
                    releases += 1
                },
                execute = { throw CancellationException("Start cancelled") },
            )
        }
        assertEquals(2, releases)
    }

    @Test
    fun successfulStartKeepsLeaseThroughWorkerReadinessAndReleaseFailureIsTypedUntilCleanupReadback() {
        var builderCalls = 0
        var tunnelDescriptors = 0
        var workerStarts = 0
        var releases = 0
        val events = mutableListOf<String>()
        val result = withVpnConfigurationActivation(
            admission = VpnConfigurationActivationAdmission.Admitted(lease),
            release = { released ->
                assertEquals(lease, released)
                releases += 1
                events += "lease-released"
            },
            execute = {
                builderCalls += 1
                events += "builder-configured"
                tunnelDescriptors += 1
                events += "tun-established"
                workerStarts += 1
                events += "worker-ready"
                "started"
            },
        )
        assertEquals("started", result)
        assertEquals(1, builderCalls)
        assertEquals(1, tunnelDescriptors)
        assertEquals(1, workerStarts)
        assertEquals(1, releases)
        assertEquals(
            listOf("builder-configured", "tun-established", "worker-ready", "lease-released"),
            events,
        )

        val pending = VpnConfigurationActivationLeaseState.Held(lease).afterReleaseAttempt(
            lease,
            AndroidVpnConfigurationActivationReleaseResult.LEASE_MISMATCH,
        )
        assertEquals(
            VpnConfigurationActivationLeaseState.ReleasePending(
                lease,
                VpnConfigurationActivationReleaseProblem.LeaseMismatch,
            ),
            pending,
        )
        assertEquals(
            VpnConfigurationActivationLeaseState.Released,
            pending.afterCancellation(VpnConfigurationActivationCleanupResult.Cleared),
        )
    }
}
