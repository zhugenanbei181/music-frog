package com.musicfrog.infiltrator.ui.common

import com.musicfrog.infiltrator.CoreOwnerFailureWire
import com.musicfrog.infiltrator.CoreOwnerIdentityWire
import com.musicfrog.infiltrator.CoreOwnerLifecycleWire
import com.musicfrog.infiltrator.VpnOwnerSnapshotWire
import infiltrator_android.FfiErrorCode
import infiltrator_android.androidSurfaceSnapshotJson
import org.json.JSONObject

internal suspend fun readOwnerSurfaceSnapshot(): OwnerSurfaceRead {
    val result = androidSurfaceSnapshotJson()
    if (result.status.code != FfiErrorCode.OK) {
        return OwnerSurfaceRead.Unavailable(
            result.status.toLocalizedFailure(),
        )
    }
    val json = result.value ?: return OwnerSurfaceRead.Unavailable(
        localizedFailure(FailureTextKey.NotReady),
    )
    return parseOwnerSurfaceSnapshot(json)
}

internal fun parseOwnerSurfaceSnapshot(json: String): OwnerSurfaceRead = try {
    val root = JSONObject(json)
    if (root.optString("origin") != "live") {
        OwnerSurfaceRead.Unavailable(localizedFailure(FailureTextKey.NotReady))
    } else {
        val pages = root.getJSONObject("pages")
        val vpn = parseVpnObservation(root.get("vpn"))
        val core = root.optJSONObject("core")
        val rawCore = rawJsonValueAtPath(json, "core")
        val coreScope = if (core != null && !core.isNull("identity") && rawCore != null) {
            parseCoreScopeObservation(core, coreScopeJsonFromSnapshot(rawCore))
        } else {
            null
        }
        validateVpnCoreBinding(vpn, core, rawCore)
        val coreRoutingIntent = core?.getJSONObject("routing_intent")
            ?.let(::parseRoutingIntentSnapshotObservation)
            ?: throw IllegalArgumentException("Core owner routing intent snapshot is missing")
        val overview = parsePage(pages.getJSONObject("overview"), ::parseOverview)
        val overviewRoutingIntent = overview.currentData()?.routingIntent
        require(overviewRoutingIntent == null || overviewRoutingIntent == coreRoutingIntent) {
            "Core and overview routing intent projections disagree"
        }
        val rawOutboundData = rawJsonValueAtPath(json, "outbound_inventory", "data")
        val outboundInventory = parseOutboundInventoryPage(
            root.optJSONObject("outbound_inventory"),
            rawOutboundData,
        )
        val coreCapabilitySnapshot = parseObservationPage(
            root.optJSONObject("core_capability_snapshot"),
            rawJsonValueAtPath(json, "core_capability_snapshot", "data"),
            ::parseCoreCapabilitySnapshot,
        ).asCoreCapabilitySnapshotPageState()
        validateCoreCapabilityBinding(
            coreScope,
            root.getJSONObject("capabilities").getString("host").toCoreHostKind(),
            coreCapabilitySnapshot,
        )
        OwnerSurfaceRead.Available(
            OwnerSurfaceSnapshot(
                generation = root.unsigned("generation"),
                revision = root.unsigned("revision"),
                coreScope = coreScope,
                vpn = vpn,
                coreRoutingIntent = coreRoutingIntent,
                overview = overview,
                outboundInventory = outboundInventory,
                outboundDetails = parseObservationPage(
                    root.optJSONObject("outbound_details"),
                    rawJsonValueAtPath(json, "outbound_details", "data"),
                    ::parseOutboundDetailsSnapshot,
                ).asOutboundDetailsPageState(),
                outboundUiPreferences = parseObservationPage(
                    root.optJSONObject("outbound_ui_preferences"),
                    rawJsonValueAtPath(json, "outbound_ui_preferences", "data"),
                    ::parseOutboundUiPreferencesSnapshot,
                ).asOutboundPreferencesPageState(),
                coreCapabilitySnapshot = coreCapabilitySnapshot,
                outboundProbeCapability = outboundProbeCapability(outboundInventory, coreCapabilitySnapshot),
                protocolStudio = parseObservationPage(
                    root.optJSONObject("protocol_studio"),
                    rawJsonValueAtPath(json, "protocol_studio", "data"),
                    ::parseProtocolStudioSnapshot,
                ).asProtocolStudioPageState(),
                connections = parsePage(pages.getJSONObject("connections"), ::parseConnections),
                logs = parsePage(pages.getJSONObject("logs"), ::parseLogs),
            ),
        )
    }
} catch (error: Exception) {
    OwnerSurfaceRead.Unavailable(error.toLocalizedFailure())
}

