package com.musicfrog.infiltrator.ui.common

import com.musicfrog.infiltrator.ui.overview.RoutingIntentOption
import com.musicfrog.infiltrator.ui.overview.buildRoutingIntentCommandJson
import com.musicfrog.infiltrator.VpnOwnerDataPlaneState
import org.junit.Assert.assertFalse
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.json.JSONObject
import org.junit.Test

class OwnerSurfaceSnapshotTest {
    @Test
    fun liveOwnerSnapshotMapsPagesWithoutInventingUnavailableConnectionFields() {
        assertEquals("live", JSONObject(liveSnapshot).optString("origin"))
        val read = parseOwnerSurfaceSnapshot(liveSnapshot)
        assertTrue("Owner snapshot parse failed: $read", read is OwnerSurfaceRead.Available)
        val snapshot = (read as OwnerSurfaceRead.Available).snapshot

        assertEquals(8uL, snapshot.generation)
        assertEquals(12uL, snapshot.revision)
        assertTrue(snapshot.vpn is OwnerVpnObservation.NotApplicable)
        assertEquals(
            RoutingIntentObservation.FollowRules,
            snapshot.overview.currentData()?.routingIntent?.current,
        )
        assertEquals(RoutingIntentStatusObservation.Ready, snapshot.coreRoutingIntent.status)
        assertEquals(snapshot.coreRoutingIntent, snapshot.overview.currentData()?.routingIntent)
        assertEquals("mihomo", snapshot.coreScope?.identity?.engineId)
        assertEquals(128uL, snapshot.overview.currentData()?.uploadBps)
        assertEquals(
            TrafficSnapshot(128uL, 1024uL, 4096uL, 8192uL, 2u),
            snapshot.trafficSnapshot(),
        )
        val connection = snapshot.connections.currentData()?.connections?.single()
        assertEquals("example.com", connection?.host)
        assertEquals(listOf("Auto", "Tokyo"), connection?.chains)
        assertNull(connection?.connectionType)
        assertEquals(OwnerLogStreamState.LIVE, snapshot.logs.currentData()?.stream)
        assertEquals(LogLevel.WARNING, snapshot.logs.currentData()?.entries?.single()?.level)
        assertEquals("2026-10-10T10:00:00Z", snapshot.logs.currentData()?.entries?.single()?.timestamp)
        assertTrue(snapshot.outboundInventory is OutboundInventoryPageState.Unavailable)
    }

    @Test
    fun androidVpnObservationKeepsOwnerSnapshotAndRequiresMatchingCoreReadback() {
        val read = parseOwnerSurfaceSnapshot(liveSnapshot.replace(
            "\"vpn\":\"NotApplicable\"",
            ownerVpnSnapshot,
        ))
        assertTrue("Android owner snapshot was rejected: $read", read is OwnerSurfaceRead.Available)
        val owner = ((read as OwnerSurfaceRead.Available).snapshot.vpn as OwnerVpnObservation.AndroidOwner)
            .snapshot
        assertEquals("vpn-owner-a", owner.ownerSessionId)
        assertEquals(14L, owner.revision)
        assertEquals(VpnOwnerDataPlaneState.Running, owner.dataPlaneState())

        val staleCoreRevision = parseOwnerSurfaceSnapshot(
            liveSnapshot.replace("\"vpn\":\"NotApplicable\"", ownerVpnSnapshot)
                .replaceFirst("\"revision\":3", "\"revision\":4"),
        )
        assertTrue(
            "stale owner Core lifecycle revision must be rejected",
            staleCoreRevision is OwnerSurfaceRead.Unavailable,
        )
    }

