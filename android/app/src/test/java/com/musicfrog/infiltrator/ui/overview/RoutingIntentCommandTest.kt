package com.musicfrog.infiltrator.ui.overview

import com.musicfrog.infiltrator.ui.common.OwnerErrorCode
import com.musicfrog.infiltrator.ui.common.RoutingIntentObservation
import com.musicfrog.infiltrator.ui.common.parseCoreScopeObservation
import com.musicfrog.infiltrator.ui.common.rawJsonValueAtPath
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class RoutingIntentCommandTest {
    @Test
    fun commandUsesObservedCoreScopeAndSharedTypedIntent() {
        val scope = observedScope()
        val request = buildRoutingIntentCommandJson(scope, RoutingIntentOption.BypassProxy)
        val command = JSONObject(request)
            .getJSONObject("command")
            .getJSONObject("Application")
            .getJSONObject("SetRoutingIntent")

        assertEquals("mihomo", JSONObject(request).getJSONObject("scope")
            .getJSONObject("identity").getString("engine"))
        assertEquals("9223372036854775808", rawJsonValueAtPath(request, "scope", "session_token"))
        assertEquals("bypass_proxy", command.getJSONObject("intent").getString("kind"))
        assertEquals(null, command.opt("mode"))
    }

    @Test
    fun onlyMatchingTerminalReadbackConfirmsTheRequestedIntent() {
        val scope = observedScope()
        val output = terminal(scope.json, "bypass_proxy")

        assertEquals(
            RoutingIntentCommandResult.Applied(RoutingIntentObservation.BypassProxy),
            parseRoutingIntentCommandResult(output, scope, RoutingIntentOption.BypassProxy),
        )

        val mismatch = terminal(scope.json, "follow_rules")
        val failed = parseRoutingIntentCommandResult(
            mismatch,
            scope,
            RoutingIntentOption.BypassProxy,
        ) as RoutingIntentCommandResult.Failed
        assertEquals(OwnerErrorCode.InvalidState, failed.failure.code)
    }

    @Test
    fun staleScopeAndTypedOwnerRejectionRemainFailures() {
        val scope = observedScope()
        val staleScope = scope.json.replace("\"generation\":8", "\"generation\":9")
        val stale = parseRoutingIntentCommandResult(
            terminal(staleScope, "bypass_proxy"),
            scope,
            RoutingIntentOption.BypassProxy,
        ) as RoutingIntentCommandResult.Failed
        assertEquals(OwnerErrorCode.InvalidState, stale.failure.code)

        val staleUnavailable = """
            {"Unavailable":{"scope":$staleScope,"failure":{"code":"NotReady","message":"scope expired","retryable":true,"reason":null}}}
        """.trimIndent()
        val staleUnavailableResult = parseRoutingIntentCommandResult(
            staleUnavailable,
            scope,
            RoutingIntentOption.BypassProxy,
        ) as RoutingIntentCommandResult.Failed
        assertEquals(OwnerErrorCode.InvalidState, staleUnavailableResult.failure.code)

        val rejected = """
            {"Terminal":{"scope":${scope.json},"result":{"Rejected":{"request_id":12,"failure":{"code":"Unsupported","message":"engine does not support this policy","retryable":false,"reason":null}}}}}
        """.trimIndent()
        val ownerFailure = parseRoutingIntentCommandResult(
            rejected,
            scope,
            RoutingIntentOption.BypassProxy,
        ) as RoutingIntentCommandResult.Failed
        assertEquals(OwnerErrorCode.Unsupported, ownerFailure.failure.code)
        assertTrue(ownerFailure.failure.message.contains("does not support"))
    }

    private fun observedScope() = parseCoreScopeObservation(
        JSONObject(scopeJson),
        scopeJson,
    )

    private fun terminal(scope: String, observed: String): String = """
        {"Terminal":{"scope":$scope,"result":{"Produced":{"request_id":12,"output":{"RoutingIntentApplied":{"observed":{"kind":"$observed"}}}}}}}
    """.trimIndent()

    private companion object {
        val scopeJson = """
            {"identity":{"engine":"mihomo","instance":"android-main","version":"1.19.12"},"generation":8,"session_token":9223372036854775808}
        """.trimIndent()
    }
}
