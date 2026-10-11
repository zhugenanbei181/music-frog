package com.musicfrog.infiltrator.ui.overview

import com.musicfrog.infiltrator.ui.common.CoreScopeObservation
import com.musicfrog.infiltrator.ui.common.AndroidProductCommandOutcome
import com.musicfrog.infiltrator.ui.common.OwnerFailure
import com.musicfrog.infiltrator.ui.common.OwnerErrorCode
import com.musicfrog.infiltrator.ui.common.ProductCommandResult
import com.musicfrog.infiltrator.ui.common.RoutingIntentObservation
import com.musicfrog.infiltrator.ui.common.RoutingIntentSnapshotObservation
import com.musicfrog.infiltrator.ui.common.parseAndroidProductCommandOutcome
import com.musicfrog.infiltrator.ui.common.parseRoutingIntentObservation
import com.musicfrog.infiltrator.ui.common.singleVariant
import com.musicfrog.infiltrator.ui.common.wrapObservedCommand
import org.json.JSONObject

internal sealed interface RoutingIntentUiState {
    data object Loading : RoutingIntentUiState
    data class Ready(
        val current: RoutingIntentObservation,
        val scope: CoreScopeObservation,
    ) : RoutingIntentUiState
    data class Updating(
        val current: RoutingIntentObservation,
        val target: RoutingIntentOption,
        val scope: CoreScopeObservation,
    ) : RoutingIntentUiState
    data class Pending(val snapshot: RoutingIntentSnapshotObservation) : RoutingIntentUiState
    data object Unobserved : RoutingIntentUiState
    data class Unsupported(val snapshot: RoutingIntentSnapshotObservation) : RoutingIntentUiState
    data class Failed(val failure: OwnerFailure?) : RoutingIntentUiState
    data object Unavailable : RoutingIntentUiState
}

internal sealed interface RoutingIntentCommandResult {
    data class Applied(val observed: RoutingIntentObservation) : RoutingIntentCommandResult
    data class Failed(val failure: OwnerFailure) : RoutingIntentCommandResult
}

/** Builds the Android owner command from the exact surface scope the user observed. */
internal fun buildRoutingIntentCommandJson(
    scope: CoreScopeObservation,
    intent: RoutingIntentOption,
): String {
    val command = """
        {"Application":{"SetRoutingIntent":{"intent":${intent.serializedIntent()}}}}
    """.trimIndent()
    return wrapObservedCommand(scope, command)
}

internal fun parseRoutingIntentCommandResult(
    resultJson: String,
    expectedScope: CoreScopeObservation,
    requested: RoutingIntentOption,
): RoutingIntentCommandResult = try {
    when (val outcome = parseAndroidProductCommandOutcome(resultJson)) {
        is AndroidProductCommandOutcome.Unavailable -> if (
            outcome.scope != null && !sameCoreScope(expectedScope, outcome.scope)
        ) {
            staleRoutingIntentScopeFailure()
        } else {
            RoutingIntentCommandResult.Failed(outcome.failure)
        }
        is AndroidProductCommandOutcome.Terminal -> {
            if (outcome.scope == null || !sameCoreScope(expectedScope, outcome.scope)) {
                staleRoutingIntentScopeFailure()
            } else {
                when (val commandResult = outcome.result) {
                    is ProductCommandResult.Rejected ->
                        RoutingIntentCommandResult.Failed(commandResult.failure)
                    is ProductCommandResult.Produced -> {
                        require(commandResult.outputVariant == "RoutingIntentApplied") {
                            "Owner returned a different output for routing intent"
                        }
                        val output = JSONObject(commandResult.outputJson)
                        require(output.singleVariant() == "RoutingIntentApplied")
                        val observed = parseRoutingIntentObservation(
                            output.getJSONObject("RoutingIntentApplied").getJSONObject("observed"),
                        )
                        require(observed == requested.observation()) {
                            "Owner routing intent readback did not match the request"
                        }
                        RoutingIntentCommandResult.Applied(observed)
                    }
                    is ProductCommandResult.Accepted,
                    is ProductCommandResult.Completed -> throw IllegalArgumentException(
                        "Owner returned a non-terminal routing result",
                    )
                }
            }
        }
    }
} catch (error: Exception) {
    RoutingIntentCommandResult.Failed(
        OwnerFailure(
            OwnerErrorCode.InvalidState,
            error.message.orEmpty(),
            false,
            null,
        ),
    )
}

private fun staleRoutingIntentScopeFailure() = RoutingIntentCommandResult.Failed(
    OwnerFailure(
        OwnerErrorCode.InvalidState,
        "",
        false,
        null,
    ),
)

private fun sameCoreScope(expected: CoreScopeObservation, observed: CoreScopeObservation): Boolean =
    expected.identity == observed.identity &&
        expected.generation == observed.generation &&
        expected.sessionToken == observed.sessionToken