    @Test
    fun failedCoreOwnerReadbackMustMatchTheCompleteTypedFailure() {
        val failure = """{"code":"Configuration","message":"profile YAML could not be parsed","retryable":true,"reason":{"YamlSyntax":{"line":23,"column":6}}}"""
        val base = liveSnapshot
            .replace("\"vpn\":\"NotApplicable\"", ownerVpnSnapshot)
            .replace("\"lifecycle\":\"Running\"", "\"lifecycle\":\"Failed\"")
            .replace("\"failure\":null,\"upload_bps\"", "\"failure\":$failure,\"upload_bps\"")
            .replace("\"revision\":3,\"failure\":null", "\"revision\":3,\"failure\":$failure")
        assertTrue(
            "matching Failed Core readbacks must retain all typed facts",
            parseOwnerSurfaceSnapshot(base) is OwnerSurfaceRead.Available,
        )

        val staleFailureCode = base.replaceFirst(
            "\"code\":\"Configuration\"",
            "\"code\":\"Network\"",
        )
        assertTrue(
            "failure category mismatch must reject the cross-owner Core snapshot",
            parseOwnerSurfaceSnapshot(staleFailureCode) is OwnerSurfaceRead.Unavailable,
        )

        val staleFailure = base.replaceFirst(
            "\"retryable\":true,\"reason\"",
            "\"retryable\":false,\"reason\"",
        )
        assertTrue(
            "retryability mismatch must reject the cross-owner Core snapshot",
            parseOwnerSurfaceSnapshot(staleFailure) is OwnerSurfaceRead.Unavailable,
        )
    }

    @Test
    fun demoAndUnavailableSnapshotsCannotBecomeSuccessfulEmptyPages() {
        val demoRead = parseOwnerSurfaceSnapshot(liveSnapshot.replace("\"live\"", "\"demo\""))
        assertTrue(demoRead is OwnerSurfaceRead.Unavailable)

        val unavailableRead = parseOwnerSurfaceSnapshot(unavailableSnapshot)
        assertTrue(unavailableRead is OwnerSurfaceRead.Available)
        val snapshot = (unavailableRead as OwnerSurfaceRead.Available).snapshot
        assertNull(snapshot.connections.currentData())
        assertNull(snapshot.logs.currentData())
        assertEquals(RoutingIntentStatusObservation.Unobserved, snapshot.coreRoutingIntent.status)
    }

    @Test
    fun coreRoutingIntentStatusIsPreservedAsAnExplicitSharedState() {
        val unsupportedIntent = """
            {"current":null,"status":"Unsupported","failure":{"code":"Unsupported","message":"routing policy is unsupported","retryable":false,"reason":null}}
        """.trimIndent()
        val unsupportedSnapshot = liveSnapshot.replace(
            """{"current":{"kind":"follow_rules"},"status":"Ready","failure":null}""",
            unsupportedIntent,
        )
        val read = parseOwnerSurfaceSnapshot(unsupportedSnapshot) as OwnerSurfaceRead.Available

        assertEquals(RoutingIntentStatusObservation.Unsupported, read.snapshot.coreRoutingIntent.status)
        assertEquals(
            OwnerErrorCode.Unsupported,
            read.snapshot.coreRoutingIntent.failure?.code,
        )
        assertEquals(read.snapshot.coreRoutingIntent, read.snapshot.overview.currentData()?.routingIntent)
    }

    @Test
    fun contradictoryCoreAndOverviewRoutingIntentReadbacksFailClosed() {
        val inconsistent = liveSnapshot.replaceFirst(
            "\"overview\":{\"status\":\"Ready\",\"data\":{\"routing_intent\":{\"current\":{\"kind\":\"follow_rules\"},\"status\":\"Ready\",\"failure\":null}",
            "\"overview\":{\"status\":\"Ready\",\"data\":{\"routing_intent\":{\"current\":{\"kind\":\"bypass_proxy\"},\"status\":\"Ready\",\"failure\":null}",
        )

        assertTrue(parseOwnerSurfaceSnapshot(inconsistent) is OwnerSurfaceRead.Unavailable)
    }

