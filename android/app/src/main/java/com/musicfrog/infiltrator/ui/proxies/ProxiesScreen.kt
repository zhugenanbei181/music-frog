package com.musicfrog.infiltrator.ui.proxies

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.animateContentSize
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ExpandLess
import androidx.compose.material.icons.filled.ExpandMore
import androidx.compose.material.icons.outlined.Check
import androidx.compose.material.icons.outlined.Tune
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ElevatedCard
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import com.musicfrog.infiltrator.R
import com.musicfrog.infiltrator.ui.common.OutboundDetailsPageState
import com.musicfrog.infiltrator.ui.common.OutboundDetailsSnapshot
import com.musicfrog.infiltrator.ui.common.OutboundDetailsState
import com.musicfrog.infiltrator.ui.common.OutboundDetailObservation
import com.musicfrog.infiltrator.ui.common.OutboundEndpointObservation
import com.musicfrog.infiltrator.ui.common.OutboundEntryObservation
import com.musicfrog.infiltrator.ui.common.OutboundHealthFilter
import com.musicfrog.infiltrator.ui.common.OutboundHealthObservation
import com.musicfrog.infiltrator.ui.common.OutboundIdObservation
import com.musicfrog.infiltrator.ui.common.OutboundInventoryPageState
import com.musicfrog.infiltrator.ui.common.OutboundInventorySnapshot
import com.musicfrog.infiltrator.ui.common.OutboundInventoryState
import com.musicfrog.infiltrator.ui.common.OutboundObservation
import com.musicfrog.infiltrator.ui.common.OutboundProbeAvailability
import com.musicfrog.infiltrator.ui.common.OutboundProbeOperationCapability
import com.musicfrog.infiltrator.ui.common.OutboundProbeSnapshot
import com.musicfrog.infiltrator.ui.common.OutboundProbeState
import com.musicfrog.infiltrator.ui.common.OutboundProtocolObservation
import com.musicfrog.infiltrator.ui.common.OutboundSelectionAvailability
import com.musicfrog.infiltrator.ui.common.OutboundSelectionCollection
import com.musicfrog.infiltrator.ui.common.OutboundSelectionState
import com.musicfrog.infiltrator.ui.common.OutboundSortOrder
import com.musicfrog.infiltrator.ui.common.OutboundUiPreferencesPageState
import com.musicfrog.infiltrator.ui.common.OutboundUiPreferencesSnapshot
import com.musicfrog.infiltrator.ui.common.OwnerProtocolStudioSnapshot
import com.musicfrog.infiltrator.ui.common.ProtocolStudioPageState
import com.musicfrog.infiltrator.ui.common.EmptyStateText
import com.musicfrog.infiltrator.ui.common.EmptyStateTextKey
import com.musicfrog.infiltrator.ui.common.ErrorDialog
import com.musicfrog.infiltrator.ui.common.LocalizedFailureText
import com.musicfrog.infiltrator.ui.common.OwnerUiStatusText
import com.musicfrog.infiltrator.ui.common.OwnerUiStatusTextKey
import com.musicfrog.infiltrator.ui.common.asDisplayString
import com.musicfrog.infiltrator.ui.common.toLocalizedFailure

