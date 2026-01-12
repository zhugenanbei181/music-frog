package com.musicfrog.despicableinfiltrator.ui.sync

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.musicfrog.despicableinfiltrator.ui.common.DEFAULT_FFI_TIMEOUT_MS
import com.musicfrog.despicableinfiltrator.ui.common.LONG_FFI_TIMEOUT_MS
import com.musicfrog.despicableinfiltrator.ui.common.runFfiCall
import com.musicfrog.despicableinfiltrator.ui.common.userMessage
import infiltrator_android.FfiErrorCode
import infiltrator_android.WebDavSettings
import infiltrator_android.webdavSettings
import infiltrator_android.webdavSettingsSave
import infiltrator_android.webdavSyncNow
import infiltrator_android.webdavTest
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

data class SyncUiState(
    val enabled: Boolean = false,
    val url: String = "",
    val username: String = "",
    val password: String = "",
    val syncInterval: String = "60",
    val syncOnStartup: Boolean = false,
    val isLoading: Boolean = false,
    val error: String? = null,
    val saved: Boolean = false,
    val testMessage: String? = null,
    val syncSummary: String? = null
)

class SyncViewModel : ViewModel() {
    private val _state = MutableStateFlow(SyncUiState(isLoading = true))
    val state: StateFlow<SyncUiState> = _state.asStateFlow()

    init {
        load()
    }

    fun load() {
        viewModelScope.launch {
            _state.value = _state.value.copy(
                isLoading = true,
                error = null,
                saved = false,
                testMessage = null,
                syncSummary = null
            )
            val call = runFfiCall(timeoutMs = DEFAULT_FFI_TIMEOUT_MS) { webdavSettings() }
            if (call.error != null) {
                _state.value = _state.value.copy(isLoading = false, error = call.error)
                return@launch
            }
            val result = call.value!!
            val settings = result.settings
            if (result.status.code == FfiErrorCode.OK && settings != null) {
                applySettings(settings)
            } else {
                _state.value = _state.value.copy(
                    isLoading = false,
                    error = result.status.userMessage("Failed to load WebDAV settings")
                )
            }
        }
    }

    fun updateEnabled(value: Boolean) {
        _state.value = _state.value.copy(enabled = value, saved = false)
    }

    fun updateUrl(value: String) {
        _state.value = _state.value.copy(url = value, saved = false)
    }

    fun updateUsername(value: String) {
        _state.value = _state.value.copy(username = value, saved = false)
    }

    fun updatePassword(value: String) {
        _state.value = _state.value.copy(password = value, saved = false)
    }

    fun updateSyncInterval(value: String) {
        _state.value = _state.value.copy(syncInterval = value, saved = false)
    }

    fun updateSyncOnStartup(value: Boolean) {
        _state.value = _state.value.copy(syncOnStartup = value, saved = false)
    }

    fun save() {
        viewModelScope.launch {
            val current = _state.value
            _state.value = current.copy(isLoading = true, error = null, testMessage = null)

            val interval = parseInterval(current.syncInterval)
            if (interval == null) {
                _state.value = current.copy(
                    isLoading = false,
                    error = "Sync interval must be a number"
                )
                return@launch
            }

            val settingsToSave = WebDavSettings(
                current.enabled,
                current.url,
                current.username,
                current.password,
                interval,
                current.syncOnStartup
            )

            val call = runFfiCall(timeoutMs = LONG_FFI_TIMEOUT_MS) { webdavSettingsSave(settingsToSave) }
            if (call.error != null) {
                _state.value = _state.value.copy(isLoading = false, error = call.error)
                return@launch
            }
            val result = call.value!!
            val savedSettings = result.settings
            if (result.status.code == FfiErrorCode.OK && savedSettings != null) {
                applySettings(savedSettings)
                _state.value = _state.value.copy(saved = true)
            } else {
                _state.value = _state.value.copy(
                    isLoading = false,
                    error = result.status.userMessage("Failed to save WebDAV settings")
                )
            }
        }
    }

    fun testConnection() {
        viewModelScope.launch {
            val current = _state.value
            _state.value = current.copy(isLoading = true, error = null, testMessage = null)

            val interval = parseInterval(current.syncInterval)
            if (interval == null) {
                _state.value = current.copy(
                    isLoading = false,
                    error = "Sync interval must be a number"
                )
                return@launch
            }

            val settings = WebDavSettings(
                current.enabled,
                current.url,
                current.username,
                current.password,
                interval,
                current.syncOnStartup
            )

            val call = runFfiCall(timeoutMs = LONG_FFI_TIMEOUT_MS) { webdavTest(settings) }
            if (call.error != null) {
                _state.value = _state.value.copy(isLoading = false, error = call.error)
                return@launch
            }
            val status = call.value!!
            if (status.code == FfiErrorCode.OK) {
                _state.value = _state.value.copy(
                    isLoading = false,
                    testMessage = "Connection OK"
                )
            } else {
                _state.value = _state.value.copy(
                    isLoading = false,
                    error = status.userMessage("WebDAV connection failed")
                )
            }
        }
    }

    fun syncNow() {
        viewModelScope.launch {
            _state.value = _state.value.copy(isLoading = true, error = null, syncSummary = null)
            val call = runFfiCall(timeoutMs = LONG_FFI_TIMEOUT_MS) { webdavSyncNow() }
            if (call.error != null) {
                _state.value = _state.value.copy(isLoading = false, error = call.error)
                return@launch
            }
            val result = call.value!!
            if (result.status.code == FfiErrorCode.OK) {
                val summary = "Sync: ${result.successCount}/${result.totalActions} ok, ${result.failedCount} failed"
                _state.value = _state.value.copy(
                    isLoading = false,
                    syncSummary = summary
                )
            } else {
                _state.value = _state.value.copy(
                    isLoading = false,
                    error = result.status.userMessage("WebDAV sync failed")
                )
            }
        }
    }

    fun clearError() {
        _state.value = _state.value.copy(error = null)
    }

    private fun applySettings(settings: WebDavSettings) {
        _state.value = SyncUiState(
            enabled = settings.enabled,
            url = settings.url,
            username = settings.username,
            password = settings.password,
            syncInterval = settings.syncIntervalMins.toString(),
            syncOnStartup = settings.syncOnStartup,
            isLoading = false,
            error = null,
            saved = false
        )
    }

    private fun parseInterval(raw: String): UInt? {
        val trimmed = raw.trim()
        if (trimmed.isEmpty()) {
            return null
        }
        return trimmed.toUIntOrNull()
    }
}
