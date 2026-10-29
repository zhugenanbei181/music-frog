package com.musicfrog.infiltrator.ui.proxies

import com.musicfrog.infiltrator.ui.common.AndroidProductCommandOutcome
import com.musicfrog.infiltrator.ui.common.CoreScopeObservation
import com.musicfrog.infiltrator.ui.common.OutboundDetailsSnapshot
import com.musicfrog.infiltrator.ui.common.OutboundDetailsState
import com.musicfrog.infiltrator.ui.common.OutboundHealthFilter
import com.musicfrog.infiltrator.ui.common.OutboundIdObservation
import com.musicfrog.infiltrator.ui.common.OutboundInventorySnapshot
import com.musicfrog.infiltrator.ui.common.OutboundInventoryState
import com.musicfrog.infiltrator.ui.common.OutboundLayout
import com.musicfrog.infiltrator.ui.common.OutboundProbeAvailability
import com.musicfrog.infiltrator.ui.common.OutboundProbeOperationCapability
import com.musicfrog.infiltrator.ui.common.OutboundProbeSnapshot
import com.musicfrog.infiltrator.ui.common.OutboundProbeState
import com.musicfrog.infiltrator.ui.common.OutboundSelectionAvailability
import com.musicfrog.infiltrator.ui.common.OutboundSelectionCollection
import com.musicfrog.infiltrator.ui.common.OutboundSortOrder
import com.musicfrog.infiltrator.ui.common.OutboundUiPreferencesSnapshot
import com.musicfrog.infiltrator.ui.common.OwnerFailure
import com.musicfrog.infiltrator.ui.common.ProductCommandResult
import com.musicfrog.infiltrator.ui.common.parseAndroidProductCommandOutcome
import com.musicfrog.infiltrator.ui.common.parseCoreScopeObservation
import com.musicfrog.infiltrator.ui.common.parseOutboundId
import com.musicfrog.infiltrator.ui.common.parseOutboundInventorySnapshot
import com.musicfrog.infiltrator.ui.common.parseOutboundUiPreferencesSnapshot
import com.musicfrog.infiltrator.ui.common.parseOwnerFailure
import com.musicfrog.infiltrator.ui.common.parseUnsignedDecimal
import com.musicfrog.infiltrator.ui.common.rawJsonValueAtPath
import com.musicfrog.infiltrator.ui.common.singleVariant
import com.musicfrog.infiltrator.ui.common.wrapObservedCommand
import org.json.JSONObject

internal sealed interface OutboundCommandHandling {
    data object Success : OutboundCommandHandling
    data class SelectionApplied(val snapshot: OutboundInventorySnapshot) : OutboundCommandHandling
    data class PreferencesApplied(val snapshot: OutboundUiPreferencesSnapshot) : OutboundCommandHandling
    data class ProbeCompleted(val snapshot: OutboundProbeSnapshot) : OutboundCommandHandling
    data class OwnerFailure(val failure: com.musicfrog.infiltrator.ui.common.OwnerFailure) : OutboundCommandHandling
    data object StaleScope : OutboundCommandHandling
    data object UnexpectedResult : OutboundCommandHandling
}

internal sealed interface OutboundUiPreferenceIntent {
    data class SetSearchQuery(val query: String) : OutboundUiPreferenceIntent
    data class SetFavorite(val outboundId: OutboundIdObservation, val pinned: Boolean) : OutboundUiPreferenceIntent
    data class SetSortOrder(val order: OutboundSortOrder) : OutboundUiPreferenceIntent
    data class SetHealthFilter(val filter: OutboundHealthFilter) : OutboundUiPreferenceIntent
    data class SetLayout(val layout: OutboundLayout) : OutboundUiPreferenceIntent
    data class SetGroupExpansion(val groupId: OutboundIdObservation, val expanded: Boolean) : OutboundUiPreferenceIntent
    data class ReorderGroups(val groupIds: List<OutboundIdObservation>) : OutboundUiPreferenceIntent
    data object ResetGroupOrder : OutboundUiPreferenceIntent
}