@Composable
internal fun ProxiesScreen(viewModel: ProxiesViewModel = viewModel()) {
    val inventory by viewModel.inventory.collectAsStateWithLifecycle()
    val details by viewModel.details.collectAsStateWithLifecycle()
    val preferences by viewModel.preferences.collectAsStateWithLifecycle()
    val protocolStudio by viewModel.protocolStudio.collectAsStateWithLifecycle()
    val probeCapability by viewModel.probeCapability.collectAsStateWithLifecycle()
    val probeReadback by viewModel.probeReadback.collectAsStateWithLifecycle()
    val commandInProgress by viewModel.commandInProgress.collectAsStateWithLifecycle()
    val error by viewModel.error.collectAsStateWithLifecycle()
    val commandFailure by viewModel.commandFailure.collectAsStateWithLifecycle()
    val visibleError = commandFailure?.toLocalizedFailure() ?: error

    Box(modifier = Modifier.fillMaxSize()) {
        when (val page = inventory) {
            OutboundInventoryPageState.Loading -> {
                CircularProgressIndicator(modifier = Modifier.align(Alignment.Center))
            }
            is OutboundInventoryPageState.Observed -> InventoryContent(
                snapshot = page.snapshot,
                details = (details as? OutboundDetailsPageState.Observed)?.snapshot,
                detailsState = details,
                preferences = (preferences as? OutboundUiPreferencesPageState.Observed)?.snapshot,
                preferencesState = preferences,
                protocolStudio = protocolStudio,
                probeCapability = probeCapability,
                probeReadback = probeReadback,
                commandInProgress = commandInProgress,
                onSelect = { collection, outbound ->
                    viewModel.selectOutbound(page.snapshot, collection, outbound.id)
                },
                onPreference = { currentPreferences, intent ->
                    viewModel.applyPreference(page.snapshot, currentPreferences, intent)
                },
                onProbe = { currentDetails, outboundId ->
                    viewModel.probeOutbound(page.snapshot, currentDetails, probeCapability, outboundId)
                },
            )
            is OutboundInventoryPageState.Unavailable -> FailureStatus(
                page.failure.toLocalizedFailure(),
                Modifier.align(Alignment.Center),
            )
            is OutboundInventoryPageState.Failed -> FailureStatus(
                page.failure.toLocalizedFailure(),
                Modifier.align(Alignment.Center),
            )
        }

        if (visibleError != null) {
            ErrorDialog(
                failure = visibleError,
                onDismiss = { viewModel.clearError() },
            )
        }
    }
}

