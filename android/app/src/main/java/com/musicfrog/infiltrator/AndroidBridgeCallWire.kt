package com.musicfrog.infiltrator

import org.json.JSONObject

internal sealed interface AndroidBridgeCall {
    data class CoreStart(val startRequestJson: String) : AndroidBridgeCall

    data object CoreStop : AndroidBridgeCall

    data object CoreProcessObservation : AndroidBridgeCall

    data class CoreProcessRecoveryCompleted(val contextJson: String) : AndroidBridgeCall

    data class CoreValidateConfiguration(val requestJson: String) : AndroidBridgeCall

    data object CoreLifecycleSnapshot : AndroidBridgeCall

    data class ExecuteProductCommand(val commandJson: String) : AndroidBridgeCall

    data object SurfaceSnapshot : AndroidBridgeCall

    data class ExecuteOwnerOperation(val operationJson: String) : AndroidBridgeCall

    data class ProfileAgeCredentialOwnerOperation(
        val metadataJson: String,
        val privateKeyBytes: ByteArray,
    ) : AndroidBridgeCall

    data class CredentialGet(val service: String, val key: String) : AndroidBridgeCall

    data class CredentialSet(val service: String, val key: String, val value: String) : AndroidBridgeCall

    data class CredentialDelete(val service: String, val key: String) : AndroidBridgeCall

    data object VpnStart : AndroidBridgeCall

    data object VpnAuthorizationState : AndroidBridgeCall

    data class VpnApplyConfiguration(val preparedJson: String) : AndroidBridgeCall

    data class VpnStop(val targetCode: Int, val generation: Long) : AndroidBridgeCall

    data object VpnOwnerSnapshot : AndroidBridgeCall
}

internal sealed interface AndroidBridgeCallDecode {
    data class Accepted(val call: AndroidBridgeCall) : AndroidBridgeCallDecode

    data class Rejected(val reason: String) : AndroidBridgeCallDecode
}

internal sealed interface AndroidBridgeCallReply {
    data class CoreStart(val wireCode: Int) : AndroidBridgeCallReply

    data class CoreStop(val wireCode: Int) : AndroidBridgeCallReply

    data class CoreProcessObservation(val json: String) : AndroidBridgeCallReply

    data class CoreProcessRecoveryCompleted(val wireCode: Int) : AndroidBridgeCallReply

    data class CoreValidateConfiguration(val json: String) : AndroidBridgeCallReply

    data class CoreLifecycleSnapshot(val json: String) : AndroidBridgeCallReply

    data class ExecuteProductCommand(val json: String) : AndroidBridgeCallReply

    data class SurfaceSnapshot(val json: String) : AndroidBridgeCallReply

    data class ExecuteOwnerOperation(val json: String) : AndroidBridgeCallReply

    data class ProfileAgeCredentialOwnerOperation(val json: String) : AndroidBridgeCallReply

    data class CredentialGet(val value: String?) : AndroidBridgeCallReply

    data class CredentialSet(val outcome: AndroidCredentialMutationOutcome) : AndroidBridgeCallReply

    data class CredentialDelete(val outcome: AndroidCredentialMutationOutcome) : AndroidBridgeCallReply

    data class VpnStart(val wireCode: Int) : AndroidBridgeCallReply

    data class VpnAuthorizationState(val wireCode: Int) : AndroidBridgeCallReply

    data class VpnConfigurationApply(val wireCode: Int) : AndroidBridgeCallReply

    data class VpnStop(val json: String) : AndroidBridgeCallReply

    data class VpnOwnerSnapshot(val json: String) : AndroidBridgeCallReply
}

