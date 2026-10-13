package infiltrator_android

/** Compile-only shapes for UniFFI types used by the isolated VPN host-state tests. */
enum class FfiErrorCode(val value: UInt) {
    OK(0u),
    INVALID_STATE(1u),
    NOT_READY(3u),
}

enum class VpnAuthorizationResult {
    GRANTED,
    DENIED,
    REVOKED,
}

data class FfiStatus(val code: FfiErrorCode, val message: String?)

enum class AndroidAppRoutingMode {
    PROXY_ALL,
    PROXY_SELECTED,
    BYPASS_SELECTED,
}

enum class AndroidCoreLifecycle {
    STOPPED,
    STARTING,
    READY,
    RUNNING,
    STOPPING,
    FAILED,
}

enum class AndroidControlIntent {
    START_PROXY,
    STOP_PROXY,
    RESUME_KERNEL,
    PAUSE_KERNEL,
    RESTART_KERNEL,
}

enum class AndroidOwnerDataDomain {
    PROFILES,
    CORE_VERSIONS,
    DNS_SETTINGS,
    TUN_SETTINGS,
    FAKE_IP_SETTINGS,
    FAKE_IP_CACHE,
    RULES,
    RULE_PROVIDERS,
    APP_ROUTING,
    WEB_DAV,
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

enum class AndroidHostProtocolVersion {
    V9,
}

enum class AndroidHostRequest {
    READ_CORE_LIFECYCLE,
    EXECUTE_PRODUCT_COMMAND,
    READ_SURFACE_SNAPSHOT,
    EXECUTE_OWNER_OPERATION,
    READ_OWNER_DATA_REVISION,
    START_VPN_SERVICE,
    STOP_VPN_SERVICE,
    READ_VPN_AUTHORIZATION,
    READ_VPN_OWNER_SNAPSHOT,
    APPLY_VPN_CONFIGURATION,
}

data class AndroidVpnRoutingActivationLease(
    val leaseId: ULong,
    val routingRevision: ULong,
)

data class AndroidVpnConfigurationActivationLease(
    val leaseId: ULong,
    val preparationId: ULong,
    val vpnGeneration: ULong,
    val ownerDataRevision: ULong,
)

sealed interface AndroidVpnConfigurationActivationResult {
    data class Acquired(
        val lease: AndroidVpnConfigurationActivationLease,
    ) : AndroidVpnConfigurationActivationResult
    data class StaleOwnerData(
        val expectedRevision: ULong,
        val observedRevision: ULong,
    ) : AndroidVpnConfigurationActivationResult
    data class PreparedRevisionMismatch(
        val preparedRevision: ULong,
        val requestedRevision: ULong,
    ) : AndroidVpnConfigurationActivationResult
    data object Busy : AndroidVpnConfigurationActivationResult
    data object Missing : AndroidVpnConfigurationActivationResult
    data object NotReady : AndroidVpnConfigurationActivationResult
    data object TransactionMismatch : AndroidVpnConfigurationActivationResult
    data object StaleGeneration : AndroidVpnConfigurationActivationResult
    data class Failed(val status: FfiStatus) : AndroidVpnConfigurationActivationResult
}

enum class AndroidVpnConfigurationActivationReleaseResult {
    RELEASED_BEFORE_CONSUME,
    RELEASED_AFTER_CONSUME,
    LEASE_MISSING,
    LEASE_MISMATCH,
}

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
