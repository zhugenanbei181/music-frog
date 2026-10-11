package com.musicfrog.infiltrator.ui.overview

import androidx.compose.runtime.Composable
import androidx.compose.ui.platform.LocalConfiguration
import infiltrator_android.AndroidRoutingIntentCopyKey
import infiltrator_android.androidEngineDefinedRoutingIntentCopy
import infiltrator_android.androidRoutingIntentCopy

@Composable
internal fun sharedRoutingIntentCopy(key: AndroidRoutingIntentCopyKey): String =
    androidRoutingIntentCopy(key, currentApplicationLanguage())

@Composable
internal fun sharedEngineDefinedRoutingIntentCopy(extensionId: String): String =
    androidEngineDefinedRoutingIntentCopy(currentApplicationLanguage(), extensionId)

@Composable
private fun currentApplicationLanguage(): String =
    LocalConfiguration.current.locales[0]?.toLanguageTag() ?: "en-US"