/** Serialize the shared Rust selection command from the observation the user saw. */
internal fun buildSelectOutboundCommandJson(
    snapshot: OutboundInventorySnapshot,
    collection: OutboundSelectionCollection,
    outboundId: OutboundIdObservation,
): String {
    val ready = snapshot.inventory as? OutboundInventoryState.Ready
        ?: throw IllegalArgumentException("Outbound inventory is no longer selectable")
    val observedCollection = ready.collections.singleOrNull { it.id == collection.id }
        ?: throw IllegalArgumentException("Outbound collection was not part of the observed inventory")
    require(observedCollection.availability == OutboundSelectionAvailability.Selectable) {
        "Outbound collection is read-only"
    }
    require(observedCollection.memberIds.contains(outboundId) && ready.entries.any { it.id == outboundId }) {
        "Outbound was not part of the observed collection"
    }
    requireRevision(snapshot.revision, "Observed outbound revision")
    val requestJson = """
        {"collection_id":${outboundIdJson(
        observedCollection.id,
    )},"outbound_id":${outboundIdJson(outboundId)},"expected_revision":${snapshot.revision}}
    """.trimIndent()
    val commandJson = """
        {"Application":{"SelectOutbound":{"request":$requestJson}}}
    """.trimIndent()
    return wrapObservedCommand(snapshot.scope, commandJson)
}

/** Serialize one typed preference edit using the exact owner preference revision rendered. */
internal fun buildApplyOutboundUiPreferenceCommandJson(
    inventory: OutboundInventorySnapshot,
    preferences: OutboundUiPreferencesSnapshot,
    intent: OutboundUiPreferenceIntent,
): String {
    require(sameScope(inventory.scope, preferences.scope)) {
        "Outbound preferences belong to a different CoreScope"
    }
    requireRevision(preferences.revision, "Observed outbound preference revision")
    validatePreferenceIntent(inventory, intent)
    val intentJson = when (intent) {
        is OutboundUiPreferenceIntent.SetSearchQuery ->
            """{"operation":"set_search_query","value":${JSONObject.quote(intent.query)}}"""

        is OutboundUiPreferenceIntent.SetFavorite -> """
            {"operation":"set_favorite","value":{"outbound_id":${outboundIdJson(
            intent.outboundId,
        )},"state":"${if (intent.pinned) "pinned" else "unpinned"}"}}
        """.trimIndent()

        is OutboundUiPreferenceIntent.SetSortOrder ->
            """{"operation":"set_sort_order","value":"${intent.order.toWire()}"}"""

        is OutboundUiPreferenceIntent.SetHealthFilter ->
            """{"operation":"set_health_filter","value":"${intent.filter.toWire()}"}"""

        is OutboundUiPreferenceIntent.SetLayout ->
            """{"operation":"set_layout","value":"${intent.layout.toWire()}"}"""

        is OutboundUiPreferenceIntent.SetGroupExpansion -> """
            {"operation":"set_group_expansion","value":{"group_id":${outboundIdJson(
            intent.groupId,
        )},"expansion":"${if (intent.expanded) "expanded" else "collapsed"}"}}
        """.trimIndent()

        is OutboundUiPreferenceIntent.ReorderGroups ->
            """{"operation":"reorder_groups","value":[${intent.groupIds.joinToString(",", transform = ::outboundIdJson)}]}"""

        OutboundUiPreferenceIntent.ResetGroupOrder -> """{"operation":"reset_group_order"}"""
    }
    val requestJson = """
        {"expected_revision":${preferences.revision},"intent":$intentJson}
    """.trimIndent()
    val commandJson = """
        {"Application":{"ApplyOutboundUiPreference":{"request":$requestJson}}}
    """.trimIndent()
    return wrapObservedCommand(inventory.scope, commandJson)
}

