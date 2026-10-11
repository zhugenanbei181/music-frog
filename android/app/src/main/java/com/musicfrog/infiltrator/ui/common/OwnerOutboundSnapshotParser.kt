package com.musicfrog.infiltrator.ui.common

import org.json.JSONArray
import org.json.JSONObject

internal fun parseOutboundInventoryPage(
    source: JSONObject?,
    rawDataJson: String?,
): OutboundInventoryPageState {
    if (source == null) {
        return OutboundInventoryPageState.Unavailable(
            OwnerFailure(
                code = OwnerErrorCode.NotReady,
                message = "",
                retryable = true,
                reason = null,
            ),
        )
    }
    val status = source.get("status")
    return when (status) {
        "Loading" -> OutboundInventoryPageState.Loading
        "Ready", "Empty" -> {
            val data = source.optJSONObject("data")
                ?: throw IllegalArgumentException("Owner outbound inventory page marked $status has no data")
            OutboundInventoryPageState.Observed(
                parseOutboundInventorySnapshot(
                    data,
                    rawDataJson ?: throw IllegalArgumentException("Owner outbound inventory data is malformed"),
                ),
            )
        }
        is JSONObject -> {
            val variant = status.keys().asSequence().singleOrNull()
                ?: throw IllegalArgumentException("Owner outbound inventory page state must have one variant")
            val failure = status.optJSONObject(variant)?.optJSONObject("failure")?.let(::parseOwnerFailure)
                ?: throw IllegalArgumentException("Owner outbound inventory $variant state has no failure")
            when (variant) {
                "Unavailable" -> OutboundInventoryPageState.Unavailable(failure)
                "Failed" -> OutboundInventoryPageState.Failed(failure)
                else -> throw IllegalArgumentException("Unknown owner outbound inventory page state: $variant")
            }
        }
        else -> throw IllegalArgumentException("Owner outbound inventory page state is malformed")
    }
}

internal fun <T> parseObservationPage(
    source: JSONObject?,
    rawDataJson: String?,
    decode: (JSONObject, String) -> T,
): OutboundObservationPageState<T> {
    if (source == null) {
        return OutboundObservationPageState.Unavailable(
            OwnerFailure(OwnerErrorCode.NotReady, "", true, null),
        )
    }
    val status = source.get("status")
    return when (status) {
        "Loading" -> OutboundObservationPageState.Loading
        "Ready", "Empty" -> {
            val data = source.optJSONObject("data")
                ?: throw IllegalArgumentException("Owner typed page marked $status has no data")
            OutboundObservationPageState.Observed(
                decode(
                    data,
                    rawDataJson ?: throw IllegalArgumentException("Owner typed page data is malformed"),
                ),
            )
        }
        is JSONObject -> {
            val variant = status.keys().asSequence().singleOrNull()
                ?: throw IllegalArgumentException("Owner typed page state must have one variant")
            val failure = status.optJSONObject(variant)?.optJSONObject("failure")?.let(::parseOwnerFailure)
                ?: throw IllegalArgumentException("Owner typed page $variant state has no failure")
            when (variant) {
                "Unavailable" -> OutboundObservationPageState.Unavailable(failure)
                "Failed" -> OutboundObservationPageState.Failed(failure)
                else -> throw IllegalArgumentException("Unknown owner typed page state: $variant")
            }
        }
        else -> throw IllegalArgumentException("Owner typed page state is malformed")
    }
}

internal fun parseOutboundInventorySnapshot(
    data: JSONObject,
    rawDataJson: String,
): OutboundInventorySnapshot {
    val scopeJson = data.getJSONObject("scope")
    val rawScopeJson = rawJsonValueAtPath(rawDataJson, "scope")
        ?: throw IllegalArgumentException("Owner outbound inventory has no CoreScope")
    return OutboundInventorySnapshot(
        scope = parseCoreScopeObservation(scopeJson, rawScopeJson),
        revision = parseUnsignedDecimal(
            rawJsonValueAtPath(rawDataJson, "revision")
                ?: throw IllegalArgumentException("Owner outbound inventory has no revision"),
            "revision",
        ),
        inventory = parseOutboundInventoryState(
            data.get("inventory"),
            scopeJson.getJSONObject("identity").getString("engine"),
        ),
    )
}

