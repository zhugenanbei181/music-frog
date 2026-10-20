package com.musicfrog.infiltrator.ui.settings.routing

import com.musicfrog.infiltrator.ui.common.AppFailure
import com.musicfrog.infiltrator.ui.common.DEFAULT_FFI_TIMEOUT_MS
import com.musicfrog.infiltrator.ui.common.FailureTextKey
import com.musicfrog.infiltrator.ui.common.LONG_FFI_TIMEOUT_MS
import com.musicfrog.infiltrator.ui.common.localizedFailure
import com.musicfrog.infiltrator.ui.common.toAppFailure
import infiltrator_android.AndroidAppRoutingMode
import infiltrator_android.AndroidAppRoutingPackageSelection
import infiltrator_android.AndroidOwnerOperation
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withTimeoutOrNull

internal class AppRoutingSession(private val repository: AppRoutingRepository, private val installedAppsProvider: InstalledAppsProvider) {
    private val _installedApps = MutableStateFlow<InstalledAppsState>(InstalledAppsState.Loading())
    private val _snapshot = MutableStateFlow<AppRoutingSnapshotState>(AppRoutingSnapshotState.Loading())
    private val _operation = MutableStateFlow<AppRoutingOperationState>(AppRoutingOperationState.Loading)
    private val operationMutex = Mutex()
    private var requestSequence = AppRoutingRequestId(0uL)
    private var latestRequestSequence = requestSequence

    val installedApps: StateFlow<InstalledAppsState> = _installedApps.asStateFlow()
    val snapshot: StateFlow<AppRoutingSnapshotState> = _snapshot.asStateFlow()
    val operation: StateFlow<AppRoutingOperationState> = _operation.asStateFlow()

    suspend fun load() {
        val requestId = beginRequest()
        _operation.value = AppRoutingOperationState.Loading
        _installedApps.value = InstalledAppsState.Loading(_installedApps.value.lastAccepted())

        when (val apps = loadInstalledApps()) {
            is InstalledAppsLoadResult.Loaded -> {
                if (requestAdmission(requestId) == AppRoutingRequestAdmission.Current) {
                    _installedApps.value = InstalledAppsState.Available(apps.apps)
                }
            }

            is InstalledAppsLoadResult.Failed -> {
                if (requestAdmission(requestId) == AppRoutingRequestAdmission.Current) {
                    _installedApps.value = InstalledAppsState.Unavailable(
                        _installedApps.value.lastAccepted(),
                        apps.failure,
                    )
                    _snapshot.value = AppRoutingSnapshotState.Unavailable(_snapshot.value.lastAccepted())
                    _operation.value = AppRoutingOperationState.Failed(apps.failure)
                }
                return
            }

            is InstalledAppsLoadResult.TimedOut -> {
                if (requestAdmission(requestId) == AppRoutingRequestAdmission.Current) {
                    _installedApps.value = InstalledAppsState.Unavailable(
                        _installedApps.value.lastAccepted(),
                        apps.failure,
                    )
                    _snapshot.value = AppRoutingSnapshotState.Unavailable(_snapshot.value.lastAccepted())
                    _operation.value = AppRoutingOperationState.TimedOut(apps.failure)
                }
                return
            }
        }
        executeOwnerOperation(
            operation = AndroidOwnerOperation.AppRoutingSnapshot,
            timeoutMs = DEFAULT_FFI_TIMEOUT_MS,
            requestId = requestId,
            kind = AppRoutingRequestKind.Refresh,
        )
    }

    suspend fun setRoutingMode(mode: AndroidAppRoutingMode) {
        if (_operation.value != AppRoutingOperationState.Ready) return
        executeOwnerOperation(
            operation = AndroidOwnerOperation.AppRoutingModeSet(mode),
            timeoutMs = DEFAULT_FFI_TIMEOUT_MS,
            requestId = beginRequest(),
            kind = AppRoutingRequestKind.Mutation,
        )
    }

