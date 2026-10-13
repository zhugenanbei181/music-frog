package com.musicfrog.infiltrator

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class AndroidBridgeCallWireTest {
    @Test
    fun decodesTypedRequestsAndRejectsUnknownVariantsAndFields() {
        assertEquals(
            AndroidBridgeCallDecode.Accepted(AndroidBridgeCall.CoreStop),
            AndroidBridgeCallWire.decode("""{"operation":"core_stop","payload":null}"""),
        )
        val decoded = AndroidBridgeCallWire.decode(
            """{"operation":"execute_product_command","payload":{"command_json":"{\"kind\":\"pause_kernel\"}"}}""",
        )
        assertEquals(
            AndroidBridgeCallDecode.Accepted(
                AndroidBridgeCall.ExecuteProductCommand("{\"kind\":\"pause_kernel\"}"),
            ),
            decoded,
        )
        assertTrue(
            AndroidBridgeCallWire.decode(
                """{"operation":"execute_product_command","payload":{"command_json":"{}","running":true}}""",
            ) is AndroidBridgeCallDecode.Rejected,
        )
        assertTrue(
            AndroidBridgeCallWire.decode(
                """{"operation":"launch_core","payload":null}""",
            ) is AndroidBridgeCallDecode.Rejected,
        )
        assertEquals(
            AndroidBridgeCallDecode.Accepted(AndroidBridgeCall.VpnStop(targetCode = 1, generation = 7L)),
            AndroidBridgeCallWire.decode(
                """{"operation":"vpn_stop","payload":{"target":{"Generation":7}}}""",
            ),
        )
    }

    @Test
    fun encodesCompletionAsTypedReplyOrFailureVariant() {
        val success = JSONObject(AndroidBridgeCallWire.encodeReply(AndroidBridgeCallReply.VpnStart(0)))
        assertEquals("reply", success.getString("state"))
        assertEquals("vpn_start", success.getJSONObject("value").getString("result"))
        assertEquals(0, success.getJSONObject("value").getJSONObject("value").getInt("wire_code"))

        val credentialSet = JSONObject(
            AndroidBridgeCallWire.encodeReply(
                AndroidBridgeCallReply.CredentialSet(AndroidCredentialMutationOutcome.Applied),
            ),
        )
        assertEquals("credential_set", credentialSet.getJSONObject("value").getString("result"))
        assertEquals(
            "applied",
            credentialSet.getJSONObject("value").getJSONObject("value").getString("outcome"),
        )

        val credentialDelete = JSONObject(
            AndroidBridgeCallWire.encodeReply(
                AndroidBridgeCallReply.CredentialDelete(AndroidCredentialMutationOutcome.Rejected),
            ),
        )
        assertEquals("credential_delete", credentialDelete.getJSONObject("value").getString("result"))
        assertEquals(
            "rejected",
            credentialDelete.getJSONObject("value").getJSONObject("value").getString("outcome"),
        )

        val failure = JSONObject(AndroidBridgeCallWire.encodeFailure("owner unavailable"))
        assertEquals("failed", failure.getString("state"))
        assertEquals("host", failure.getJSONObject("value").getString("reason"))
    }
}
