package com.musicfrog.infiltrator

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.ResultsReporter
import androidx.test.uiautomator.uiAutomator
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/** Exercises the installed flavor launcher and attaches an emulator screenshot. */
@RunWith(AndroidJUnit4::class)
class LauncherUiAutomatorTest {
    @Test
    fun launcherRendersAndReportsScreenshot() = uiAutomator {
        val packageName = InstrumentationRegistry
            .getInstrumentation()
            .targetContext
            .packageName

        startApp(packageName)
        waitForAppToBeVisible(packageName)
        activeWindow().waitForStable()

        val screenshot = activeWindow().takeScreenshot()
        assertTrue("launcher screenshot must have a visible surface", screenshot.width > 0)
        assertTrue("launcher screenshot must have a visible surface", screenshot.height > 0)

        val reporter = ResultsReporter("${BuildConfig.FLAVOR}-launcher")
        reporter.addNewFile(
            filename = "${BuildConfig.FLAVOR}-launcher.png",
            title = "${BuildConfig.FLAVOR} Android Emulator launcher",
        ).also(screenshot::saveToFile)
        reporter.reportToInstrumentation()
    }
}