    suspend fun toggleApp(packageName: String) {
        if (_operation.value != AppRoutingOperationState.Ready) return
        val observation = when (val accepted = _snapshot.value.lastAccepted()) {
            AcceptedAppRoutingSnapshot.Missing -> return
            is AcceptedAppRoutingSnapshot.Present -> accepted.observation
        }
        if (observation.mode == AndroidAppRoutingMode.PROXY_ALL) return
        val installedPackages = when (val accepted = _installedApps.value.lastAccepted()) {
            AcceptedInstalledApps.Missing -> return
            is AcceptedInstalledApps.Present -> accepted.apps.map(AppItem::packageName).toSet()
        }
        if (packageName !in installedPackages) return
        val selection = if (packageName in observation.selectedPackages) {
            AndroidAppRoutingPackageSelection.UNSELECTED
        } else {
            AndroidAppRoutingPackageSelection.SELECTED
        }
        mutatePackages(listOf(packageName), selection)
    }

    suspend fun selectAll(group: AppGroup) = setGroupSelection(
        group,
        AndroidAppRoutingPackageSelection.SELECTED,
    )

    suspend fun clearAll(group: AppGroup) = setGroupSelection(
        group,
        AndroidAppRoutingPackageSelection.UNSELECTED,
    )

    private suspend fun setGroupSelection(group: AppGroup, selection: AndroidAppRoutingPackageSelection) {
        if (_operation.value != AppRoutingOperationState.Ready) return
        val observation = when (val accepted = _snapshot.value.lastAccepted()) {
            AcceptedAppRoutingSnapshot.Missing -> return
            is AcceptedAppRoutingSnapshot.Present -> accepted.observation
        }
        if (observation.mode == AndroidAppRoutingMode.PROXY_ALL) return
        val apps = when (val accepted = _installedApps.value.lastAccepted()) {
            AcceptedInstalledApps.Missing -> return
            is AcceptedInstalledApps.Present -> accepted.apps
        }
        val packages = apps.asSequence()
            .filter { app ->
                when (group) {
                    AppGroup.User -> app.classification == AppClassification.User
                    AppGroup.System -> app.classification == AppClassification.System
                }
            }
            .map(AppItem::packageName)
            .toList()
        mutatePackages(packages, selection)
    }

    private suspend fun mutatePackages(packages: List<String>, selection: AndroidAppRoutingPackageSelection) {
        if (_operation.value != AppRoutingOperationState.Ready || packages.isEmpty()) return
        executeOwnerOperation(
            operation = AndroidOwnerOperation.AppRoutingPackagesSet(packages, selection),
            timeoutMs = LONG_FFI_TIMEOUT_MS,
            requestId = beginRequest(),
            kind = AppRoutingRequestKind.Mutation,
        )
    }

    private suspend fun executeOwnerOperation(
        operation: AndroidOwnerOperation,
        timeoutMs: Long,
        requestId: AppRoutingRequestId,
        kind: AppRoutingRequestKind,
    ) {
        if (requestAdmission(requestId) == AppRoutingRequestAdmission.Superseded) return
        _operation.value = when (kind) {
            AppRoutingRequestKind.Refresh -> AppRoutingOperationState.Loading
            AppRoutingRequestKind.Mutation -> AppRoutingOperationState.Updating
        }
        when (kind) {
            AppRoutingRequestKind.Refresh -> {
                _snapshot.value = AppRoutingSnapshotState.Loading(_snapshot.value.lastAccepted())
            }

            AppRoutingRequestKind.Mutation -> Unit
        }

        val result = operationMutex.withLock {
            when (requestAdmission(requestId)) {
                AppRoutingRequestAdmission.Current -> repository.execute(operation, timeoutMs)
                AppRoutingRequestAdmission.Superseded -> null
            }
        } ?: return
        if (requestAdmission(requestId) == AppRoutingRequestAdmission.Superseded) return

        when (result) {
            is AppRoutingRepositoryResult.Completed -> applyObservation(result.observation)
            is AppRoutingRepositoryResult.Failed -> fail(result.failure, result.latestSnapshot)
            is AppRoutingRepositoryResult.Cancelled -> cancel(result.failure, result.latestSnapshot)
            is AppRoutingRepositoryResult.TimedOut -> timeout(result.failure)
        }
    }

