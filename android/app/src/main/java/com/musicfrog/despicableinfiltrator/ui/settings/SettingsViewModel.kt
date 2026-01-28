package com.musicfrog.despicableinfiltrator.ui.settings

import androidx.appcompat.app.AppCompatDelegate
import androidx.core.os.LocaleListCompat
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

class SettingsViewModel : ViewModel() {
    // Current locale tag: "en", "zh-CN", "zh-TW", or "system" (empty)
    private val _currentLocale = MutableStateFlow("system")
    val currentLocale: StateFlow<String> = _currentLocale.asStateFlow()

    init {
        val locales = AppCompatDelegate.getApplicationLocales()
        if (!locales.isEmpty) {
            _currentLocale.value = locales.toLanguageTags()
        }
    }

    fun setLocale(tag: String) {
        _currentLocale.value = tag
        val localeList = if (tag == "system") {
            LocaleListCompat.getEmptyLocaleList()
        } else {
            LocaleListCompat.forLanguageTags(tag)
        }
        AppCompatDelegate.setApplicationLocales(localeList)
    }
}