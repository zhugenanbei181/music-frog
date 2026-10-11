package com.musicfrog.infiltrator.ui.common

import com.musicfrog.infiltrator.VpnOwnerSnapshotWire

internal enum class OwnerPageState {
    Loading,
    Ready,
    Empty,
    Unavailable,
    Failed,
}

internal data class OwnerPage<T>(
    val state: OwnerPageState,
    val data: T?,
    val failure: OwnerFailure?,
) {
    fun currentData(): T? = when (state) {
        OwnerPageState.Ready, OwnerPageState.Empty -> data
        OwnerPageState.Loading,
        OwnerPageState.Unavailable,
        OwnerPageState.Failed -> null
    }
}

internal data class OwnerOverviewPage(
    val routingIntent: RoutingIntentSnapshotObservation,
    val uploadBps: ULong,
    val downloadBps: ULong,
    val activeConnections: UInt,
)

internal enum class OwnerErrorCode {
    InvalidInput,
    InvalidState,
    NotReady,
    Unsupported,
    Network,
    Authentication,
    Configuration,
    Storage,
    Permission,
    Canceled,
    Internal,
}

internal sealed interface OwnerFailureReason {
    data class YamlSyntax(val line: ULong, val column: ULong) : OwnerFailureReason
    data object MixinYaml : OwnerFailureReason
    data object ActiveProfileInconsistent : OwnerFailureReason
    data object QuotaSourceChanged : OwnerFailureReason
    data object CurrentTimeUnavailable : OwnerFailureReason
    data object DownloadCanceled : OwnerFailureReason
}

internal data class OwnerFailure(
    val code: OwnerErrorCode,
    val message: String,
    val retryable: Boolean,
    val reason: OwnerFailureReason?,
)

internal data class CoreIdentityObservation(
    val engineId: String,
    val instanceId: String,
    val version: String?,
)

/** The scope attached to the exact outbound observation shown to the user. */
internal data class CoreScopeObservation(
    val identity: CoreIdentityObservation,
    val generation: String,
    val sessionToken: String?,
    internal val json: String,
)

internal data class OutboundIdObservation(
    val engineId: String,
    val value: String,
)

internal data class OutboundEntryObservation(
    val id: OutboundIdObservation,
    val displayName: String?,
)

internal enum class OutboundSelectionAvailability {
    Selectable,
    ReadOnly,
    Automatic,
    Unsupported,
    Unknown,
}

internal sealed interface OutboundSelectionState {
    data object Unknown : OutboundSelectionState
    data class Ready(val selected: OutboundIdObservation) : OutboundSelectionState
    data object Empty : OutboundSelectionState
    data object Unsupported : OutboundSelectionState
    data class Failed(val failure: OwnerFailure) : OutboundSelectionState
}

internal data class OutboundSelectionCollection(
    val id: OutboundIdObservation,
    val memberIds: List<OutboundIdObservation>,
    val availability: OutboundSelectionAvailability,
    val selection: OutboundSelectionState,
)

internal sealed interface OutboundInventoryState {
    data object Unknown : OutboundInventoryState
    data class Ready(
        val entries: List<OutboundEntryObservation>,
        val collections: List<OutboundSelectionCollection>,
    ) : OutboundInventoryState
    data object Empty : OutboundInventoryState
    data object Unsupported : OutboundInventoryState
    data class Failed(val failure: OwnerFailure) : OutboundInventoryState
}

internal data class OutboundInventorySnapshot(
    val scope: CoreScopeObservation,
    val revision: String,
    val inventory: OutboundInventoryState,
)

internal sealed interface OutboundObservation<out T> {
    data object Unknown : OutboundObservation<Nothing>
    data class Observed<T>(val value: T) : OutboundObservation<T>
    data object Unsupported : OutboundObservation<Nothing>
}

internal sealed interface OutboundProtocolObservation {
    data object Unknown : OutboundProtocolObservation
    data object Unsupported : OutboundProtocolObservation
    data class Reported(val id: OutboundIdObservation) : OutboundProtocolObservation
}

internal enum class OutboundHealthObservation {
    Unknown,
    Healthy,
    Unhealthy,
    Unsupported,
}

internal enum class OutboundUdpSupport {
    Unknown,
    Supported,
    Unsupported,
}

