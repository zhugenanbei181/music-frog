package com.musicfrog.infiltrator

import android.app.Activity
import android.os.Build
import android.os.Bundle
import android.os.ParcelFileDescriptor
import androidx.test.runner.AndroidJUnitRunner

/** Keeps Android tests on an isolated, offline emulator guest. */
class OfflineEmulatorTestRunner : AndroidJUnitRunner() {
    private var restoreAirplaneMode: Boolean? = null

    override fun onStart() {
        if (Build.HARDWARE !in EMULATOR_HARDWARE) {
            finish(
                Activity.RESULT_CANCELED,
                Bundle().apply {
                    putString(
                        STREAM_RESULT,
                        "Android instrumentation is restricted to Android Emulator; " +
                            "the test made no device or host network changes.",
                    )
                },
            )
            return
        }

        try {
            val previous = shell("settings get global airplane_mode_on")
            check(previous == "0" || previous == "1") {
                "Cannot determine emulator airplane mode; refusing to run tests."
            }
            restoreAirplaneMode = previous == "1"
            if (previous == "0") {
                shell("cmd connectivity airplane-mode enable")
                check(shell("settings get global airplane_mode_on") == "1") {
                    "Could not isolate emulator guest networking; refusing to run tests."
                }
            }
        } catch (error: Exception) {
            finish(
                Activity.RESULT_CANCELED,
                Bundle().apply {
                    putString(
                        STREAM_RESULT,
                        "Could not make the Android Emulator guest offline: ${error.message}",
                    )
                },
            )
            return
        }

        super.onStart()
    }

    override fun finish(resultCode: Int, results: Bundle) {
        var finalCode = resultCode
        val finalResults = Bundle(results)
        if (restoreAirplaneMode == false) {
            try {
                shell("cmd connectivity airplane-mode disable")
                check(shell("settings get global airplane_mode_on") == "0") {
                    "Could not restore the Android Emulator guest airplane mode."
                }
            } catch (error: Exception) {
                finalCode = Activity.RESULT_CANCELED
                finalResults.putString(
                    STREAM_RESULT,
                    "Android Emulator guest network restoration failed: ${error.message}",
                )
            }
        }
        super.finish(finalCode, finalResults)
    }

    private fun shell(command: String): String {
        val descriptor = uiAutomation.executeShellCommand(command)
        return ParcelFileDescriptor.AutoCloseInputStream(descriptor)
            .bufferedReader()
            .use { it.readText().trim() }
    }

    private companion object {
        const val STREAM_RESULT = "stream"
        val EMULATOR_HARDWARE = setOf("ranchu", "goldfish")
    }
}