private fun parseVpnObservation(value: Any): OwnerVpnObservation = when (value) {
    "NotApplicable" -> OwnerVpnObservation.NotApplicable

    is JSONObject -> when (val variant = value.singleVariant()) {
        "AndroidOwner" -> {
            val snapshotJson = value.getJSONObject(variant).toString()
            val snapshot = VpnOwnerSnapshotWire.parse(snapshotJson)
                ?: throw IllegalArgumentException("Android VPN owner snapshot is malformed")
            OwnerVpnObservation.AndroidOwner(snapshot)
        }

        "Unsupported" -> OwnerVpnObservation.Unsupported(
            value.getJSONObject(variant).getString("reason"),
        )

        "Unavailable" -> OwnerVpnObservation.Unavailable(
            parseOwnerFailure(value.getJSONObject(variant).getJSONObject("failure")),
        )

        else -> throw IllegalArgumentException("Unknown VPN surface observation: $variant")
    }

    else -> throw IllegalArgumentException("VPN surface observation is malformed")
}

private fun validateVpnCoreBinding(vpn: OwnerVpnObservation, surfaceCore: JSONObject?, rawSurfaceCore: String?) {
    val ownerCore = when (vpn) {
        is OwnerVpnObservation.AndroidOwner -> vpn.snapshot.core

        is OwnerVpnObservation.Unsupported,
        OwnerVpnObservation.NotApplicable,
        is OwnerVpnObservation.Unavailable,
        -> return
    }
    when (ownerCore) {
        is com.musicfrog.infiltrator.VpnOwnerCoreReadbackWire.Unavailable -> {
            require(surfaceCore == null || surfaceCore.isNull("identity")) {
                "VPN owner Core readback is unavailable while Surface Core identity is present"
            }
        }

        is com.musicfrog.infiltrator.VpnOwnerCoreReadbackWire.Available -> {
            val owner = ownerCore.snapshot
            val core = surfaceCore ?: throw IllegalArgumentException("Surface Core snapshot is missing")
            val rawCore = rawSurfaceCore ?: throw IllegalArgumentException("Surface Core JSON is missing")
            val surfaceIdentity = CoreOwnerIdentityWire.parse(core.get("identity").toString())
                ?: throw IllegalArgumentException("Surface Core identity is invalid")
            val rawGeneration = rawJsonValueAtPath(rawCore, "generation")
                ?: throw IllegalArgumentException("Surface Core generation is missing")
            val rawSession = rawJsonValueAtPath(rawCore, "session_token")
                ?: throw IllegalArgumentException("Surface Core session token is missing")
            val rawRevision = rawJsonValueAtPath(rawCore, "revision")
                ?: throw IllegalArgumentException("Surface Core revision is missing")
            val surfaceFailureJson = core.get("failure")
            val surfaceFailure = when (surfaceFailureJson) {
                JSONObject.NULL -> null
                is JSONObject -> CoreOwnerFailureWire.parse(surfaceFailureJson)
                else -> throw IllegalArgumentException("Surface Core failure is invalid")
            }
            val surfaceLifecycle = CoreOwnerLifecycleWire.entries.firstOrNull {
                it.wireName == core.getString("lifecycle")
            } ?: throw IllegalArgumentException("Surface Core lifecycle is invalid")
            val ownerSession = owner.sessionToken
            val surfaceSession = rawSession
                .takeUnless { it == "null" }
                ?.removeSurrounding("\"")
            require(
                owner.identity == surfaceIdentity &&
                    owner.lifecycle == surfaceLifecycle &&
                    owner.generation.toULong().toString() ==
                    parseUnsignedDecimal(rawGeneration, "Core generation") &&
                    ownerSession == surfaceSession &&
                    owner.revision.toULong().toString() ==
                    parseUnsignedDecimal(rawRevision, "Core revision") &&
                    owner.failure == surfaceFailure,
            ) {
                "Surface Core facts and :vpn owner Core lifecycle snapshot do not match"
            }
        }
    }
}