/** Serialize a probe using the inventory revision and probe capability the user observed. */
internal fun buildProbeOutboundCommandJson(
    inventory: OutboundInventorySnapshot,
    details: OutboundDetailsSnapshot,
    operationCapability: OutboundProbeOperationCapability,
    outboundId: OutboundIdObservation,
    testUrl: String = DEFAULT_OUTBOUND_PROBE_URL,
    timeoutMs: Long = DEFAULT_OUTBOUND_PROBE_TIMEOUT_MS,
): String {
    require(
        canProbeOutbound(
            (details.state as? OutboundDetailsState.Ready)?.details?.singleOrNull { it.id == outboundId }
                ?.probeAvailability ?: OutboundProbeAvailability.Unknown,
            operationCapability,
        ),
    ) { "Outbound.Probe is not available for this observed outbound and Core scope" }
    require(sameScope(inventory.scope, details.scope)) {
        "Outbound details belong to a different CoreScope"
    }
    requireRevision(inventory.revision, "Observed outbound revision")
    require(timeoutMs > 0) { "Outbound probe timeout is invalid" }
    require(testUrl.isHttpUrlWithoutCredentials()) { "Outbound probe target must be an absolute HTTP(S) URL without credentials" }
    val ready = inventory.inventory as? OutboundInventoryState.Ready
        ?: throw IllegalArgumentException("Outbound inventory is no longer probeable")
    require(ready.entries.any { it.id == outboundId }) { "Outbound was not part of the observed inventory" }
    val detail = (details.state as? OutboundDetailsState.Ready)?.details
        ?.singleOrNull { it.id == outboundId }
        ?: throw IllegalArgumentException("Outbound probe capability was not observed")
    require(detail.probeAvailability == OutboundProbeAvailability.Supported) {
        "Outbound probe is unsupported or unknown"
    }
    val requestJson = """
        {"outbound_id":${outboundIdJson(
        outboundId,
    )},"expected_inventory_revision":${inventory.revision},"test_url":${JSONObject.quote(testUrl)},"timeout_ms":$timeoutMs}
    """.trimIndent()
    val commandJson = """
        {"Application":{"ProbeOutbound":{"request":$requestJson}}}
    """.trimIndent()
    return wrapObservedCommand(inventory.scope, commandJson)
}

/** Serialize a set runtime probe settings command with the scope the user observed. */
internal fun buildSetRuntimeProbeSettingsCommandJson(
    scope: CoreScopeObservation,
    testUrl: String = DEFAULT_OUTBOUND_PROBE_URL,
    timeoutMs: Long = DEFAULT_OUTBOUND_PROBE_TIMEOUT_MS,
): String {
    require(timeoutMs > 0) { "Outbound probe timeout is invalid" }
    require(testUrl.isHttpUrlWithoutCredentials()) { "Outbound probe target must be an absolute HTTP(S) URL without credentials" }
    val settingsJson = """{"url":${JSONObject.quote(testUrl)},"timeout_ms":$timeoutMs}"""
    val commandJson = """{"SetRuntimeProbeSettings":{"settings":$settingsJson}}"""
    return wrapObservedCommand(scope, commandJson)
}

internal fun canProbeOutbound(itemCapability: OutboundProbeAvailability, operationCapability: OutboundProbeOperationCapability): Boolean =
    itemCapability == OutboundProbeAvailability.Supported &&
        operationCapability == OutboundProbeOperationCapability.Supported

internal fun evaluateOutboundCommandOutcome(
    outcome: AndroidProductCommandOutcome,
    observedScope: CoreScopeObservation,
): OutboundCommandHandling = when (outcome) {
    is AndroidProductCommandOutcome.Unavailable -> {
        if (outcome.scope != null && !sameScope(observedScope, outcome.scope)) {
            OutboundCommandHandling.StaleScope
        } else {
            OutboundCommandHandling.OwnerFailure(outcome.failure)
        }
    }

    is AndroidProductCommandOutcome.Terminal -> {
        if (outcome.scope == null || !sameScope(observedScope, outcome.scope)) {
            OutboundCommandHandling.StaleScope
        } else {
            when (val result = outcome.result) {
                is ProductCommandResult.Produced -> {
                    if (result.outputVariant == "OutboundInventory") {
                        OutboundCommandHandling.Success
                    } else {
                        OutboundCommandHandling.UnexpectedResult
                    }
                }

                is ProductCommandResult.Rejected -> OutboundCommandHandling.OwnerFailure(result.failure)

                is ProductCommandResult.Accepted -> OutboundCommandHandling.UnexpectedResult

                is ProductCommandResult.Completed -> OutboundCommandHandling.UnexpectedResult
            }
        }
    }
}

