package com.musicfrog.infiltrator.ui.settings.routing

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import infiltrator_android.AndroidAppRoutingMode
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch

internal class AppRoutingViewModel internal constructor(
    application: Application,
    repository: AppRoutingRepository,
    installedAppsProvider: InstalledAppsProvider,
) : AndroidViewModel(application) {
    constructor(application: Application) : this(
        application,
        UniFfiAppRoutingRepository,
        PackageManagerInstalledAppsProvider(application.packageManager),
    )

    private val session = AppRoutingSession(repository, installedAppsProvider)
    private val _searchQuery = MutableStateFlow("")

    val operation: StateFlow<AppRoutingOperationState> = session.operation
    val snapshot: StateFlow<AppRoutingSnapshotState> = session.snapshot
    val routingMode: StateFlow<AppRoutingModeState> = snapshot
        .map(::appRoutingModeState)
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), AppRoutingModeState.Unavailable)
    val uiState: StateFlow<List<AppItem>> = combine(session.installedApps, _searchQuery, snapshot) {
            installedApps,
            query,
            ownerSnapshot,
        ->
        visibleAppItems(installedApps, query, ownerSnapshot)
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    init {
        load()
    }

    fun load() {
        viewModelScope.launch { session.load() }
    }

    fun toggleApp(packageName: String) {
        viewModelScope.launch { session.toggleApp(packageName) }
    }

    fun selectAll(group: AppGroup) {
        viewModelScope.launch { session.selectAll(group) }
    }

    fun clearAll(group: AppGroup) {
        viewModelScope.launch { session.clearAll(group) }
    }

    fun setRoutingMode(mode: AndroidAppRoutingMode) {
        viewModelScope.launch { session.setRoutingMode(mode) }
    }

    fun search(query: String) {
        _searchQuery.value = query
    }
}
