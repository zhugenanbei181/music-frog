package com.musicfrog.infiltrator

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Test

class AndroidVpnSnapshotFenceTest {
    @Test
    fun vpnProjectionKeepsPermissionStartupAndTerminalStatesDistinct() {
        val base = snapshot(owner = "owner", revision = 1L, vpnGeneration = 3L)
        val noResources = VpnOwnerResourceKindWire.entries.map { kind ->
            VpnOwnerResourceFactWire(kind, VpnOwnerResourceStateWire.Absent)
        }
        val awaitingPermission = base.copy(
            vpnState = VpnOwnerLifecycleWire.PermissionRequired,
            readiness = VpnOwnerReadinessWire.PermissionRequired,
            resources = noResources,
        )
        assertEquals(VpnOwnerDataPlaneState.PermissionRequired, awaitingPermission.dataPlaneState())

        val starting = base.copy(
            vpnState = VpnOwnerLifecycleWire.TunnelEstablished,
            readiness = VpnOwnerReadinessWire.AwaitingDataPlane,
            resources = noResources,
        )
        assertEquals(VpnOwnerDataPlaneState.Starting, starting.dataPlaneState())

        val stopped = base.copy(
            vpnState = VpnOwnerLifecycleWire.Stopped,
            readiness = VpnOwnerReadinessWire.Stopped,
            resources = noResources,
        )
        assertEquals(VpnOwnerDataPlaneState.Stopped, stopped.dataPlaneState())

        val revoked = stopped.copy(
            vpnState = VpnOwnerLifecycleWire.Revoked,
            readiness = VpnOwnerReadinessWire.Revoked,
        )
        assertEquals(VpnOwnerDataPlaneState.Revoked, revoked.dataPlaneState())
    }

    @Test
    fun vpnRunningRequiresForegroundAndCurrentWorkerAndTunnelResources() {
        val base = snapshot(owner = "owner", revision = 1L, vpnGeneration = 3L)
        val runningResources = VpnOwnerResourceKindWire.entries.map { kind ->
            val state = when (kind) {
                VpnOwnerResourceKindWire.Worker,
                VpnOwnerResourceKindWire.TunnelDescriptor -> VpnOwnerResourceStateWire.Held(3L)
                VpnOwnerResourceKindWire.PreparedConfiguration,
                VpnOwnerResourceKindWire.RoutingLease -> VpnOwnerResourceStateWire.Absent
            }
            VpnOwnerResourceFactWire(kind, state)
        }
        val running = base.copy(resources = runningResources)
        assertEquals(VpnOwnerDataPlaneState.Running, running.dataPlaneState())

        assertEquals(
            VpnOwnerDataPlaneState.Failed("VPN Running readback lacks foreground protection"),
            running.copy(foreground = false).dataPlaneState(),
        )
        assertEquals(
            VpnOwnerDataPlaneState.Failed("VPN Running readback lacks its current worker or tunnel resource"),
            running.copy(
                resources = runningResources.filterNot { it.kind == VpnOwnerResourceKindWire.TunnelDescriptor },
            ).dataPlaneState(),
        )
        assertEquals(
            VpnOwnerDataPlaneState.CleanupPending("descriptor close failed"),
            running.copy(
                resources = runningResources.map { fact ->
                    if (fact.kind == VpnOwnerResourceKindWire.TunnelDescriptor) {
                        fact.copy(state = VpnOwnerResourceStateWire.CleanupPending(3L, "descriptor close failed"))
                    } else {
                        fact
                    }
                },
            ).dataPlaneState(),
        )
    }

    @Test
    fun coreAndVpnLifecycleRemainSeparateFactsInAStoppedOwnerSnapshot() {
        val base = snapshot(owner = "owner", revision = 1L, vpnGeneration = 3L)
        val stoppedCore = (base.core as VpnOwnerCoreReadbackWire.Available).snapshot.copy(
            lifecycle = CoreOwnerLifecycleWire.Stopped,
            sessionToken = null,
            revision = 0L,
        )
        val noResources = VpnOwnerResourceKindWire.entries.map { kind ->
            VpnOwnerResourceFactWire(kind, VpnOwnerResourceStateWire.Absent)
        }
        val stopped = base.copy(
            vpnState = VpnOwnerLifecycleWire.Stopped,
            readiness = VpnOwnerReadinessWire.Stopped,
            resources = noResources,
            core = VpnOwnerCoreReadbackWire.Available(stoppedCore),
        )

        assertEquals(VpnOwnerDataPlaneState.Stopped, stopped.dataPlaneState())
        assertEquals(
            CoreOwnerLifecycleWire.Stopped,
            (stopped.core as VpnOwnerCoreReadbackWire.Available).snapshot.lifecycle,
        )
    }