    @Test
    fun routingIntentCommandFromOwnerSnapshotSerializesOnlyCoreScopeFields() {
        val ownerSnapshotJson = liveSnapshot
            .replace("\"generation\":8", "\"generation\":18446744073709551615")
            .replace("\"session_token\":42", "\"session_token\":340282366920938463463374607431768211455")
        val snapshot = (parseOwnerSurfaceSnapshot(ownerSnapshotJson) as OwnerSurfaceRead.Available).snapshot
        val scope = snapshot.coreScope ?: error("Live Core snapshot should provide a scope")
        val commandJson = buildRoutingIntentCommandJson(scope, RoutingIntentOption.BypassProxy)
        val rawScopeJson = rawJsonValueAtPath(commandJson, "scope")
            ?: error("Routing command should include the observed scope")
        val scopeJson = JSONObject(rawScopeJson)

        assertEquals(
            setOf("identity", "generation", "session_token"),
            scopeJson.keys().asSequence().toSet(),
        )
        assertEquals("18446744073709551615", rawJsonValueAtPath(commandJson, "scope", "generation"))
        assertEquals(
            "340282366920938463463374607431768211455",
            rawJsonValueAtPath(commandJson, "scope", "session_token"),
        )
        assertFalse(scopeJson.has("lifecycle"))
        assertFalse(scopeJson.has("revision"))
        assertFalse(scopeJson.has("routing_intent"))
    }

    @Test
    fun readyPageWithoutDataFailsClosed() {
        val malformed = liveSnapshot.replace(
            "\"overview\":{\"status\":\"Ready\",\"data\":{\"routing_intent\":{\"current\":{\"kind\":\"follow_rules\"},\"status\":\"Ready\",\"failure\":null},\"upload_bps\":128.4,\"download_bps\":1024.0,\"active_connections\":2}},",
            "\"overview\":{\"status\":\"Ready\",\"data\":null},",
        )

        assertTrue(parseOwnerSurfaceSnapshot(malformed) is OwnerSurfaceRead.Unavailable)
    }

    @Test
    fun ownerSnapshotMissingOutboundInventoryIsUnavailable() {
        val read = parseOwnerSurfaceSnapshot(liveSnapshot) as OwnerSurfaceRead.Available
        val snapshot = read.snapshot
        val page = snapshot.outboundInventory as OutboundInventoryPageState.Unavailable

        assertEquals(OwnerErrorCode.NotReady, page.failure.code)
        assertEquals(FailureTextKey.NotReady, page.failure.toLocalizedFailure().key)
        assertTrue(page.failure.message.isEmpty())
        assertTrue(snapshot.outboundDetails is OutboundDetailsPageState.Unavailable)
        assertTrue(snapshot.outboundUiPreferences is OutboundUiPreferencesPageState.Unavailable)
        assertTrue(snapshot.protocolStudio is ProtocolStudioPageState.Unavailable)
        assertTrue(snapshot.coreCapabilitySnapshot is CoreCapabilitySnapshotPageState.Unavailable)
    }