internal object AndroidBridgeCallWire {
    fun decode(raw: String): AndroidBridgeCallDecode = try {
        val root = JSONObject(raw)
        requireKeys(root, setOf("operation", "payload"))
        val operation = root.getString("operation")
        val payload = root.get("payload")
        when (operation) {
            "core_start" -> {
                val body = payloadObject(payload)
                requireKeys(body, setOf("start_request_json"))
                AndroidBridgeCallDecode.Accepted(
                    AndroidBridgeCall.CoreStart(body.getString("start_request_json")),
                )
            }
            "core_stop" -> unit(payload, AndroidBridgeCall.CoreStop)
            "core_process_observation" -> unit(payload, AndroidBridgeCall.CoreProcessObservation)
            "core_process_recovery_completed" -> {
                val body = payloadObject(payload)
                requireKeys(body, setOf("context_json"))
                AndroidBridgeCallDecode.Accepted(
                    AndroidBridgeCall.CoreProcessRecoveryCompleted(body.getString("context_json")),
                )
            }
            "core_validate_configuration" -> {
                val body = payloadObject(payload)
                requireKeys(body, setOf("request_json"))
                AndroidBridgeCallDecode.Accepted(
                    AndroidBridgeCall.CoreValidateConfiguration(body.getString("request_json")),
                )
            }
            "core_lifecycle_snapshot" -> unit(payload, AndroidBridgeCall.CoreLifecycleSnapshot)
            "execute_product_command" -> {
                val body = payloadObject(payload)
                requireKeys(body, setOf("command_json"))
                AndroidBridgeCallDecode.Accepted(
                    AndroidBridgeCall.ExecuteProductCommand(body.getString("command_json")),
                )
            }
            "surface_snapshot" -> unit(payload, AndroidBridgeCall.SurfaceSnapshot)
            "execute_owner_operation" -> {
                val body = payloadObject(payload)
                requireKeys(body, setOf("operation_json"))
                AndroidBridgeCallDecode.Accepted(
                    AndroidBridgeCall.ExecuteOwnerOperation(body.getString("operation_json")),
                )
            }
            "credential_get" -> {
                val body = payloadObject(payload)
                requireKeys(body, setOf("service", "key"))
                AndroidBridgeCallDecode.Accepted(
                    AndroidBridgeCall.CredentialGet(
                        service = body.getString("service"),
                        key = body.getString("key"),
                    ),
                )
            }
            "credential_set" -> {
                val body = payloadObject(payload)
                requireKeys(body, setOf("service", "key", "value"))
                AndroidBridgeCallDecode.Accepted(
                    AndroidBridgeCall.CredentialSet(
                        service = body.getString("service"),
                        key = body.getString("key"),
                        value = body.getString("value"),
                    ),
                )
            }
            "credential_delete" -> {
                val body = payloadObject(payload)
                requireKeys(body, setOf("service", "key"))
                AndroidBridgeCallDecode.Accepted(
                    AndroidBridgeCall.CredentialDelete(
                        service = body.getString("service"),
                        key = body.getString("key"),
                    ),
                )
            }
            "vpn_start" -> unit(payload, AndroidBridgeCall.VpnStart)
            "vpn_authorization_state" -> unit(payload, AndroidBridgeCall.VpnAuthorizationState)
            "vpn_apply_configuration" -> {
                val body = payloadObject(payload)
                requireKeys(body, setOf("prepared_json"))
                AndroidBridgeCallDecode.Accepted(
                    AndroidBridgeCall.VpnApplyConfiguration(body.getString("prepared_json")),
                )
            }
            "vpn_stop" -> {
                val body = payloadObject(payload)
                requireKeys(body, setOf("target"))
                val target = body.getJSONObject("target")
                val targetKeys = target.keys().asSequence().toSet()
                val stopTarget = when {
                    targetKeys == setOf("Current") && target.isNull("Current") ->
                        AndroidBridgeCall.VpnStop(targetCode = 0, generation = 0L)
                    targetKeys == setOf("Generation") -> {
                        val generation = target.get("Generation")
                            .toString()
                            .toLongOrNull()
                            ?: error("VPN stop generation exceeds Android Long range")
                        require(generation > 0L) { "VPN stop generation must be positive" }
                        AndroidBridgeCall.VpnStop(targetCode = 1, generation = generation)
                    }
                    else -> error("VPN stop target variant is invalid")
                }
                AndroidBridgeCallDecode.Accepted(stopTarget)
            }
            "vpn_owner_snapshot" -> unit(payload, AndroidBridgeCall.VpnOwnerSnapshot)
            else -> AndroidBridgeCallDecode.Rejected("unknown Android bridge operation variant")
        }
    } catch (error: Exception) {
        AndroidBridgeCallDecode.Rejected(error.message ?: "Android bridge call is malformed")
    }