    @Test
    fun olderOwnerRevisionAndRetiredOwnerCannotReplaceSnapshot() {
        val fence = AndroidVpnSnapshotFence()
        val current = snapshot(owner = "owner-b", revision = 12L, vpnGeneration = 8L)
        assertEquals(VpnSnapshotAcceptance.Accepted, fence.admit(version(22L, current)))

        assertEquals(
            VpnSnapshotAcceptance.Stale,
            fence.admit(version(23L, snapshot(owner = "owner-a", revision = 13L, vpnGeneration = 9L))),
        )
        assertEquals(
            VpnSnapshotAcceptance.Stale,
            fence.admit(version(24L, snapshot(owner = "owner-b", revision = 11L, vpnGeneration = 7L))),
        )
    }

    @Test
    fun equalOwnerRevisionOnlyAcceptsIdenticalCompleteFacts() {
        val fence = AndroidVpnSnapshotFence()
        val current = snapshot(owner = "owner", revision = 5L, vpnGeneration = 3L)
        assertEquals(VpnSnapshotAcceptance.Accepted, fence.admit(version(5L, current)))
        assertEquals(VpnSnapshotAcceptance.Accepted, fence.admit(version(6L, current)))

        val changedResource = snapshot(
            owner = "owner",
            revision = 5L,
            vpnGeneration = 3L,
            resourceState = VpnOwnerResourceStateWire.CleanupPending(3L, "descriptor close failed"),
        )
        assertEquals(VpnSnapshotAcceptance.Stale, fence.admit(version(7L, changedResource)))
    }

    @Test
    fun higherVpnRevisionCannotRollCoreGenerationBackward() {
        val fence = AndroidVpnSnapshotFence()
        val current = snapshot(owner = "owner", revision = 7L, vpnGeneration = 4L, coreGeneration = 12L)
        val staleCore = snapshot(owner = "owner", revision = 8L, vpnGeneration = 4L, coreGeneration = 11L)
        assertEquals(VpnSnapshotAcceptance.Accepted, fence.admit(version(1L, current)))
        assertEquals(VpnSnapshotAcceptance.Stale, fence.admit(version(2L, staleCore)))
    }

    @Test
    fun sameCoreGenerationCannotReplaceItsOwnerIdentity() {
        val fence = AndroidVpnSnapshotFence()
        val current = snapshot(owner = "owner", revision = 7L, vpnGeneration = 4L, coreGeneration = 12L)
        val currentCore = current.core as VpnOwnerCoreReadbackWire.Available
        val changedIdentity = current.copy(
            revision = 8L,
            core = VpnOwnerCoreReadbackWire.Available(
                currentCore.snapshot.copy(
                    identity = CoreOwnerIdentityWire.Present("sing-box", "core-b", "1.14.3"),
                ),
            ),
        )

        assertEquals(VpnSnapshotAcceptance.Accepted, fence.admit(version(1L, current)))
        assertEquals(VpnSnapshotAcceptance.Stale, fence.admit(version(2L, changedIdentity)))
    }

    @Test
    fun wireParserKeepsCoreAndVpnFactsInOneSnapshot() {
        val snapshot = snapshot(owner = "owner", revision = 9L, vpnGeneration = 5L)
        assertNotNull(snapshot.core as? VpnOwnerCoreReadbackWire.Available)
        assertEquals(5L, snapshot.vpnGeneration)
        assertEquals(9L, snapshot.revision)
        assertEquals(VpnOwnerLifecycleWire.Running, snapshot.vpnState)
        assertEquals(VpnOwnerReadinessWire.Ready, snapshot.readiness)
        assertEquals(12L, (snapshot.core as VpnOwnerCoreReadbackWire.Available).snapshot.generation)
    }

    @Test
    fun publisherKeepsRevisionForIdenticalFactsAndAdvancesForChangedFacts() {
        val publisher = VpnOwnerSnapshotPublisher()
        val facts = facts(owner = "owner", vpnGeneration = 3L)
        val first = VpnOwnerSnapshotWire.parse(publisher.publish(facts)) ?: error("first snapshot must parse")
        val duplicate = VpnOwnerSnapshotWire.parse(publisher.publish(facts)) ?: error("duplicate must parse")
        val changed = VpnOwnerSnapshotWire.parse(
            publisher.publish(facts.copy(
                vpnState = VpnOwnerLifecycleWire.Stopping,
                readiness = VpnOwnerReadinessWire.Stopping,
            )),
        ) ?: error("changed snapshot must parse")

        assertEquals(1L, first.revision)
        assertEquals(first.revision, duplicate.revision)
        assertEquals(2L, changed.revision)
    }

    private fun version(requestId: Long, snapshot: VpnOwnerSnapshotWire) =
        VpnOwnerSnapshotVersion(snapshot.ownerSessionId, requestId, snapshot)

