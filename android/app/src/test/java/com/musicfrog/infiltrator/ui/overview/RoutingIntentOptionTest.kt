package com.musicfrog.infiltrator.ui.overview

import com.musicfrog.infiltrator.ui.common.RoutingIntentObservation
import infiltrator_android.AndroidRoutingIntentCopyKey
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class RoutingIntentOptionTest {
    @Test
    fun selectableLabelsUseSharedTypedCopyKeys() {
        assertEquals(AndroidRoutingIntentCopyKey.FOLLOW_RULES, RoutingIntentOption.FollowRules.copyKey)
        assertEquals(
            AndroidRoutingIntentCopyKey.ROUTE_ALL_THROUGH_PROXY,
            RoutingIntentOption.RouteAllThroughProxy.copyKey,
        )
        assertEquals(AndroidRoutingIntentCopyKey.BYPASS_PROXY, RoutingIntentOption.BypassProxy.copyKey)
    }

    @Test
    fun onlyTheSharedRoutingPoliciesAreSelectableByTheNativeUi() {
        assertEquals(
            RoutingIntentOption.FollowRules,
            RoutingIntentOption.fromObserved(RoutingIntentObservation.FollowRules),
        )
        assertEquals(
            RoutingIntentOption.RouteAllThroughProxy,
            RoutingIntentOption.fromObserved(RoutingIntentObservation.RouteAllThroughProxy),
        )
        assertEquals(
            RoutingIntentOption.BypassProxy,
            RoutingIntentOption.fromObserved(RoutingIntentObservation.BypassProxy),
        )
        assertNull(
            RoutingIntentOption.fromObserved(
                RoutingIntentObservation.EngineDefined("mihomo", "script"),
            ),
        )
    }
}