internal fun evaluateSelectOutboundOutcome(
    outcome: AndroidProductCommandOutcome,
    observedSnapshot: OutboundInventorySnapshot,
    collection: OutboundSelectionCollection,
    outboundId: OutboundIdObservation,
): OutboundCommandHandling {
    val scopeHandling = evaluateOutboundCommandOutcome(outcome, observedSnapshot.scope)
    if (scopeHandling != OutboundCommandHandling.Success) return scopeHandling
    val result = (outcome as? AndroidProductCommandOutcome.Terminal)?.result
    val produced = result as? ProductCommandResult.Produced
        ?: return OutboundCommandHandling.UnexpectedResult
    val readback = try {
        parseOutboundInventoryReadback(produced.outputJson)
    } catch (expected: Exception) {
        return OutboundCommandHandling.UnexpectedResult
    }
    if (!sameScope(observedSnapshot.scope, readback.scope)) return OutboundCommandHandling.StaleScope
    val expectedRevision = observedSnapshot.revision.toULongOrNull()
        ?: return OutboundCommandHandling.UnexpectedResult
    val readbackRevision = readback.revision.toULongOrNull()
        ?: return OutboundCommandHandling.UnexpectedResult
    if (readbackRevision < expectedRevision) {
        return OutboundCommandHandling.UnexpectedResult
    }
    val ready = readback.inventory as? OutboundInventoryState.Ready
        ?: return OutboundCommandHandling.UnexpectedResult
    val readbackCollection = ready.collections.singleOrNull { it.id == collection.id }
        ?: return OutboundCommandHandling.UnexpectedResult
    return if (readbackCollection.memberIds.contains(outboundId) &&
        (readbackCollection.selection as? com.musicfrog.infiltrator.ui.common.OutboundSelectionState.Ready)
            ?.selected == outboundId
    ) {
        OutboundCommandHandling.SelectionApplied(readback)
    } else {
        OutboundCommandHandling.UnexpectedResult
    }
}

internal fun evaluateOutboundUiPreferenceOutcome(
    outcome: AndroidProductCommandOutcome,
    observedScope: CoreScopeObservation,
    preferences: OutboundUiPreferencesSnapshot,
    intent: OutboundUiPreferenceIntent,
): OutboundCommandHandling = when (outcome) {
    is AndroidProductCommandOutcome.Unavailable -> {
        if (outcome.scope != null && !sameScope(observedScope, outcome.scope)) {
            OutboundCommandHandling.StaleScope
        } else {
            OutboundCommandHandling.OwnerFailure(outcome.failure)
        }
    }

    is AndroidProductCommandOutcome.Terminal -> {
        if (outcome.scope == null || !sameScope(observedScope, outcome.scope)) {
            OutboundCommandHandling.StaleScope
        } else {
            when (val result = outcome.result) {
                is ProductCommandResult.Rejected -> OutboundCommandHandling.OwnerFailure(result.failure)

                is ProductCommandResult.Produced -> {
                    if (result.outputVariant != "OutboundUiPreferencesApplied") {
                        OutboundCommandHandling.UnexpectedResult
                    } else {
                        val readback = parseOutboundUiPreferencesReadback(result.outputJson)
                        if (!sameScope(observedScope, readback.scope) ||
                            !preferenceReadbackConfirms(readback, preferences, intent)
                        ) {
                            OutboundCommandHandling.UnexpectedResult
                        } else {
                            OutboundCommandHandling.PreferencesApplied(readback)
                        }
                    }
                }

                is ProductCommandResult.Accepted -> OutboundCommandHandling.UnexpectedResult

                is ProductCommandResult.Completed -> OutboundCommandHandling.UnexpectedResult
            }
        }
    }
}

internal fun evaluateProbeOutboundOutcome(
    outcome: AndroidProductCommandOutcome,
    observedInventory: OutboundInventorySnapshot,
    outboundId: OutboundIdObservation,
): OutboundCommandHandling = when (outcome) {
    is AndroidProductCommandOutcome.Unavailable -> {
        if (outcome.scope != null && !sameScope(observedInventory.scope, outcome.scope)) {
            OutboundCommandHandling.StaleScope
        } else {
            OutboundCommandHandling.OwnerFailure(outcome.failure)
        }
    }

    is AndroidProductCommandOutcome.Terminal -> {
        if (outcome.scope == null || !sameScope(observedInventory.scope, outcome.scope)) {
            OutboundCommandHandling.StaleScope
        } else {
            when (val result = outcome.result) {
                is ProductCommandResult.Rejected -> OutboundCommandHandling.OwnerFailure(result.failure)

                is ProductCommandResult.Produced -> {
                    if (result.outputVariant != "OutboundProbe") {
                        OutboundCommandHandling.UnexpectedResult
                    } else {
                        val readback = parseOutboundProbeReadback(result.outputJson)
                        if (!sameScope(observedInventory.scope, readback.scope) ||
                            readback.outboundId != outboundId ||
                            readback.observedInventoryRevision != observedInventory.revision ||
                            readback.state == OutboundProbeState.Unknown ||
                            readback.state == OutboundProbeState.Probing
                        ) {
                            OutboundCommandHandling.UnexpectedResult
                        } else {
                            OutboundCommandHandling.ProbeCompleted(readback)
                        }
                    }
                }

                is ProductCommandResult.Accepted -> OutboundCommandHandling.UnexpectedResult

                is ProductCommandResult.Completed -> OutboundCommandHandling.UnexpectedResult
            }
        }
    }
}

