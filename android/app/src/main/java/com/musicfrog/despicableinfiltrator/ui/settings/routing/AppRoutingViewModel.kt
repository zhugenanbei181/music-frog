package com.musicfrog.despicableinfiltrator.ui.settings.routing

import android.app.Application
import android.content.Context
import android.content.pm.ApplicationInfo
import android.content.pm.PackageManager
import android.graphics.drawable.Drawable
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
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
    val isSystem: Boolean,
    val isSelected: Boolean
)

enum class RoutingMode(val value: String) {
    ProxyAll("proxy_all"),
    ProxySelected("proxy_selected"),
    BypassSelected("bypass_selected")
}

private const val PREFS_NAME = "app_routing"
private const val PREF_SELECTED_PACKAGES = "selected_packages"
private const val PREF_ROUTING_MODE = "routing_mode"

class AppRoutingViewModel(application: Application) : AndroidViewModel(application) {
    private val _apps = MutableStateFlow<List<AppItem>>(emptyList())
    private val _searchQuery = MutableStateFlow("")
    private val _routingMode = MutableStateFlow(RoutingMode.ProxyAll)
    private val _isLoading = MutableStateFlow(true)
    
    private val prefs = application.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)

    val isLoading: StateFlow<Boolean> = _isLoading

    val uiState: StateFlow<List<AppItem>> = combine(_apps, _searchQuery) { apps, query ->
        if (query.isBlank()) apps else apps.filter { 
            it.name.contains(query, ignoreCase = true) || it.packageName.contains(query, ignoreCase = true)
        }
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5000), emptyList())

    val routingMode: StateFlow<RoutingMode> = _routingMode

    init {
        loadSettings()
        loadApps()
    }

    private fun loadSettings() {
        val modeStr = prefs.getString(PREF_ROUTING_MODE, RoutingMode.ProxyAll.value)
        _routingMode.value = RoutingMode.values().find { it.value == modeStr } ?: RoutingMode.ProxyAll
    }

    fun loadApps() {
        viewModelScope.launch {
            _isLoading.value = true
            val installedApps = withContext(Dispatchers.IO) {
                val pm = getApplication<Application>().packageManager
                val packages = pm.getInstalledApplications(PackageManager.GET_META_DATA)
                val selectedSet = getSelectedPackages()
                
                packages.map { appInfo ->
                        AppItem(
                            name = pm.getApplicationLabel(appInfo).toString(),
                            packageName = appInfo.packageName,
                            icon = pm.getApplicationIcon(appInfo),
                            isSystem = (appInfo.flags and ApplicationInfo.FLAG_SYSTEM) != 0,
                            isSelected = selectedSet.contains(appInfo.packageName)
                        )
                    }.sortedBy { it.name.lowercase() }
            }
            _apps.value = installedApps
            _isLoading.value = false
        }
    }

    fun toggleApp(packageName: String) {
        val currentList = _apps.value.toMutableList()
        val index = currentList.indexOfFirst { it.packageName == packageName }
        if (index != -1) {
            val item = currentList[index]
            val newItem = item.copy(isSelected = !item.isSelected)
            currentList[index] = newItem
            _apps.value = currentList
            saveSelection(packageName, newItem.isSelected)
        }
    }

    fun setRoutingMode(mode: RoutingMode) {
        _routingMode.value = mode
        prefs.edit().putString(PREF_ROUTING_MODE, mode.value).apply()
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