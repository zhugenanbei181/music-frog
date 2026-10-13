package com.musicfrog.infiltrator

import org.junit.Assert.assertEquals
import org.junit.Test

class AndroidBridgeCallRouterTest {
    @Test
    fun eachRustCallRoutesToOneMatchingTypedHostReply() {
        val host = RecordingHost()
        val router = AndroidBridgeCallRouter(host)
        val cases = listOf(
            AndroidBridgeCall.CoreStart("start-request") to AndroidBridgeCallReply.CoreStart(1),
            AndroidBridgeCall.CoreStop to AndroidBridgeCallReply.CoreStop(2),
            AndroidBridgeCall.CoreProcessObservation to
                AndroidBridgeCallReply.CoreProcessObservation("observation"),
            AndroidBridgeCall.CoreProcessRecoveryCompleted("context") to
                AndroidBridgeCallReply.CoreProcessRecoveryCompleted(3),
            AndroidBridgeCall.CoreValidateConfiguration("validation") to
                AndroidBridgeCallReply.CoreValidateConfiguration("validation-receipt"),
            AndroidBridgeCall.CoreLifecycleSnapshot to
                AndroidBridgeCallReply.CoreLifecycleSnapshot("core-snapshot"),
            AndroidBridgeCall.ExecuteProductCommand("product") to
                AndroidBridgeCallReply.ExecuteProductCommand("product-result"),
            AndroidBridgeCall.SurfaceSnapshot to
                AndroidBridgeCallReply.SurfaceSnapshot("surface-snapshot"),
            AndroidBridgeCall.ExecuteOwnerOperation("operation") to
                AndroidBridgeCallReply.ExecuteOwnerOperation("operation-result"),
            AndroidBridgeCall.CredentialGet("service", "key") to
                AndroidBridgeCallReply.CredentialGet("secret"),
            AndroidBridgeCall.CredentialSet("service", "key", "value") to
                AndroidBridgeCallReply.CredentialSet(AndroidCredentialMutationOutcome.Applied),
            AndroidBridgeCall.CredentialDelete("service", "key") to
                AndroidBridgeCallReply.CredentialDelete(AndroidCredentialMutationOutcome.Applied),
            AndroidBridgeCall.VpnStart to AndroidBridgeCallReply.VpnStart(4),
            AndroidBridgeCall.VpnAuthorizationState to AndroidBridgeCallReply.VpnAuthorizationState(5),
            AndroidBridgeCall.VpnApplyConfiguration("prepared") to
                AndroidBridgeCallReply.VpnConfigurationApply(6),
            AndroidBridgeCall.VpnStop(1, 7L) to AndroidBridgeCallReply.VpnStop("stop-receipt"),
            AndroidBridgeCall.VpnOwnerSnapshot to AndroidBridgeCallReply.VpnOwnerSnapshot("vpn-snapshot"),
        )

        val deadline = VpnOwnerRequestDeadline.after(1L, 30_000L)
        cases.forEach { (call, expected) -> assertEquals(expected, router.execute(call, deadline)) }
        assertEquals(cases.size, host.calls.size)
        assertEquals(9, host.deadlines.size)
        assertEquals(setOf(deadline.elapsedRealtimeMillis), host.deadlines.toSet())
    }

    private class RecordingHost : AndroidBridgeCallHost {
        val calls = mutableListOf<String>()
        val deadlines = mutableListOf<Long>()

        override fun coreStart(startRequestJson: String): Int = record("core-start:$startRequestJson", 1)
        override fun coreValidateConfiguration(requestJson: String): String =
            record("validate:$requestJson", "validation-receipt")
        override fun coreStop(): Int = record("core-stop", 2)
        override fun coreProcessObservationJson(): String = record("process-observation", "observation")
        override fun coreProcessExitRecovered(contextJson: String): Int = record("recovered:$contextJson", 3)
        override fun coreLifecycleSnapshot(deadline: VpnOwnerRequestDeadline): String? =
            recordDeadline(deadline, "core-snapshot", "core-snapshot")
        override fun executeProductCommandJson(
            commandJson: String,
            deadline: VpnOwnerRequestDeadline,
        ): String? = recordDeadline(deadline, "product:$commandJson", "product-result")
        override fun surfaceSnapshotJson(deadline: VpnOwnerRequestDeadline): String? =
            recordDeadline(deadline, "surface", "surface-snapshot")
        override fun executeOwnerOperationJson(
            operationJson: String,
            deadline: VpnOwnerRequestDeadline,
        ): String? = recordDeadline(deadline, "operation:$operationJson", "operation-result")
        override fun credentialGet(service: String, key: String): String? =
            record("credential-get:$service:$key", "secret")
        override fun credentialSet(
            service: String,
            key: String,
            value: String,
        ): AndroidCredentialMutationOutcome {
            record("credential-set:$service:$key:$value", Unit)
            return AndroidCredentialMutationOutcome.Applied
        }
        override fun credentialDelete(
            service: String,
            key: String,
        ): AndroidCredentialMutationOutcome {
            record("credential-delete:$service:$key", Unit)
            return AndroidCredentialMutationOutcome.Applied
        }
        override fun vpnStartOutcome(deadline: VpnOwnerRequestDeadline): Int =
            recordDeadline(deadline, "vpn-start", 4)
        override fun vpnAuthorizationState(deadline: VpnOwnerRequestDeadline): Int =
            recordDeadline(deadline, "vpn-authorization", 5)
        override fun vpnApplyConfiguration(
            configJson: String,
            deadline: VpnOwnerRequestDeadline,
        ): Int = recordDeadline(deadline, "vpn-apply:$configJson", 6)
        override fun vpnStopResultJson(
            targetCode: Int,
            generation: Long,
            deadline: VpnOwnerRequestDeadline,
        ): String? = recordDeadline(deadline, "vpn-stop:$targetCode:$generation", "stop-receipt")
        override fun vpnOwnerSnapshotJson(deadline: VpnOwnerRequestDeadline): String? =
            recordDeadline(deadline, "vpn-snapshot", "vpn-snapshot")

        private fun <T> recordDeadline(
            deadline: VpnOwnerRequestDeadline,
            call: String,
            result: T,
        ): T {
            deadlines += deadline.elapsedRealtimeMillis
            return record(call, result)
        }

        private fun <T> record(call: String, result: T): T {
            calls += call
            return result
        }
    }
}
