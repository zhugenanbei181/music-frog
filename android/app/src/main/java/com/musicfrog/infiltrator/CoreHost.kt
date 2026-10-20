package com.musicfrog.infiltrator

import android.content.Context
import android.os.SystemClock
import android.util.Log
import infiltrator_android.AndroidHostReply
import java.io.File
import java.io.IOException

private const val TAG = "CoreHost"

internal class CoreHost(private val context: Context) :
    BridgeHost,
    AndroidBridgeCallHost {
    private val processController = CoreProcessController(context)
    private val credentialStore = AndroidCredentialStore(context)
    private val processRole = AndroidProcessRole.current(context)
    private val controlClient = (context.applicationContext as? MihomoApplication)?.ownerIpcClient
        ?: throw IllegalStateException("MihomoApplication must own the Android control client")
    private val bridgeCallRouter = AndroidBridgeCallRouter(this)

    override fun processRole(): Int = processRole.toWireCode()

    override fun dispatchAndroidBridgeCall(requestId: Long, requestJson: String, timeoutMillis: Long): Int {
        if (requestId <= 0L || timeoutMillis <= 0L) return AndroidBridgeDispatchState.InvalidRequest.wireCode
        val call = when (val decoded = AndroidBridgeCallWire.decode(requestJson)) {
            is AndroidBridgeCallDecode.Accepted -> decoded.call

            is AndroidBridgeCallDecode.Rejected -> {
                Log.w(TAG, "Rust host call was rejected: ${decoded.reason}")
                return AndroidBridgeDispatchState.InvalidRequest.wireCode
            }
        }
        return dispatchAndroidBridgeCall(requestId, call, timeoutMillis)
    }

    override fun dispatchProfileAgeCredentialOwnerOperation(
        requestId: Long,
        metadataJson: String,
        privateKeyBytes: ByteArray,
        timeoutMillis: Long,
    ): Int {
        if (requestId <= 0L || timeoutMillis <= 0L) {
            privateKeyBytes.fill(0)
            return AndroidBridgeDispatchState.InvalidRequest.wireCode
        }
        return dispatchAndroidBridgeCall(
            requestId,
            AndroidBridgeCall.ProfileAgeCredentialOwnerOperation(metadataJson, privateKeyBytes),
            timeoutMillis,
        )
    }

    private fun dispatchAndroidBridgeCall(requestId: Long, call: AndroidBridgeCall, timeoutMillis: Long): Int {
        val deadline = VpnOwnerRequestDeadline.after(
            SystemClock.elapsedRealtime(),
            timeoutMillis,
        )
        val application = context.applicationContext as? MihomoApplication
            ?: run {
                wipeSensitiveBridgePayload(call)
                return AndroidBridgeDispatchState.HostUnavailable.wireCode
            }
        val acceptance = application.bridgeCallDispatcher.submit(
            requestId = requestId,
            call = call,
            execute = { admittedCall ->
                if (deadline.remainingAt(SystemClock.elapsedRealtime()) == VpnDeadlineState.Expired) {
                    throw IllegalStateException("Android host call expired before execution")
                }
                bridgeCallRouter.execute(admittedCall, deadline)
            },
            complete = { completedId, reply ->
                RustBridge.completeAndroidBridgeCall(
                    completedId,
                    AndroidBridgeCallWire.encodeReply(reply),
                )
            },
            fail = { failedId, reason ->
                RustBridge.completeAndroidBridgeCall(
                    failedId,
                    AndroidBridgeCallWire.encodeFailure(reason),
                )
            },
        )
        if (acceptance != AndroidBridgeDispatchState.Accepted) wipeSensitiveBridgePayload(call)
        return acceptance.wireCode
    }

    private fun wipeSensitiveBridgePayload(call: AndroidBridgeCall) {
        if (call is AndroidBridgeCall.ProfileAgeCredentialOwnerOperation) {
            call.privateKeyBytes.fill(0)
        }
    }

    override fun cancelAndroidBridgeCall(requestId: Long): Int {
        val application = context.applicationContext as? MihomoApplication
            ?: return AndroidBridgeCallCancellation.UnknownRequest.wireCode
        return application.bridgeCallDispatcher.cancel(requestId).wireCode
    }

    override fun coreStart(startRequestJson: String): Int = when (processRole) {
        AndroidProcessRole.VpnOwner -> startCoreLocally(startRequestJson).wireCode

        AndroidProcessRole.Ui,
        is AndroidProcessRole.Unsupported,
        -> CoreProcessStartReceipt.NotVpnOwner.wireCode
    }

    override fun coreValidateConfiguration(requestJson: String): String = configurationValidationHost.validate(requestJson).toJson()

    private val configurationValidationHost = CoreConfigurationValidationHost(
        processRole = { processRole },
        controller = processController,
    )

    internal fun startCoreLocally(startRequestJson: String): CoreProcessStartReceipt {
        val request = when (val decoded = CoreStartRequestWire.decode(startRequestJson)) {
            is CoreStartRequestDecode.Accepted -> decoded.request

            is CoreStartRequestDecode.Rejected -> {
                Log.w(TAG, "Rust owner supplied an invalid Core start request: ${decoded.reason}")
                return CoreProcessStartReceipt.LaunchSpecRejected
            }
        }
        when (request.launchSpec.binary) {
            CoreLaunchBinaryWire.MihomoBundled -> ensureCoreAssets()
            is CoreLaunchBinaryWire.SingBoxManaged -> Unit
        }
        return processController.start(request.launchSpec, request.context)
    }

    private fun ensureCoreAssets() {
        val assetFiles = listOf("Country.mmdb", "geoip.dat", "geosite.dat")
        for (fileName in assetFiles) {
            val targetFile = File(context.filesDir, fileName)
            if (!targetFile.exists()) {
                try {
                    context.assets.open(fileName).use { input ->
                        targetFile.outputStream().use { output ->
                            input.copyTo(output)
                        }
                    }
                    Log.i(TAG, "Deployed asset: $fileName")
                } catch (e: IOException) {
                    Log.w(TAG, "Failed to deploy asset $fileName from assets (might be missing in build): ${e.message}")
                }
            }
        }
    }

    override fun coreStop(): Int = when (processRole) {
        AndroidProcessRole.VpnOwner -> stopCoreLocally().wireCode

        AndroidProcessRole.Ui,
        is AndroidProcessRole.Unsupported,
        -> CoreProcessStopReceipt.NotVpnOwner.wireCode
    }

    internal fun stopCoreLocally(): CoreProcessStopReceipt = processController.stop()

    override fun coreProcessObservationJson(): String = when (processRole) {
        AndroidProcessRole.VpnOwner -> processController.observation().toJson()

        AndroidProcessRole.Ui,
        is AndroidProcessRole.Unsupported,
        -> AndroidCoreProcessObservationWire.Unavailable(
            AndroidCoreProcessUnavailableReasonWire.ProcessInspectionUnavailable,
        ).toJson()
    }

    override fun coreProcessExitRecovered(contextJson: String): Int = when (processRole) {
        AndroidProcessRole.VpnOwner -> when (
            val decoded = CoreProcessStartContextWire.decode(contextJson)
        ) {
            is CoreProcessStartContextDecode.Accepted ->
                processController.consumeRecoveredExit(decoded.context).wireCode

            is CoreProcessStartContextDecode.Rejected ->
                AndroidCoreProcessRecoveryReceiptWire.ContextMismatch.wireCode
        }

        AndroidProcessRole.Ui,
        is AndroidProcessRole.Unsupported,
        ->
            AndroidCoreProcessRecoveryReceiptWire.NotVpnOwner.wireCode
    }

    override fun coreLifecycleSnapshot(deadline: VpnOwnerRequestDeadline): String? = when (processRole) {
        AndroidProcessRole.Ui -> controlClient.readCoreLifecycleSnapshotJson(deadline)

        AndroidProcessRole.VpnOwner,
        is AndroidProcessRole.Unsupported,
        -> null
    }

    override fun executeProductCommandJson(commandJson: String, deadline: VpnOwnerRequestDeadline): String? = when (processRole) {
        AndroidProcessRole.Ui -> controlClient.executeProductCommandJson(commandJson, deadline)

        AndroidProcessRole.VpnOwner,
        is AndroidProcessRole.Unsupported,
        -> null
    }

    override fun surfaceSnapshotJson(deadline: VpnOwnerRequestDeadline): String? = when (processRole) {
        AndroidProcessRole.Ui -> controlClient.readSurfaceSnapshotJson(deadline)

        AndroidProcessRole.VpnOwner,
        is AndroidProcessRole.Unsupported,
        -> null
    }

    override fun executeOwnerOperationJson(operationJson: String, deadline: VpnOwnerRequestDeadline): String? = when (processRole) {
        AndroidProcessRole.Ui -> controlClient.executeOwnerOperationJson(operationJson, deadline)

        AndroidProcessRole.VpnOwner,
        is AndroidProcessRole.Unsupported,
        -> null
    }

    override fun executeProfileAgeCredentialOwnerOperation(
        metadataJson: String,
        privateKeyBytes: ByteArray,
        deadline: VpnOwnerRequestDeadline,
    ): String? = when (processRole) {
        AndroidProcessRole.Ui -> controlClient.executeProfileAgeCredentialOwnerOperation(
            metadataJson,
            privateKeyBytes,
            deadline,
        )

        AndroidProcessRole.VpnOwner,
        is AndroidProcessRole.Unsupported,
        -> {
            privateKeyBytes.fill(0)
            null
        }
    }

    override fun credentialGet(service: String, key: String): String? = credentialStore.get(service, key)

    override fun credentialSet(service: String, key: String, value: String): AndroidCredentialMutationOutcome =
        credentialStore.set(service, key, value)

    override fun credentialDelete(service: String, key: String): AndroidCredentialMutationOutcome = credentialStore.delete(service, key)

    override fun dataDir(): String? = context.filesDir.absolutePath

    override fun cacheDir(): String? = context.cacheDir.absolutePath

    override fun vpnStartOutcome(deadline: VpnOwnerRequestDeadline): Int {
        val outcome = when (processRole) {
            AndroidProcessRole.VpnOwner ->
                MihomoVpnService.startAndAwaitRunning(context, deadline).toWireOutcome()

            AndroidProcessRole.Ui -> controlClient.startVpnService(deadline)

            is AndroidProcessRole.Unsupported -> VpnStartOutcomeWire.OwnerUnavailable
        }
        return outcome.wireCode
    }

    override fun vpnAuthorizationState(deadline: VpnOwnerRequestDeadline): Int {
        val state = when (processRole) {
            AndroidProcessRole.VpnOwner -> when (android.net.VpnService.prepare(context)) {
                null -> AndroidHostReply.VPN_AUTHORIZED
                else -> AndroidHostReply.VPN_PERMISSION_REQUIRED
            }

            AndroidProcessRole.Ui ->
                controlClient.readVpnAuthorization(deadline)

            is AndroidProcessRole.Unsupported -> AndroidHostReply.SERVICE_UNAVAILABLE
        }
        return when (state) {
            AndroidHostReply.VPN_AUTHORIZED -> VPN_AUTHORIZATION_GRANTED
            AndroidHostReply.VPN_PERMISSION_REQUIRED -> VPN_AUTHORIZATION_REQUIRED
            else -> VPN_AUTHORIZATION_UNAVAILABLE
        }
    }

    override fun vpnApplyConfiguration(configJson: String, deadline: VpnOwnerRequestDeadline): Int {
        val outcome = when (processRole) {
            AndroidProcessRole.VpnOwner -> MihomoVpnService.setPendingConfiguration(context, configJson)
            AndroidProcessRole.Ui -> controlClient.applyVpnConfiguration(configJson, deadline)
            is AndroidProcessRole.Unsupported -> VpnConfigurationApplyOutcomeWire.OwnerUnavailable
        }
        return outcome.wireCode
    }

    override fun vpnStopResultJson(targetCode: Int, generation: Long, deadline: VpnOwnerRequestDeadline): String? {
        val target = VpnStopTarget.fromWire(targetCode, generation)
        if (target == null) {
            val evidence = if (processRole == AndroidProcessRole.VpnOwner) {
                (context.applicationContext as? MihomoApplication)
                    ?.vpnOwnerSnapshotJson()
                    ?.let(VpnOwnerSnapshotWire::parse)
                    ?.let { VpnStopReadbackEvidence.OwnerSnapshot(it) }
                    ?: VpnStopReadbackEvidence.Missing
            } else {
                VpnStopReadbackEvidence.Missing
            }
            return vpnOwnerStopReceiptJson(
                VpnStopResult.Failed("VPN stop target is invalid").toOwnerStopReceipt(evidence),
            )
        }
        return when (processRole) {
            AndroidProcessRole.VpnOwner -> {
                val result = MihomoVpnService.stopAndAwaitStopped(context, target, deadline)
                if (result is VpnStopResult.Failed) {
                    Log.e(TAG, "VpnService shutdown failed: ${result.message}")
                }
                val snapshotJson =
                    (context.applicationContext as? MihomoApplication)?.vpnOwnerSnapshotJson()
                val snapshot = snapshotJson?.let(VpnOwnerSnapshotWire::parse)
                val evidence = when (snapshot) {
                    null -> VpnStopReadbackEvidence.Missing
                    else -> VpnStopReadbackEvidence.OwnerSnapshot(snapshot)
                }
                vpnOwnerStopReceiptJson(result.toOwnerStopReceipt(evidence))
            }

            AndroidProcessRole.Ui -> when (target) {
                VpnStopTarget.Current -> controlClient.stopVpnService(deadline)

                is VpnStopTarget.Generation -> {
                    val evidence = controlClient.readVpnOwnerSnapshot(deadline)?.let {
                        VpnStopReadbackEvidence.OwnerSnapshot(it)
                    } ?: VpnStopReadbackEvidence.Missing
                    vpnOwnerStopReceiptJson(
                        VpnStopResult.Failed(
                            "UI VPN stop must use the current owner service",
                        ).toOwnerStopReceipt(evidence),
                    )
                }
            }

            is AndroidProcessRole.Unsupported -> vpnOwnerStopReceiptJson(
                VpnOwnerStopReceiptWire.OwnerUnavailable(
                    VpnStopFailureWire(
                        code = VpnStopFailureCode.NotReady,
                        message = "Android VPN owner is unavailable during stop",
                        retryable = true,
                    ),
                ),
            )
        }
    }

    override fun vpnOwnerSnapshotJson(deadline: VpnOwnerRequestDeadline): String? = when (processRole) {
        AndroidProcessRole.VpnOwner ->
            (context.applicationContext as? MihomoApplication)?.vpnOwnerSnapshotJson()

        AndroidProcessRole.Ui -> controlClient.readVpnOwnerSnapshotJson(deadline)

        is AndroidProcessRole.Unsupported -> null
    }

    private companion object {
        const val VPN_AUTHORIZATION_GRANTED = 0
        const val VPN_AUTHORIZATION_REQUIRED = 1
        const val VPN_AUTHORIZATION_UNAVAILABLE = -1
    }
}