private fun validateCoreCapabilityBinding(
    coreScope: CoreScopeObservation?,
    surfaceHost: CoreHostKindObservation,
    capabilityPage: CoreCapabilitySnapshotPageState,
) {
    val capability = when (capabilityPage) {
        CoreCapabilitySnapshotPageState.Loading,
        is CoreCapabilitySnapshotPageState.Unavailable,
        is CoreCapabilitySnapshotPageState.Failed,
        -> return

        is CoreCapabilitySnapshotPageState.Observed -> capabilityPage.snapshot
    }
    val core = coreScope ?: throw IllegalArgumentException(
        "Core capability snapshot is present without a matching active Core scope",
    )
    require(
        capability.scope.engineId == core.identity.engineId &&
            capability.scope.coreVersion == core.identity.version &&
            capability.scope.host == surfaceHost &&
            capability.scope.binaryTarget.host == surfaceHost,
    ) {
        "Core capability scope does not match the surface Core identity, version, or host target"
    }
}

private fun String.toCoreHostKind(): CoreHostKindObservation = when (this) {
    "Desktop" -> CoreHostKindObservation.Desktop
    "Android" -> CoreHostKindObservation.Android
    "Ios" -> CoreHostKindObservation.Ios
    else -> throw IllegalArgumentException("Unknown surface host kind: $this")
}

private fun OutboundObservationPageState<OutboundDetailsSnapshot>.asOutboundDetailsPageState(): OutboundDetailsPageState = when (this) {
    OutboundObservationPageState.Loading -> OutboundDetailsPageState.Loading
    is OutboundObservationPageState.Observed -> OutboundDetailsPageState.Observed(snapshot)
    is OutboundObservationPageState.Unavailable -> OutboundDetailsPageState.Unavailable(failure)
    is OutboundObservationPageState.Failed -> OutboundDetailsPageState.Failed(failure)
}

private fun OutboundObservationPageState<OutboundUiPreferencesSnapshot>.asOutboundPreferencesPageState(): OutboundUiPreferencesPageState =
    when (this) {
        OutboundObservationPageState.Loading -> OutboundUiPreferencesPageState.Loading
        is OutboundObservationPageState.Observed -> OutboundUiPreferencesPageState.Observed(snapshot)
        is OutboundObservationPageState.Unavailable -> OutboundUiPreferencesPageState.Unavailable(failure)
        is OutboundObservationPageState.Failed -> OutboundUiPreferencesPageState.Failed(failure)
    }

private fun OutboundObservationPageState<CoreCapabilitySnapshotObservation>.asCoreCapabilitySnapshotPageState():
    CoreCapabilitySnapshotPageState =
    when (this) {
        OutboundObservationPageState.Loading -> CoreCapabilitySnapshotPageState.Loading
        is OutboundObservationPageState.Observed -> CoreCapabilitySnapshotPageState.Observed(snapshot)
        is OutboundObservationPageState.Unavailable -> CoreCapabilitySnapshotPageState.Unavailable(failure)
        is OutboundObservationPageState.Failed -> CoreCapabilitySnapshotPageState.Failed(failure)
    }

private fun OutboundObservationPageState<OwnerProtocolStudioSnapshot>.asProtocolStudioPageState(): ProtocolStudioPageState = when (this) {
    OutboundObservationPageState.Loading -> ProtocolStudioPageState.Loading
    is OutboundObservationPageState.Observed -> ProtocolStudioPageState.Observed(snapshot)
    is OutboundObservationPageState.Unavailable -> ProtocolStudioPageState.Unavailable(failure)
    is OutboundObservationPageState.Failed -> ProtocolStudioPageState.Failed(failure)
}

internal fun OwnerSurfaceSnapshot.trafficSnapshot(): TrafficSnapshot? {
    val overviewData = overview.currentData() ?: return null
    val connectionsData = connections.currentData()
    return TrafficSnapshot(
        upRate = overviewData.uploadBps,
        downRate = overviewData.downloadBps,
        upTotal = connectionsData?.uploadTotal,
        downTotal = connectionsData?.downloadTotal,
        connections = overviewData.activeConnections,
    )
}

private fun <T> parsePage(source: JSONObject, decode: (JSONObject) -> T): OwnerPage<T> {
    val stateValue = source.get("status")
    val (state, failure) = parsePageState(stateValue)
    val dataObject = source.optJSONObject("data")
    if ((state == OwnerPageState.Ready || state == OwnerPageState.Empty) && dataObject == null) {
        throw IllegalArgumentException("Owner page marked $state has no data")
    }
    val data = dataObject?.let(decode)
    return OwnerPage(state, data, failure)
}

