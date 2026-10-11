package com.musicfrog.infiltrator

import infiltrator_android.AndroidControlIntent
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class AndroidControlIntentWireTest {
    @Test
    fun rustControlIntentWireCodesRoundTripThroughKotlin() {
        val intents = listOf(
            AndroidControlIntent.START_PROXY,
            AndroidControlIntent.STOP_PROXY,
            AndroidControlIntent.RESUME_KERNEL,
            AndroidControlIntent.PAUSE_KERNEL,
            AndroidControlIntent.RESTART_KERNEL,
        )

        intents.forEach { intent ->
            assertEquals(intent, androidControlIntentFromWireCode(intent.toWireCode()))
        }
    }

    @Test
    fun unknownRustWireCodesAreRejected() {
        assertNull(androidControlIntentFromWireCode(-1))
        (5..11).forEach { code ->
            assertNull("Retired or unassigned control code $code", androidControlIntentFromWireCode(code))
        }
    }
}
