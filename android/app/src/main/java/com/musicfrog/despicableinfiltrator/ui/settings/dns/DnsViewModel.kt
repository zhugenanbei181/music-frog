package com.musicfrog.despicableinfiltrator.ui.settings.dns

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.musicfrog.despicableinfiltrator.ui.common.DEFAULT_FFI_TIMEOUT_MS
import com.musicfrog.despicableinfiltrator.ui.common.LONG_FFI_TIMEOUT_MS
import com.musicfrog.despicableinfiltrator.ui.common.runFfiCall
import com.musicfrog.despicableinfiltrator.ui.common.userMessage
import infiltrator_android.DnsSettings
import infiltrator_android.DnsSettingsPatch
import infiltrator_android.FfiErrorCode
import infiltrator_android.dnsSettings
import infiltrator_android.dnsSettingsSave
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

data class DnsUiState(
    val enabled: Boolean = false,
    val ipv6: Boolean = false,
    val enhancedMode: String = "",
    val nameserver: String = "",
    val defaultNameserver: String = "",
    val fallback: String = "",
    val isLoading: Boolean = false,
    val error: String? = null,
    val saved: Boolean = false
)

class DnsViewModel : ViewModel() {
    private val _state = MutableStateFlow(DnsUiState(isLoading = true))
    val state: StateFlow<DnsUiState> = _state.asStateFlow()

    init {
        load()
    }

    fun load() {
        viewModelScope.launch {
            _state.value = _state.value.copy(isLoading = true, error = null, saved = false)
            val call = runFfiCall(timeoutMs = DEFAULT_FFI_TIMEOUT_MS) { dnsSettings() }
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
                    error = result.status.userMessage("Failed to load DNS settings")
                )
            }
        }
    }

    fun updateEnabled(value: Boolean) {
        _state.value = _state.value.copy(enabled = value, saved = false)
    }

    fun updateIpv6(value: Boolean) {
        _state.value = _state.value.copy(ipv6 = value, saved = false)
    }

    fun updateEnhancedMode(value: String) {
        _state.value = _state.value.copy(enhancedMode = value, saved = false)
    }

    fun updateNameserver(value: String) {
        _state.value = _state.value.copy(nameserver = value, saved = false)
    }

    fun updateDefaultNameserver(value: String) {
        _state.value = _state.value.copy(defaultNameserver = value, saved = false)
    }

    fun updateFallback(value: String) {
        _state.value = _state.value.copy(fallback = value, saved = false)
    }

    fun save() {
        viewModelScope.launch {
            val current = _state.value
            _state.value = current.copy(isLoading = true, error = null)

            val patch = DnsSettingsPatch(
                current.enabled,
                current.ipv6,
                current.enhancedMode.trim().ifBlank { null },
                parseList(current.nameserver),
                parseList(current.defaultNameserver),
                parseList(current.fallback)
            )

            val call = runFfiCall(timeoutMs = LONG_FFI_TIMEOUT_MS) { dnsSettingsSave(patch) }
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
                    error = result.status.userMessage("Failed to save DNS settings")
                )
            }
        }
    }

    fun clearError() {
        _state.value = _state.value.copy(error = null)
    }

    private fun applySettings(settings: DnsSettings) {
        _state.value = DnsUiState(
            enabled = settings.enable ?: false,
            ipv6 = settings.ipv6 ?: false,
            enhancedMode = settings.enhancedMode ?: "",
            nameserver = settings.nameserver.joinToString(separator = "\n"),
            defaultNameserver = settings.defaultNameserver.joinToString(separator = "\n"),
            fallback = settings.fallback.joinToString(separator = "\n"),
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