private fun parseOutboundUiPreferencesReadback(outputJson: String): OutboundUiPreferencesSnapshot {
    val output = JSONObject(outputJson)
    require(output.singleVariant() == "OutboundUiPreferencesApplied") {
        "Outbound preference readback has the wrong output variant"
    }
    val rawData = rawJsonValueAtPath(outputJson, "OutboundUiPreferencesApplied")
        ?: throw IllegalArgumentException("Outbound preference readback is malformed")
    return parseOutboundUiPreferencesSnapshot(output.getJSONObject("OutboundUiPreferencesApplied"), rawData)
}

private fun parseOutboundInventoryReadback(outputJson: String): OutboundInventorySnapshot {
    val output = JSONObject(outputJson)
    require(output.singleVariant() == "OutboundInventory") {
        "Outbound selection readback has the wrong output variant"
    }
    val rawData = rawJsonValueAtPath(outputJson, "OutboundInventory")
        ?: throw IllegalArgumentException("Outbound selection readback is malformed")
    return parseOutboundInventorySnapshot(output.getJSONObject("OutboundInventory"), rawData)
}

private fun parseOutboundProbeReadback(outputJson: String): OutboundProbeSnapshot {
    val output = JSONObject(outputJson)
    require(output.singleVariant() == "OutboundProbe") { "Outbound probe readback has the wrong output variant" }
    val data = output.getJSONObject("OutboundProbe")
    val rawData = rawJsonValueAtPath(outputJson, "OutboundProbe")
        ?: throw IllegalArgumentException("Outbound probe readback is malformed")
    val scopeJson = data.getJSONObject("scope")
    val rawScopeJson = rawJsonValueAtPath(rawData, "scope")
        ?: throw IllegalArgumentException("Outbound probe readback has no CoreScope")
    return OutboundProbeSnapshot(
        scope = parseCoreScopeObservation(scopeJson, rawScopeJson),
        observedInventoryRevision = parseUnsignedDecimal(
            rawJsonValueAtPath(rawData, "observed_inventory_revision")
                ?: throw IllegalArgumentException("Outbound probe readback has no inventory revision"),
            "observed_inventory_revision",
        ),
        detailRevision = parseUnsignedDecimal(
            rawJsonValueAtPath(rawData, "detail_revision")
                ?: throw IllegalArgumentException("Outbound probe readback has no details revision"),
            "detail_revision",
        ),
        outboundId = parseOutboundId(
            data.getJSONObject("outbound_id"),
            scopeJson.getJSONObject("identity").getString("engine"),
        ),
        state = parseProbeState(data.get("state"), rawData),
    )
}

private fun parseProbeState(value: Any, rawDataJson: String): OutboundProbeState = when (value) {
    is String -> when (value) {
        "unknown" -> OutboundProbeState.Unknown
        "probing" -> OutboundProbeState.Probing
        "timed_out" -> OutboundProbeState.TimedOut
        "unsupported" -> OutboundProbeState.Unsupported
        else -> throw IllegalArgumentException("Unknown outbound probe state: $value")
    }

    is JSONObject -> when (val variant = value.singleVariant()) {
        "measured" -> {
            val rawDelay = rawJsonValueAtPath(rawDataJson, "state", variant, "delay_ms")
                ?: throw IllegalArgumentException("Outbound probe delay is missing")
            val delay = parseUnsignedDecimal(rawDelay, "delay_ms").toULong()
            require(delay <= 4_294_967_295uL) { "Outbound probe delay is outside the valid range" }
            OutboundProbeState.Measured(delay.toLong())
        }

        "failed" -> OutboundProbeState.Failed(
            parseOwnerFailure(value.getJSONObject(variant).getJSONObject("failure")),
        )

        else -> throw IllegalArgumentException("Unknown outbound probe state: $variant")
    }

    else -> throw IllegalArgumentException("Outbound probe state is malformed")
}

