package com.musicfrog.despicableinfiltrator.ui.settings.routing

import android.content.Context
import android.content.pm.ApplicationInfo
import android.content.pm.PackageManager
import android.graphics.drawable.Drawable
import androidx.lifecycle.ViewModel
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
    val isSelected: Boolean
)

class AppRoutingViewModel(private val context: Context) : ViewModel() {
    private val _apps = MutableStateFlow<List<AppItem>>(emptyList())
    private val _searchQuery = MutableStateFlow("")
    
    // Mode: "allow" (Allowlist) or "block" (Blocklist)
    // For MVP, we'll default to "allow" mode: only selected apps go through VPN.
    // Or maybe "proxy_all" vs "proxy_selected". Let's stick to "Proxy Selected" for clarity.
    private val prefs = context.getSharedPreferences("app_routing", Context.MODE_PRIVATE)

    val uiState: StateFlow<List<AppItem>> = combine(_apps, _searchQuery) { apps, query ->
        if (query.isBlank()) apps else apps.filter { 
            it.name.contains(query, ignoreCase = true) || it.packageName.contains(query, ignoreCase = true)
        }
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5000), emptyList())

    init {
        loadApps()
    }

    fun loadApps() {
        viewModelScope.launch {
            val installedApps = withContext(Dispatchers.IO) {
                val pm = context.packageManager
                val packages = pm.getInstalledApplications(PackageManager.GET_META_DATA)
                val selectedSet = getSelectedPackages()
                
                packages.filter { it.flags and ApplicationInfo.FLAG_SYSTEM == 0 || it.packageName == "com.android.chrome" } // Filter system apps usually, keep Chrome for testing
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

    private fun getSelectedPackages(): Set<String> {
        return prefs.getStringSet("selected_packages", emptySet()) ?: emptySet()
    }

    private fun saveSelection(packageName: String, isSelected: Boolean) {
        val currentSet = getSelectedPackages().toMutableSet()
        if (isSelected) {
            currentSet.add(packageName)
        } else {
            currentSet.remove(packageName)
        }
        prefs.edit().putStringSet("selected_packages", currentSet).apply()
    }

    fun search(query: String) {
        _searchQuery.value = query
    }
}