    @Test
    fun outboundInventoryAndSelectionStatesStayTyped() {
        val readyRead = parseOwnerSurfaceSnapshot(snapshotWithInventory(readyInventory))
        val readyPage = (readyRead as OwnerSurfaceRead.Available).snapshot
            .outboundInventory as OutboundInventoryPageState.Observed
        val ready = readyPage.snapshot.inventory as OutboundInventoryState.Ready
        val collection = ready.collections.single()

        assertEquals("node-group:opaque", collection.id.value)
        assertEquals(OutboundSelectionAvailability.Selectable, collection.availability)
        assertEquals(
            OutboundSelectionState.Ready(OutboundIdObservation("mihomo", "node-1:opaque")),
            collection.selection,
        )
        assertEquals("Tokyo", ready.entries.single().displayName)
        assertEquals("8", readyPage.snapshot.scope.generation)
        assertEquals("9223372036854775808", readyPage.snapshot.scope.sessionToken)
        assertEquals("42", readyPage.snapshot.revision)

        val unsupportedRead = parseOwnerSurfaceSnapshot(snapshotWithInventory("\"unsupported\""))
        val unsupported = ((unsupportedRead as OwnerSurfaceRead.Available).snapshot
            .outboundInventory as OutboundInventoryPageState.Observed).snapshot.inventory
        assertEquals(OutboundInventoryState.Unsupported, unsupported)

        val unsupportedSelectionInventory = readyInventory.replace(
            "\"selection\":{\"ready\":{\"selected\":{\"engine_id\":\"mihomo\",\"value\":\"node-1:opaque\"}}}",
            "\"selection\":\"unsupported\"",
        )
        val unsupportedSelectionRead = parseOwnerSurfaceSnapshot(
            snapshotWithInventory(unsupportedSelectionInventory),
        ) as OwnerSurfaceRead.Available
        val unsupportedSelection = (((unsupportedSelectionRead.snapshot.outboundInventory
            as OutboundInventoryPageState.Observed).snapshot.inventory
            as OutboundInventoryState.Ready).collections.single().selection)
        assertEquals(OutboundSelectionState.Unsupported, unsupportedSelection)

        val unknownRead = parseOwnerSurfaceSnapshot(snapshotWithInventory("\"unknown\""))
        val unknown = ((unknownRead as OwnerSurfaceRead.Available).snapshot
            .outboundInventory as OutboundInventoryPageState.Observed).snapshot.inventory
        assertEquals(OutboundInventoryState.Unknown, unknown)

        val failedRead = parseOwnerSurfaceSnapshot(snapshotWithInventory(failedInventory))
        val failed = ((failedRead as OwnerSurfaceRead.Available).snapshot
            .outboundInventory as OutboundInventoryPageState.Observed).snapshot.inventory
        assertTrue(failed is OutboundInventoryState.Failed)

        val failedPageJson = """
            {"Failed":{"failure":{"code":"Internal","message":"page read failed","retryable":false}}}
        """.trimIndent()
        val failedPageRead = parseOwnerSurfaceSnapshot(
            snapshotWithPageStatus(failedPageJson, "null"),
        ) as OwnerSurfaceRead.Available
        assertTrue(failedPageRead.snapshot.outboundInventory is OutboundInventoryPageState.Failed)
    }

    @Test
    fun detailsPreferencesAndProtocolStudioAreTypedOwnerSnapshots() {
        val read = parseOwnerSurfaceSnapshot(snapshotWithInventory(readyInventory))
        assertTrue("Owner snapshot parse failed: $read", read is OwnerSurfaceRead.Available)
        val snapshot = (read as OwnerSurfaceRead.Available).snapshot
        val detailsPage = snapshot.outboundDetails as OutboundDetailsPageState.Observed
        val detailSnapshot = detailsPage.snapshot
        val detail = (detailSnapshot.state as OutboundDetailsState.Ready).details.single()

        assertEquals(
            OutboundProtocolObservation.Reported(OutboundIdObservation("mihomo", "shadowsocks")),
            detail.protocol,
        )
        assertEquals("Shadowsocks", detail.protocolDisplayName)
        assertEquals(
            OutboundObservation.Observed(OutboundIdObservation("mihomo", "aes-256-gcm")),
            detail.cipher,
        )
        assertEquals("AEAD · AES-256-GCM", detail.cipherDisplayName)
        assertEquals(
            OutboundObservation.Observed(
                OutboundEndpointObservation("edge.example", OutboundObservation.Observed(443)),
            ),
            detail.endpoint,
        )
        assertEquals(OutboundHealthObservation.Healthy, detail.health)
        assertEquals(OutboundObservation.Observed(32L), detail.currentLatencyMs)
        assertEquals(
            "2026-10-10T10:00:00Z",
            (detail.latencyHistory as OutboundObservation.Observed).value.single().observedAt,
        )
        assertEquals(
            "18446744073709551614",
            (detail.historicalEgress as OutboundObservation.Observed).value.single().observedAtEpochMs,
        )
        assertEquals("18446744073709551613", detailSnapshot.detailRevision)

        val preferences = (snapshot.outboundUiPreferences as OutboundUiPreferencesPageState.Observed).snapshot
        assertEquals("18446744073709551614", preferences.revision)
        assertEquals("9223372036854775808", preferences.scope.sessionToken)
        assertEquals(listOf(OutboundIdObservation("mihomo", "node-1:opaque")), preferences.favoriteOutboundIds)
        assertEquals(OutboundSortOrder.LatencyAscending, preferences.sortOrder)
        assertEquals(OutboundHealthFilter.HealthyOnly, preferences.healthFilter)
        assertEquals(OutboundLayout.CompactList, preferences.layout)
        assertEquals(OutboundProbeOperationCapability.Supported, snapshot.outboundProbeCapability)

        val studio = (snapshot.protocolStudio as ProtocolStudioPageState.Observed).snapshot
        assertEquals("Tokyo draft", studio.draft?.name)
        assertEquals("ss", studio.draft?.nodeType)
        assertEquals("AEAD · AES-256-GCM", studio.report?.cipherChip)
        assertTrue(studio.report?.issues.orEmpty().isEmpty())
    }

