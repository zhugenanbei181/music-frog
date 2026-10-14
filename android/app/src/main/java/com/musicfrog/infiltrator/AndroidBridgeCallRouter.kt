package com.musicfrog.infiltrator

/** Local Android host operations invoked only by the bounded async dispatcher. */
internal interface AndroidBridgeCallHost {
    fun coreStart(startRequestJson: String): Int
    fun coreValidateConfiguration(requestJson: String): String
    fun coreStop(): Int
    fun coreProcessObservationJson(): String
    fun coreProcessExitRecovered(contextJson: String): Int
    fun coreLifecycleSnapshot(deadline: VpnOwnerRequestDeadline): String?
    fun executeProductCommandJson(commandJson: String, deadline: VpnOwnerRequestDeadline): String?
    fun surfaceSnapshotJson(deadline: VpnOwnerRequestDeadline): String?
    fun executeOwnerOperationJson(operationJson: String, deadline: VpnOwnerRequestDeadline): String?
    fun executeProfileAgeCredentialOwnerOperation(
        metadataJson: String,
        privateKeyBytes: ByteArray,
        deadline: VpnOwnerRequestDeadline,
    ): String?
    fun credentialGet(service: String, key: String): String?
    fun credentialSet(service: String, key: String, value: String): AndroidCredentialMutationOutcome
    fun credentialDelete(service: String, key: String): AndroidCredentialMutationOutcome
    fun vpnStartOutcome(deadline: VpnOwnerRequestDeadline): Int
    fun vpnAuthorizationState(deadline: VpnOwnerRequestDeadline): Int
    fun vpnApplyConfiguration(configJson: String, deadline: VpnOwnerRequestDeadline): Int
    fun vpnStopResultJson(targetCode: Int, generation: Long, deadline: VpnOwnerRequestDeadline): String?
    fun vpnOwnerSnapshotJson(deadline: VpnOwnerRequestDeadline): String?
}

internal class AndroidBridgeCallRouter(private val host: AndroidBridgeCallHost) {
    fun execute(
        call: AndroidBridgeCall,
        deadline: VpnOwnerRequestDeadline,
    ): AndroidBridgeCallReply = when (call) {
        is AndroidBridgeCall.CoreStart -> AndroidBridgeCallReply.CoreStart(
            host.coreStart(call.startRequestJson),
        )
        AndroidBridgeCall.CoreStop -> AndroidBridgeCallReply.CoreStop(host.coreStop())
        AndroidBridgeCall.CoreProcessObservation ->
            AndroidBridgeCallReply.CoreProcessObservation(host.coreProcessObservationJson())
        is AndroidBridgeCall.CoreProcessRecoveryCompleted ->
            AndroidBridgeCallReply.CoreProcessRecoveryCompleted(
                host.coreProcessExitRecovered(call.contextJson),
            )
        is AndroidBridgeCall.CoreValidateConfiguration ->
            AndroidBridgeCallReply.CoreValidateConfiguration(
                host.coreValidateConfiguration(call.requestJson),
            )
        AndroidBridgeCall.CoreLifecycleSnapshot -> AndroidBridgeCallReply.CoreLifecycleSnapshot(
            host.coreLifecycleSnapshot(deadline).required("Core lifecycle snapshot"),
        )
        is AndroidBridgeCall.ExecuteProductCommand -> AndroidBridgeCallReply.ExecuteProductCommand(
            host.executeProductCommandJson(call.commandJson, deadline).required("product command result"),
        )
        AndroidBridgeCall.SurfaceSnapshot -> AndroidBridgeCallReply.SurfaceSnapshot(
            host.surfaceSnapshotJson(deadline).required("owner surface snapshot"),
        )
        is AndroidBridgeCall.ExecuteOwnerOperation -> AndroidBridgeCallReply.ExecuteOwnerOperation(
            host.executeOwnerOperationJson(call.operationJson, deadline).required("owner operation result"),
        )
        is AndroidBridgeCall.ProfileAgeCredentialOwnerOperation -> try {
            AndroidBridgeCallReply.ProfileAgeCredentialOwnerOperation(
                host.executeProfileAgeCredentialOwnerOperation(
                    call.metadataJson,
                    call.privateKeyBytes,
                    deadline,
                ).required("Age credential owner result"),
            )
        } finally {
            call.privateKeyBytes.fill(0)
        }
        is AndroidBridgeCall.CredentialGet -> AndroidBridgeCallReply.CredentialGet(
            host.credentialGet(call.service, call.key),
        )
        is AndroidBridgeCall.CredentialSet -> AndroidBridgeCallReply.CredentialSet(
            host.credentialSet(call.service, call.key, call.value),
        )
        is AndroidBridgeCall.CredentialDelete -> AndroidBridgeCallReply.CredentialDelete(
            host.credentialDelete(call.service, call.key),
        )
        AndroidBridgeCall.VpnStart -> AndroidBridgeCallReply.VpnStart(host.vpnStartOutcome(deadline))
        AndroidBridgeCall.VpnAuthorizationState -> AndroidBridgeCallReply.VpnAuthorizationState(
            host.vpnAuthorizationState(deadline),
        )
        is AndroidBridgeCall.VpnApplyConfiguration -> AndroidBridgeCallReply.VpnConfigurationApply(
            host.vpnApplyConfiguration(call.preparedJson, deadline),
        )
        is AndroidBridgeCall.VpnStop -> AndroidBridgeCallReply.VpnStop(
            host.vpnStopResultJson(call.targetCode, call.generation, deadline)
                .required("VPN stop receipt"),
        )
        AndroidBridgeCall.VpnOwnerSnapshot -> AndroidBridgeCallReply.VpnOwnerSnapshot(
            host.vpnOwnerSnapshotJson(deadline).required("VPN owner snapshot"),
        )
    }

    private fun String?.required(label: String): String =
        this ?: throw IllegalStateException("Android host returned no $label")
}