    private fun snapshot(
        owner: String,
        revision: Long,
        vpnGeneration: Long,
        coreGeneration: Long = 12L,
        resourceState: VpnOwnerResourceStateWire = VpnOwnerResourceStateWire.Held(vpnGeneration),
    ): VpnOwnerSnapshotWire {
        val publisher = VpnOwnerSnapshotPublisher()
        var currentFacts = facts(owner, vpnGeneration, coreGeneration, resourceState)
        val core = currentFacts.core as VpnOwnerCoreReadbackWire.Available
        currentFacts = currentFacts.copy(
            core = VpnOwnerCoreReadbackWire.Available(core.snapshot.copy(revision = 0L)),
        )
        var published = VpnOwnerSnapshotWire.parse(publisher.publish(currentFacts))
            ?: error("initial owner snapshot must parse")
        while (published.revision < revision) {
            val currentCore = currentFacts.core as VpnOwnerCoreReadbackWire.Available
            currentFacts = currentFacts.copy(
                core = VpnOwnerCoreReadbackWire.Available(
                    currentCore.snapshot.copy(revision = published.revision),
                ),
            )
            published = VpnOwnerSnapshotWire.parse(publisher.publish(currentFacts))
                ?: error("advanced owner snapshot must parse")
        }
        return published
    }

    private fun facts(
        owner: String,
        vpnGeneration: Long,
        coreGeneration: Long = 12L,
        resourceState: VpnOwnerResourceStateWire = VpnOwnerResourceStateWire.Held(vpnGeneration),
    ) = VpnOwnerSnapshotFacts(
        ownerSessionId = owner,
        vpnGeneration = vpnGeneration,
        vpnState = VpnOwnerLifecycleWire.Running,
        readiness = VpnOwnerReadinessWire.Ready,
        resources = VpnOwnerResourceKindWire.entries.map { kind ->
            VpnOwnerResourceFactWire(
                kind,
                if (kind == VpnOwnerResourceKindWire.Worker) resourceState else VpnOwnerResourceStateWire.Absent,
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
                generation = coreGeneration,
                sessionToken = "123",
                revision = 21L,
                failure = null,
            ),
        ),
    )

    @Test
    fun failedCoreSnapshotKeepsTypedFailureAcrossVpnOwnerWireRoundTrip() {
        val failure = CoreOwnerFailureWire(
            code = CoreOwnerFailureCodeWire.Configuration,
            message = "profile YAML could not be parsed",
            retryable = true,
            reason = CoreOwnerFailureReasonWire.YamlSyntax(line = 23, column = 6),
        )
        val facts = facts("owner-a", 8L).copy(
            core = VpnOwnerCoreReadbackWire.Available(
                CoreOwnerSnapshotWire(
                    identity = CoreOwnerIdentityWire.Present("mihomo", "core-a", "1.20.0"),
                    lifecycle = CoreOwnerLifecycleWire.Failed,
                    generation = 12L,
                    sessionToken = "340282366920938463463374607431768211455",
                    revision = 22L,
                    failure = failure,
                ),
            ),
        )
        val publisher = VpnOwnerSnapshotPublisher()
        val raw = publisher.publish(facts)
        val coreSnapshotJson = org.json.JSONObject(raw)
            .getJSONObject("core")
            .getJSONObject("Available")
        assertEquals(
            setOf("identity", "lifecycle", "generation", "session_token", "revision", "failure"),
            coreSnapshotJson.keys().asSequence().toSet(),
        )
        assertEquals(
            setOf("code", "message", "retryable", "reason"),
            coreSnapshotJson.getJSONObject("failure").keys().asSequence().toSet(),
        )
        val parsed = VpnOwnerSnapshotWire.parse(raw) ?: error("failed Core facts must parse")
        val available = parsed.core as? VpnOwnerCoreReadbackWire.Available
            ?: error("successfully read failed Core lifecycle remains Available")

        assertEquals(CoreOwnerLifecycleWire.Failed, available.snapshot.lifecycle)
        assertEquals(failure, available.snapshot.failure)
        assertEquals(
            "340282366920938463463374607431768211455",
            available.snapshot.sessionToken,
        )
        assertEquals(
            failure,
            CoreOwnerFailureWire.parse(available.snapshot.failure!!.toJson().toString()),
        )

        val changedFailureFacts = facts.copy(
            core = VpnOwnerCoreReadbackWire.Available(
                available.snapshot.copy(
                    failure = failure.copy(retryable = false),
                ),
            ),
        )
        val revised = VpnOwnerSnapshotWire.parse(publisher.publish(changedFailureFacts))
            ?: error("changed Core failure must parse")
        assertEquals(parsed.revision + 1L, revised.revision)
    }
}
