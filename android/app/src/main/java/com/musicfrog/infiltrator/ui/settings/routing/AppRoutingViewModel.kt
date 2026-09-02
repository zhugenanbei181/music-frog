package com.musicfrog.infiltrator.ui.settings.routing

import android.app.Application
import android.content.pm.ApplicationInfo
import android.content.pm.PackageManager
import android.graphics.drawable.Drawable
import android.util.Log
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import infiltrator_android.AppRoutingMode
import infiltrator_android.FfiErrorCode
import infiltrator_android.appRoutingLoad
import infiltrator_android.appRoutingSetMode
import infiltrator_android.appRoutingTogglePackage
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

private data class RoutingState(
    val mode: RoutingMode,
    val selectedPackages: Set<String>,
)

private const val TAG = "AppRoutingViewModel"

class AppRoutingViewModel(application: Application) : AndroidViewModel(application) {
    private val _apps = MutableStateFlow<List<AppItem>>(emptyList())
    private val _searchQuery = MutableStateFlow("")
    private val _routingMode = MutableStateFlow(RoutingMode.ProxyAll)
    private val _isLoading = MutableStateFlow(true)

    val isLoading: StateFlow<Boolean> = _isLoading

    val uiState: StateFlow<List<AppItem>> = combine(_apps, _searchQuery) { apps, query ->
        if (query.isBlank()) apps else apps.filter { 
            it.name.contains(query, ignoreCase = true) || it.packageName.contains(query, ignoreCase = true)
        }
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5000), emptyList())

    val routingMode: StateFlow<RoutingMode> = _routingMode

    init {
        loadApps()
    }

    fun loadApps() {
        viewModelScope.launch {
            _isLoading.value = true
            val routingState = loadRoutingState()
            _routingMode.value = routingState.mode
            val installedApps = withContext(Dispatchers.IO) {
                val pm = getApplication<Application>().packageManager
                val packages = pm.getInstalledApplications(PackageManager.GET_META_DATA)
                val selectedSet = routingState.selectedPackages
                
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
        if (_routingMode.value == RoutingMode.ProxyAll) {
            return
        }
        viewModelScope.launch {
            val result = withContext(Dispatchers.IO) {
                appRoutingTogglePackage(packageName)
            }
            if (result.status.code != FfiErrorCode.OK) {
                Log.w(
                    TAG,
                    "toggle package failed: ${result.status.code} ${result.status.message.orEmpty()}",
                )
                loadApps()
                return@launch
            }

            val currentList = _apps.value.toMutableList()
            val index = currentList.indexOfFirst { it.packageName == packageName }
            if (index >= 0) {
                currentList[index] = currentList[index].copy(isSelected = result.isSelected)
                _apps.value = currentList
            }
        }
    }

    fun setRoutingMode(mode: RoutingMode) {
        viewModelScope.launch {
            val status = withContext(Dispatchers.IO) {
                appRoutingSetMode(mode.toFfiMode())
            }
            if (status.code == FfiErrorCode.OK) {
                _routingMode.value = mode
                return@launch
            }

            Log.w(
                TAG,
                "set routing mode failed: ${status.code} ${status.message.orEmpty()}",
            )
            _routingMode.value = loadRoutingState().mode
        }
    }

    fun search(query: String) {
        _searchQuery.value = query
    }

    private suspend fun loadRoutingState(): RoutingState {
        return withContext(Dispatchers.IO) {
            try {
                val result = appRoutingLoad()
                val config = result.config
                if (result.status.code == FfiErrorCode.OK && config != null) {
                    RoutingState(config.mode.toUiMode(), config.packages.toSet())
                } else {
                    Log.w(
                        TAG,
                        "load routing config failed: ${result.status.code} ${result.status.message.orEmpty()}",
                    )
                    RoutingState(RoutingMode.ProxyAll, emptySet())
                }
            } catch (err: Exception) {
                Log.w(TAG, "load routing config exception: ${err.message}", err)
                RoutingState(RoutingMode.ProxyAll, emptySet())
            }
        }
    }
}

private fun AppRoutingMode.toUiMode(): RoutingMode {
    return when (this) {
        AppRoutingMode.PROXY_ALL -> RoutingMode.ProxyAll
        AppRoutingMode.PROXY_SELECTED -> RoutingMode.ProxySelected
        AppRoutingMode.BYPASS_SELECTED -> RoutingMode.BypassSelected
    }
}

private fun RoutingMode.toFfiMode(): AppRoutingMode {
    return when (this) {
        RoutingMode.ProxyAll -> AppRoutingMode.PROXY_ALL
        RoutingMode.ProxySelected -> AppRoutingMode.PROXY_SELECTED
        RoutingMode.BypassSelected -> AppRoutingMode.BYPASS_SELECTED
    }
}