@Composable
private fun InventoryContent(
    snapshot: OutboundInventorySnapshot,
    details: OutboundDetailsSnapshot?,
    detailsState: OutboundDetailsPageState,
    preferences: OutboundUiPreferencesSnapshot?,
    preferencesState: OutboundUiPreferencesPageState,
    protocolStudio: ProtocolStudioPageState,
    probeCapability: OutboundProbeOperationCapability,
    probeReadback: OutboundProbeSnapshot?,
    commandInProgress: Boolean,
    onSelect: (OutboundSelectionCollection, OutboundEntryObservation) -> Unit,
    onPreference: (OutboundUiPreferencesSnapshot, OutboundUiPreferenceIntent) -> Unit,
    onProbe: (OutboundDetailsSnapshot, OutboundIdObservation) -> Unit,
) {
    when (val inventory = snapshot.inventory) {
        OutboundInventoryState.Unknown -> OwnerUiStatusText(
            OwnerUiStatusTextKey.OutboundInventoryUnknown,
            Modifier.fillMaxSize(),
        )
        OutboundInventoryState.Empty -> EmptyStateText(
            EmptyStateTextKey.Outbounds,
            Modifier.fillMaxSize(),
        )
        OutboundInventoryState.Unsupported -> OwnerUiStatusText(
            OwnerUiStatusTextKey.OutboundInventoryUnsupported,
            Modifier.fillMaxSize(),
        )
        is OutboundInventoryState.Failed -> FailureStatus(
            inventory.failure.toLocalizedFailure(),
            Modifier.fillMaxSize(),
        )
        is OutboundInventoryState.Ready -> {
            val detailsById = (details?.state as? OutboundDetailsState.Ready)
                ?.details?.associateBy { it.id }.orEmpty()
            val entries = filterAndSortEntries(inventory.entries, detailsById, preferences)
            val collections = orderCollections(inventory.collections, preferences)
            LazyColumn(
                modifier = Modifier.fillMaxSize(),
                contentPadding = PaddingValues(16.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                item(key = "typed-toolbar") {
                    OutboundPreferencesControls(
                        preferences = preferences,
                        commandInProgress = commandInProgress,
                        onPreference = onPreference,
                    )
                }
                item(key = "owner-page-status") {
                    OwnerPageStatus(
                        detailsState = detailsState,
                        preferencesState = preferencesState,
                        protocolStudio = protocolStudio,
                    )
                }
                items(collections, key = { "collection:${it.id.value}" }) { collection ->
                    OutboundCollectionCard(
                        collection = collection,
                        entriesById = inventory.entries.associateBy { it.id },
                        preferences = preferences,
                        commandInProgress = commandInProgress,
                        onSelect = { entry -> onSelect(collection, entry) },
                        onGroupExpansion = { expanded ->
                            if (preferences != null) {
                                onPreference(
                                    preferences,
                                    OutboundUiPreferenceIntent.SetGroupExpansion(collection.id, expanded),
                                )
                            }
                        },
                    )
                }
                item(key = "outbound-details-heading") {
                    Text(
                        "Outbound details",
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = FontWeight.SemiBold,
                    )
                }
                items(entries, key = { "outbound:${it.id.engineId}:${it.id.value}" }) { entry ->
                    OutboundDetailCard(
                        entry = entry,
                        detail = detailsById[entry.id],
                        isFavorite = preferences?.favoriteOutboundIds?.contains(entry.id) == true,
                        preferencesAvailable = preferences != null,
                        probeCapability = probeCapability,
                        probeReadback = probeReadback?.takeIf {
                            it.outboundId == entry.id &&
                                it.scope.identity == snapshot.scope.identity &&
                                it.scope.generation == snapshot.scope.generation &&
                                it.scope.sessionToken == snapshot.scope.sessionToken &&
                                it.observedInventoryRevision == snapshot.revision
                        },
                        commandInProgress = commandInProgress,
                        onToggleFavorite = {
                            if (preferences != null) {
                                onPreference(
                                    preferences,
                                    OutboundUiPreferenceIntent.SetFavorite(
                                        entry.id,
                                        pinned = entry.id !in preferences.favoriteOutboundIds,
                                    ),
                                )
                            }
                        },
                        onProbe = {
                            if (details != null) onProbe(details, entry.id)
                        },
                    )
                }
            }
        }
    }
}

@Composable
private fun OutboundPreferencesControls(
    preferences: OutboundUiPreferencesSnapshot?,
    commandInProgress: Boolean,
    onPreference: (OutboundUiPreferencesSnapshot, OutboundUiPreferenceIntent) -> Unit,
) {
    ElevatedCard(modifier = Modifier.fillMaxWidth()) {
        Column(modifier = Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            if (preferences == null) {
                OwnerUiStatusText(OwnerUiStatusTextKey.OutboundPreferencesUnavailable)
            } else {
                Text("Preferences revision ${preferences.revision}", style = MaterialTheme.typography.labelSmall)
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    OutlinedButton(
                        enabled = !commandInProgress,
                        onClick = {
                            onPreference(
                                preferences,
                                OutboundUiPreferenceIntent.SetSortOrder(preferences.sortOrder.next()),
                            )
                        },
                    ) {
                        Text("Sort: ${preferences.sortOrder.label()}")
                    }
                    OutlinedButton(
                        enabled = !commandInProgress,
                        onClick = {
                            onPreference(
                                preferences,
                                OutboundUiPreferenceIntent.SetHealthFilter(
                                    if (preferences.healthFilter == OutboundHealthFilter.All) {
                                        OutboundHealthFilter.HealthyOnly
                                    } else {
                                        OutboundHealthFilter.All
                                    },
                                ),
                            )
                        },
                    ) {
                        Text(if (preferences.healthFilter == OutboundHealthFilter.All) "All health" else "Healthy only")
                    }
                }
            }
        }
    }
}

