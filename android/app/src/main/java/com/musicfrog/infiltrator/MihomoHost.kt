package com.musicfrog.infiltrator

import android.content.Context
import android.util.Log
import java.io.File
import java.io.IOException
import infiltrator_android.AndroidHostReply

private const val TAG = "MihomoHost"

class MihomoHost(private val context: Context) : BridgeHost {
    private val processController = MihomoProcessController(context)
    private val credentialStore = AndroidCredentialStore(context)
    private val processRole = AndroidProcessRole.current(context)
    private val controlClient = (context.applicationContext as? MihomoApplication)?.ownerIpcClient
        ?: throw IllegalStateException("MihomoApplication must own the Android control client")

    override fun processRole(): Int = processRole.toWireCode()

    override fun coreStart(launchSpecJson: String): Int = when (processRole) {
        AndroidProcessRole.VpnOwner -> startCoreLocally(launchSpecJson).wireCode
        AndroidProcessRole.Ui,
        is AndroidProcessRole.Unsupported -> CoreProcessStartReceipt.NotVpnOwner.wireCode
    }

    override fun coreValidateConfiguration(requestJson: String): String =
        configurationValidationHost.validate(requestJson).toJson()

    private val configurationValidationHost = CoreConfigurationValidationHost(
        processRole = { processRole },
        controller = processController,
    )

    internal fun startCoreLocally(launchSpecJson: String): CoreProcessStartReceipt {
        val spec = when (val decoded = CoreLaunchSpecWire.decode(launchSpecJson)) {
            is CoreLaunchSpecDecode.Accepted -> decoded.spec
            is CoreLaunchSpecDecode.Rejected -> {
                Log.w(TAG, "Rust owner supplied an invalid Core launch spec: ${decoded.reason}")
                return CoreProcessStartReceipt.LaunchSpecRejected
            }
        }
        ensureCoreAssets()
        return processController.start(spec)
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
        is AndroidProcessRole.Unsupported -> CoreProcessStopReceipt.NotVpnOwner.wireCode
    }

    internal fun stopCoreLocally(): CoreProcessStopReceipt = processController.stop()

    override fun coreProcessState(): Int {
        val state = when (processRole) {
            AndroidProcessRole.VpnOwner -> coreProcessStateLocally()
            AndroidProcessRole.Ui -> AndroidCoreProcessState.UNAVAILABLE
            is AndroidProcessRole.Unsupported -> AndroidCoreProcessState.UNAVAILABLE
        }
        return state.wireCode
    }

    internal fun coreProcessStateLocally(): AndroidCoreProcessState = processController.state()

    override fun coreLifecycleSnapshot(): String? = when (processRole) {
        AndroidProcessRole.Ui -> controlClient.readCoreLifecycleSnapshotJson()
        AndroidProcessRole.VpnOwner,
        is AndroidProcessRole.Unsupported -> null
    }

    override fun executeProductCommandJson(commandJson: String): String? = when (processRole) {
        AndroidProcessRole.Ui -> controlClient.executeProductCommandJson(commandJson)
        AndroidProcessRole.VpnOwner,
        is AndroidProcessRole.Unsupported -> null
    }

    override fun surfaceSnapshotJson(): String? = when (processRole) {
        AndroidProcessRole.Ui -> controlClient.readSurfaceSnapshotJson()
        AndroidProcessRole.VpnOwner,
        is AndroidProcessRole.Unsupported -> null
    }

    override fun executeOwnerOperationJson(operationJson: String): String? = when (processRole) {
        AndroidProcessRole.Ui -> controlClient.executeOwnerOperationJson(operationJson)
        AndroidProcessRole.VpnOwner,
        is AndroidProcessRole.Unsupported -> null
    }

    override fun credentialGet(service: String, key: String): String? {
        return credentialStore.get(service, key)
    }

    override fun credentialSet(service: String, key: String, value: String): Boolean {
        return credentialStore.set(service, key, value)
    }