internal fun parseOutboundDetailsSnapshot(
    data: JSONObject,
    rawDataJson: String,
): OutboundDetailsSnapshot {
    val scopeJson = data.getJSONObject("scope")
    val rawScopeJson = rawJsonValueAtPath(rawDataJson, "scope")
        ?: throw IllegalArgumentException("Owner outbound details have no CoreScope")
    val engineId = scopeJson.getJSONObject("identity").getString("engine")
    return OutboundDetailsSnapshot(
        scope = parseCoreScopeObservation(scopeJson, rawScopeJson),
        detailRevision = parseUnsignedDecimal(
            rawJsonValueAtPath(rawDataJson, "detail_revision")
                ?: throw IllegalArgumentException("Owner outbound details have no detail revision"),
            "detail_revision",
        ),
        state = parseOutboundDetailsState(data.get("state"), engineId),
    )
}

private fun parseOutboundDetailsState(value: Any, engineId: String): OutboundDetailsState =
    when (value) {
        is String -> when (value) {
            "unknown" -> OutboundDetailsState.Unknown
            "empty" -> OutboundDetailsState.Empty
            "unsupported" -> OutboundDetailsState.Unsupported
            else -> throw IllegalArgumentException("Unknown outbound details state: $value")
        }
        is JSONObject -> {
            val variant = value.singleVariant()
            when (variant) {
                "ready" -> {
                    val details = value.getJSONObject(variant).getJSONArray("details").mapObjects { detail ->
                        parseOutboundDetail(detail, engineId)
                    }
                    require(details.isNotEmpty()) { "Ready outbound details must not be empty" }
                    OutboundDetailsState.Ready(details)
                }
                "failed" -> OutboundDetailsState.Failed(
                    parseOwnerFailure(value.getJSONObject(variant).getJSONObject("failure")),
                )
                else -> throw IllegalArgumentException("Unknown outbound details state: $variant")
            }
        }
        else -> throw IllegalArgumentException("Outbound details state is malformed")
    }

private fun parseOutboundDetail(value: JSONObject, engineId: String): OutboundDetailObservation =
    OutboundDetailObservation(
        id = parseOutboundId(value.getJSONObject("id"), engineId),
        protocol = parseOutboundProtocolObservation(value.getJSONObject("protocol"), engineId),
        protocolDisplayName = value.nullableString("protocol_display_name"),
        cipher = parseOutboundObservation(value.getJSONObject("cipher")) { cipher ->
            parseOutboundId(cipher as JSONObject, engineId)
        },
        cipherDisplayName = value.nullableString("cipher_display_name"),
        collectionKind = parseOutboundObservation(value.getJSONObject("collection_kind")) { kind ->
            kind as? String ?: throw IllegalArgumentException("Outbound collection kind is malformed")
        },
        features = parseOutboundObservation(value.getJSONObject("features")) { features ->
            (features as? JSONArray)?.strings()
                ?: throw IllegalArgumentException("Outbound feature list is malformed")
        },
        endpoint = parseOutboundObservation(value.getJSONObject("endpoint")) { endpoint ->
            val endpointJson = endpoint as? JSONObject
                ?: throw IllegalArgumentException("Outbound endpoint is malformed")
            OutboundEndpointObservation(
                host = endpointJson.getString("host"),
                port = parseOutboundObservation(endpointJson.getJSONObject("port")) { port ->
                    (port as? Number)?.toInt()?.takeIf { it in 0..65535 }
                        ?: throw IllegalArgumentException("Outbound endpoint port is malformed")
                },
            )
        },
        udp = value.getString("udp").toOutboundUdpSupport(),
        health = value.getString("health").toOutboundHealthObservation(),
        currentLatencyMs = parseOutboundObservation(value.getJSONObject("current_latency_ms")) { latency ->
            latency.asUnsignedLong("current_latency_ms", MAX_U32)
        },
        latencyHistory = parseOutboundObservation(value.getJSONObject("latency_history")) { history ->
            val samples = history as? JSONArray
                ?: throw IllegalArgumentException("Outbound latency history is malformed")
            samples.mapObjects { sample ->
                OutboundLatencySample(
                    observedAt = sample.getString("observed_at"),
                    delayMs = sample.get("delay_ms").asUnsignedLong("delay_ms", MAX_U32),
                )
            }
        },
        rtt = parseOutboundObservation(value.getJSONObject("rtt")) { stats ->
            val statsJson = stats as? JSONObject
                ?: throw IllegalArgumentException("Outbound RTT statistics are malformed")
            OutboundRttStatistics(
                minMs = statsJson.optionalUnsignedLong("min_ms", MAX_U32),
                maxMs = statsJson.optionalUnsignedLong("max_ms", MAX_U32),
                avgMs = statsJson.optionalUnsignedLong("avg_ms", MAX_U32),
                validCount = parseUnsignedDecimal(statsJson.get("valid_count").toString(), "valid_count"),
            )
        },
        historicalEgress = parseOutboundObservation(value.getJSONObject("historical_egress")) { egress ->
            val samples = egress as? JSONArray
                ?: throw IllegalArgumentException("Outbound historical egress is malformed")
            samples.mapObjects { sample ->
                val epoch = sample.opt("observed_at_epoch_ms")
                OutboundEgressSample(
                    ip = sample.getString("ip"),
                    country = sample.nullableString("country"),
                    observedAtEpochMs = if (epoch == null || epoch === JSONObject.NULL) null
                    else parseUnsignedDecimal(epoch.toString(), "observed_at_epoch_ms"),
                )
            }
        },
        probeAvailability = value.getString("probe_availability").toOutboundProbeAvailability(),
    )

