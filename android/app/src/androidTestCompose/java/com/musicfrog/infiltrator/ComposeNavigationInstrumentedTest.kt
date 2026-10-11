package com.musicfrog.infiltrator

import androidx.compose.ui.test.assertTextEquals
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/** Verifies the installed Compose APK can navigate between its main sections. */
@RunWith(AndroidJUnit4::class)
class ComposeNavigationInstrumentedTest {
    @get:Rule
    val composeRule = createAndroidComposeRule<MainActivity>()

    @Test
    fun mainNavigationUpdatesTheVisibleScreenTitle() {
        composeRule.onNodeWithTag("screen-title")
            .assertTextEquals(composeRule.activity.getString(R.string.nav_overview))

        composeRule.onNodeWithTag("nav_profiles").performClick()
        composeRule.onNodeWithTag("screen-title")
            .assertTextEquals(composeRule.activity.getString(R.string.nav_profiles))

        composeRule.onNodeWithTag("nav_settings").performClick()
        composeRule.onNodeWithTag("screen-title")
            .assertTextEquals(composeRule.activity.getString(R.string.nav_settings))
    }
}