    override fun credentialDelete(service: String, key: String): Boolean {
        return credentialStore.delete(service, key)
    }

    override fun dataDir(): String? {
        return context.filesDir.absolutePath
    }

    override fun cacheDir(): String? {
        return context.cacheDir.absolutePath
    }

    override fun vpnStartOutcome(): Int {
        val outcome = when (processRole) {
            AndroidProcessRole.VpnOwner ->
                MihomoVpnService.startAndAwaitRunning(context).toWireOutcome()
            AndroidProcessRole.Ui -> controlClient.startVpnService()
            is AndroidProcessRole.Unsupported -> VpnStartOutcomeWire.OwnerUnavailable
        }
        return outcome.wireCode
    }

    override fun vpnAuthorizationState(): Int {
        val state = when (processRole) {
            AndroidProcessRole.VpnOwner -> when (android.net.VpnService.prepare(context)) {
                null -> AndroidHostReply.VPN_AUTHORIZED
                else -> AndroidHostReply.VPN_PERMISSION_REQUIRED
            }
            AndroidProcessRole.Ui ->
                controlClient.readVpnAuthorization()
            is AndroidProcessRole.Unsupported -> AndroidHostReply.SERVICE_UNAVAILABLE
        }
        return when (state) {
            AndroidHostReply.VPN_AUTHORIZED -> VPN_AUTHORIZATION_GRANTED
            AndroidHostReply.VPN_PERMISSION_REQUIRED -> VPN_AUTHORIZATION_REQUIRED
            else -> VPN_AUTHORIZATION_UNAVAILABLE
        }
    }

    override fun vpnApplyConfiguration(configJson: String): Int {
        val outcome = when (processRole) {
            AndroidProcessRole.VpnOwner -> MihomoVpnService.setPendingConfiguration(context, configJson)
            AndroidProcessRole.Ui -> controlClient.applyVpnConfiguration(configJson)
            is AndroidProcessRole.Unsupported -> VpnConfigurationApplyOutcomeWire.OwnerUnavailable
        }
        return outcome.wireCode
    }

    override fun vpnStopResultJson(targetCode: Int, generation: Long): String? {
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
                val result = MihomoVpnService.stopAndAwaitStopped(context, target)
                if (result is VpnStopResult.Failed) {
                    Log.e(TAG, "VpnService shutdown failed: ${result.message}")
                }
                val snapshotJson = (context.applicationContext as? MihomoApplication)?.vpnOwnerSnapshotJson()
                val snapshot = snapshotJson?.let(VpnOwnerSnapshotWire::parse)
                val evidence = when (snapshot) {
                    null -> VpnStopReadbackEvidence.Missing
                    else -> VpnStopReadbackEvidence.OwnerSnapshot(snapshot)
                }
                vpnOwnerStopReceiptJson(result.toOwnerStopReceipt(evidence))
            }
            AndroidProcessRole.Ui -> when (target) {
                VpnStopTarget.Current -> controlClient.stopVpnService()
                is VpnStopTarget.Generation -> vpnOwnerStopReceiptJson(
                    VpnStopResult.Failed("UI VPN stop must use the current owner service").toOwnerStopReceipt(
                        controlClient.readVpnOwnerSnapshot()?.let {
                            VpnStopReadbackEvidence.OwnerSnapshot(it)
                        }
                            ?: VpnStopReadbackEvidence.Missing,
                    ),
                )
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

    override fun vpnOwnerSnapshotJson(): String? = when (processRole) {
        AndroidProcessRole.VpnOwner ->
            (context.applicationContext as? MihomoApplication)?.vpnOwnerSnapshotJson()
        AndroidProcessRole.Ui -> controlClient.readVpnOwnerSnapshotJson()
        is AndroidProcessRole.Unsupported -> null
    }

    private companion object {
        const val VPN_AUTHORIZATION_GRANTED = 0
        const val VPN_AUTHORIZATION_REQUIRED = 1
        const val VPN_AUTHORIZATION_UNAVAILABLE = -1
    }
}