private fun parseOutboundProtocolObservation(
    value: JSONObject,
    engineId: String,
): OutboundProtocolObservation = when (value.getString("state")) {
    "unknown" -> OutboundProtocolObservation.Unknown
    "unsupported" -> OutboundProtocolObservation.Unsupported
    "reported" -> OutboundProtocolObservation.Reported(
        parseOutboundId(value.getJSONObject("value"), engineId),
    )
    else -> throw IllegalArgumentException("Unknown outbound protocol observation state")
}

private fun <T> parseOutboundObservation(value: JSONObject, decode: (Any) -> T): OutboundObservation<T> =
    when (value.getString("state")) {
        "unknown" -> OutboundObservation.Unknown
        "unsupported" -> OutboundObservation.Unsupported
        "observed" -> {
            val observed = value.opt("value")
            require(observed != null && observed != JSONObject.NULL) {
                "Observed outbound fact has no value"
            }
            OutboundObservation.Observed(decode(observed))
        }
        else -> throw IllegalArgumentException("Unknown outbound observation state")
    }

internal fun parseOutboundUiPreferencesSnapshot(
    data: JSONObject,
    rawDataJson: String,
): OutboundUiPreferencesSnapshot {
    val scopeJson = data.getJSONObject("scope")
    val rawScopeJson = rawJsonValueAtPath(rawDataJson, "scope")
        ?: throw IllegalArgumentException("Owner outbound preferences have no CoreScope")
    val engineId = scopeJson.getJSONObject("identity").getString("engine")
    return OutboundUiPreferencesSnapshot(
        scope = parseCoreScopeObservation(scopeJson, rawScopeJson),
        revision = parseUnsignedDecimal(
            rawJsonValueAtPath(rawDataJson, "revision")
                ?: throw IllegalArgumentException("Owner outbound preferences have no revision"),
            "revision",
        ),
        searchQuery = data.getString("search_query"),
        favoriteOutboundIds = data.getJSONArray("favorite_outbound_ids").mapObjects {
            parseOutboundId(it, engineId)
        },
        groupOrder = data.getJSONArray("group_order").mapObjects { parseOutboundId(it, engineId) },
        collapsedGroupIds = data.getJSONArray("collapsed_group_ids").mapObjects {
            parseOutboundId(it, engineId)
        },
        sortOrder = parseOutboundSortOrder(data.getString("sort_order")),
        healthFilter = parseOutboundHealthFilter(data.getString("health_filter")),
        layout = parseOutboundLayout(data.getString("layout")),
    )
}