private fun preferenceReadbackConfirms(
    readback: OutboundUiPreferencesSnapshot,
    observed: OutboundUiPreferencesSnapshot,
    intent: OutboundUiPreferenceIntent,
): Boolean {
    val beforeRevision = observed.revision.toULongOrNull() ?: return false
    val afterRevision = readback.revision.toULongOrNull() ?: return false
    if (afterRevision <= beforeRevision) return false
    return when (intent) {
        is OutboundUiPreferenceIntent.SetSearchQuery -> readback.searchQuery == intent.query

        is OutboundUiPreferenceIntent.SetFavorite ->
            (intent.outboundId in readback.favoriteOutboundIds) == intent.pinned

        is OutboundUiPreferenceIntent.SetSortOrder -> readback.sortOrder == intent.order

        is OutboundUiPreferenceIntent.SetHealthFilter -> readback.healthFilter == intent.filter

        is OutboundUiPreferenceIntent.SetLayout -> readback.layout == intent.layout

        is OutboundUiPreferenceIntent.SetGroupExpansion ->
            (intent.groupId !in readback.collapsedGroupIds) == intent.expanded

        is OutboundUiPreferenceIntent.ReorderGroups -> readback.groupOrder == intent.groupIds

        OutboundUiPreferenceIntent.ResetGroupOrder -> readback.groupOrder.isEmpty()
    }
}

private fun validatePreferenceIntent(inventory: OutboundInventorySnapshot, intent: OutboundUiPreferenceIntent) {
    val ready = inventory.inventory as? OutboundInventoryState.Ready
        ?: throw IllegalArgumentException("Outbound preferences cannot be changed without ready inventory")
    val entryIds = ready.entries.map { it.id }.toSet()
    val collectionIds = ready.collections.map { it.id }.toSet()
    val engine = inventory.scope.identity.engineId
    when (intent) {
        is OutboundUiPreferenceIntent.SetFavorite -> require(
            intent.outboundId.engineId == engine && intent.outboundId in entryIds,
        ) { "Favorite outbound was not part of the observed inventory" }

        is OutboundUiPreferenceIntent.SetGroupExpansion -> require(
            intent.groupId.engineId == engine && intent.groupId in collectionIds,
        ) { "Outbound group was not part of the observed inventory" }

        is OutboundUiPreferenceIntent.ReorderGroups -> {
            require(intent.groupIds.distinct().size == intent.groupIds.size) {
                "Outbound group order contains duplicate IDs"
            }
            require(intent.groupIds.all { it.engineId == engine && it in collectionIds }) {
                "Outbound group order contains an unobserved group"
            }
        }

        is OutboundUiPreferenceIntent.SetSearchQuery,
        is OutboundUiPreferenceIntent.SetSortOrder,
        is OutboundUiPreferenceIntent.SetHealthFilter,
        is OutboundUiPreferenceIntent.SetLayout,
        OutboundUiPreferenceIntent.ResetGroupOrder,
        -> Unit
    }
}

private fun requireRevision(value: String, label: String) {
    require(value.matches(Regex("0|[1-9][0-9]*")) && value.toULongOrNull() != null) {
        "$label is invalid"
    }
}

private fun sameScope(first: CoreScopeObservation, second: CoreScopeObservation): Boolean = first.identity == second.identity &&
    first.generation == second.generation &&
    first.sessionToken == second.sessionToken

private fun String.isHttpUrlWithoutCredentials(): Boolean = try {
    val uri = java.net.URI(this)
    (uri.scheme.equals("http", true) || uri.scheme.equals("https", true)) &&
        !uri.host.isNullOrBlank() && uri.userInfo == null
} catch (_: Exception) {
    false
}

private fun OutboundSortOrder.toWire(): String = when (this) {
    OutboundSortOrder.LatencyAscending -> "latency_ascending"
    OutboundSortOrder.LatencyDescending -> "latency_descending"
    OutboundSortOrder.NameAscending -> "name_ascending"
    OutboundSortOrder.NameDescending -> "name_descending"
}

private fun OutboundHealthFilter.toWire(): String = when (this) {
    OutboundHealthFilter.All -> "all"
    OutboundHealthFilter.HealthyOnly -> "healthy_only"
}

private fun OutboundLayout.toWire(): String = when (this) {
    OutboundLayout.CompactList -> "compact_list"
    OutboundLayout.Grid -> "grid"
}

private fun outboundIdJson(id: OutboundIdObservation): String = """
    {"engine_id":${JSONObject.quote(id.engineId)},"value":${JSONObject.quote(id.value)}}
""".trimIndent()

internal const val DEFAULT_OUTBOUND_PROBE_URL = "https://www.gstatic.com/generate_204"
internal const val DEFAULT_OUTBOUND_PROBE_TIMEOUT_MS = 5_000L