internal enum class OutboundProbeAvailability {
    Unknown,
    Supported,
    Unsupported,
}

internal sealed interface OutboundProbeOperationCapability {
    data object Unknown : OutboundProbeOperationCapability
    data object Supported : OutboundProbeOperationCapability
    data class Unavailable(val failure: LocalizedFailure) : OutboundProbeOperationCapability
    data class Unsupported(val failure: LocalizedFailure) : OutboundProbeOperationCapability
}

internal data class CoreCapabilityScopeObservation(
    val engineId: String,
    val coreVersion: String?,
    val host: CoreHostKindObservation,
    val binaryTarget: CoreBinaryTargetObservation,
    val adapterRevision: String,
)

internal enum class CoreHostKindObservation {
    Desktop,
    Android,
    Ios,
}

internal enum class CoreBinaryTargetObservation(val host: CoreHostKindObservation?) {
    X86_64UnknownLinuxGnu(CoreHostKindObservation.Desktop),
    Aarch64UnknownLinuxGnu(CoreHostKindObservation.Desktop),
    X86_64UnknownLinuxMusl(CoreHostKindObservation.Desktop),
    Aarch64UnknownLinuxMusl(CoreHostKindObservation.Desktop),
    X86_64PcWindowsMsvc(CoreHostKindObservation.Desktop),
    Aarch64PcWindowsMsvc(CoreHostKindObservation.Desktop),
    X86_64PcWindowsGnu(CoreHostKindObservation.Desktop),
    X86_64PcWindowsGnullvm(CoreHostKindObservation.Desktop),
    Aarch64PcWindowsGnullvm(CoreHostKindObservation.Desktop),
    X86_64AppleDarwin(CoreHostKindObservation.Desktop),
    Aarch64AppleDarwin(CoreHostKindObservation.Desktop),
    X86_64LinuxAndroid(CoreHostKindObservation.Android),
    Aarch64LinuxAndroid(CoreHostKindObservation.Android),
    X86_64AppleIos(CoreHostKindObservation.Ios),
    Aarch64AppleIos(CoreHostKindObservation.Ios),
    Aarch64AppleIosSim(CoreHostKindObservation.Ios),
    Aarch64AppleIosMacabi(CoreHostKindObservation.Ios),
    Unregistered(null),
}

internal sealed interface CoreCapabilityStatusObservation {
    data class Available(val access: String, val maturity: String) : CoreCapabilityStatusObservation
    data class Unsupported(val reasons: List<String>) : CoreCapabilityStatusObservation
    data class VersionUnavailable(val reasons: List<String>) : CoreCapabilityStatusObservation
    data class HostUnavailable(val reasons: List<String>) : CoreCapabilityStatusObservation
    data class AdapterMissing(val reasons: List<String>) : CoreCapabilityStatusObservation
    data object Unregistered : CoreCapabilityStatusObservation
    data object ScopeMismatch : CoreCapabilityStatusObservation
}

internal data class CoreCapabilityResolutionObservation(
    val operationCategory: String,
    val operation: String,
    val status: CoreCapabilityStatusObservation,
)

internal data class CoreCapabilitySnapshotObservation(
    val scope: CoreCapabilityScopeObservation,
    val resolutions: List<CoreCapabilityResolutionObservation>,
)

internal sealed interface CoreCapabilitySnapshotPageState {
    data object Loading : CoreCapabilitySnapshotPageState
    data class Observed(val snapshot: CoreCapabilitySnapshotObservation) : CoreCapabilitySnapshotPageState
    data class Unavailable(val failure: OwnerFailure) : CoreCapabilitySnapshotPageState
    data class Failed(val failure: OwnerFailure) : CoreCapabilitySnapshotPageState
}

internal data class OutboundEndpointObservation(
    val host: String,
    val port: OutboundObservation<Int>,
)

internal data class OutboundLatencySample(
    val observedAt: String,
    val delayMs: Long,
)

internal data class OutboundEgressSample(
    val ip: String,
    val country: String?,
    val observedAtEpochMs: String?,
)

internal data class OutboundRttStatistics(
    val minMs: Long?,
    val maxMs: Long?,
    val avgMs: Long?,
    val validCount: String,
)

