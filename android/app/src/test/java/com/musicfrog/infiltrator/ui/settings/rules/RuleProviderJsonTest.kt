package com.musicfrog.infiltrator.ui.settings.rules

import infiltrator_android.AndroidJsonEntry
import infiltrator_android.AndroidJsonValue
import infiltrator_android.AndroidRuleProviderEntry
import org.junit.Assert.assertEquals
import org.junit.Test

class RuleProviderJsonTest {
    @Test
    fun nestedProviderDocumentRoundTripsThroughTypedEntries() {
        val entries = ruleProvidersJsonToEntries(
            """{"remote":{"type":"http","interval":86400,"tags":["a","b"],"nullable":null}}""",
        )

        assertEquals(
            listOf(
                AndroidRuleProviderEntry(
                    "remote",
                    AndroidJsonValue.ObjectValue(
                        listOf(
                            AndroidJsonEntry("interval", AndroidJsonValue.NumberValue("86400")),
                            AndroidJsonEntry("nullable", AndroidJsonValue.Null),
                            AndroidJsonEntry(
                                "tags",
                                AndroidJsonValue.ArrayValue(
                                    listOf(
                                        AndroidJsonValue.StringValue("a"),
                                        AndroidJsonValue.StringValue("b"),
                                    ),
                                ),
                            ),
                            AndroidJsonEntry("type", AndroidJsonValue.StringValue("http")),
                        ),
                    ),
                ),
            ),
            entries,
        )
        assertEquals(entries, ruleProvidersJsonToEntries(ruleProviderEntriesToJson(entries)))
    }

    @Test(expected = IllegalArgumentException::class)
    fun providerDocumentMustHaveAnObjectRoot() {
        ruleProvidersJsonToEntries("[]")
    }

    @Test(expected = IllegalArgumentException::class)
    fun providerDocumentRejectsDuplicateProviderNames() {
        ruleProvidersJsonToEntries("""{"remote":{},"remote":{}}""")
    }

    @Test(expected = IllegalArgumentException::class)
    fun providerDocumentRejectsDuplicateNestedFields() {
        ruleProvidersJsonToEntries("""{"remote":{"interval":1,"interval":2}}""")
    }

    @Test
    fun providerDocumentAcceptsSupportedMaximumNestingDepth() {
        val entries = ruleProvidersJsonToEntries(nestedProviderDocument(MAX_TEST_NESTING))
        assertEquals(entries, ruleProvidersJsonToEntries(ruleProviderEntriesToJson(entries)))
    }

    @Test(expected = IllegalArgumentException::class)
    fun providerDocumentRejectsExcessiveNestingDepth() {
        ruleProvidersJsonToEntries(nestedProviderDocument(MAX_TEST_NESTING + 1))
    }

    @Test(expected = IllegalArgumentException::class)
    fun typedProviderValuesRejectDuplicateNamesAndFields() {
        ruleProviderEntriesToJson(
            listOf(
                AndroidRuleProviderEntry("remote", AndroidJsonValue.Null),
                AndroidRuleProviderEntry("remote", AndroidJsonValue.Null),
            ),
        )
    }

    @Test(expected = IllegalArgumentException::class)
    fun typedProviderValuesRejectDuplicateNestedFields() {
        ruleProviderEntriesToJson(
            listOf(
                AndroidRuleProviderEntry(
                    "remote",
                    AndroidJsonValue.ObjectValue(
                        listOf(
                            AndroidJsonEntry("interval", AndroidJsonValue.NumberValue("1")),
                            AndroidJsonEntry("interval", AndroidJsonValue.NumberValue("2")),
                        ),
                    ),
                ),
            ),
        )
    }

    @Test(expected = IllegalArgumentException::class)
    fun typedProviderValuesRejectExcessiveNestingDepth() {
        var value: AndroidJsonValue = AndroidJsonValue.Null
        repeat(MAX_TEST_NESTING + 1) { index ->
            value = AndroidJsonValue.ObjectValue(
                listOf(AndroidJsonEntry("nested-$index", value)),
            )
        }
        ruleProviderEntriesToJson(listOf(AndroidRuleProviderEntry("remote", value)))
    }

    private fun nestedProviderDocument(levels: Int): String = buildString {
        append("{\"remote\":")
        repeat(levels) { append("{\"nested\":") }
        append("null")
        repeat(levels) { append('}') }
        append('}')
    }

    private companion object {
        const val MAX_TEST_NESTING = 128
    }
}
