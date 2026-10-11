package com.musicfrog.infiltrator

import android.app.Application
import android.content.Intent
import android.net.VpnService
import android.util.Log
import infiltrator_android.AndroidCoreLifecycleResult
import infiltrator_android.FfiErrorCode
import infiltrator_android.androidCoreLifecycleStatus
import infiltrator_android.bridgeShutdown
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex

class MihomoApplication : Application() {
    private sealed interface VpnSnapshotOwnerSession {
        data object Unavailable : VpnSnapshotOwnerSession

        data class Active(
            val id: String,
            val publisher: VpnOwnerSnapshotPublisher,
        ) : VpnSnapshotOwnerSession
    }

    @Volatile
    private var vpnSnapshotOwnerSession: VpnSnapshotOwnerSession = VpnSnapshotOwnerSession.Unavailable

    @Volatile
    private var latestCoreReadback: VpnOwnerCoreReadbackWire =
        VpnOwnerCoreReadbackWire.Unavailable("Core lifecycle readback has not completed")
    private val coreReadbackMutex = Mutex()

    internal val ownerIpcScope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    private val ownerIpcConnectionDelegate = lazy(LazyThreadSafetyMode.SYNCHRONIZED) {
        MihomoOwnerIpcConnection(applicationContext)
    }

    internal val ownerIpcConnection: MihomoOwnerIpcConnection by ownerIpcConnectionDelegate

    private val ownerIpcClientDelegate = lazy(LazyThreadSafetyMode.SYNCHRONIZED) {
        MihomoControlClient(applicationContext)
    }

    internal val ownerIpcClient: MihomoControlClient by ownerIpcClientDelegate

    internal var processHost: MihomoHost? = null
        private set

    internal fun beginVpnOwnerSession(sessionId: String) {
        synchronized(this) {
            vpnSnapshotOwnerSession = VpnSnapshotOwnerSession.Active(
                sessionId,
                VpnOwnerSnapshotPublisher(),
            )
            latestCoreReadback =
                VpnOwnerCoreReadbackWire.Unavailable("Core lifecycle readback is being refreshed")
        }
        ownerIpcScope.launch { refreshCoreReadback() }
    }

    internal fun endVpnOwnerSession(sessionId: String) {
        synchronized(this) {
            val current = vpnSnapshotOwnerSession
            if (current is VpnSnapshotOwnerSession.Active && current.id == sessionId) {
                vpnSnapshotOwnerSession = VpnSnapshotOwnerSession.Unavailable
            }
        }
    }

    internal fun vpnOwnerSnapshotJson(): String? {
        val active = vpnSnapshotOwnerSession as? VpnSnapshotOwnerSession.Active ?: return null
        val lifecycle = MihomoVpnService.lifecycleSnapshot()
        val resources = MihomoVpnService.ownerResourceFacts()
        val configuration = MihomoVpnService.ownerConfiguration()
        val permissionRequired = VpnService.prepare(this) != null && lifecycle.state == VpnProcessState.Stopped
        val state = lifecycle.state.toOwnerLifecycle(permissionRequired)
        return active.publisher.publish(
            VpnOwnerSnapshotFacts(
                ownerSessionId = active.id,
                vpnGeneration = lifecycle.generation,
                vpnState = state,
                readiness = state.toOwnerReadiness(resources),
                resources = resources,
                foreground = lifecycle.state.isForegroundProtected(),
                mtu = configuration?.mtu,
                routeCount = configuration?.routeCount ?: 0,
                dnsServers = configuration?.dnsServers.orEmpty(),
                ipv6 = configuration?.ipv6 ?: false,
                core = latestCoreReadback,
            ),
        )
    }

    internal suspend fun refreshAndPublishVpnOwnerSnapshotJson(): String? {
        refreshCoreReadback()
        return vpnOwnerSnapshotJson()
    }

    private suspend fun refreshCoreReadback() {
        coreReadbackMutex.lock()
        try {
            val active = vpnSnapshotOwnerSession as? VpnSnapshotOwnerSession.Active ?: return
            val core = androidCoreLifecycleStatus().toOwnerCoreReadback()
            val current = vpnSnapshotOwnerSession as? VpnSnapshotOwnerSession.Active
            if (current?.id == active.id) latestCoreReadback = core
        } finally {
            coreReadbackMutex.unlock()
        }
    }

    override fun onCreate() {
        super.onCreate()
        Log.i("MihomoApp", "Initializing application...")
        val processRole = AndroidProcessRole.current(this)

        try {
            System.loadLibrary("infiltrator_android")
            Log.i("MihomoApp", "Native library loaded.")
        } catch (e: UnsatisfiedLinkError) {
            Log.e("MihomoApp", "Failed to load native library", e)
        }

        // Every process gets its own host adapter registry before an Activity
        // or Service can issue a Rust command. Registration only exposes host
        // ports; it never starts the Mihomo process or enables the VPN.
        registerProcessBridge()
        when (processRole) {
            AndroidProcessRole.Ui -> {
                try {
                    startService(Intent(this, MihomoControlService::class.java))
                } catch (error: Exception) {
                    Log.w("MihomoApp", "control service start deferred until a command: ${error.message}")
                }
            }
            AndroidProcessRole.VpnOwner -> Unit
            is AndroidProcessRole.Unsupported ->
                Log.e("MihomoApp", "unsupported application process: ${processRole.processName}")
        }
    }