internal data class OutboundDetailObservation(
    val id: OutboundIdObservation,
    val protocol: OutboundProtocolObservation,
    val protocolDisplayName: String?,
    val cipher: OutboundObservation<OutboundIdObservation>,
    val cipherDisplayName: String?,
    val collectionKind: OutboundObservation<String>,
    val features: OutboundObservation<List<String>>,
    val endpoint: OutboundObservation<OutboundEndpointObservation>,
    val udp: OutboundUdpSupport,
    val health: OutboundHealthObservation,
    val currentLatencyMs: OutboundObservation<Long>,
    val latencyHistory: OutboundObservation<List<OutboundLatencySample>>,
    val rtt: OutboundObservation<OutboundRttStatistics>,
    val historicalEgress: OutboundObservation<List<OutboundEgressSample>>,
    val probeAvailability: OutboundProbeAvailability,
)

internal sealed interface OutboundDetailsState {
    data object Unknown : OutboundDetailsState
    data class Ready(val details: List<OutboundDetailObservation>) : OutboundDetailsState
    data object Empty : OutboundDetailsState
    data object Unsupported : OutboundDetailsState
    data class Failed(val failure: OwnerFailure) : OutboundDetailsState
}

internal data class OutboundDetailsSnapshot(
    val scope: CoreScopeObservation,
    val detailRevision: String,
    val state: OutboundDetailsState,
)

internal sealed interface OutboundDetailsPageState {
    data object Loading : OutboundDetailsPageState
    data class Observed(val snapshot: OutboundDetailsSnapshot) : OutboundDetailsPageState
    data class Unavailable(val failure: OwnerFailure) : OutboundDetailsPageState
    data class Failed(val failure: OwnerFailure) : OutboundDetailsPageState
}

internal enum class OutboundSortOrder {
    LatencyAscending,
    LatencyDescending,
    NameAscending,
    NameDescending,
}

internal enum class OutboundHealthFilter {
    All,
    HealthyOnly,
}

internal enum class OutboundLayout {
    CompactList,
    Grid,
}

internal data class OutboundUiPreferencesSnapshot(
    val scope: CoreScopeObservation,
    val revision: String,
    val searchQuery: String,
    val favoriteOutboundIds: List<OutboundIdObservation>,
    val groupOrder: List<OutboundIdObservation>,
    val collapsedGroupIds: List<OutboundIdObservation>,
    val sortOrder: OutboundSortOrder,
    val healthFilter: OutboundHealthFilter,
    val layout: OutboundLayout,
)

internal sealed interface OutboundUiPreferencesPageState {
    data object Loading : OutboundUiPreferencesPageState
    data class Observed(val snapshot: OutboundUiPreferencesSnapshot) : OutboundUiPreferencesPageState
    data class Unavailable(val failure: OwnerFailure) : OutboundUiPreferencesPageState
    data class Failed(val failure: OwnerFailure) : OutboundUiPreferencesPageState
}

internal data class ProtocolStudioDraftObservation(
    val name: String,
    val server: String,
    val port: Int,
    val nodeType: String,
    val cipher: String,
    val flow: String,
)

internal data class ProtocolStudioIssueObservation(
    val field: String,
    val message: String,
)

internal data class ProtocolStudioReportObservation(
    val family: String,
    val cipherChip: String?,
    val flowChip: String?,
    val issues: List<ProtocolStudioIssueObservation>,
)

internal data class ProtocolStudioAuditObservation(
    val sourceFormat: String,
    val targetFormat: String,
    val nodeCount: String,
    val unknownFields: List<String>,
    val structurePreserved: Boolean,
    val lossless: Boolean,
    val detail: String,
)

internal data class OwnerProtocolStudioSnapshot(
    val draft: ProtocolStudioDraftObservation?,
    val report: ProtocolStudioReportObservation?,
    val uriPreview: String?,
    val audit: ProtocolStudioAuditObservation?,
    val lastError: String?,
    val lastSavedNode: String?,
    val uriGaps: List<String>,
)

internal sealed interface ProtocolStudioPageState {
    data object Loading : ProtocolStudioPageState
    data class Observed(val snapshot: OwnerProtocolStudioSnapshot) : ProtocolStudioPageState
    data class Unavailable(val failure: OwnerFailure) : ProtocolStudioPageState
    data class Failed(val failure: OwnerFailure) : ProtocolStudioPageState
}

