package com.musicfrog.infiltrator

import infiltrator_android.AndroidOwnerDataDomain
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class OwnerResponsePayloadTest {
    @Test
    fun everyOwnerReplyUsesOneTypedPayloadVariant() {
        val core = OwnerCoreSnapshot(
            lifecycle = CoreOwnerLifecycleWire.Stopped,
            generation = 7L,
            sessionToken = "core-session",
            revision = 12L,
            identity = CoreOwnerIdentityWire.Absent,
            failure = null,
        )
        val payloads = listOf(
            OwnerResponsePayload.Failure,
            OwnerResponsePayload.CoreLifecycleSnapshot(core),
            OwnerResponsePayload.VpnOwnerSnapshot("{}"),
            OwnerResponsePayload.SurfaceSnapshot("{}", OwnerCoreReadback.Unavailable),
            OwnerResponsePayload.OwnerDataRevision(4L, 7L, OwnerCoreReadback.Available(core)),
            OwnerResponsePayload.Authorization(OwnerVpnAuthorization.Authorized),
            OwnerResponsePayload.ProductCommand("{}", OwnerCoreReadback.Available(core)),
            OwnerResponsePayload.OwnerOperation(
                outcomeJson = "{}",
                revision = 4L,
                generation = 7L,
                dataFacts = OwnerOperationDataFacts.Snapshot(
                    AndroidOwnerDataDomain.PROFILES,
                    "{}",
                ),
                coreReadback = OwnerCoreReadback.Available(core),
            ),
            OwnerResponsePayload.StopVpn("{}"),
            OwnerResponsePayload.VpnStart(VpnStartOutcomeWire.Running),
            OwnerResponsePayload.VpnConfiguration(VpnConfigurationApplyOutcomeWire.Accepted),
            OwnerResponsePayload.OwnerSessionReadback(4L, OwnerCoreReadback.Available(core)),
        )

        assertEquals(
            OwnerResponsePayloadKind.entries,
            payloads.map { it.kind() },
        )
    }

    @Test
    fun ownerOperationKeepsRevisionDomainSnapshotAndCoreGenerationTogether() {
        val payload = OwnerResponsePayload.OwnerOperation(
            outcomeJson = "{\"outcome\":\"Applied\"}",
            revision = 31L,
            generation = 6L,
            dataFacts = OwnerOperationDataFacts.Snapshot(
                AndroidOwnerDataDomain.PROFILES,
                "{\"profiles\":[]}",
            ),
            coreReadback = OwnerCoreReadback.Unavailable,
        )
        val newerCore = OwnerCoreSnapshot(
            lifecycle = CoreOwnerLifecycleWire.Running,
            generation = 8L,
            sessionToken = "core-session-8",
            revision = 14L,
            identity = CoreOwnerIdentityWire.Present("mihomo", "core-8", "1.19.32"),
            failure = null,
        )

        val readback = payload.withCoreReadback(OwnerCoreReadback.Available(newerCore))
        assertTrue(readback is OwnerResponsePayload.OwnerOperation)
        val operation = readback as OwnerResponsePayload.OwnerOperation
        assertEquals(OwnerResponsePayloadKind.OwnerOperation, operation.kind())
        assertEquals(31L, operation.revision)
        assertEquals(8L, operation.generation)
        assertEquals(
            OwnerOperationDataFacts.Snapshot(
                AndroidOwnerDataDomain.PROFILES,
                "{\"profiles\":[]}",
            ),
            operation.dataFacts,
        )
        assertEquals(OwnerCoreReadback.Available(newerCore), operation.coreReadback)
    }

    @Test
    fun revisionOnlyReadbackDoesNotInventOptionalOwnerDataFacts() {
        val payload = OwnerResponsePayload.OwnerDataRevision(
            revision = 5L,
            generation = 3L,
            coreReadback = OwnerCoreReadback.Unavailable,
        )

        assertEquals(
            OwnerDataReadback.Available(
                revision = 5L,
                generation = 3L,
                facts = OwnerOperationDataFacts.RevisionOnly,
            ),
            payload.ownerDataReadback(),
        )
    }
}