@Composable
private fun OwnerPageStatus(
    detailsState: OutboundDetailsPageState,
    preferencesState: OutboundUiPreferencesPageState,
    protocolStudio: ProtocolStudioPageState,
) {
    Column(verticalArrangement = Arrangement.spacedBy(3.dp)) {
        when (detailsState) {
            OutboundDetailsPageState.Loading -> OwnerUiStatusText(OwnerUiStatusTextKey.OutboundDetailsLoading)
            is OutboundDetailsPageState.Unavailable -> LocalizedFailureText(detailsState.failure.toLocalizedFailure())
            is OutboundDetailsPageState.Failed -> LocalizedFailureText(detailsState.failure.toLocalizedFailure())
            is OutboundDetailsPageState.Observed -> when (detailsState.snapshot.state) {
                OutboundDetailsState.Unknown -> OwnerUiStatusText(OwnerUiStatusTextKey.OutboundDetailsUnknown)
                OutboundDetailsState.Empty -> OwnerUiStatusText(OwnerUiStatusTextKey.OutboundDetailsEmpty)
                OutboundDetailsState.Unsupported -> OwnerUiStatusText(OwnerUiStatusTextKey.OutboundDetailsUnsupported)
                is OutboundDetailsState.Failed -> LocalizedFailureText(detailsState.snapshot.state.failure.toLocalizedFailure())
                is OutboundDetailsState.Ready -> Unit
            }
        }
        when (preferencesState) {
            OutboundUiPreferencesPageState.Loading -> OwnerUiStatusText(OwnerUiStatusTextKey.OutboundPreferencesLoading)
            is OutboundUiPreferencesPageState.Unavailable -> LocalizedFailureText(preferencesState.failure.toLocalizedFailure())
            is OutboundUiPreferencesPageState.Failed -> LocalizedFailureText(preferencesState.failure.toLocalizedFailure())
            is OutboundUiPreferencesPageState.Observed -> Unit
        }
        when (protocolStudio) {
            ProtocolStudioPageState.Loading -> Unit
            is ProtocolStudioPageState.Unavailable -> Unit
            is ProtocolStudioPageState.Failed -> LocalizedFailureText(protocolStudio.failure.toLocalizedFailure())
            is ProtocolStudioPageState.Observed -> ProtocolStudioSummary(protocolStudio.snapshot)
        }
    }
}