internal sealed interface OutboundProbeState {
    data object Unknown : OutboundProbeState
    data object Probing : OutboundProbeState
    data class Measured(val delayMs: Long) : OutboundProbeState
    data object TimedOut : OutboundProbeState
    data object Unsupported : OutboundProbeState
    data class Failed(val failure: OwnerFailure) : OutboundProbeState
}

internal data class OutboundProbeSnapshot(
    val scope: CoreScopeObservation,
    val observedInventoryRevision: String,
    val detailRevision: String,
    val outboundId: OutboundIdObservation,
    val state: OutboundProbeState,
)

internal sealed interface OutboundObservationPageState<out T> {
    data object Loading : OutboundObservationPageState<Nothing>
    data class Observed<T>(val snapshot: T) : OutboundObservationPageState<T>
    data class Unavailable(val failure: OwnerFailure) : OutboundObservationPageState<Nothing>
    data class Failed(val failure: OwnerFailure) : OutboundObservationPageState<Nothing>
}

internal sealed interface OutboundInventoryPageState {
    data object Loading : OutboundInventoryPageState
    data class Observed(val snapshot: OutboundInventorySnapshot) : OutboundInventoryPageState
    data class Unavailable(
        val failure: OwnerFailure,
        val lastValue: OutboundInventorySnapshot? = null,
    ) : OutboundInventoryPageState
    data class Failed(val failure: OwnerFailure) : OutboundInventoryPageState
}

internal data class OwnerConnectionsPage(
    val totalConnections: Int,
    val uploadTotal: ULong,
    val downloadTotal: ULong,
    val connections: List<ConnectionRecord>,
)

data class ConnectionRecord(
    val id: String,
    val host: String,
    val processPath: String,
    val network: String,
    val connectionType: String?,
    val rule: String,
    val upload: ULong,
    val download: ULong,
    val chains: List<String>,
    val destinationIpAsn: String,
    val destinationGeoIp: List<String>?,
)

enum class LogLevel {
    DEBUG,
    INFO,
    WARNING,
    ERROR,
    SILENT,
    UNKNOWN,
}

internal enum class OwnerLogStreamState {
    IDLE,
    CONNECTING,
    LIVE,
    RECONNECTING,
    FAILED,
    UNKNOWN,
}

data class LogEntry(
    val level: LogLevel,
    val message: String,
    val timestamp: String,
)

internal data class OwnerLogsPage(
    val stream: OwnerLogStreamState,
    val totalEntries: Int,
    val entries: List<LogEntry>,
)

data class TrafficSnapshot(
    val upRate: ULong,
    val downRate: ULong,
    val upTotal: ULong?,
    val downTotal: ULong?,
    val connections: UInt,
)

internal data class OwnerSurfaceSnapshot(
    val generation: ULong,
    val revision: ULong,
    val coreScope: CoreScopeObservation?,
    val vpn: OwnerVpnObservation,
    val coreRoutingIntent: RoutingIntentSnapshotObservation,
    val overview: OwnerPage<OwnerOverviewPage>,
    val outboundInventory: OutboundInventoryPageState,
    val outboundDetails: OutboundDetailsPageState,
    val outboundUiPreferences: OutboundUiPreferencesPageState,
    val coreCapabilitySnapshot: CoreCapabilitySnapshotPageState,
    val outboundProbeCapability: OutboundProbeOperationCapability,
    val protocolStudio: ProtocolStudioPageState,
    val connections: OwnerPage<OwnerConnectionsPage>,
    val logs: OwnerPage<OwnerLogsPage>,
)

internal sealed interface OwnerVpnObservation {
    data class AndroidOwner(val snapshot: VpnOwnerSnapshotWire) : OwnerVpnObservation
    data class Unsupported(val reason: String) : OwnerVpnObservation
    data object NotApplicable : OwnerVpnObservation
    data class Unavailable(val failure: OwnerFailure) : OwnerVpnObservation
}

internal sealed interface OwnerSurfaceRead {
    data class Available(val snapshot: OwnerSurfaceSnapshot) : OwnerSurfaceRead

    data class Unavailable(val failure: LocalizedFailure) : OwnerSurfaceRead
}
