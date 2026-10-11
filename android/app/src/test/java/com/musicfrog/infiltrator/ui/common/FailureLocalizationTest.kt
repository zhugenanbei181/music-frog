package com.musicfrog.infiltrator.ui.common

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class FailureLocalizationTest {
    @Test
    fun appFailureSeparatesMachineCodeLocalizationParamsAndDiagnostic() {
        val failure = AppFailure(
            code = AppFailureCode.Configuration,
            localization = FailureTextKey.YamlSyntax,
            params = listOf("7", "3"),
            diagnostic = "parser detail",
        )

        assertEquals(
            LocalizedFailure(
                key = FailureTextKey.YamlSyntax,
                arguments = listOf("7", "3"),
                diagnostic = "parser detail",
            ),
            failure.toLocalizedFailure(),
        )
    }

    @Test
    fun ownerFailureReasonProjectsTypedCopyAndKeepsDiagnosticSeparate() {
        val failure = parseOwnerFailure(
            JSONObject(
                """{"code":"Configuration","message":"opaque {line} {detail}","retryable":false,"reason":{"YamlSyntax":{"line":7,"column":3}}}""",
            ),
        )

        assertEquals(OwnerFailureReason.YamlSyntax(7uL, 3uL), failure.reason)
        val presentation = failure.toLocalizedFailure()
        assertEquals(FailureTextKey.YamlSyntax, presentation.key)
        assertEquals(listOf("7", "3"), presentation.arguments)
        assertEquals("opaque {line} {detail}", presentation.diagnostic)
    }

    @Test
    fun ownerFailureCategoryUsesClosedTypedProjectionAndOpaqueMessage() {
        val failure = parseOwnerFailure(
            JSONObject(
                """{"code":"Network","message":"backend-specific wording","retryable":true,"reason":null}""",
            ),
        )
        val presentation = failure.toLocalizedFailure()

        assertEquals(FailureTextKey.Network, presentation.key)
        assertTrue(presentation.arguments.isEmpty())
        assertEquals("backend-specific wording", presentation.diagnostic)
    }

    @Test
    fun knownUnitReasonsAreParsedWithoutStringHeuristics() {
        val reason = parseOwnerFailureReason("QuotaSourceChanged")
        assertEquals(OwnerFailureReason.QuotaSourceChanged, reason)
        assertEquals(
            FailureTextKey.QuotaSourceChanged,
            OwnerFailure(OwnerErrorCode.Internal, "opaque", false, reason).toLocalizedFailure().key,
        )

        var rejectedUnknownReason = false
        try {
            parseOwnerFailureReason("English fallback reason")
        } catch (_: IllegalArgumentException) {
            rejectedUnknownReason = true
        }
        assertTrue(rejectedUnknownReason)
    }

    @Test
    fun everyFailureAndEmptyStateKeyHasAnAndroidResource() {
        FailureTextKey.values().forEach { key -> assertTrue(key.resource != 0) }
        EmptyStateTextKey.values().forEach { key -> assertTrue(key.resource != 0) }
        OwnerUiStatusTextKey.values().forEach { key -> assertTrue(key.resource != 0) }
    }
}
