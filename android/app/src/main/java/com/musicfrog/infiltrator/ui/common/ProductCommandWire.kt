package com.musicfrog.infiltrator.ui.common

import org.json.JSONObject

internal sealed interface AndroidApplicationCommand {
    data object ClearLogs : AndroidApplicationCommand
    data class CloseConnection(val id: String) : AndroidApplicationCommand
    data object CloseAllConnections : AndroidApplicationCommand
}

internal sealed interface ScopedProductCommandResult {
    data class Completed(val requestId: String) : ScopedProductCommandResult
    data class Failed(val failure: OwnerFailure) : ScopedProductCommandResult
    data object ScopeMismatch : ScopedProductCommandResult
    data object NonTerminal : ScopedProductCommandResult
    data object Invalid : ScopedProductCommandResult
}

internal sealed interface ProductCommandResult {
    data class Accepted(val requestId: String) : ProductCommandResult
    data class Completed(val requestId: String) : ProductCommandResult
    data class Produced(val requestId: String, val outputVariant: String, val outputJson: String) : ProductCommandResult
    data class Rejected(val requestId: String, val failure: OwnerFailure) : ProductCommandResult
}

internal sealed interface AndroidProductCommandOutcome {
    data class Terminal(val scope: CoreScopeObservation?, val result: ProductCommandResult) : AndroidProductCommandOutcome

    data class Unavailable(val scope: CoreScopeObservation?, val failure: OwnerFailure) : AndroidProductCommandOutcome
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

/** Serialize an exhaustive Android product command with the scope the user observed. */
internal fun buildApplicationCommandJson(scope: CoreScopeObservation, command: AndroidApplicationCommand): String {
    val commandJson = when (command) {
        AndroidApplicationCommand.ClearLogs -> """{"Application":"ClearLogs"}"""

        is AndroidApplicationCommand.CloseConnection -> {
            val id = command.id.trim()
            require(id.isNotEmpty()) { "Connection ID is empty" }
            """{"Application":{"CloseConnection":{"id":${JSONObject.quote(id)}}}}"""
        }

        AndroidApplicationCommand.CloseAllConnections -> """{"Application":"CloseAllConnections"}"""
    }
    return wrapObservedCommand(scope, commandJson)
}

/** Accept only a synchronous terminal result from the exact observed Core activation. */
internal fun parseScopedProductCommandResult(json: String, expectedScope: CoreScopeObservation): ScopedProductCommandResult = try {
    when (val outcome = parseAndroidProductCommandOutcome(json)) {
        is AndroidProductCommandOutcome.Unavailable -> when {
            outcome.scope != null && !sameCoreScope(expectedScope, outcome.scope) ->
                ScopedProductCommandResult.ScopeMismatch

            else -> ScopedProductCommandResult.Failed(outcome.failure)
        }

        is AndroidProductCommandOutcome.Terminal -> {
            val scope = outcome.scope
            if (scope == null || !sameCoreScope(expectedScope, scope)) {
                ScopedProductCommandResult.ScopeMismatch
            } else {
                when (val result = outcome.result) {
                    is ProductCommandResult.Completed -> ScopedProductCommandResult.Completed(result.requestId)

                    is ProductCommandResult.Rejected -> ScopedProductCommandResult.Failed(result.failure)

                    is ProductCommandResult.Accepted,
                    is ProductCommandResult.Produced,
                    -> ScopedProductCommandResult.NonTerminal
                }
            }
        }
    }
} catch (_: Exception) {
    ScopedProductCommandResult.Invalid
}

private fun sameCoreScope(expected: CoreScopeObservation, observed: CoreScopeObservation): Boolean =
    expected.identity == observed.identity &&
        expected.generation == observed.generation &&
        expected.sessionToken == observed.sessionToken

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

private fun JSONObject.unsignedId(name: String): String = parseUnsignedDecimal(get(name).toString(), name)
