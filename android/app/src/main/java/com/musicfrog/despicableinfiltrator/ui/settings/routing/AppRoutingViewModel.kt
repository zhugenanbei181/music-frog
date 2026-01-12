package com.musicfrog.despicableinfiltrator.ui.settings.routing

import android.content.Context
import android.content.pm.ApplicationInfo
import android.content.pm.PackageManager
import android.graphics.drawable.Drawable
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import infiltrator_android.AppRoutingMode as FfiRoutingMode
import infiltrator_android.appRoutingLoad
import infiltrator_android.appRoutingSave
import infiltrator_android.appRoutingSetMode
import infiltrator_android.appRoutingTogglePackage
import infiltrator_android.FfiErrorCode
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

data class AppItem(
    val name: String,
    val packageName: String,
    val icon: Drawable?,
    val isSelected: Boolean
)

enum class RoutingMode {
    ProxyAll,
    ProxySelected,
}

private const val PREFS_NAME = "app_routing"
private const val PREF_SELECTED_PACKAGES = "selected_packages"
private const val PREF_ROUTING_MODE = "routing_mode"
private const val ROUTING_MODE_PROXY_ALL = "proxy_all"
private const val ROUTING_MODE_PROXY_SELECTED = "proxy_selected"

class AppRoutingViewModel(private val context: Context) : ViewModel() {
    private val _apps = MutableStateFlow<List<AppItem>>(emptyList())
    private val _searchQuery = MutableStateFlow("")
    private val _routingMode = MutableStateFlow(RoutingMode.ProxyAll)
    private val prefs = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)

    val uiState: StateFlow<List<AppItem>> = combine(_apps, _searchQuery) { apps, query ->
        if (query.isBlank()) apps else apps.filter { 
            it.name.contains(query, ignoreCase = true) || it.packageName.contains(query, ignoreCase = true)
        }
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5000), emptyList())

    val routingMode: StateFlow<RoutingMode> = _routingMode

    init {
        loadFromRust()
        loadApps()
    }

    private fun loadFromRust() {
        viewModelScope.launch {
            withContext(Dispatchers.IO) {
                val result = appRoutingLoad()
                if (result.status.code == FfiErrorCode.OK && result.config != null) {
                    val config = result.config!!
                    _routingMode.value = when (config.mode) {
                        FfiRoutingMode.PROXY_SELECTED -> RoutingMode.ProxySelected
                        else -> RoutingMode.ProxyAll
                    }
                    // Store to SharedPreferences for VPN Service to read
                    val stored = when (config.mode) {
                        FfiRoutingMode.PROXY_SELECTED -> ROUTING_MODE_PROXY_SELECTED
                        else -> ROUTING_MODE_PROXY_ALL
                    }
                    prefs.edit()
                        .putString(PREF_ROUTING_MODE, stored)
                        .putStringSet(PREF_SELECTED_PACKAGES, config.packages.toSet())
                        .apply()
                }
            }
        }
    }

    fun loadApps() {
        viewModelScope.launch {
            val installedApps = withContext(Dispatchers.IO) {
                val pm = context.packageManager
                val packages = pm.getInstalledApplications(PackageManager.GET_META_DATA)
                val selectedSet = getSelectedPackages()
                
                packages.filter { it.flags and ApplicationInfo.FLAG_SYSTEM == 0 || it.packageName == "com.android.chrome" }
                    .map { appInfo ->
                        AppItem(
                            name = pm.getApplicationLabel(appInfo).toString(),
                            packageName = appInfo.packageName,
                            icon = pm.getApplicationIcon(appInfo),
                            isSelected = selectedSet.contains(appInfo.packageName)
                        )
                    }.sortedBy { it.name }
            }
            _apps.value = installedApps
        }
    }

    fun toggleApp(packageName: String) {
        viewModelScope.launch {
            withContext(Dispatchers.IO) {
                val result = appRoutingTogglePackage(packageName)
                if (result.status.code == FfiErrorCode.OK) {
                    // Update local state
                    val currentList = _apps.value.toMutableList()
                    val index = currentList.indexOfFirst { it.packageName == packageName }
                    if (index != -1) {
                        val item = currentList[index]
                        val newItem = item.copy(isSelected = result.isSelected)
                        currentList[index] = newItem
                        _apps.value = currentList
                    }
                    // Also update SharedPreferences for VPN Service
                    saveSelection(packageName, result.isSelected)
                }
            }
        }
    }

    fun setRoutingMode(mode: RoutingMode) {
        viewModelScope.launch {
            withContext(Dispatchers.IO) {
                val ffiMode = when (mode) {
                    RoutingMode.ProxyAll -> FfiRoutingMode.PROXY_ALL
                    RoutingMode.ProxySelected -> FfiRoutingMode.PROXY_SELECTED
                }
                val result = appRoutingSetMode(ffiMode)
                if (result.code == FfiErrorCode.OK) {
                    _routingMode.value = mode
                    // Also update SharedPreferences for VPN Service
                    val stored = when (mode) {
                        RoutingMode.ProxyAll -> ROUTING_MODE_PROXY_ALL
                        RoutingMode.ProxySelected -> ROUTING_MODE_PROXY_SELECTED
                    }
                    prefs.edit().putString(PREF_ROUTING_MODE, stored).apply()
                }
            }
        }
    }

    private fun getSelectedPackages(): Set<String> {
        return prefs.getStringSet(PREF_SELECTED_PACKAGES, emptySet()) ?: emptySet()
    }

    private fun saveSelection(packageName: String, isSelected: Boolean) {
        val currentSet = getSelectedPackages().toMutableSet()
        if (isSelected) {
            currentSet.add(packageName)
        } else {
            currentSet.remove(packageName)
        }
        prefs.edit().putStringSet(PREF_SELECTED_PACKAGES, currentSet).apply()
    }

    fun search(query: String) {
        _searchQuery.value = query
    }
}
