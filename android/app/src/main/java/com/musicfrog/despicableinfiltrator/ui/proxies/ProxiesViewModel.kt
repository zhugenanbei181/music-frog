package com.musicfrog.despicableinfiltrator.ui.proxies

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import infiltrator_android.FfiErrorCode
import infiltrator_android.ProxyGroupSummary
import infiltrator_android.proxiesGroups
import infiltrator_android.proxySelect
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch

class ProxiesViewModel : ViewModel() {
    private val _groups = MutableStateFlow<List<ProxyGroupSummary>>(emptyList())
    val groups: StateFlow<List<ProxyGroupSummary>> = _groups.asStateFlow()
    
    // Auto-refresh logic (e.g. every 2 seconds to see latency updates if available)
    init {
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

    fun loadGroups() {
        viewModelScope.launch {
            try {
                val result = proxiesGroups()
                if (result.status.code == FfiErrorCode.OK) {
                    _groups.value = result.groups
                }
            } catch (e: Exception) {
                // log error
            }
        }
    }

    fun selectProxy(group: String, server: String) {
        viewModelScope.launch {
            try {
                proxySelect(group, server)
                loadGroups() // Refresh immediately to show update
            } catch (e: Exception) {
                // log error
            }
        }
    }
}
