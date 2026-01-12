package com.musicfrog.despicableinfiltrator.ui.settings.fakeip

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.musicfrog.despicableinfiltrator.ui.common.DEFAULT_FFI_TIMEOUT_MS
import com.musicfrog.despicableinfiltrator.ui.common.LONG_FFI_TIMEOUT_MS
import com.musicfrog.despicableinfiltrator.ui.common.runFfiCall
import com.musicfrog.despicableinfiltrator.ui.common.userMessage
import infiltrator_android.FakeIpSettings
import infiltrator_android.FakeIpSettingsPatch
import infiltrator_android.FfiErrorCode
import infiltrator_android.fakeIpCacheClear
import infiltrator_android.fakeIpSettings
import infiltrator_android.fakeIpSettingsSave
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

data class FakeIpUiState(
    val fakeIpRange: String = "",
    val fakeIpFilter: String = "",
    val storeFakeIp: Boolean = false,
    val isLoading: Boolean = false,
    val error: String? = null,
    val saved: Boolean = false,
    val cacheMessage: String? = null
)

class FakeIpViewModel : ViewModel() {
    private val _state = MutableStateFlow(FakeIpUiState(isLoading = true))
    val state: StateFlow<FakeIpUiState> = _state.asStateFlow()

    init {
        load()
    }

    fun load() {
        viewModelScope.launch {
            _state.value = _state.value.copy(
                isLoading = true,
                error = null,
                saved = false,
                cacheMessage = null
            )
            val call = runFfiCall(timeoutMs = DEFAULT_FFI_TIMEOUT_MS) { fakeIpSettings() }
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
                    error = result.status.userMessage("Failed to load Fake-IP settings")
                )
            }
        }
    }

    fun updateRange(value: String) {
        _state.value = _state.value.copy(fakeIpRange = value, saved = false)
    }

    fun updateFilter(value: String) {
        _state.value = _state.value.copy(fakeIpFilter = value, saved = false)
    }

    fun updateStoreFakeIp(value: Boolean) {
        _state.value = _state.value.copy(storeFakeIp = value, saved = false)
    }

    fun save() {
        viewModelScope.launch {
            val current = _state.value
            _state.value = current.copy(isLoading = true, error = null, cacheMessage = null)

            val patch = FakeIpSettingsPatch(
                current.fakeIpRange.trim().ifBlank { null },
                parseList(current.fakeIpFilter),
                current.storeFakeIp
            )

            val call = runFfiCall(timeoutMs = LONG_FFI_TIMEOUT_MS) { fakeIpSettingsSave(patch) }
            if (call.error != null) {
                _state.value = _state.value.copy(isLoading = false, error = call.error)
                return@launch
            }
            val result = call.value!!
            val settings = result.settings
            if (result.status.code == FfiErrorCode.OK && settings != null) {
                applySettings(settings)
                _state.value = _state.value.copy(saved = true)
            } else {
                _state.value = _state.value.copy(
                    isLoading = false,
                    error = result.status.userMessage("Failed to save Fake-IP settings")
                )
            }
        }
    }

    fun clearCache() {
        viewModelScope.launch {
            _state.value = _state.value.copy(isLoading = true, error = null, cacheMessage = null)
            val call = runFfiCall(timeoutMs = LONG_FFI_TIMEOUT_MS) { fakeIpCacheClear() }
            if (call.error != null) {
                _state.value = _state.value.copy(isLoading = false, error = call.error)
                return@launch
            }
            val result = call.value!!
            if (result.status.code == FfiErrorCode.OK) {
                val message = if (result.value) {
                    "Fake-IP cache cleared"
                } else {
                    "Fake-IP cache already empty"
                }
                _state.value = _state.value.copy(
                    isLoading = false,
                    cacheMessage = message
                )
            } else {
                _state.value = _state.value.copy(
                    isLoading = false,
                    error = result.status.userMessage("Failed to clear Fake-IP cache")
                )
            }
        }
    }

    fun clearError() {
        _state.value = _state.value.copy(error = null)
    }

    fun clearCacheMessage() {
        _state.value = _state.value.copy(cacheMessage = null)
    }

    private fun applySettings(settings: FakeIpSettings) {
        _state.value = FakeIpUiState(
            fakeIpRange = settings.fakeIpRange ?: "",
            fakeIpFilter = settings.fakeIpFilter.joinToString(separator = "\n"),
            storeFakeIp = settings.storeFakeIp ?: false,
            isLoading = false,
            error = null,
            saved = false
        )
    }

    private fun parseList(raw: String): List<String> {
        return raw.split('\n', ',')
            .map { it.trim() }
            .filter { it.isNotEmpty() }
    }
}
