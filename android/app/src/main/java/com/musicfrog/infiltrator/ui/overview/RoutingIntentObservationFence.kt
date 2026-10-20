package com.musicfrog.infiltrator.ui.overview

/** Identifies one owner read relative to routing-intent mutations. */
internal data class RoutingIntentReadEpoch(val mutation: Long, val observation: Long)

/** Rejects owner reads that began before a newer read or routing-intent write. */
internal class RoutingIntentObservationFence {
    private var mutation: Long = 0
    private var observation: Long = 0

    @Synchronized
    fun beginRead(): RoutingIntentReadEpoch = RoutingIntentReadEpoch(
        mutation = mutation,
        observation = ++observation,
    )

    @Synchronized
    fun beginMutation() {
        mutation += 1
        observation += 1
    }

    @Synchronized
    fun finishMutation() {
        mutation += 1
        observation += 1
    }

    @Synchronized
    fun accepts(epoch: RoutingIntentReadEpoch): Boolean = epoch.mutation == mutation && epoch.observation == observation
}