    @Test
    fun outboundDetailsKeepUnknownUnsupportedAndFailedDistinct() {
        val unknown = (parseOwnerSurfaceSnapshot(snapshotWithTypedPages(readyInventory, "\"unknown\""))
            as OwnerSurfaceRead.Available).snapshot.outboundDetails as OutboundDetailsPageState.Observed
        assertEquals(OutboundDetailsState.Unknown, unknown.snapshot.state)

        val unsupported = (parseOwnerSurfaceSnapshot(snapshotWithTypedPages(readyInventory, "\"unsupported\""))
            as OwnerSurfaceRead.Available).snapshot.outboundDetails as OutboundDetailsPageState.Observed
        assertEquals(OutboundDetailsState.Unsupported, unsupported.snapshot.state)

        val failedState = """
            {"failed":{"failure":{"code":"Network","message":"details timed out","retryable":true}}}
        """.trimIndent()
        val failed = (parseOwnerSurfaceSnapshot(snapshotWithTypedPages(readyInventory, failedState))
            as OwnerSurfaceRead.Available).snapshot.outboundDetails as OutboundDetailsPageState.Observed
        val failure = (failed.snapshot.state as OutboundDetailsState.Failed).failure
        assertEquals(OwnerErrorCode.Network, failure.code)
        assertEquals("details timed out", failure.message)
    }

