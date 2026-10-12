package infiltrator_android

/** Compile-only shapes for UniFFI types used by the isolated VPN host-state tests. */
enum class FfiErrorCode {
    OK,
    INVALID_STATE,
    NOT_READY,
}

data class FfiStatus(val code: FfiErrorCode, val message: String?)

enum class AndroidAppRoutingMode {
    PROXY_ALL,
    PROXY_SELECTED,
    BYPASS_SELECTED,
}

enum class AndroidHostReply {
    CORE_LIFECYCLE_AVAILABLE,
    PRODUCT_COMMAND_COMPLETED,
    SURFACE_SNAPSHOT_AVAILABLE,
    OWNER_OPERATION_COMPLETED,
    VPN_CONFIGURATION_BUSY,
    OWNER_DATA_REVISION_AVAILABLE,
    STALE_OWNER_DATA,
    OWNER_SESSION_CHANGED,
    STALE_SNAPSHOT,
    VPN_OWNER_SNAPSHOT_AVAILABLE,
    VPN_STOP_TIMED_OUT,
    VPN_STOP_SUPERSEDED,
    VPN_STOP_CANCELED,
    VPN_CONFIGURATION_ACCEPTED,
    VPN_AUTHORIZED,
    VPN_STOPPED,
    VPN_RUNNING,
    VPN_PERMISSION_REQUIRED,
    VPN_START_TIMED_OUT,
    SERVICE_UNAVAILABLE,
    FAILED,
}

data class AndroidVpnRoutingActivationLease(
    val leaseId: ULong,
    val routingRevision: ULong,
)

sealed interface AndroidVpnRoutingActivationReleaseResult {
    data object Released : AndroidVpnRoutingActivationReleaseResult
    data object LeaseMismatch : AndroidVpnRoutingActivationReleaseResult
    data object LeaseMissing : AndroidVpnRoutingActivationReleaseResult
    data class Failed(val status: FfiStatus) : AndroidVpnRoutingActivationReleaseResult
}

data class AndroidAppRoutingSnapshot(
    val mode: AndroidAppRoutingMode,
    val packages: List<String>,
)

data class AndroidVpnRoutingSnapshotResult(
    val status: FfiStatus,
    val snapshot: AndroidAppRoutingSnapshot?,
)

data class AndroidVpnRoutingSnapshot(
    val result: AndroidVpnRoutingSnapshotResult,
    val routingRevision: ULong,
)

fun androidVpnRoutingSnapshot(): AndroidVpnRoutingSnapshot = AndroidVpnRoutingSnapshot(
    result = AndroidVpnRoutingSnapshotResult(
        status = FfiStatus(FfiErrorCode.NOT_READY, "isolated VPN tests do not call the owner bridge"),
        snapshot = null,
    ),
    routingRevision = 0uL,
)
