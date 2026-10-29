package com.musicfrog.infiltrator.ui.proxies

import com.musicfrog.infiltrator.ui.common.AndroidProductCommandOutcome
import com.musicfrog.infiltrator.ui.common.OutboundDetailsPageState
import com.musicfrog.infiltrator.ui.common.OutboundInventoryPageState
import com.musicfrog.infiltrator.ui.common.OutboundInventoryState
import com.musicfrog.infiltrator.ui.common.OutboundProbeAvailability
import com.musicfrog.infiltrator.ui.common.OutboundProbeOperationCapability
import com.musicfrog.infiltrator.ui.common.OutboundProbeState
import com.musicfrog.infiltrator.ui.common.OutboundSortOrder
import com.musicfrog.infiltrator.ui.common.OutboundUiPreferencesPageState
import com.musicfrog.infiltrator.ui.common.OwnerErrorCode
import com.musicfrog.infiltrator.ui.common.OwnerFailure
import com.musicfrog.infiltrator.ui.common.OwnerSurfaceRead
import com.musicfrog.infiltrator.ui.common.ProductCommandResult
import com.musicfrog.infiltrator.ui.common.parseAndroidProductCommandOutcome
import com.musicfrog.infiltrator.ui.common.parseOwnerSurfaceSnapshot
import com.musicfrog.infiltrator.ui.common.rawJsonValueAtPath
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class OutboundCommandWireTest {
    @Test
    fun requestUsesObservedScopeOpaqueIdsAndInventoryRevision() {
        val snapshot = observedSnapshot()
        val ready = snapshot.inventory as OutboundInventoryState.Ready
        val collection = ready.collections.single()
        val outbound = ready.entries.single().id

        val requestJson = buildSelectOutboundCommandJson(snapshot, collection, outbound)
        val request = JSONObject(requestJson)
        val scope = request.getJSONObject("scope")
        val command = request.getJSONObject("command")
            .getJSONObject("Application")
            .getJSONObject("SelectOutbound")
            .getJSONObject("request")

        assertEquals("observed-core", scope.getJSONObject("identity").getString("instance"))
        assertEquals(8L, scope.getLong("generation"))
        assertEquals(snapshot.scope.json, rawJsonValueAtPath(requestJson, "scope"))
        assertEquals("9223372036854775808", rawJsonValueAtPath(requestJson, "scope", "session_token"))
        assertEquals("mihomo", command.getJSONObject("collection_id").getString("engine_id"))
        assertEquals("node-group:opaque", command.getJSONObject("collection_id").getString("value"))
        assertEquals("node-1:opaque", command.getJSONObject("outbound_id").getString("value"))
        assertEquals(42L, command.getLong("expected_revision"))
        assertEquals(
            snapshot.revision,
            rawJsonValueAtPath(
                requestJson,
                "command",
                "Application",
                "SelectOutbound",
                "request",
                "expected_revision",
            ),
        )
    }

    @Test
    fun oldGenerationTerminalOutcomeIsRejectedAndUnavailableFailureStaysTyped() {
        val observed = observedSnapshot().scope
        val oldGenerationOutcome = """
            {"Terminal":{"scope":{"identity":{"engine":"mihomo","instance":"observed-core","version":"1.19.12"},"generation":7,"session_token":9223372036854775808},"result":{"Rejected":{"request_id":6,"failure":{"code":"InvalidState","message":"scope expired","retryable":false}}}}}
        """.trimIndent()

        assertEquals(
            OutboundCommandHandling.StaleScope,
            evaluateOutboundCommandOutcome(
                parseAndroidProductCommandOutcome(oldGenerationOutcome),
                observed,
            ),
        )

        val ownerFailureJson = """
            {"Unavailable":{"scope":{"identity":{"engine":"mihomo","instance":"observed-core","version":"1.19.12"},"generation":8,"session_token":9223372036854775808},"failure":{"code":"NotReady","message":"owner is stopping","retryable":true}}}
        """.trimIndent()
        val result = evaluateOutboundCommandOutcome(
            parseAndroidProductCommandOutcome(ownerFailureJson),
            observed,
        ) as OutboundCommandHandling.OwnerFailure

        assertEquals(
            OwnerFailure(OwnerErrorCode.NotReady, "owner is stopping", true, null),
            result.failure,
        )
    }

    @Test
    fun terminalProducedOutboundReadbackIsAValidSuccess() {
        val snapshot = observedSnapshot()
        val ready = snapshot.inventory as OutboundInventoryState.Ready
        val collection = ready.collections.single()
        val outboundId = ready.entries.single().id
        val scopeJson = snapshot.scope.json
        val outcome = """
            {"Terminal":{"scope":$scopeJson,"result":{"Produced":{"request_id":11,"output":{"OutboundInventory":{"scope":$scopeJson,"revision":43,"inventory":{"ready":{"entries":[{"id":{"engine_id":"mihomo","value":"node-1:opaque"},"display_name":"Tokyo"}],"collections":[{"id":{"engine_id":"mihomo","value":"node-group:opaque"},"member_ids":[{"engine_id":"mihomo","value":"node-1:opaque"}],"availability":"selectable","selection":{"ready":{"selected":{"engine_id":"mihomo","value":"node-1:opaque"}}}}]}}}}}}}}
        """.trimIndent()

        val handling = evaluateSelectOutboundOutcome(
            parseAndroidProductCommandOutcome(outcome),
            snapshot,
            collection,
            outboundId,
        )
        assertTrue(handling is OutboundCommandHandling.SelectionApplied)
        assertEquals("43", (handling as OutboundCommandHandling.SelectionApplied).snapshot.revision)

        val unconfirmedOutcome = outcome.replace(
            "\"selected\":{\"engine_id\":\"mihomo\",\"value\":\"node-1:opaque\"}",
            "\"selected\":{\"engine_id\":\"mihomo\",\"value\":\"other-node\"}",
        )
        assertTrue(
            evaluateSelectOutboundOutcome(
                parseAndroidProductCommandOutcome(unconfirmedOutcome),
                snapshot,
                collection,
                outboundId,
            ) is OutboundCommandHandling.UnexpectedResult,
        )
    }

    @Test
    fun preferenceCommandUsesObservedScopeAndExactPreferenceRevision() {
        val inventory = observedSnapshot()
        val preferences = observedPreferences()
        val outboundId = (inventory.inventory as OutboundInventoryState.Ready).entries.single().id
        val json = buildApplyOutboundUiPreferenceCommandJson(
            inventory,
            preferences,
            OutboundUiPreferenceIntent.SetFavorite(outboundId, pinned = false),
        )
        val request = JSONObject(json)
        val payload = request.getJSONObject("command")
            .getJSONObject("Application")
            .getJSONObject("ApplyOutboundUiPreference")
            .getJSONObject("request")

        assertEquals(inventory.scope.json, rawJsonValueAtPath(json, "scope"))
        assertEquals(
            "18446744073709551614",
            rawJsonValueAtPath(
                json,
                "command",
                "Application",
                "ApplyOutboundUiPreference",
                "request",
                "expected_revision",
            ),
        )
        assertEquals(OutboundSortOrder.LatencyAscending, preferences.sortOrder)
        assertEquals("set_favorite", payload.getJSONObject("intent").getString("operation"))
        assertEquals(
            "node-1:opaque",
            payload.getJSONObject("intent").getJSONObject("value")
                .getJSONObject("outbound_id").getString("value"),
        )
        assertEquals("unpinned", payload.getJSONObject("intent").getJSONObject("value").getString("state"))

        val mismatchedScope = preferences.copy(scope = preferences.scope.copy(generation = "9"))
        try {
            buildApplyOutboundUiPreferenceCommandJson(
                inventory,
                mismatchedScope,
                OutboundUiPreferenceIntent.SetFavorite(outboundId, pinned = false),
            )
            throw AssertionError("preference builder accepted a different CoreScope")
        } catch (expected: IllegalArgumentException) {
            assertTrue(expected.message.orEmpty().contains("different CoreScope"))
        }
    }

    @Test
    fun preferenceSuccessRequiresExactScopeAndRevisionAdvancedTypedReadback() {
        val inventory = observedSnapshot()
        val preferences = observedPreferences()
        val outboundId = (inventory.inventory as OutboundInventoryState.Ready).entries.single().id
        val intent = OutboundUiPreferenceIntent.SetFavorite(outboundId, pinned = false)
        val scope = inventory.scope.json
        val output = """
            {"OutboundUiPreferencesApplied":{"scope":$scope,"revision":18446744073709551615,"search_query":"","favorite_outbound_ids":[],"group_order":[],"collapsed_group_ids":[],"sort_order":"latency_ascending","health_filter":"all","layout":"compact_list"}}
        """.trimIndent()
        val outcome = """
            {"Terminal":{"scope":$scope,"result":{"Produced":{"request_id":19,"output":$output}}}}
        """.trimIndent()

        val handling = evaluateOutboundUiPreferenceOutcome(
            parseAndroidProductCommandOutcome(outcome),
            inventory.scope,
            preferences,
            intent,
        )
        assertTrue(handling is OutboundCommandHandling.PreferencesApplied)
        assertEquals("18446744073709551615", (handling as OutboundCommandHandling.PreferencesApplied).snapshot.revision)

        val staleReadback = output.replace("18446744073709551615", "18446744073709551614")
        val staleOutcome = outcome.replace(output, staleReadback)
        assertTrue(
            evaluateOutboundUiPreferenceOutcome(
                parseAndroidProductCommandOutcome(staleOutcome),
                inventory.scope,
                preferences,
                intent,
            ) is OutboundCommandHandling.UnexpectedResult,
        )
    }

    @Test
    fun probeCommandAndReadbackRemainFencedToObservedOutboundRevision() {
        val inventory = observedSnapshot()
        val details = observedDetails()
        val outboundId = (inventory.inventory as OutboundInventoryState.Ready).entries.single().id
        val json = buildProbeOutboundCommandJson(
            inventory,
            details,
            OutboundProbeOperationCapability.Supported,
            outboundId,
        )
        val request = JSONObject(json)
            .getJSONObject("command").getJSONObject("Application")
            .getJSONObject("ProbeOutbound").getJSONObject("request")
        assertEquals(inventory.scope.json, rawJsonValueAtPath(json, "scope"))
        assertEquals("42", rawJsonValueAtPath(json, "command", "Application", "ProbeOutbound", "request", "expected_inventory_revision"))
        assertEquals("node-1:opaque", request.getJSONObject("outbound_id").getString("value"))

        val scope = inventory.scope.json
        val output = """
            {"OutboundProbe":{"scope":$scope,"observed_inventory_revision":42,"detail_revision":43,"outbound_id":{"engine_id":"mihomo","value":"node-1:opaque"},"state":{"measured":{"delay_ms":31}}}}
        """.trimIndent()
        val outcome = """
            {"Terminal":{"scope":$scope,"result":{"Produced":{"request_id":20,"output":$output}}}}
        """.trimIndent()
        val handling = evaluateProbeOutboundOutcome(
            parseAndroidProductCommandOutcome(outcome),
            inventory,
            outboundId,
        )
        assertTrue(handling is OutboundCommandHandling.ProbeCompleted)
        assertEquals(31L, ((handling as OutboundCommandHandling.ProbeCompleted).snapshot.state as OutboundProbeState.Measured).delayMs)

        val wrongRevision = output.replace("\"observed_inventory_revision\":42", "\"observed_inventory_revision\":43")
        val wrongOutcome = outcome.replace(output, wrongRevision)
        assertTrue(
            evaluateProbeOutboundOutcome(
                parseAndroidProductCommandOutcome(wrongOutcome),
                inventory,
                outboundId,
            ) is OutboundCommandHandling.UnexpectedResult,
        )
    }

    @Test
    fun perItemProbeSupportCannotOverrideMissingGlobalCapability() {
        val inventory = observedSnapshot()
        val details = observedDetails()
        val outboundId = (inventory.inventory as OutboundInventoryState.Ready).entries.single().id
        val unavailable = OutboundProbeOperationCapability.Unavailable(
            com.musicfrog.infiltrator.ui.common.localizedFailure(
                com.musicfrog.infiltrator.ui.common.FailureTextKey.NotReady,
                "adapter evidence missing",
            ),
        )

        assertFalse(canProbeOutbound(OutboundProbeAvailability.Supported, unavailable))
        try {
            buildProbeOutboundCommandJson(inventory, details, unavailable, outboundId)
            throw AssertionError("probe builder accepted a globally unavailable operation")
        } catch (expected: IllegalArgumentException) {
            assertTrue(expected.message.orEmpty().contains("not available"))
        }
    }

    @Test
    fun probeCommandWithCustomTimeoutEncodesCorrectly() {
        val inventory = observedSnapshot()
        val details = observedDetails()
        val outboundId = (inventory.inventory as OutboundInventoryState.Ready).entries.single().id
        val json = buildProbeOutboundCommandJson(
            inventory,
            details,
            OutboundProbeOperationCapability.Supported,
            outboundId,
            testUrl = "https://probe.custom.test/generate_204",
            timeoutMs = 8500L,
        )
        val request = JSONObject(json)
            .getJSONObject("command").getJSONObject("Application")
            .getJSONObject("ProbeOutbound").getJSONObject("request")
        assertEquals(8500L, request.getLong("timeout_ms"))
        assertEquals("https://probe.custom.test/generate_204", request.getString("test_url"))
    }

    @Test
    fun setRuntimeProbeSettingsCommandEncodesScopeAndSettings() {
        val inventory = observedSnapshot()
        val json = buildSetRuntimeProbeSettingsCommandJson(
            inventory.scope,
            testUrl = "https://probe.custom.test/generate_204",
            timeoutMs = 9000L,
        )
        assertEquals(inventory.scope.json, rawJsonValueAtPath(json, "scope"))
        val settings = JSONObject(json)
            .getJSONObject("command").getJSONObject("SetRuntimeProbeSettings")
            .getJSONObject("settings")
        assertEquals(9000L, settings.getLong("timeout_ms"))
        assertEquals("https://probe.custom.test/generate_204", settings.getString("url"))
    }

    private fun ownerSnapshot(): com.musicfrog.infiltrator.ui.common.OwnerSurfaceSnapshot =
        when (val read = parseOwnerSurfaceSnapshot(snapshotWithOutbound)) {
            is OwnerSurfaceRead.Available -> read.snapshot
            is OwnerSurfaceRead.Unavailable -> error("Fixture owner snapshot failed to parse: ${read.failure.diagnostic}")
        }

    private fun observedSnapshot() = (ownerSnapshot().outboundInventory as OutboundInventoryPageState.Observed).snapshot

    private fun observedDetails() = (ownerSnapshot().outboundDetails as OutboundDetailsPageState.Observed).snapshot

    private fun observedPreferences() = (ownerSnapshot().outboundUiPreferences as OutboundUiPreferencesPageState.Observed).snapshot

    private companion object {
        val snapshotWithOutbound = """
            {
              "origin":"live",
              "generation":8,
              "revision":12,
              "capabilities":{"host":"Android","revision":1,"entries":[]},
              "vpn":"NotApplicable",
              "core":{"identity":{"engine":"mihomo","instance":"observed-core","version":"1.19.12"},"lifecycle":"Running","generation":8,"session_token":9223372036854775808,"revision":3,"routing_intent":{"current":{"kind":"follow_rules"},"status":"Ready","failure":null},"core_version":"1.19.12","sampled_at_epoch_ms":1,"failure":null,"upload_bps":0,"download_bps":0,"active_connections":0,"memory_bytes":null,"watchdog":{"status":"Running"}},
              "outbound_inventory":{
                "status":"Ready",
                "data":{
                  "scope":{"identity":{"engine":"mihomo","instance":"observed-core","version":"1.19.12"},"generation":8,"session_token":9223372036854775808},
                  "revision":42,
                  "inventory":{"ready":{
                    "entries":[{"id":{"engine_id":"mihomo","value":"node-1:opaque"},"display_name":"Tokyo"}],
                    "collections":[{"id":{"engine_id":"mihomo","value":"node-group:opaque"},"member_ids":[{"engine_id":"mihomo","value":"node-1:opaque"}],"availability":"selectable","selection":{"ready":{"selected":{"engine_id":"mihomo","value":"node-1:opaque"}}}}]
                  }}
                }
              },
              "outbound_details":{"status":"Ready","data":{"scope":{"identity":{"engine":"mihomo","instance":"observed-core","version":"1.19.12"},"generation":8,"session_token":9223372036854775808},"detail_revision":43,"state":{"ready":{"details":[{"id":{"engine_id":"mihomo","value":"node-1:opaque"},"protocol":{"state":"reported","value":{"engine_id":"mihomo","value":"shadowsocks"}},"protocol_display_name":"Shadowsocks","cipher":{"state":"unknown"},"cipher_display_name":null,"collection_kind":{"state":"unknown"},"features":{"state":"unknown"},"endpoint":{"state":"unknown"},"udp":"unknown","health":"unknown","current_latency_ms":{"state":"unknown"},"latency_history":{"state":"unknown"},"rtt":{"state":"unknown"},"historical_egress":{"state":"unknown"},"probe_availability":"supported"}]}}}},
              "outbound_ui_preferences":{"status":"Ready","data":{"scope":{"identity":{"engine":"mihomo","instance":"observed-core","version":"1.19.12"},"generation":8,"session_token":9223372036854775808},"revision":18446744073709551614,"search_query":"","favorite_outbound_ids":[{"engine_id":"mihomo","value":"node-1:opaque"}],"group_order":[],"collapsed_group_ids":[],"sort_order":"latency_ascending","health_filter":"all","layout":"compact_list"}},
              "protocol_studio":{"status":{"Unavailable":{"failure":{"code":"NotReady","message":"not published","retryable":true}}},"data":null},
              "pages":{
                "overview":{"status":"Ready","data":{"routing_intent":{"current":{"kind":"follow_rules"},"status":"Ready","failure":null},"upload_bps":0,"download_bps":0,"active_connections":0}},
                "connections":{"status":"Ready","data":{"total_connections":0,"total_upload_bytes":0,"total_download_bytes":0,"connections":[]}},
                "logs":{"status":"Ready","data":{"stream":"Idle","total_entries":0,"entries":[]}}
              }
            }
        """.trimIndent()
    }
}