    @Test
    fun outboundProbeRequiresOwnerEffectiveAvailableSupportedResolution() {
        val available = parseOwnerSurfaceSnapshot(snapshotWithInventory(readyInventory)) as OwnerSurfaceRead.Available
        assertEquals(OutboundProbeOperationCapability.Supported, available.snapshot.outboundProbeCapability)
        val observedCapabilities = available.snapshot.coreCapabilitySnapshot
            as CoreCapabilitySnapshotPageState.Observed
        assertEquals(CoreHostKindObservation.Desktop, observedCapabilities.snapshot.scope.host)
        assertEquals(
            CoreBinaryTargetObservation.X86_64UnknownLinuxGnu,
            observedCapabilities.snapshot.scope.binaryTarget,
        )

        val hostTargetMismatch = snapshotWithInventory(readyInventory).replace(
            "\"host\":\"Desktop\"",
            "\"host\":\"Android\"",
        )
        assertTrue(parseOwnerSurfaceSnapshot(hostTargetMismatch) is OwnerSurfaceRead.Unavailable)

        val unknownTarget = snapshotWithInventory(readyInventory).replace(
            "\"binary_target\":\"x86_64-unknown-linux-gnu\"",
            "\"binary_target\":\"future-target\"",
        )
        assertTrue(parseOwnerSurfaceSnapshot(unknownTarget) is OwnerSurfaceRead.Unavailable)

        val adapterMissing = snapshotWithInventory(readyInventory).replace(
            "\"status\":\"available\",\"access\":\"supported\",\"maturity\":\"stable\"",
            "\"status\":\"adapter_missing\",\"blockers\":[{\"blocker\":\"adapter_missing\",\"reason\":\"probe adapter evidence missing\"}]",
        )
        val unavailable = parseOwnerSurfaceSnapshot(adapterMissing) as OwnerSurfaceRead.Available
        assertEquals(
            OutboundProbeOperationCapability.Unavailable(
                localizedFailure(FailureTextKey.NotReady, "probe adapter evidence missing"),
            ),
            unavailable.snapshot.outboundProbeCapability,
        )

        val wrongOperation = snapshotWithInventory(readyInventory).replace(
            "\"operation\":\"probe\"",
            "\"operation\":\"list\"",
        )
        val unregistered = parseOwnerSurfaceSnapshot(wrongOperation) as OwnerSurfaceRead.Available
        assertTrue(unregistered.snapshot.outboundProbeCapability is OutboundProbeOperationCapability.Unsupported)

        val wrongCoreVersion = snapshotWithInventory(readyInventory).replace(
            "\"core_version\":\"v1.19.32\"",
            "\"core_version\":\"1.19.31\"",
        )
        assertTrue(
            "a capability snapshot for another Core version is not a valid surface read",
            parseOwnerSurfaceSnapshot(wrongCoreVersion) is OwnerSurfaceRead.Unavailable,
        )
    }

    @Test
    fun capabilitySnapshotCannotBeReusedForAnOlderCoreIdentityOrVersion() {
        val wrongEngine = snapshotWithInventory(readyInventory).replace(
            "\"engine_id\":\"mihomo\"",
            "\"engine_id\":\"sing-box\"",
        )
        assertTrue(
            "capability evidence for another engine must be rejected",
            parseOwnerSurfaceSnapshot(wrongEngine) is OwnerSurfaceRead.Unavailable,
        )

        val wrongVersion = snapshotWithInventory(readyInventory).replace(
            "\"core_version\":\"v1.19.32\"",
            "\"core_version\":\"v1.20.0\"",
        )
        assertTrue(
            "capability evidence for another Core version must be rejected",
            parseOwnerSurfaceSnapshot(wrongVersion) is OwnerSurfaceRead.Unavailable,
        )
    }

    @Test
    fun androidCapabilityScopePreservesTheExactEngineBinaryTargetTriple() {
        val rawScope = """
            {"engine_id":"mihomo","core_version":"v1.19.32","host":"Android","binary_target":"aarch64-linux-android","adapter_revision":1}
        """.trimIndent()
        val rawSnapshot = """
            {"scope":$rawScope,"resolutions":[]}
        """.trimIndent()
        val snapshot = parseCoreCapabilitySnapshot(JSONObject(rawSnapshot), rawSnapshot)

        assertEquals(CoreHostKindObservation.Android, snapshot.scope.host)
        assertEquals(CoreBinaryTargetObservation.Aarch64LinuxAndroid, snapshot.scope.binaryTarget)
    }

