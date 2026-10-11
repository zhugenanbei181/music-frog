package com.musicfrog.infiltrator.ui.common

import org.json.JSONObject

internal sealed interface ProductCommandResult {
    data class Accepted(val requestId: String) : ProductCommandResult
    data class Completed(val requestId: String) : ProductCommandResult
    data class Produced(
        val requestId: String,
        val outputVariant: String,
        val outputJson: String,
    ) : ProductCommandResult
    data class Rejected(val requestId: String, val failure: OwnerFailure) : ProductCommandResult
}

internal sealed interface AndroidProductCommandOutcome {
    data class Terminal(
        val scope: CoreScopeObservation?,
        val result: ProductCommandResult,
    ) : AndroidProductCommandOutcome

    data class Unavailable(
        val scope: CoreScopeObservation?,
        val failure: OwnerFailure,
    ) : AndroidProductCommandOutcome
}

/** Parse the one shared owner-command result contract used by all Android pages. */
internal fun parseAndroidProductCommandOutcome(json: String): AndroidProductCommandOutcome {
    val root = JSONObject(json)
    return when (val variant = root.singleVariant()) {
        "Terminal" -> {
            val payload = root.getJSONObject(variant)
            val rawResult = rawJsonValueAtPath(json, variant, "result")
                ?: throw IllegalArgumentException("Terminal product outcome has no raw result")
            AndroidProductCommandOutcome.Terminal(
                scope = payload.nullableScope(rawJsonValueAtPath(json, variant, "scope")),
                result = parseProductCommandResult(payload.getJSONObject("result"), rawResult),
            )
        }
        "Unavailable" -> {
            val payload = root.getJSONObject(variant)
            AndroidProductCommandOutcome.Unavailable(
                scope = payload.nullableScope(rawJsonValueAtPath(json, variant, "scope")),
                failure = parseOwnerFailure(payload.getJSONObject("failure")),
            )
        }
        else -> throw IllegalArgumentException("Unknown AndroidProductCommandOutcome variant: $variant")
    }
}

/** Use the exact scope observed from the owner; do not normalize large unsigned IDs. */
internal fun wrapObservedCommand(scope: CoreScopeObservation, commandJson: String): String =
    """{"scope":${scope.json},"command":$commandJson}"""

private fun parseProductCommandResult(value: JSONObject, rawResultJson: String): ProductCommandResult =
    when (val variant = value.singleVariant()) {
        "Accepted" -> ProductCommandResult.Accepted(
            value.getJSONObject(variant).unsignedId("request_id"),
        )
        "Completed" -> ProductCommandResult.Completed(
            value.getJSONObject(variant).unsignedId("request_id"),
        )
        "Produced" -> {
            val payload = value.getJSONObject(variant)
            val rawOutput = rawJsonValueAtPath(rawResultJson, variant, "output")
                ?: throw IllegalArgumentException("Produced command result has no raw output")
            val output = JSONObject(rawOutput)
            ProductCommandResult.Produced(
                requestId = payload.unsignedId("request_id"),
                outputVariant = output.singleVariant(),
                outputJson = rawOutput,
            )
        }
        "Rejected" -> {
            val payload = value.getJSONObject(variant)
            ProductCommandResult.Rejected(
                requestId = payload.unsignedId("request_id"),
                failure = parseOwnerFailure(payload.getJSONObject("failure")),
            )
        }
        else -> throw IllegalArgumentException("Unknown CommandResult variant: $variant")
    }

private fun JSONObject.nullableScope(rawScopeJson: String?): CoreScopeObservation? {
    val value = opt("scope") ?: return null
    if (value === JSONObject.NULL) return null
    val scope = value as? JSONObject ?: throw IllegalArgumentException("Outcome scope is malformed")
    return parseCoreScopeObservation(
        scope,
        rawScopeJson ?: throw IllegalArgumentException("Outcome scope is missing from raw JSON"),
    )
}

private fun JSONObject.unsignedId(name: String): String =
    parseUnsignedDecimal(get(name).toString(), name)