    private fun registerProcessBridge() {
        val initCode = RustBridge.init(filesDir.absolutePath, cacheDir.absolutePath)
        if (initCode != 0) {
            Log.w("MihomoApp", "native init failed: $initCode")
            return
        }
        val host = MihomoHost(applicationContext)
        processHost = host
        val registerCode = RustBridge.registerBridge(host)
        if (registerCode != 0) {
            Log.w("MihomoApp", "process bridge registration failed: $registerCode")
        } else {
            Log.i("MihomoApp", "process bridge registered: ${Application.getProcessName()}")
        }
    }

    override fun onTerminate() {
        super.onTerminate()
        ownerIpcScope.cancel()
        if (ownerIpcConnectionDelegate.isInitialized()) ownerIpcConnectionDelegate.value.close()
        bridgeShutdown()
    }
}

private fun AndroidCoreLifecycleResult.toOwnerCoreReadback(): VpnOwnerCoreReadbackWire {
    if (status.code != FfiErrorCode.OK || lifecycle == null) {
        return VpnOwnerCoreReadbackWire.Unavailable(
            status.message ?: "Core lifecycle readback is unavailable",
        )
    }
    if (generation < 0L || revision < 0L ||
        (sessionToken != null && (sessionToken.isBlank() || !sessionToken.all(Char::isDigit)))
    ) {
        return VpnOwnerCoreReadbackWire.Unavailable("Core lifecycle counters or session token are invalid")
    }
    val identity = CoreOwnerIdentityWire.fromNullableParts(engineId, coreInstanceId, coreVersion)
        ?: return VpnOwnerCoreReadbackWire.Unavailable("Core identity readback is incomplete")
    val failure = when (val rawFailure = failureJson) {
        null -> null
        else -> CoreOwnerFailureWire.parse(rawFailure)
            ?: return VpnOwnerCoreReadbackWire.Unavailable("Core failure facts are malformed")
    }
    return VpnOwnerCoreReadbackWire.Available(
        CoreOwnerSnapshotWire(
            identity = identity,
            lifecycle = lifecycle.toCoreOwnerLifecycleWire(),
            generation = generation.toLong(),
            sessionToken = sessionToken,
            revision = revision.toLong(),
            failure = failure,
        ),
    )
}

private fun VpnProcessState.toOwnerLifecycle(permissionRequired: Boolean): VpnOwnerLifecycleWire = when {
    permissionRequired -> VpnOwnerLifecycleWire.PermissionRequired
    else -> when (this) {
        VpnProcessState.StartRequested -> VpnOwnerLifecycleWire.StartRequested
        VpnProcessState.Starting -> VpnOwnerLifecycleWire.Starting
        VpnProcessState.Foreground -> VpnOwnerLifecycleWire.Foreground
        VpnProcessState.TunnelEstablished -> VpnOwnerLifecycleWire.TunnelEstablished
        VpnProcessState.Running -> VpnOwnerLifecycleWire.Running
        VpnProcessState.Stopping -> VpnOwnerLifecycleWire.Stopping
        VpnProcessState.Stopped -> VpnOwnerLifecycleWire.Stopped
        VpnProcessState.Revoked -> VpnOwnerLifecycleWire.Revoked
        is VpnProcessState.Failed -> VpnOwnerLifecycleWire.Failed(message)
    }
}

private fun VpnOwnerLifecycleWire.toOwnerReadiness(
    resources: List<VpnOwnerResourceFactWire>,
): VpnOwnerReadinessWire {
    if (resources.any { it.state is VpnOwnerResourceStateWire.CleanupPending }) {
        return VpnOwnerReadinessWire.CleanupPending
    }
    return when (this) {
        VpnOwnerLifecycleWire.PermissionRequired -> VpnOwnerReadinessWire.PermissionRequired
        VpnOwnerLifecycleWire.StartRequested,
        VpnOwnerLifecycleWire.Starting -> VpnOwnerReadinessWire.AwaitingForeground
        VpnOwnerLifecycleWire.Foreground -> VpnOwnerReadinessWire.AwaitingTunnel
        VpnOwnerLifecycleWire.TunnelEstablished -> VpnOwnerReadinessWire.AwaitingDataPlane
        VpnOwnerLifecycleWire.Running -> VpnOwnerReadinessWire.Ready
        VpnOwnerLifecycleWire.Stopping -> VpnOwnerReadinessWire.Stopping
        VpnOwnerLifecycleWire.Stopped -> VpnOwnerReadinessWire.Stopped
        VpnOwnerLifecycleWire.Revoked -> VpnOwnerReadinessWire.Revoked
        is VpnOwnerLifecycleWire.Failed -> VpnOwnerReadinessWire.Failed
        is VpnOwnerLifecycleWire.Unavailable -> VpnOwnerReadinessWire.Unavailable
    }
}
