package com.musicfrog.infiltrator

import infiltrator_android.AndroidHostRequest

internal enum class OwnerIpcRequestRole {
    SessionReadback,
    Operation,
    VpnStop,
    VpnOwnerSnapshotReadback,
}

/** Only valid request/argument combinations can cross the owner IPC boundary. */
internal sealed interface OwnerIpcPayload {
    val request: AndroidHostRequest
    val role: OwnerIpcRequestRole

    data object CoreLifecycleReadback : OwnerIpcPayload {
        override val request = AndroidHostRequest.READ_CORE_LIFECYCLE
        override val role = OwnerIpcRequestRole.SessionReadback
    }

    data object VpnOwnerSnapshotReadback : OwnerIpcPayload {
        override val request = AndroidHostRequest.READ_VPN_OWNER_SNAPSHOT
        override val role = OwnerIpcRequestRole.VpnOwnerSnapshotReadback
    }

    data object SurfaceSnapshotReadback : OwnerIpcPayload {
        override val request = AndroidHostRequest.READ_SURFACE_SNAPSHOT
        override val role = OwnerIpcRequestRole.Operation
    }

    data object OwnerDataRevisionReadback : OwnerIpcPayload {
        override val request = AndroidHostRequest.READ_OWNER_DATA_REVISION
        override val role = OwnerIpcRequestRole.Operation
    }

    data object VpnAuthorizationReadback : OwnerIpcPayload {
        override val request = AndroidHostRequest.READ_VPN_AUTHORIZATION
        override val role = OwnerIpcRequestRole.Operation
    }

    data object StartVpnService : OwnerIpcPayload {
        override val request = AndroidHostRequest.START_VPN_SERVICE
        override val role = OwnerIpcRequestRole.Operation
    }

    data class StopVpnService(val target: VpnStopTarget) : OwnerIpcPayload {
        override val request = AndroidHostRequest.STOP_VPN_SERVICE
        override val role = OwnerIpcRequestRole.VpnStop
    }

    data class ApplyVpnConfiguration(val configurationJson: String) : OwnerIpcPayload {
        override val request = AndroidHostRequest.APPLY_VPN_CONFIGURATION
        override val role = OwnerIpcRequestRole.Operation
    }

    data class ExecuteProductCommand(val commandJson: String) : OwnerIpcPayload {
        override val request = AndroidHostRequest.EXECUTE_PRODUCT_COMMAND
        override val role = OwnerIpcRequestRole.Operation
    }

    data class ExecuteOwnerOperation(val operationJson: String, val expectedCoreGeneration: Long, val expectedOwnerDataRevision: Long) :
        OwnerIpcPayload {
        override val request = AndroidHostRequest.EXECUTE_OWNER_OPERATION
        override val role = OwnerIpcRequestRole.Operation
    }

    data class ExecuteProfileAgeCredentialOwnerOperation(
        val metadataJson: String,
        val privateKeyBytes: ByteArray,
        val expectedCoreGeneration: Long,
        val expectedOwnerDataRevision: Long,
    ) : OwnerIpcPayload {
        override val request = AndroidHostRequest.EXECUTE_PROFILE_AGE_CREDENTIAL_OWNER_OPERATION
        override val role = OwnerIpcRequestRole.Operation
    }
}
