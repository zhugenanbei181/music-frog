package com.musicfrog.despicableinfiltrator.ui.proxies

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import infiltrator_android.ProxyGroupSummary
import infiltrator_android.proxiesGroups
import infiltrator_android.proxySelect
import com.musicfrog.despicableinfiltrator.ui.common.DEFAULT_FFI_TIMEOUT_MS
import com.musicfrog.despicableinfiltrator.ui.common.emptyMessage
import com.musicfrog.despicableinfiltrator.ui.common.runFfiCall
import com.musicfrog.despicableinfiltrator.ui.common.userMessage
import infiltrator_android.FfiErrorCode
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch

class ProxiesViewModel : ViewModel() {
    private val _groups = MutableStateFlow<List<ProxyGroupSummary>>(emptyList())
    val groups: StateFlow<List<ProxyGroupSummary>> = _groups.asStateFlow()

    private val _isLoading = MutableStateFlow(false)
    val isLoading: StateFlow<Boolean> = _isLoading.asStateFlow()

    private val _error = MutableStateFlow<String?>(null)
    val error: StateFlow<String?> = _error.asStateFlow()

    private val _emptyMessage = MutableStateFlow<String?>(null)
    val emptyMessage: StateFlow<String?> = _emptyMessage.asStateFlow()
    
    // Auto-refresh logic (e.g. every 2 seconds to see latency updates if available)
    init {
        loadGroups(showLoading = true)
        startRefreshLoop()
    }

    private fun startRefreshLoop() {
        viewModelScope.launch {
            while (isActive) {
                loadGroups()
                delay(3000)
            }
        }
    }

    fun loadGroups(showLoading: Boolean = false) {
        viewModelScope.launch {
            if (showLoading) {
                _isLoading.value = true
            }
            _emptyMessage.value = null
            try {
                val call = runFfiCall(timeoutMs = DEFAULT_FFI_TIMEOUT_MS) { proxiesGroups() }
                if (call.error != null) {
                    _error.value = call.error
                } else {
                    val result = call.value!!
                    if (result.status.code == FfiErrorCode.OK) {
                        _groups.value = result.groups
                        _error.value = null
                        _emptyMessage.value = if (result.groups.isEmpty()) {
                            emptyMessage("proxy groups")
                        } else {
                            null
                        }
                    } else {
                        _error.value = result.status.userMessage("Failed to load proxy groups")
                    }
                }
            } catch (e: Exception) {
                _error.value = e.message
            } finally {
                if (showLoading) {
                    _isLoading.value = false
                }
            }
        }
    }

    fun selectProxy(group: String, server: String) {
        viewModelScope.launch {
            try {
                val call = runFfiCall { proxySelect(group, server) }
                if (call.error != null) {
                    _error.value = call.error
                } else {
                    val status = call.value!!
                    if (status.code == FfiErrorCode.OK) {
                        loadGroups(showLoading = true)
                    } else {
                        _error.value = status.userMessage("Failed to switch proxy")
                    }
                }
            } catch (e: Exception) {
                _error.value = e.message
            }
        }
    }

    fun clearError() {
        _error.value = null
    }
}