@Composable
private fun ProtocolStudioSummary(snapshot: OwnerProtocolStudioSnapshot) {
    val draft = snapshot.draft ?: return
    val report = snapshot.report
    Text(
        text = buildString {
            append("Protocol studio: ")
            append(draft.name.ifBlank { draft.nodeType })
            append(" · ")
            append(report?.family ?: draft.nodeType)
            report?.cipherChip?.let { append(" · "); append(it) }
            report?.flowChip?.let { append(" · "); append(it) }
            if (report?.issues?.isNotEmpty() == true) append(" · ${report.issues.size} issue(s)")
            snapshot.lastError?.let { append(" · "); append(it) }
        },
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}

@Composable
private fun OutboundCollectionCard(
    collection: OutboundSelectionCollection,
    entriesById: Map<OutboundIdObservation, OutboundEntryObservation>,
    preferences: OutboundUiPreferencesSnapshot?,
    commandInProgress: Boolean,
    onSelect: (OutboundEntryObservation) -> Unit,
    onGroupExpansion: (Boolean) -> Unit,
) {
    var localExpanded by remember(collection.id) { mutableStateOf(false) }
    val expanded = preferences?.let { collection.id !in it.collapsedGroupIds } ?: localExpanded
    val selectedId = (collection.selection as? OutboundSelectionState.Ready)?.selected
    val selectedLabel = selectedId?.let { entriesById[it]?.displayName ?: it.value }
    val canSelect = collection.availability == OutboundSelectionAvailability.Selectable

    ElevatedCard(
        modifier = Modifier
            .fillMaxWidth()
            .animateContentSize(),
        colors = androidx.compose.material3.CardDefaults.elevatedCardColors(
            containerColor = MaterialTheme.colorScheme.surfaceContainerLow,
        ),
    ) {
        Column {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable(enabled = !commandInProgress) {
                        if (preferences == null) localExpanded = !localExpanded
                        else onGroupExpansion(!expanded)
                    }
                    .padding(16.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Icon(Icons.Outlined.Tune, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
                Spacer(modifier = Modifier.width(16.dp))
                Column(modifier = Modifier.weight(1f)) {
                    Text(collection.id.value, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
                    Text(
                        collectionStatusLabel(collection, selectedLabel),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                Icon(
                    if (expanded) Icons.Default.ExpandLess else Icons.Default.ExpandMore,
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            AnimatedVisibility(visible = expanded) {
                Column(modifier = Modifier.padding(bottom = 8.dp)) {
                    collection.memberIds.forEach { id ->
                        val entry = entriesById[id] ?: return@forEach
                        val isSelected = id == selectedId
                        Surface(
                            modifier = Modifier
                                .fillMaxWidth()
                                .clickable(enabled = canSelect && !isSelected && !commandInProgress) { onSelect(entry) },
                            color = if (isSelected) {
                                MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.5f)
                            } else {
                                MaterialTheme.colorScheme.surface
                            },
                        ) {
                            Row(
                                modifier = Modifier.padding(horizontal = 48.dp, vertical = 12.dp),
                                verticalAlignment = Alignment.CenterVertically,
                            ) {
                                Text(
                                    text = entry.displayName ?: entry.id.value,
                                    modifier = Modifier.weight(1f),
                                    style = MaterialTheme.typography.bodyMedium,
                                    color = if (isSelected) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurface,
                                )
                                if (isSelected) {
                                    Icon(Icons.Outlined.Check, contentDescription = "Selected", tint = MaterialTheme.colorScheme.primary)
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun OutboundDetailCard(
    entry: OutboundEntryObservation,
    detail: OutboundDetailObservation?,
    isFavorite: Boolean,
    preferencesAvailable: Boolean,
    probeCapability: OutboundProbeOperationCapability,
    probeReadback: OutboundProbeSnapshot?,
    commandInProgress: Boolean,
    onToggleFavorite: () -> Unit,
    onProbe: () -> Unit,
) {
    ElevatedCard(modifier = Modifier.fillMaxWidth()) {
        Column(modifier = Modifier.padding(14.dp), verticalArrangement = Arrangement.spacedBy(5.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Column(modifier = Modifier.weight(1f)) {
                    Text(entry.displayName ?: entry.id.value, style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.SemiBold)
                    Text(entry.id.value, style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
                TextButton(enabled = preferencesAvailable && !commandInProgress, onClick = onToggleFavorite) {
                    Text(if (isFavorite) "Unpin" else "Pin")
                }
            }
            if (detail == null) {
                OwnerUiStatusText(OwnerUiStatusTextKey.OutboundDetailsNotObserved)
            } else {
                DetailFact("Protocol", protocolLabel(detail))
                DetailFact("Cipher", observationLabel(detail.cipher, detail.cipherDisplayName))
                DetailFact("Endpoint", observationLabel(detail.endpoint) { endpoint -> endpoint.label() })
                DetailFact("Health", detail.health.label())
                DetailFact("Latency", observationLabel(detail.currentLatencyMs) { "$it ms" })
                DetailFact("Latency history", observationLabel(detail.latencyHistory) { history ->
                    history.takeLast(4).joinToString(" · ") { "${it.delayMs} ms (${it.observedAt})" }
                        .ifBlank { stringResource(R.string.empty_latency_samples) }
                })
                DetailFact("RTT", observationLabel(detail.rtt) { stats ->
                    "min ${stats.minMs ?: "—"} · avg ${stats.avgMs ?: "—"} · max ${stats.maxMs ?: "—"} ms (${stats.validCount} samples)"
                })
                DetailFact("Historical egress", observationLabel(detail.historicalEgress) { samples ->
                    samples.joinToString(" · ") { sample -> listOfNotNull(sample.ip, sample.country, sample.observedAtEpochMs).joinToString(" / ") }
                        .ifBlank { stringResource(R.string.empty_egress_samples) }
                })
                DetailFact("UDP", detail.udp.name.lowercase())
                probeAvailabilityLabel(detail.probeAvailability, probeCapability, probeReadback)?.let {
                    DetailFact("Probe", it)
                }
                if (canProbeOutbound(detail.probeAvailability, probeCapability)) {
                    Button(enabled = !commandInProgress, onClick = onProbe) { Text("Probe") }
                }
            }
        }
    }
}

@Composable
private fun DetailFact(label: String, value: String) {
    Row(modifier = Modifier.fillMaxWidth()) {
        Text(label, modifier = Modifier.weight(0.32f), style = MaterialTheme.typography.labelMedium)
        Text(value, modifier = Modifier.weight(0.68f), style = MaterialTheme.typography.bodySmall)
    }
}

private fun protocolLabel(detail: OutboundDetailObservation): String = when (val protocol = detail.protocol) {
    OutboundProtocolObservation.Unknown -> "Unknown"
    OutboundProtocolObservation.Unsupported -> "Unsupported"
    is OutboundProtocolObservation.Reported -> detail.protocolDisplayName ?: protocol.id.value
}

@Composable
private fun OutboundEndpointObservation.label(): String {
    val portText = when (val observation = port) {
        OutboundObservation.Unknown -> stringResource(R.string.owner_value_unknown)
        OutboundObservation.Unsupported -> stringResource(R.string.owner_value_unsupported)
        is OutboundObservation.Observed -> "${observation.value}"
    }
    return "$host:$portText"
}

@Composable
private fun OutboundHealthObservation.label(): String = when (this) {
    OutboundHealthObservation.Unknown -> stringResource(R.string.owner_value_unknown)
    OutboundHealthObservation.Healthy -> stringResource(R.string.owner_health_healthy)
    OutboundHealthObservation.Unhealthy -> stringResource(R.string.owner_health_unhealthy)
    OutboundHealthObservation.Unsupported -> stringResource(R.string.owner_value_unsupported)
}

@Composable
private fun probeAvailabilityLabel(
    item: OutboundProbeAvailability,
    operation: OutboundProbeOperationCapability,
    readback: OutboundProbeSnapshot?,
): String? {
    if (readback != null) return readback.state.label()
    return when (item) {
        OutboundProbeAvailability.Unknown -> stringResource(R.string.owner_probe_availability_unknown)
        OutboundProbeAvailability.Unsupported -> stringResource(R.string.owner_probe_unsupported)
        OutboundProbeAvailability.Supported -> when (operation) {
            OutboundProbeOperationCapability.Unknown -> stringResource(R.string.owner_probe_capability_unknown)
            is OutboundProbeOperationCapability.Unavailable -> operation.failure.asDisplayString()
            is OutboundProbeOperationCapability.Unsupported -> operation.failure.asDisplayString()
            OutboundProbeOperationCapability.Supported -> stringResource(R.string.owner_probe_available)
        }
    }
}

@Composable
private fun OutboundProbeState.label(): String = when (this) {
    OutboundProbeState.Unknown -> stringResource(R.string.owner_value_unknown)
    OutboundProbeState.Probing -> stringResource(R.string.owner_probe_working)
    is OutboundProbeState.Measured -> "${delayMs} ms"
    OutboundProbeState.TimedOut -> stringResource(R.string.owner_probe_timed_out)
    OutboundProbeState.Unsupported -> stringResource(R.string.owner_value_unsupported)
    is OutboundProbeState.Failed -> failure.toLocalizedFailure().asDisplayString()
}

@Composable
private fun <T> observationLabel(
    observation: OutboundObservation<T>,
    format: @Composable (T) -> String,
): String =
    when (observation) {
        OutboundObservation.Unknown -> stringResource(R.string.owner_value_unknown)
        OutboundObservation.Unsupported -> stringResource(R.string.owner_value_unsupported)
        is OutboundObservation.Observed -> format(observation.value)
    }

@Composable
private fun observationLabel(
    observation: OutboundObservation<OutboundIdObservation>,
    displayName: String?,
): String = observationLabel(observation) { displayName ?: it.value }

private fun filterAndSortEntries(
    entries: List<OutboundEntryObservation>,
    details: Map<OutboundIdObservation, OutboundDetailObservation>,
    preferences: OutboundUiPreferencesSnapshot?,
): List<OutboundEntryObservation> {
    val query = preferences?.searchQuery.orEmpty()
    val filtered = entries.filter { entry ->
        val matchesQuery = query.isBlank() ||
            listOfNotNull(entry.displayName, entry.id.value)
                .any { it.contains(query, ignoreCase = true) }
        val matchesHealth = preferences?.healthFilter != OutboundHealthFilter.HealthyOnly ||
            details[entry.id]?.health == OutboundHealthObservation.Healthy
        matchesQuery && matchesHealth
    }
    val comparator = when (preferences?.sortOrder) {
        OutboundSortOrder.LatencyAscending -> compareBy<OutboundEntryObservation> {
            (details[it.id]?.currentLatencyMs as? OutboundObservation.Observed)?.value ?: Long.MAX_VALUE
        }.thenBy { it.displayName ?: it.id.value }
        OutboundSortOrder.LatencyDescending -> compareByDescending<OutboundEntryObservation> {
            (details[it.id]?.currentLatencyMs as? OutboundObservation.Observed)?.value ?: Long.MIN_VALUE
        }.thenBy { it.displayName ?: it.id.value }
        OutboundSortOrder.NameAscending -> compareBy { it.displayName ?: it.id.value }
        OutboundSortOrder.NameDescending -> compareByDescending { it.displayName ?: it.id.value }
        null -> null
    }
    return comparator?.let(filtered::sortedWith) ?: filtered
}

private fun orderCollections(
    collections: List<OutboundSelectionCollection>,
    preferences: OutboundUiPreferencesSnapshot?,
): List<OutboundSelectionCollection> {
    val order = preferences?.groupOrder.orEmpty()
    if (order.isEmpty()) return collections
    val rank = order.withIndex().associate { it.value to it.index }
    return collections.sortedWith(compareBy { rank[it.id] ?: Int.MAX_VALUE })
}

private fun OutboundSortOrder.next(): OutboundSortOrder = when (this) {
    OutboundSortOrder.LatencyAscending -> OutboundSortOrder.LatencyDescending
    OutboundSortOrder.LatencyDescending -> OutboundSortOrder.NameAscending
    OutboundSortOrder.NameAscending -> OutboundSortOrder.NameDescending
    OutboundSortOrder.NameDescending -> OutboundSortOrder.LatencyAscending
}

private fun OutboundSortOrder.label(): String = when (this) {
    OutboundSortOrder.LatencyAscending -> "Latency ↑"
    OutboundSortOrder.LatencyDescending -> "Latency ↓"
    OutboundSortOrder.NameAscending -> "Name A–Z"
    OutboundSortOrder.NameDescending -> "Name Z–A"
}

@Composable
private fun collectionStatusLabel(
    collection: OutboundSelectionCollection,
    selectedLabel: String?,
): String {
    val selection = when (val state = collection.selection) {
        OutboundSelectionState.Unknown -> stringResource(R.string.owner_selection_unknown)
        is OutboundSelectionState.Ready -> stringResource(
            R.string.owner_outbound_selected,
            selectedLabel ?: state.selected.value,
        )
        OutboundSelectionState.Empty -> stringResource(R.string.empty_outbound_selection)
        OutboundSelectionState.Unsupported -> stringResource(R.string.owner_selection_unsupported)
        is OutboundSelectionState.Failed -> state.failure.toLocalizedFailure().asDisplayString()
    }
    val availability = when (collection.availability) {
        OutboundSelectionAvailability.Selectable -> null
        OutboundSelectionAvailability.ReadOnly -> stringResource(R.string.owner_selection_read_only)
        OutboundSelectionAvailability.Automatic -> stringResource(R.string.owner_selection_automatic)
        OutboundSelectionAvailability.Unsupported -> stringResource(R.string.owner_selection_unsupported)
        OutboundSelectionAvailability.Unknown -> stringResource(R.string.owner_selection_capability_unknown)
    }
    return listOfNotNull(selection, availability).distinct().joinToString(" • ")
}

@Composable
private fun FailureStatus(failure: com.musicfrog.infiltrator.ui.common.LocalizedFailure, modifier: Modifier = Modifier) {
    Box(modifier = modifier) {
        LocalizedFailureText(
            failure = failure,
            modifier = Modifier.align(Alignment.Center),
        )
    }
}
