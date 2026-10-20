package com.musicfrog.infiltrator.ui.common

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class ProductCommandWireTest {
    @Test
    fun applicationCommandsMatchRustExternallyTaggedVariantsAndRetainObservedScope() {
        val scope = testScope()

        assertEquals(
            """{"scope":${scope.json},"command":{"Application":"ClearLogs"}}""",
            buildApplicationCommandJson(scope, AndroidApplicationCommand.ClearLogs),
        )
        assertEquals(
            """{"scope":${scope.json},"command":{"Application":"CloseAllConnections"}}""",
            buildApplicationCommandJson(scope, AndroidApplicationCommand.CloseAllConnections),
        )
        assertEquals(
            """{"scope":${scope.json},"command":{"Application":{"CloseConnection":{"id":"connection-\"one"}}}}""",
            buildApplicationCommandJson(
                scope,
                AndroidApplicationCommand.CloseConnection(" connection-\"one "),
            ),
        )
    }

    @Test
    fun productCommandRequiresMatchingObservedScopeAndCompletedTerminal() {
        val scope = testScope()
        val matching = terminal(scope.json)

        assertEquals(
            ScopedProductCommandResult.Completed("9"),
            parseScopedProductCommandResult(matching, scope),
        )
        assertEquals(
            ScopedProductCommandResult.ScopeMismatch,
            parseScopedProductCommandResult(terminal(scope.json.replace("\"generation\":7", "\"generation\":8")), scope),
        )
        assertTrue(
            parseScopedProductCommandResult(
                """{"Terminal":{"scope":${scope.json},"result":{"Accepted":{"request_id":9}}}}""",
                scope,
            ) is ScopedProductCommandResult.NonTerminal,
        )
    }

    private fun terminal(scopeJson: String) = """{"Terminal":{"scope":$scopeJson,"result":{"Completed":{"request_id":9}}}}"""
}

private fun testScope() = CoreScopeObservation(
    identity = CoreIdentityObservation("mihomo", "android-vpn", "1.19.12"),
    generation = "7",
    sessionToken = "8",
    json = """{"identity":{"engine":"mihomo","instance":"android-vpn","version":"1.19.12"},"generation":7,"session_token":8}""",
)
