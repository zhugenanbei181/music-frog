package com.musicfrog.infiltrator

import infiltrator_android.AndroidVpnConfigurationActivationLease
import infiltrator_android.AndroidVpnConfigurationActivationReleaseResult
import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test
import java.util.concurrent.CancellationException

class VpnConfigurationActivationTest {
    private val prepared = requireNotNull(NativePreparedVpnConfiguration.parse(PREPARED_CONFIGURATION))
    private val lease = AndroidVpnConfigurationActivationLease(
        leaseId = 7uL,
        preparationId = 19uL,
        vpnGeneration = 23uL,
        ownerDataRevision = 41uL,
    )
    private val matchingLease = lease.copy(
        preparationId = prepared.preparationId,
        vpnGeneration = prepared.vpnGeneration,
        ownerDataRevision = prepared.ownerDataRevision.value,
    )

    @Test
    fun staleRevisionOrGenerationRunsNoBuilderTunnelOrWorkerSideEffects() {
        var builderCalls = 0
        var tunnelDescriptors = 0
        var workerStarts = 0
        var releases = 0

        val rejectedAdmissions = listOf(
            VpnConfigurationActivationAdmission.StaleOwnerData(
                requireNotNull(VpnOwnerDataRevision.fromWire(41uL)),
                requireNotNull(VpnOwnerDataRevision.fromWire(42uL)),
            ),
            VpnConfigurationActivationAdmission.StaleGeneration,
        )
        rejectedAdmissions.forEach { admission ->
            assertThrows(java.io.IOException::class.java) {
                withVpnConfigurationActivation(
                    prepared = prepared,
                    admission = admission,
                    release = { releases += 1 },
                    execute = { _, _ ->
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
                prepared = prepared,
                admission = VpnConfigurationActivationAdmission.Admitted(matchingLease),
                release = { released ->
                    assertEquals(matchingLease, released)
                    releases += 1
                },
                execute = { _, _ -> throw IllegalStateException("Builder failed") },
            )
        }
        assertEquals(1, releases)

        assertThrows(CancellationException::class.java) {
            withVpnConfigurationActivation(
                prepared = prepared,
                admission = VpnConfigurationActivationAdmission.Admitted(matchingLease),
                release = { released ->
                    assertEquals(matchingLease, released)
                    releases += 1
                },
                execute = { _, _ -> throw CancellationException("Start cancelled") },
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
            prepared = prepared,
            admission = VpnConfigurationActivationAdmission.Admitted(matchingLease),
            release = { released ->
                assertEquals(matchingLease, released)
                releases += 1
                events += "lease-released"
            },
            execute = { configuration, admittedLease ->
                builderCalls += 1
                assertEquals(prepared.configuration, configuration)
                assertEquals(matchingLease, admittedLease)
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

    @Test
    fun settingsWriteRevisionAdvanceRejectsOldFrozenConfigAndOnlyLatestFreezeReachesBuilder() {
        val revisionAfterWrite = requireNotNull(VpnOwnerDataRevision.fromWire(45uL))
        assertEquals(prepared.ownerDataRevision.value + 1uL, revisionAfterWrite.value)

        var builderCalls = 0
        var releases = 0
        assertThrows(java.io.IOException::class.java) {
            withVpnConfigurationActivation(
                prepared = prepared,
                admission = VpnConfigurationActivationAdmission.StaleOwnerData(
                    prepared.ownerDataRevision,
                    revisionAfterWrite,
                ),
                release = { releases += 1 },
                execute = { _, _ -> builderCalls += 1 },
            )
        }
        assertEquals(0, builderCalls)
        assertEquals(0, releases)

        val latestPrepared = requireNotNull(
            NativePreparedVpnConfiguration.parse(
                PREPARED_CONFIGURATION
                    .replace("\"preparation_id\": 21", "\"preparation_id\": 22")
                    .replace("\"owner_data_revision\": 44", "\"owner_data_revision\": 45")
                    .replace("\"mtu\": 1420", "\"mtu\": 1500")
                    .replace("\"1.1.1.1\"", "\"9.9.9.9\""),
            ),
        )
        val latestLease = lease.copy(
            preparationId = latestPrepared.preparationId,
            vpnGeneration = latestPrepared.vpnGeneration,
            ownerDataRevision = latestPrepared.ownerDataRevision.value,
        )
        var builderConfiguration: NativeVpnConfiguration? = null
        val result = withVpnConfigurationActivation(
            prepared = latestPrepared,
            admission = VpnConfigurationActivationAdmission.Admitted(latestLease),
            release = { released ->
                assertEquals(latestLease, released)
                releases += 1
            },
            execute = { configuration, _ ->
                builderCalls += 1
                builderConfiguration = configuration
                "started"
            },
        )

        assertEquals("started", result)
        assertEquals(1, builderCalls)
        assertEquals(1, releases)
        assertEquals(1500, builderConfiguration?.mtu)
        assertEquals(listOf("9.9.9.9"), builderConfiguration?.dnsServers)
        assertEquals(revisionAfterWrite, latestPrepared.ownerDataRevision)
    }

    @Test
    fun admittedLeaseWithDifferentRevisionCannotReachBuilderAndIsReleased() {
        val mismatchedLease = lease.copy(
            preparationId = prepared.preparationId,
            vpnGeneration = prepared.vpnGeneration,
            ownerDataRevision = prepared.ownerDataRevision.value + 1uL,
        )
        assertEquals(
            VpnPreparedConfigurationLeaseMatch.RevisionMismatch(
                prepared.ownerDataRevision,
                requireNotNull(VpnOwnerDataRevision.fromWire(mismatchedLease.ownerDataRevision)),
            ),
            prepared.matchActivationLease(mismatchedLease),
        )

        var builderCalls = 0
        var releases = 0
        assertThrows(java.io.IOException::class.java) {
            withVpnConfigurationActivation(
                prepared = prepared,
                admission = VpnConfigurationActivationAdmission.Admitted(mismatchedLease),
                release = { released ->
                    assertEquals(mismatchedLease, released)
                    releases += 1
                },
                execute = { _, _ -> builderCalls += 1 },
            )
        }
        assertEquals(0, builderCalls)
        assertEquals(1, releases)

        val invalidLease = matchingLease.copy(ownerDataRevision = 0uL)
        assertEquals(
            VpnPreparedConfigurationLeaseMatch.InvalidLeaseRevision,
            prepared.matchActivationLease(invalidLease),
        )
    }

    @Test
    fun activationLeaseMustMatchFrozenPreparationAndVpnGeneration() {
        assertEquals(
            VpnPreparedConfigurationLeaseMatch.Matched,
            prepared.matchActivationLease(matchingLease),
        )
        assertEquals(
            VpnPreparedConfigurationLeaseMatch.TransactionMismatch,
            prepared.matchActivationLease(matchingLease.copy(preparationId = matchingLease.preparationId + 1uL)),
        )
        assertEquals(
            VpnPreparedConfigurationLeaseMatch.StaleGeneration,
            prepared.matchActivationLease(matchingLease.copy(vpnGeneration = matchingLease.vpnGeneration + 1uL)),
        )
    }

    private companion object {
        const val PREPARED_CONFIGURATION = """
            {
              "preparation_id": 21,
              "vpn_generation": 8,
              "owner_data_revision": 44,
              "core_scope": {
                "engine_id": "mihomo",
                "instance_id": "primary",
                "version": "1.19.12",
                "generation": 17,
                "session_token": "340282366920938463463374607431768211455"
              },
              "configuration": {
                "proxy_endpoint": "socks5://127.0.0.1:7891",
                "mtu": 1420,
                "routes": [{"address":"0.0.0.0","prefix":0,"exclude":false}],
                "dns_servers": ["1.1.1.1"],
                "ipv6": false,
                "foreground_requested": true
              }
            }
        """
    }
}
