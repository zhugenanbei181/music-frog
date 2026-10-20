package com.musicfrog.infiltrator.ui.common

import org.json.JSONObject

/** Rust-authored routing policy observation, preserving engine-specific policies. */
internal sealed interface RoutingIntentObservation {
    data object FollowRules : RoutingIntentObservation
    data object RouteAllThroughProxy : RoutingIntentObservation
    data object BypassProxy : RoutingIntentObservation
    data class EngineDefined(val engineId: String, val extensionId: String) : RoutingIntentObservation
}

internal enum class RoutingIntentStatusObservation {
    Unobserved,
    Ready,
    Pending,
    Unsupported,
    Failed,
}

internal data class RoutingIntentSnapshotObservation(
    val current: RoutingIntentObservation?,
    val status: RoutingIntentStatusObservation,
    val failure: OwnerFailure?,
)

/** Parses the tagged Rust `RoutingIntent` enum; unknown variants fail closed. */
internal fun parseRoutingIntentObservation(value: JSONObject): RoutingIntentObservation = when (value.getString("kind")) {
    "follow_rules" -> RoutingIntentObservation.FollowRules

    "route_all_through_proxy" -> RoutingIntentObservation.RouteAllThroughProxy

    "bypass_proxy" -> RoutingIntentObservation.BypassProxy

    "engine_defined" -> {
        val extension = value.getJSONObject("extension")
        val engineId = extension.getString("engine")
        val extensionId = extension.getString("extension_id")
        require(engineId.isNotBlank() && extensionId.isNotBlank()) {
            "Owner engine-defined routing intent has an empty identifier"
        }
        RoutingIntentObservation.EngineDefined(
            engineId = engineId,
            extensionId = extensionId,
        )
    }

    else -> throw IllegalArgumentException("Unknown owner routing intent")
}

/** Parses the application-owned state projection without guessing from absence. */
internal fun parseRoutingIntentSnapshotObservation(value: JSONObject): RoutingIntentSnapshotObservation {
    require(value.has("current") && value.has("status") && value.has("failure")) {
        "Owner routing intent snapshot is incomplete"
    }
    val currentValue = value.opt("current")
    val current = when {
        currentValue == null || currentValue === JSONObject.NULL -> null
        currentValue is JSONObject -> parseRoutingIntentObservation(currentValue)
        else -> throw IllegalArgumentException("Owner routing intent current value is malformed")
    }
    val status = when (value.getString("status")) {
        "Unobserved" -> RoutingIntentStatusObservation.Unobserved
        "Ready" -> RoutingIntentStatusObservation.Ready
        "Pending" -> RoutingIntentStatusObservation.Pending
        "Unsupported" -> RoutingIntentStatusObservation.Unsupported
        "Failed" -> RoutingIntentStatusObservation.Failed
        else -> throw IllegalArgumentException("Unknown owner routing intent status")
    }
    val failureValue = value.opt("failure")
    val failure = when {
        failureValue == null || failureValue === JSONObject.NULL -> null
        failureValue is JSONObject -> parseOwnerFailure(failureValue)
        else -> throw IllegalArgumentException("Owner routing intent failure is malformed")
    }
    require(status != RoutingIntentStatusObservation.Ready || current != null) {
        "Owner marked routing intent ready without an observed value"
    }
    return RoutingIntentSnapshotObservation(current, status, failure)
}