internal fun parseProtocolStudioSnapshot(
    data: JSONObject,
    @Suppress("UNUSED_PARAMETER") rawDataJson: String,
): OwnerProtocolStudioSnapshot {
    val draft = data.optionalObject("draft")?.let { value ->
        ProtocolStudioDraftObservation(
            name = value.getString("name"),
            server = value.getString("server"),
            port = value.getInt("port").takeIf { it in 0..65535 }
                ?: throw IllegalArgumentException("Protocol studio draft port is outside the valid range"),
            nodeType = value.getString("node_type"),
            cipher = value.getString("cipher"),
            flow = value.getString("flow"),
        )
    }
    val report = data.optionalObject("report")?.let { value ->
        ProtocolStudioReportObservation(
            family = value.getString("family"),
            cipherChip = value.nullableString("cipher_chip"),
            flowChip = value.nullableString("flow_chip"),
            issues = value.getJSONArray("issues").mapObjects { issue ->
                ProtocolStudioIssueObservation(issue.getString("field"), issue.getString("message"))
            },
        )
    }
    val audit = data.optionalObject("audit")?.let { value ->
        ProtocolStudioAuditObservation(
            sourceFormat = value.getString("source_format"),
            targetFormat = value.getString("target_format"),
            nodeCount = parseUnsignedDecimal(value.get("node_count").toString(), "node_count"),
            unknownFields = value.getJSONArray("unknown_fields").strings(),
            structurePreserved = value.getBoolean("structure_preserved"),
            lossless = value.getBoolean("lossless"),
            detail = value.getString("detail"),
        )
    }
    return OwnerProtocolStudioSnapshot(
        draft = draft,
        report = report,
        uriPreview = data.nullableString("uri_preview"),
        audit = audit,
        lastError = data.nullableString("last_error"),
        lastSavedNode = data.nullableString("last_saved_node"),
        uriGaps = data.optJSONArray("uri_gaps")?.strings().orEmpty(),
    )
}

private fun Any.asUnsignedLong(name: String, maximum: Long = Long.MAX_VALUE): Long {
    val decimal = toString()
    require(decimal.matches(Regex("0|[1-9][0-9]*"))) { "Owner field $name is not an unsigned integer" }
    val value = decimal.toLongOrNull()
        ?: throw IllegalArgumentException("Owner field $name is outside the supported range")
    require(value <= maximum) { "Owner field $name is outside the supported range" }
    return value
}

private fun JSONObject.optionalUnsignedLong(name: String, maximum: Long): Long? {
    val value = opt(name) ?: return null
    if (value === JSONObject.NULL) return null
    return value.asUnsignedLong(name, maximum)
}

private fun String.toOutboundHealthObservation(): OutboundHealthObservation = when (this) {
    "unknown" -> OutboundHealthObservation.Unknown
    "healthy" -> OutboundHealthObservation.Healthy
    "unhealthy" -> OutboundHealthObservation.Unhealthy
    "unsupported" -> OutboundHealthObservation.Unsupported
    else -> throw IllegalArgumentException("Unknown outbound health state: $this")
}

private fun String.toOutboundUdpSupport(): OutboundUdpSupport = when (this) {
    "unknown" -> OutboundUdpSupport.Unknown
    "supported" -> OutboundUdpSupport.Supported
    "unsupported" -> OutboundUdpSupport.Unsupported
    else -> throw IllegalArgumentException("Unknown outbound UDP support state: $this")
}

private fun String.toOutboundProbeAvailability(): OutboundProbeAvailability = when (this) {
    "unknown" -> OutboundProbeAvailability.Unknown
    "supported" -> OutboundProbeAvailability.Supported
    "unsupported" -> OutboundProbeAvailability.Unsupported
    else -> throw IllegalArgumentException("Unknown outbound probe availability: $this")
}

private fun parseOutboundSortOrder(value: String): OutboundSortOrder = when (value) {
    "latency_ascending" -> OutboundSortOrder.LatencyAscending
    "latency_descending" -> OutboundSortOrder.LatencyDescending
    "name_ascending" -> OutboundSortOrder.NameAscending
    "name_descending" -> OutboundSortOrder.NameDescending
    else -> throw IllegalArgumentException("Unknown outbound sort order: $value")
}

private fun parseOutboundHealthFilter(value: String): OutboundHealthFilter = when (value) {
    "all" -> OutboundHealthFilter.All
    "healthy_only" -> OutboundHealthFilter.HealthyOnly
    else -> throw IllegalArgumentException("Unknown outbound health filter: $value")
}

