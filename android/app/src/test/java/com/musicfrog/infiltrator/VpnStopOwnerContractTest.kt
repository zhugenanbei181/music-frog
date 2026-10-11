package com.musicfrog.infiltrator

import infiltrator_android.AndroidHostReply
import infiltrator_android.FfiErrorCode
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class VpnStopOwnerContractTest {
    @Test
    fun unavailableAndCallerTerminalsRemainDistinct() {
        listOf(
            OwnerClientTerminal.OwnerUnavailable,
            OwnerClientTerminal.BindRejected,
            OwnerClientTerminal.NullBinding,
            OwnerClientTerminal.BinderDied,
            OwnerClientTerminal.BindingDied,
            OwnerClientTerminal.SendFailed,
        ).forEach { terminal ->
            assertTrue(
                readback(terminal = terminal).toReceipt() is VpnOwnerStopReceiptWire.OwnerUnavailable,
            )
        }
        assertTrue(
            readback(terminal = OwnerClientTerminal.ReplyTimedOut).toReceipt() is
                VpnOwnerStopReceiptWire.TimedOut,
        )
        assertTrue(
            readback(terminal = OwnerClientTerminal.OwnerSessionStale).toReceipt() is
                VpnOwnerStopReceiptWire.Superseded,
        )
        listOf(
            OwnerClientTerminal.InvalidReply,
            OwnerClientTerminal.OwnerSessionReadbackRequired,
            OwnerClientTerminal.CoreSnapshotStale,
            OwnerClientTerminal.VpnSnapshotStale,
            OwnerClientTerminal.OwnerDataStale,
        ).forEach { terminal ->
            assertTrue(
                readback(terminal = terminal, reply = AndroidHostReply.STALE_SNAPSHOT).toReceipt() is
                    VpnOwnerStopReceiptWire.Failed,
            )
        }
        assertTrue(
            readback(
                terminal = OwnerClientTerminal.OwnerSessionReadbackRequired,
                reply = AndroidHostReply.SERVICE_UNAVAILABLE,
            ).toReceipt() is VpnOwnerStopReceiptWire.OwnerUnavailable,
        )
    }

    @Test
    fun eachOwnerTerminalUsesTheMatchingSnapshotBearingReceiptVariant() {
        val stoppedSnapshot = snapshot(
            VpnOwnerLifecycleWire.Stopped,
            VpnOwnerReadinessWire.Stopped,
        )
        assertEquals(
            VpnOwnerStopReceiptWire.Stopped(stoppedSnapshot),
            readback(
                reply = AndroidHostReply.VPN_STOPPED,
                statusCode = FfiErrorCode.OK.value.toInt(),
                evidence = VpnStopReadbackEvidence.OwnerSnapshot(stoppedSnapshot),
            ).toReceipt(),
        )
        assertTrue(
            readback(
                reply = AndroidHostReply.VPN_STOPPED,
                statusCode = FfiErrorCode.OK.value.toInt(),
                evidence = VpnStopReadbackEvidence.OwnerSnapshot(
                    snapshot(VpnOwnerLifecycleWire.Stopped, VpnOwnerReadinessWire.CleanupPending),
                ),
            ).toReceipt() is VpnOwnerStopReceiptWire.FailedWithReadback,
        )
        assertTrue(
            readback(
                reply = AndroidHostReply.VPN_STOPPED,
                statusCode = FfiErrorCode.OK.value.toInt(),
                evidence = VpnStopReadbackEvidence.OwnerSnapshot(
                    snapshot(VpnOwnerLifecycleWire.Running, VpnOwnerReadinessWire.Ready),
                ),
            ).toReceipt() is VpnOwnerStopReceiptWire.FailedWithReadback,
        )
        assertTrue(
            readback(
                reply = AndroidHostReply.VPN_STOP_TIMED_OUT,
                statusCode = FfiErrorCode.NOT_READY.value.toInt(),
                evidence = VpnStopReadbackEvidence.OwnerSnapshot(
                    snapshot(VpnOwnerLifecycleWire.Stopping, VpnOwnerReadinessWire.Stopping),
                ),
            ).toReceipt() is VpnOwnerStopReceiptWire.TimedOutWithReadback,
        )
        assertTrue(
            readback(
                reply = AndroidHostReply.VPN_STOP_TIMED_OUT,
                statusCode = FfiErrorCode.OK.value.toInt(),
                evidence = VpnStopReadbackEvidence.OwnerSnapshot(
                    snapshot(VpnOwnerLifecycleWire.Stopping, VpnOwnerReadinessWire.Stopping),
                ),
            ).toReceipt() is VpnOwnerStopReceiptWire.FailedWithReadback,
        )
        assertTrue(
            readback(
                reply = AndroidHostReply.VPN_STOP_TIMED_OUT,
                statusCode = FfiErrorCode.NOT_READY.value.toInt(),
                evidence = VpnStopReadbackEvidence.Missing,
            ).toReceipt() is VpnOwnerStopReceiptWire.Failed,
        )
        assertTrue(
            readback(
                reply = AndroidHostReply.VPN_STOP_SUPERSEDED,
                statusCode = FfiErrorCode.INVALID_STATE.value.toInt(),
                evidence = VpnStopReadbackEvidence.OwnerSnapshot(
                    snapshot(VpnOwnerLifecycleWire.Running, VpnOwnerReadinessWire.Ready),
                ),
            ).toReceipt() is VpnOwnerStopReceiptWire.SupersededWithReadback,
        )
        assertTrue(
            readback(
                reply = AndroidHostReply.VPN_STOP_CANCELED,
                statusCode = FfiErrorCode.NOT_READY.value.toInt(),
                evidence = VpnStopReadbackEvidence.OwnerSnapshot(
                    snapshot(VpnOwnerLifecycleWire.Stopping, VpnOwnerReadinessWire.Stopping),
                ),
            ).toReceipt() is VpnOwnerStopReceiptWire.CanceledWithReadback,
        )
        assertTrue(
            readback(
                reply = AndroidHostReply.VPN_STOP_CANCELED,
                statusCode = FfiErrorCode.OK.value.toInt(),
                evidence = VpnStopReadbackEvidence.Missing,
            ).toReceipt() is VpnOwnerStopReceiptWire.Failed,
        )
    }

    @Test
    fun localStopResultsBecomeTypedReceiptsWithMandatoryStoppedReadback() {
        val stopped = snapshot(VpnOwnerLifecycleWire.Stopped, VpnOwnerReadinessWire.Stopped)
        assertEquals(
            VpnOwnerStopReceiptWire.Stopped(stopped),
            VpnStopResult.Stopped.toOwnerStopReceipt(VpnStopReadbackEvidence.OwnerSnapshot(stopped)),
        )
        assertTrue(
            VpnStopResult.Stopped.toOwnerStopReceipt(VpnStopReadbackEvidence.Missing) is
                VpnOwnerStopReceiptWire.Failed,
        )
        assertEquals(
            VpnOwnerStopReceiptWire.TimedOut(
                VpnStopFailureWire(
                    VpnStopFailureCode.NotReady,
                    "VPN owner stop timed out",
                    true,
                ),
            ),
            VpnStopResult.TimedOut.toOwnerStopReceipt(VpnStopReadbackEvidence.Missing),
        )
        assertTrue(
            VpnStopResult.Failed("worker cleanup failed").toOwnerStopReceipt(
                VpnStopReadbackEvidence.OwnerSnapshot(stopped),
            ) is VpnOwnerStopReceiptWire.FailedWithReadback,
        )
    }

    @Test
    fun receiptWireSerializesOneTaggedVariantAndItsRequiredPayload() {
        val snapshot = snapshot(VpnOwnerLifecycleWire.Stopped, VpnOwnerReadinessWire.Stopped)
        val encoded = readback(
            reply = AndroidHostReply.VPN_STOPPED,
            statusCode = FfiErrorCode.OK.value.toInt(),
            evidence = VpnStopReadbackEvidence.OwnerSnapshot(snapshot),
        ).toReceiptJson()
        val receipt = org.json.JSONObject(encoded)
        assertEquals("Stopped", receipt.getString("receipt"))
        assertEquals(snapshot.ownerSessionId, receipt.getJSONObject("snapshot").getString("owner_session_id"))
        assertEquals(snapshot.revision, receipt.getJSONObject("snapshot").getLong("revision"))
        assertTrue(!receipt.has("outcome"))
        val ownerUnavailable = org.json.JSONObject(
            vpnOwnerStopReceiptJson(
                VpnOwnerStopReceiptWire.OwnerUnavailable(
                    VpnStopFailureWire(
                        VpnStopFailureCode.NotReady,
                        "owner unavailable",
                        true,
                    ),
                ),
            ),
        )
        assertEquals("OwnerUnavailable", ownerUnavailable.getString("receipt"))
        assertEquals(
            "NotReady",
            ownerUnavailable.getJSONObject("failure").getString("code"),
        )
        assertTrue(!ownerUnavailable.has("snapshot"))

        val failure = VpnStopFailureWire(
            VpnStopFailureCode.NotReady,
            "stop did not complete",
            true,
        )
        val cases = listOf(
            Triple(VpnOwnerStopReceiptWire.OwnerUnavailable(failure), "OwnerUnavailable", false),
            Triple(
                VpnOwnerStopReceiptWire.OwnerUnavailableWithReadback(failure, snapshot),
                "OwnerUnavailableWithReadback",
                true,
            ),
            Triple(VpnOwnerStopReceiptWire.Failed(failure), "Failed", false),
            Triple(VpnOwnerStopReceiptWire.FailedWithReadback(failure, snapshot), "FailedWithReadback", true),
            Triple(VpnOwnerStopReceiptWire.TimedOut(failure), "TimedOut", false),
            Triple(VpnOwnerStopReceiptWire.TimedOutWithReadback(failure, snapshot), "TimedOutWithReadback", true),
            Triple(VpnOwnerStopReceiptWire.Superseded(failure), "Superseded", false),
            Triple(VpnOwnerStopReceiptWire.SupersededWithReadback(failure, snapshot), "SupersededWithReadback", true),
            Triple(VpnOwnerStopReceiptWire.Canceled(failure), "Canceled", false),
            Triple(VpnOwnerStopReceiptWire.CanceledWithReadback(failure, snapshot), "CanceledWithReadback", true),
        )
        cases.forEach { (typedReceipt, terminal, hasSnapshot) ->
            val encoded = org.json.JSONObject(vpnOwnerStopReceiptJson(typedReceipt))
            assertEquals(terminal, encoded.getString("receipt"))
            assertEquals(hasSnapshot, encoded.has("snapshot"))
            assertTrue(encoded.has("failure"))
            assertTrue(!encoded.has("outcome"))
        }
    }

    @Test
    fun repeatedStopOnCleanOwnerIsIdempotent() {
        val lifecycle = VpnLifecycleCoordinator()
        val first = lifecycle.requestCleanup(VpnCleanupIntent.Stop)
        val repeated = lifecycle.requestCleanup(VpnCleanupIntent.Stop)

        assertEquals(VpnCleanupPlan.AlreadyClean(0L), first)
        assertEquals(VpnCleanupPlan.AlreadyClean(0L), repeated)
    }

    private fun readback(
        terminal: OwnerClientTerminal = OwnerClientTerminal.ReplyReceived,
        reply: AndroidHostReply = AndroidHostReply.SERVICE_UNAVAILABLE,
        statusCode: Int = FfiErrorCode.NOT_READY.value.toInt(),
        evidence: VpnStopReadbackEvidence = VpnStopReadbackEvidence.Missing,
    ) = VpnStopOwnerReadback(
        reply = reply,
        statusCode = statusCode,
        evidence = evidence,
        terminal = terminal,
    )

    private fun snapshot(
        state: VpnOwnerLifecycleWire,
        readiness: VpnOwnerReadinessWire,
    ): VpnOwnerSnapshotWire {
        val resources = VpnOwnerResourceKindWire.entries.map { kind ->
            val resourceState = if (
                readiness == VpnOwnerReadinessWire.CleanupPending &&
                kind == VpnOwnerResourceKindWire.TunnelDescriptor
            ) {
                VpnOwnerResourceStateWire.CleanupPending(4L, "close failed")
            } else {
                VpnOwnerResourceStateWire.Absent
            }
            VpnOwnerResourceFactWire(kind, resourceState)
        }
        val json = VpnOwnerSnapshotPublisher().publish(
            VpnOwnerSnapshotFacts(
                ownerSessionId = "owner-stop-session",
                vpnGeneration = 4L,
                vpnState = state,
                readiness = readiness,
                resources = resources,
                foreground = state != VpnOwnerLifecycleWire.Stopped && state != VpnOwnerLifecycleWire.Revoked,
                mtu = null,
                routeCount = 0,
                dnsServers = emptyList(),
                ipv6 = false,
                core = VpnOwnerCoreReadbackWire.Unavailable("core readback unavailable"),
            ),
        )
        return VpnOwnerSnapshotWire.parse(json) ?: error("owner snapshot must parse")
    }
}
