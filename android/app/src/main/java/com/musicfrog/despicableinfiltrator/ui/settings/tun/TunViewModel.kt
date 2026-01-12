package com.musicfrog.despicableinfiltrator.ui.settings.tun

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import infiltrator_android.FfiErrorCode
import infiltrator_android.VpnTunSettings
import infiltrator_android.VpnTunSettingsPatch
import infiltrator_android.vpnTunSettings
import infiltrator_android.vpnTunSettingsSave
import com.musicfrog.despicableinfiltrator.ui.common.DEFAULT_FFI_TIMEOUT_MS
import com.musicfrog.despicableinfiltrator.ui.common.LONG_FFI_TIMEOUT_MS
import com.musicfrog.despicableinfiltrator.ui.common.runFfiCall
import com.musicfrog.despicableinfiltrator.ui.common.userMessage
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

data class TunUiState(
    val mtu: String = "",
    val autoRoute: Boolean = true,
    val strictRoute: Boolean = false,
    val ipv6: Boolean = false,
    val dnsServers: String = "",
    val isLoading: Boolean = false,
    val error: String? = null,
    val saved: Boolean = false
)

class TunViewModel : ViewModel() {
    private val _state = MutableStateFlow(TunUiState(isLoading = true))
    val state: StateFlow<TunUiState> = _state.asStateFlow()

    init {
        load()
    }

    fun load() {
        viewModelScope.launch {
            _state.value = _state.value.copy(isLoading = true, error = null, saved = false)
            try {
                val call = runFfiCall(timeoutMs = DEFAULT_FFI_TIMEOUT_MS) { vpnTunSettings() }
                if (call.error != null) {
                    _state.value = _state.value.copy(isLoading = false, error = call.error)
                } else {
                    val result = call.value!!
                    val settings = result.settings
                    if (result.status.code == FfiErrorCode.OK && settings != null) {
                        applySettings(settings)
                    } else {
                        _state.value = _state.value.copy(
                            isLoading = false,
                            error = result.status.userMessage("Failed to load TUN settings")
                        )
                    }
                }
            } catch (err: Exception) {
                _state.value = _state.value.copy(
                    isLoading = false,
                    error = err.message ?: "Failed to load TUN settings"
                )
            }
        }
    }

    fun updateMtu(value: String) {
        _state.value = _state.value.copy(mtu = value, saved = false)
    }

    fun updateAutoRoute(value: Boolean) {
        _state.value = _state.value.copy(autoRoute = value, saved = false)
    }

    fun updateStrictRoute(value: Boolean) {
        _state.value = _state.value.copy(strictRoute = value, saved = false)
    }

    fun updateIpv6(value: Boolean) {
        _state.value = _state.value.copy(ipv6 = value, saved = false)
    }

    fun updateDnsServers(value: String) {
        _state.value = _state.value.copy(dnsServers = value, saved = false)
    }

    fun save() {
        viewModelScope.launch {
            val current = _state.value
            _state.value = current.copy(isLoading = true, error = null)

            val mtuValue = parseMtu(current.mtu)
            if (current.mtu.isNotBlank() && mtuValue == null) {
                _state.value = current.copy(
                    isLoading = false,
                    error = "MTU must be a positive number"
                )
                return@launch
            }
            if (mtuValue != null && mtuValue == 0u) {
                _state.value = current.copy(
                    isLoading = false,
                    error = "MTU must be greater than 0"
                )
                return@launch
            }

            val dnsServers = parseDnsServers(current.dnsServers)
            val patch = VpnTunSettingsPatch(
                mtuValue,
                current.autoRoute,
                current.strictRoute,
                dnsServers,
                current.ipv6
            )

            try {
                val call = runFfiCall(timeoutMs = LONG_FFI_TIMEOUT_MS) {
                    vpnTunSettingsSave(patch)
                }
                if (call.error != null) {
                    _state.value = _state.value.copy(isLoading = false, error = call.error)
                } else {
                    val result = call.value!!
                    val settings = result.settings
                    if (result.status.code == FfiErrorCode.OK && settings != null) {
                        applySettings(settings)
                        _state.value = _state.value.copy(saved = true)
                    } else {
                        _state.value = _state.value.copy(
                            isLoading = false,
                            error = result.status.userMessage("Failed to save TUN settings")
                        )
                    }
                }
            } catch (err: Exception) {
                _state.value = _state.value.copy(
                    isLoading = false,
                    error = err.message ?: "Failed to save TUN settings"
                )
            }
        }
    }

    fun clearError() {
        _state.value = _state.value.copy(error = null)
    }

    private fun applySettings(settings: VpnTunSettings) {
        _state.value = TunUiState(
            mtu = settings.mtu?.toString() ?: "",
            autoRoute = settings.autoRoute ?: true,
            strictRoute = settings.strictRoute ?: false,
            ipv6 = settings.ipv6 ?: false,
            dnsServers = settings.dnsServers.joinToString(separator = "\n"),
            isLoading = false,
            error = null,
            saved = false
        )
    }

    private fun parseMtu(value: String): UInt? {
        val trimmed = value.trim()
        if (trimmed.isEmpty()) {
            return null
        }
        return trimmed.toUIntOrNull()
    }

    private fun parseDnsServers(raw: String): List<String> {
        return raw.split('\n', ',')
            .map { it.trim() }
            .filter { it.isNotEmpty() }
    }
}