private fun parseOutboundLayout(value: String): OutboundLayout = when (value) {
    "compact_list" -> OutboundLayout.CompactList
    "grid" -> OutboundLayout.Grid
    else -> throw IllegalArgumentException("Unknown outbound layout: $value")
}

private fun parseOutboundInventoryState(value: Any, engineId: String): OutboundInventoryState =
    when (value) {
        is String -> when (value) {
            "unknown" -> OutboundInventoryState.Unknown
            "empty" -> OutboundInventoryState.Empty
            "unsupported" -> OutboundInventoryState.Unsupported
            else -> throw IllegalArgumentException("Unknown outbound inventory state: $value")
        }
        is JSONObject -> {
            val variant = value.keys().asSequence().singleOrNull()
                ?: throw IllegalArgumentException("Outbound inventory state must have one variant")
            when (variant) {
                "ready" -> {
                    val payload = value.getJSONObject(variant)
                    OutboundInventoryState.Ready(
                        entries = payload.getJSONArray("entries").mapObjects { entry ->
                            val id = parseOutboundId(entry.getJSONObject("id"), engineId)
                            OutboundEntryObservation(id, entry.nullableString("display_name"))
                        },
                        collections = payload.getJSONArray("collections").mapObjects { collection ->
                            OutboundSelectionCollection(
                                id = parseOutboundId(collection.getJSONObject("id"), engineId),
                                memberIds = collection.getJSONArray("member_ids").mapObjects { member ->
                                    parseOutboundId(member, engineId)
                                },
                                availability = parseSelectionAvailability(
                                    collection.getString("availability"),
                                ),
                                selection = parseOutboundSelectionState(
                                    collection.get("selection"),
                                    engineId,
                                ),
                            )
                        },
                    )
                }
                "failed" -> OutboundInventoryState.Failed(
                    parseOwnerFailure(value.getJSONObject(variant).getJSONObject("failure")),
                )
                else -> throw IllegalArgumentException("Unknown outbound inventory state: $variant")
            }
        }
        else -> throw IllegalArgumentException("Outbound inventory state is malformed")
    }

private fun parseOutboundSelectionState(
    value: Any,
    engineId: String,
): OutboundSelectionState = when (value) {
    is String -> when (value) {
        "unknown" -> OutboundSelectionState.Unknown
        "empty" -> OutboundSelectionState.Empty
        "unsupported" -> OutboundSelectionState.Unsupported
        else -> throw IllegalArgumentException("Unknown outbound selection state: $value")
    }
    is JSONObject -> {
        val variant = value.keys().asSequence().singleOrNull()
            ?: throw IllegalArgumentException("Outbound selection state must have one variant")
        when (variant) {
            "ready" -> OutboundSelectionState.Ready(
                parseOutboundId(value.getJSONObject(variant).getJSONObject("selected"), engineId),
            )
            "failed" -> OutboundSelectionState.Failed(
                parseOwnerFailure(value.getJSONObject(variant).getJSONObject("failure")),
            )
            else -> throw IllegalArgumentException("Unknown outbound selection state: $variant")
        }
    }
    else -> throw IllegalArgumentException("Outbound selection state is malformed")
}

private fun parseSelectionAvailability(value: String): OutboundSelectionAvailability = when (value) {
    "selectable" -> OutboundSelectionAvailability.Selectable
    "read_only" -> OutboundSelectionAvailability.ReadOnly
    "automatic" -> OutboundSelectionAvailability.Automatic
    "unsupported" -> OutboundSelectionAvailability.Unsupported
    "unknown" -> OutboundSelectionAvailability.Unknown
    else -> throw IllegalArgumentException("Unknown outbound selection availability: $value")
}

internal fun parseOutboundId(value: JSONObject, expectedEngineId: String): OutboundIdObservation {
    val id = OutboundIdObservation(
        engineId = value.getString("engine_id"),
        value = value.getString("value"),
    )
    if (id.engineId != expectedEngineId || id.value.isBlank()) {
        throw IllegalArgumentException("Outbound ID does not match its observed engine scope")
    }
    return id
}

private const val MAX_U32 = 4_294_967_295L
