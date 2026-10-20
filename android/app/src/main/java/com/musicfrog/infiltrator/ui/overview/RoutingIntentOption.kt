package com.musicfrog.infiltrator.ui.overview

import com.musicfrog.infiltrator.ui.common.RoutingIntentObservation
import infiltrator_android.AndroidRoutingIntentCopyKey

/** Common cross-engine routing choices. Engine extensions are never implied here. */
internal enum class RoutingIntentOption(val copyKey: AndroidRoutingIntentCopyKey) {
    FollowRules(AndroidRoutingIntentCopyKey.FOLLOW_RULES),
    RouteAllThroughProxy(AndroidRoutingIntentCopyKey.ROUTE_ALL_THROUGH_PROXY),
    BypassProxy(AndroidRoutingIntentCopyKey.BYPASS_PROXY),
    ;

    internal fun observation(): RoutingIntentObservation = when (this) {
        FollowRules -> RoutingIntentObservation.FollowRules
        RouteAllThroughProxy -> RoutingIntentObservation.RouteAllThroughProxy
        BypassProxy -> RoutingIntentObservation.BypassProxy
    }

    internal fun serializedIntent(): String = when (this) {
        FollowRules -> "{\"kind\":\"follow_rules\"}"
        RouteAllThroughProxy -> "{\"kind\":\"route_all_through_proxy\"}"
        BypassProxy -> "{\"kind\":\"bypass_proxy\"}"
    }

    companion object {
        val commonChoices: List<RoutingIntentOption> = entries.toList()

        fun fromObserved(intent: RoutingIntentObservation?): RoutingIntentOption? = when (intent) {
            RoutingIntentObservation.FollowRules -> FollowRules
            RoutingIntentObservation.RouteAllThroughProxy -> RouteAllThroughProxy
            RoutingIntentObservation.BypassProxy -> BypassProxy
            is RoutingIntentObservation.EngineDefined, null -> null
        }
    }
}
