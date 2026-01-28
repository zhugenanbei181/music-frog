package com.musicfrog.despicableinfiltrator.ui.overview

import android.content.Context
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.musicfrog.despicableinfiltrator.MihomoHost
import com.musicfrog.despicableinfiltrator.VpnStateManager
import com.musicfrog.despicableinfiltrator.ui.common.DEFAULT_FFI_TIMEOUT_MS
import com.musicfrog.despicableinfiltrator.ui.common.runFfiCall
import com.musicfrog.despicableinfiltrator.ui.common.userMessage
import infiltrator_android.FfiErrorCode
import infiltrator_android.LogEntry
import infiltrator_android.LogLevel
import infiltrator_android.TrafficSnapshot
import infiltrator_android.configPatchMode
import infiltrator_android.logsGet
import infiltrator_android.logsStartStreaming
import infiltrator_android.trafficSnapshot
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

data class OverviewUiState(
    val currentMode: String = "rule",
    val traffic: TrafficSnapshot? = null,
    val logs: List<LogEntry> = emptyList(),
    val trafficLoading: Boolean = true,
    val logsLoading: Boolean = true,
    val error: String? = null
)

class OverviewViewModel : ViewModel() {
    private val _state = MutableStateFlow(OverviewUiState())
    val state: StateFlow<OverviewUiState> = _state.asStateFlow()

    init {
        startTrafficPolling()
        startLogStreaming()
    }

    private fun startTrafficPolling() {
        viewModelScope.launch {
            while (true) {
                try {
                    val result = trafficSnapshot()
                    if (result.status.code == FfiErrorCode.OK && result.snapshot != null) {
                        _state.value = _state.value.copy(
                            traffic = result.snapshot,
                            trafficLoading = false
                        )
                    }
                } catch (e: Exception) {
                    // Ignore transient errors
                }
                delay(2000)
            }
        }
    }

    private fun startLogStreaming() {
        viewModelScope.launch {
            try {
                logsStartStreaming()
                _state.value = _state.value.copy(logsLoading = false)
                while (true) {
                    val result = logsGet(5u)
                    if (result.status.code == FfiErrorCode.OK) {
                        _state.value = _state.value.copy(
                            logs = result.entries.takeLast(5)
                        )
                    }
                    delay(3000)
                }
            } catch (e: Exception) {
                // Ignore
            }
        }
    }

    fun changeMode(mode: String, context: Context) {
        viewModelScope.launch {
            try {
                val call = runFfiCall(timeoutMs = DEFAULT_FFI_TIMEOUT_MS) {
                    configPatchMode(mode)
                }
                if (call.error != null) {
                    _state.value = _state.value.copy(error = call.error)
                    return@launch
                }
                val status = call.value!!
                if (status.code == FfiErrorCode.OK) {
                    _state.value = _state.value.copy(currentMode = mode, error = null)
                } else {
                    _state.value = _state.value.copy(error = status.userMessage("Failed to switch mode"))
                }
            } catch (e: Exception) {
                _state.value = _state.value.copy(error = "Failed to switch mode: ${e.message}")
            }
        }
    }

    fun restartCore(host: MihomoHost?) {
        viewModelScope.launch {
            if (host != null) {
                host.coreStop()
                if (host.coreStart()) {
                    VpnStateManager.updateCoreState(true)
                } else {
                    _state.value = _state.value.copy(error = "Failed to restart Core")
                    VpnStateManager.updateCoreState(false)
                }
            }
        }
    }

    fun toggleVpn(host: MihomoHost?) {
        val vpnState = VpnStateManager.vpnState.value
        val isStarting = vpnState == VpnStateManager.VpnState.STARTING
        val isStopping = vpnState == VpnStateManager.VpnState.STOPPING
        val isRunning = vpnState == VpnStateManager.VpnState.RUNNING

        if (!isStarting && !isStopping) {
            viewModelScope.launch {
                if (isRunning) {
                    if (host?.vpnStop() != true) {
                        _state.value = _state.value.copy(error = "Failed to stop VPN")
                    }
                } else {
                    if (host?.vpnStart() != true) {
                        _state.value = _state.value.copy(error = "Failed to start VPN")
                    }
                }
            }
        }
    }

    fun clearError() {
        _state.value = _state.value.copy(error = null)
        VpnStateManager.clearError()
    }
}