private fun parsePageState(value: Any): Pair<OwnerPageState, OwnerFailure?> = when (value) {
    is String -> when (value) {
        "Loading" -> OwnerPageState.Loading to null
        "Ready" -> OwnerPageState.Ready to null
        "Empty" -> OwnerPageState.Empty to null
        else -> throw IllegalArgumentException("Unknown owner page state: $value")
    }

    is JSONObject -> {
        val variant = value.keys().asSequence().singleOrNull()
            ?: throw IllegalArgumentException("Owner page state must have one variant")
        val payload = value.optJSONObject(variant)
        when (variant) {
            "Unavailable" -> OwnerPageState.Unavailable to ownerPageFailure(payload, variant)
            "Failed" -> OwnerPageState.Failed to ownerPageFailure(payload, variant)
            else -> throw IllegalArgumentException("Unknown owner page state: $variant")
        }
    }

    else -> throw IllegalArgumentException("Owner page state is malformed")
}

private fun ownerPageFailure(payload: JSONObject?, variant: String): OwnerFailure =
    payload?.optJSONObject("failure")?.let(::parseOwnerFailure)
        ?: throw IllegalArgumentException("Owner $variant page state has no typed failure")

private fun parseOverview(data: JSONObject): OwnerOverviewPage = OwnerOverviewPage(
    routingIntent = parseRoutingIntentSnapshotObservation(data.getJSONObject("routing_intent")),
    uploadBps = data.rate("upload_bps"),
    downloadBps = data.rate("download_bps"),
    activeConnections = data.unsigned("active_connections").toUInt(),
)

private fun parseConnections(data: JSONObject): OwnerConnectionsPage = OwnerConnectionsPage(
    totalConnections = data.getInt("total_connections"),
    uploadTotal = data.unsigned("total_upload_bytes"),
    downloadTotal = data.unsigned("total_download_bytes"),
    connections = data.getJSONArray("connections").mapObjects { connection ->
        ConnectionRecord(
            id = connection.getString("id"),
            host = connection.optString("host"),
            processPath = connection.optString("process"),
            network = connection.optString("network"),
            connectionType = null,
            rule = connection.optString("rule"),
            upload = connection.unsigned("upload_total"),
            download = connection.unsigned("download_total"),
            chains = connection.getJSONArray("chains").strings()
                .ifEmpty { connection.optString("chain").takeIf(String::isNotBlank)?.let(::listOf).orEmpty() },
            destinationIpAsn = connection.optString("destination_ip_asn"),
            destinationGeoIp = connection.optJSONArray("destination_geo_ip")?.strings(),
        )
    },
)

private fun parseLogs(data: JSONObject): OwnerLogsPage = OwnerLogsPage(
    stream = parseLogStreamState(data.get("stream")),
    totalEntries = data.getInt("total_entries"),
    entries = data.getJSONArray("entries").mapObjects { entry ->
        LogEntry(
            level = parseLogLevel(entry.optString("level")),
            message = entry.optString("message"),
            timestamp = entry.optString("timestamp"),
        )
    },
)

private fun parseLogStreamState(value: Any): OwnerLogStreamState = when (value) {
    is String -> when (value) {
        "Idle" -> OwnerLogStreamState.IDLE
        "Connecting" -> OwnerLogStreamState.CONNECTING
        "Live" -> OwnerLogStreamState.LIVE
        else -> OwnerLogStreamState.UNKNOWN
    }

    is JSONObject -> when (value.keys().asSequence().singleOrNull()) {
        "Reconnecting" -> OwnerLogStreamState.RECONNECTING
        "Failed" -> OwnerLogStreamState.FAILED
        else -> OwnerLogStreamState.UNKNOWN
    }

    else -> OwnerLogStreamState.UNKNOWN
}

private fun parseLogLevel(value: String): LogLevel = when (value.trim().lowercase()) {
    "debug" -> LogLevel.DEBUG
    "info" -> LogLevel.INFO
    "warn", "warning" -> LogLevel.WARNING
    "error" -> LogLevel.ERROR
    "silent" -> LogLevel.SILENT
    else -> LogLevel.UNKNOWN
}