    fun encodeReply(reply: AndroidBridgeCallReply): String {
        val result = when (reply) {
            is AndroidBridgeCallReply.CoreStart -> tagged("core_start", JSONObject().put("wire_code", reply.wireCode))
            is AndroidBridgeCallReply.CoreStop -> tagged("core_stop", JSONObject().put("wire_code", reply.wireCode))
            is AndroidBridgeCallReply.CoreProcessObservation -> tagged("core_process_observation", reply.json)
            is AndroidBridgeCallReply.CoreProcessRecoveryCompleted ->
                tagged("core_process_recovery_completed", JSONObject().put("wire_code", reply.wireCode))
            is AndroidBridgeCallReply.CoreValidateConfiguration -> tagged("core_validate_configuration", reply.json)
            is AndroidBridgeCallReply.CoreLifecycleSnapshot -> tagged("core_lifecycle_snapshot", reply.json)
            is AndroidBridgeCallReply.ExecuteProductCommand -> tagged("execute_product_command", reply.json)
            is AndroidBridgeCallReply.SurfaceSnapshot -> tagged("surface_snapshot", reply.json)
            is AndroidBridgeCallReply.ExecuteOwnerOperation -> tagged("execute_owner_operation", reply.json)
            is AndroidBridgeCallReply.ProfileAgeCredentialOwnerOperation ->
                tagged("profile_age_credential_owner_operation", reply.json)
            is AndroidBridgeCallReply.CredentialGet -> tagged("credential_get", reply.value ?: JSONObject.NULL)
            is AndroidBridgeCallReply.CredentialSet -> tagged(
                "credential_set",
                JSONObject().put("outcome", reply.outcome.wireValue),
            )
            is AndroidBridgeCallReply.CredentialDelete -> tagged(
                "credential_delete",
                JSONObject().put("outcome", reply.outcome.wireValue),
            )
            is AndroidBridgeCallReply.VpnStart -> tagged("vpn_start", JSONObject().put("wire_code", reply.wireCode))
            is AndroidBridgeCallReply.VpnAuthorizationState ->
                tagged("vpn_authorization_state", JSONObject().put("wire_code", reply.wireCode))
            is AndroidBridgeCallReply.VpnConfigurationApply ->
                tagged("vpn_configuration_apply", JSONObject().put("wire_code", reply.wireCode))
            is AndroidBridgeCallReply.VpnStop -> tagged("vpn_stop", reply.json)
            is AndroidBridgeCallReply.VpnOwnerSnapshot -> tagged("vpn_owner_snapshot", reply.json)
        }
        return JSONObject()
            .put("state", "reply")
            .put("value", result)
            .toString()
    }

    fun encodeFailure(message: String): String = JSONObject()
        .put("state", "failed")
        .put(
            "value",
            JSONObject()
                .put("reason", "host")
                .put("message", message),
        )
        .toString()

    private fun tagged(result: String, value: Any): JSONObject = JSONObject()
        .put("result", result)
        .put("value", value)

    private fun payloadObject(payload: Any): JSONObject {
        require(payload is JSONObject) { "Android bridge call payload must be an object" }
        return payload
    }

    private fun requireKeys(value: JSONObject, expected: Set<String>) {
        val actual = value.keys().asSequence().toSet()
        require(actual == expected) { "Android bridge call has missing or unknown fields" }
    }

    private fun unit(payload: Any, call: AndroidBridgeCall): AndroidBridgeCallDecode {
        require(payload == JSONObject.NULL) { "unit Android bridge call must have a null payload" }
        return AndroidBridgeCallDecode.Accepted(call)
    }
}