    private companion object {
        val liveSnapshot = """
            {
              "origin":"live",
              "generation":8,
              "revision":12,
              "capabilities":{"host":"Desktop","revision":1,"entries":[]},
              "vpn":"NotApplicable",
              "core":{"identity":{"engine":"mihomo","instance":"android-main","version":"v1.19.32"},"lifecycle":"Running","generation":8,"session_token":42,"revision":3,"routing_intent":{"current":{"kind":"follow_rules"},"status":"Ready","failure":null},"core_version":"v1.19.32","sampled_at_epoch_ms":1,"failure":null,"upload_bps":128.4,"download_bps":1024.0,"active_connections":2,"memory_bytes":null,"watchdog":{"status":"Running"}},
              "pages":{
                "overview":{"status":"Ready","data":{"routing_intent":{"current":{"kind":"follow_rules"},"status":"Ready","failure":null},"upload_bps":128.4,"download_bps":1024.0,"active_connections":2}},
                "connections":{"status":"Ready","data":{"total_connections":1,"total_upload_bytes":4096,"total_download_bytes":8192,"connections":[{"id":"connection-1","host":"example.com","process":"/system/browser","network":"tcp","rule":"MATCH","chain":"Auto > Tokyo","chains":["Auto","Tokyo"],"destination_ip_asn":"","destination_geo_ip":null,"upload_total":100,"download_total":200}]}},
                "logs":{"status":"Ready","data":{"stream":"Live","total_entries":1,"active_level":null,"entries":[{"id":1,"timestamp":"2026-10-10T10:00:00Z","level":"warning","tag":"","message":"connected"}]}}
              }
            }
        """.trimIndent()

        val ownerVpnSnapshot = """
            {"AndroidOwner":{
              "owner_session_id":"vpn-owner-a","revision":14,"vpn_generation":9,
              "vpn_state":"Running","readiness":"ready",
              "resources":[
                {"kind":"prepared_configuration","state":"Absent"},
                {"kind":"worker","state":{"Held":{"generation":9}}},
                {"kind":"tunnel_descriptor","state":{"Held":{"generation":9}}},
                {"kind":"routing_lease","state":"Absent"}
              ],
              "foreground":true,"mtu":1500,"route_count":2,"dns_servers":["1.1.1.1"],"ipv6":false,
              "core":{"Available":{"identity":{"engine":"mihomo","instance":"android-main","version":"v1.19.32"},"lifecycle":"Running","generation":8,"session_token":"42","revision":3,"failure":null}}
            }}
        """.trimIndent()

        val unavailableSnapshot = """
            {
              "origin":"live",
              "generation":0,
              "revision":1,
              "capabilities":{"host":"Android","revision":1,"entries":[]},
              "vpn":"NotApplicable",
              "core":{"identity":null,"lifecycle":"Stopped","generation":0,"session_token":null,"revision":1,"routing_intent":{"current":null,"status":"Unobserved","failure":null},"core_version":null,"sampled_at_epoch_ms":1,"failure":null,"upload_bps":0.0,"download_bps":0.0,"active_connections":0,"memory_bytes":null,"watchdog":{"status":"Stopped"}},
              "pages":{
                "overview":{"status":{"Unavailable":{"failure":{"message":"core not running"}}},"data":null},
                "connections":{"status":{"Unavailable":{"failure":{"message":"core not running"}}},"data":null},
                "logs":{"status":{"Unavailable":{"failure":{"message":"core not running"}}},"data":null}
              }
            }
        """.trimIndent()

        val outboundScope = """
            {"identity":{"engine":"mihomo","instance":"android-main","version":"v1.19.32"},"generation":8,"session_token":9223372036854775808}
        """.trimIndent()

        val readyInventory = """
            {"ready":{"entries":[{"id":{"engine_id":"mihomo","value":"node-1:opaque"},"display_name":"Tokyo"}],"collections":[{"id":{"engine_id":"mihomo","value":"node-group:opaque"},"member_ids":[{"engine_id":"mihomo","value":"node-1:opaque"}],"availability":"selectable","selection":{"ready":{"selected":{"engine_id":"mihomo","value":"node-1:opaque"}}}}]}}
        """.trimIndent()

        val failedInventory = """
            {"failed":{"failure":{"code":"NotReady","message":"inventory read failed","retryable":true}}}
        """.trimIndent()

        val readyDetailsState = """
            {"ready":{"details":[{"id":{"engine_id":"mihomo","value":"node-1:opaque"},"protocol":{"state":"reported","value":{"engine_id":"mihomo","value":"shadowsocks"}},"protocol_display_name":"Shadowsocks","cipher":{"state":"observed","value":{"engine_id":"mihomo","value":"aes-256-gcm"}},"cipher_display_name":"AEAD · AES-256-GCM","collection_kind":{"state":"observed","value":"manual_selection"},"features":{"state":"observed","value":["udp"]},"endpoint":{"state":"observed","value":{"host":"edge.example","port":{"state":"observed","value":443}}},"udp":"supported","health":"healthy","current_latency_ms":{"state":"observed","value":32},"latency_history":{"state":"observed","value":[{"observed_at":"2026-10-10T10:00:00Z","delay_ms":32}]},"rtt":{"state":"observed","value":{"min_ms":31,"max_ms":48,"avg_ms":39,"valid_count":2}},"historical_egress":{"state":"observed","value":[{"ip":"203.0.113.8","country":"JP","observed_at_epoch_ms":18446744073709551614}]},"probe_availability":"supported"}]}}
        """.trimIndent()

        val protocolStudio = """
            {"draft":{"name":"Tokyo draft","server":"edge.example","port":443,"node_type":"ss","password":"not copied to the Compose model","uuid":"","sni":"","cipher":"aes-256-gcm","flow":"","reality":{},"smux":{},"dialer_proxy":"","tls":true,"skip_cert_verify":false,"alpn":[],"params":{},"preserved_fields":[]},"report":{"family":"shadowsocks","cipher_chip":"AEAD · AES-256-GCM","cipher_is_2022":false,"cipher_key_bytes":null,"flow_chip":null,"reality_chips":[],"smux_chips":[],"smux_overrides":false,"issues":[],"params":{}},"uri_preview":null,"audit":null,"last_error":null,"last_saved_node":null,"uri_gaps":[],"dialer":{},"ca_trust":{}}
        """.trimIndent()

        val coreCapabilitySnapshot = """
            {"scope":{"engine_id":"mihomo","core_version":"v1.19.32","host":"Desktop","binary_target":"x86_64-unknown-linux-gnu","adapter_revision":1},"resolutions":[{"key":{"scope":{"engine_id":"mihomo","core_version":"v1.19.32","host":"Desktop","binary_target":"x86_64-unknown-linux-gnu","adapter_revision":1},"operation":{"category":"outbound","operation":"probe"}},"engine":"Supported","host":"Supported","adapter":"Supported","status":{"status":"available","access":"supported","maturity":"stable"}}]}
        """.trimIndent()

        fun snapshotWithInventory(inventory: String): String = snapshotWithTypedPages(inventory, readyDetailsState)

        fun snapshotWithTypedPages(inventory: String, detailsState: String): String {
            val withInventory = snapshotWithPageStatus(
                "\"Ready\"",
                "{\"scope\":$outboundScope,\"revision\":42,\"inventory\":$inventory}",
            )
            return withInventory.replace(
                "\"outbound_inventory\":{",
                "\"outbound_details\":{\"status\":\"Ready\",\"data\":{\"scope\":$outboundScope,\"detail_revision\":18446744073709551613,\"state\":$detailsState}}," +
                    "\"outbound_ui_preferences\":{\"status\":\"Ready\",\"data\":{\"scope\":$outboundScope,\"revision\":18446744073709551614,\"search_query\":\"\",\"favorite_outbound_ids\":[{\"engine_id\":\"mihomo\",\"value\":\"node-1:opaque\"}],\"group_order\":[],\"collapsed_group_ids\":[],\"sort_order\":\"latency_ascending\",\"health_filter\":\"healthy_only\",\"layout\":\"compact_list\"}}," +
                    "\"core_capability_snapshot\":{\"status\":\"Ready\",\"data\":$coreCapabilitySnapshot}," +
                    "\"protocol_studio\":{\"status\":\"Ready\",\"data\":$protocolStudio},\"outbound_inventory\":{",
            )
        }

        fun snapshotWithPageStatus(status: String, data: String): String = liveSnapshot.replace(
            "\"origin\":\"live\",",
            "\"origin\":\"live\",\"outbound_inventory\":{\"status\":$status,\"data\":$data},",
        )
    }
}