    private fun applyObservation(incoming: AppRoutingObservation) {
        when (admitAppRoutingSnapshot(_snapshot.value.lastAccepted(), incoming)) {
            AppRoutingSnapshotAdmission.Accepted -> {
                _snapshot.value = AppRoutingSnapshotState.Observed(incoming)
                _operation.value = AppRoutingOperationState.Ready
            }

            AppRoutingSnapshotAdmission.Stale,
            AppRoutingSnapshotAdmission.Conflict,
            -> fail(
                localizedFailure(FailureTextKey.InvalidState).toAppFailure(),
                AcceptedAppRoutingSnapshot.Missing,
            )
        }
    }

    private fun fail(failure: AppFailure, latestSnapshot: AcceptedAppRoutingSnapshot) {
        retain(latestSnapshot)
        _snapshot.value = _snapshot.value.markUnavailable()
        _operation.value = AppRoutingOperationState.Failed(failure)
    }

    private fun cancel(failure: AppFailure, latestSnapshot: AcceptedAppRoutingSnapshot) {
        retain(latestSnapshot)
        _snapshot.value = _snapshot.value.markUnavailable()
        _operation.value = AppRoutingOperationState.Cancelled(failure)
    }

    private fun timeout(failure: AppFailure) {
        _snapshot.value = _snapshot.value.markUnavailable()
        _operation.value = AppRoutingOperationState.TimedOut(failure)
    }

    private fun retain(candidate: AcceptedAppRoutingSnapshot) {
        when (candidate) {
            AcceptedAppRoutingSnapshot.Missing -> Unit

            is AcceptedAppRoutingSnapshot.Present -> when (
                admitAppRoutingSnapshot(_snapshot.value.lastAccepted(), candidate.observation)
            ) {
                AppRoutingSnapshotAdmission.Accepted -> {
                    _snapshot.value = AppRoutingSnapshotState.Observed(candidate.observation)
                }

                AppRoutingSnapshotAdmission.Stale,
                AppRoutingSnapshotAdmission.Conflict,
                -> Unit
            }
        }
    }

    private suspend fun loadInstalledApps(): InstalledAppsLoadResult = try {
        when (
            val apps = withTimeoutOrNull(DEFAULT_FFI_TIMEOUT_MS) {
                installedAppsProvider.loadInstalledApps()
            }
        ) {
            null -> InstalledAppsLoadResult.TimedOut(localizedFailure(FailureTextKey.NotReady).toAppFailure())
            else -> InstalledAppsLoadResult.Loaded(apps)
        }
    } catch (error: CancellationException) {
        throw error
    } catch (error: Exception) {
        InstalledAppsLoadResult.Failed(error.toAppFailure())
    }

    private fun beginRequest(): AppRoutingRequestId {
        requestSequence = AppRoutingRequestId(requestSequence.value + 1uL)
        latestRequestSequence = requestSequence
        return requestSequence
    }

    private fun requestAdmission(requestId: AppRoutingRequestId): AppRoutingRequestAdmission = when (
        requestId == latestRequestSequence
    ) {
        true -> AppRoutingRequestAdmission.Current
        false -> AppRoutingRequestAdmission.Superseded
    }

    private sealed interface InstalledAppsLoadResult {
        data class Loaded(val apps: List<AppItem>) : InstalledAppsLoadResult
        data class Failed(val failure: AppFailure) : InstalledAppsLoadResult
        data class TimedOut(val failure: AppFailure) : InstalledAppsLoadResult
    }
}
