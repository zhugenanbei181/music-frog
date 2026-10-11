package com.musicfrog.infiltrator.ui.settings.routing

import android.graphics.drawable.Drawable
import com.musicfrog.infiltrator.ui.common.AppFailure
import com.musicfrog.infiltrator.ui.common.ObservationRevision
import infiltrator_android.AndroidAppRoutingMode

internal enum class AppClassification {
    User,
    System,
}

internal enum class AppPackageSelection {
    Selected,
    Unselected,
}

internal data class AppItem(
    val name: String,
    val packageName: String,
    val icon: Drawable?,
    val classification: AppClassification,
    val selection: AppPackageSelection = AppPackageSelection.Unselected,
)

internal enum class AppGroup {
    User,
    System,
}

internal data class AppRoutingObservation(
    val mode: AndroidAppRoutingMode,
    val selectedPackages: Set<String>,
    val ownerSessionId: String,
    val revision: ObservationRevision.Owner,
)

internal sealed interface AcceptedAppRoutingSnapshot {
    data object Missing : AcceptedAppRoutingSnapshot
    data class Present(val observation: AppRoutingObservation) : AcceptedAppRoutingSnapshot
}

internal sealed interface AppRoutingSnapshotState {
    data class Loading(
        val lastAccepted: AcceptedAppRoutingSnapshot = AcceptedAppRoutingSnapshot.Missing,
    ) : AppRoutingSnapshotState

    data class Observed(val observation: AppRoutingObservation) : AppRoutingSnapshotState

    data class Unavailable(
        val lastAccepted: AcceptedAppRoutingSnapshot,
    ) : AppRoutingSnapshotState
}

internal sealed interface InstalledAppsState {
    data class Loading(
        val lastAccepted: AcceptedInstalledApps = AcceptedInstalledApps.Missing,
    ) : InstalledAppsState
    data class Available(val apps: List<AppItem>) : InstalledAppsState
    data class Unavailable(
        val lastAccepted: AcceptedInstalledApps,
        val failure: AppFailure,
    ) : InstalledAppsState
}

internal sealed interface AcceptedInstalledApps {
    data object Missing : AcceptedInstalledApps
    data class Present(val apps: List<AppItem>) : AcceptedInstalledApps
}

internal sealed interface AppRoutingOperationState {
    data object Loading : AppRoutingOperationState
    data object Ready : AppRoutingOperationState
    data object Updating : AppRoutingOperationState
    data class Failed(val failure: AppFailure) : AppRoutingOperationState
    data class Cancelled(val failure: AppFailure) : AppRoutingOperationState
    data class TimedOut(val failure: AppFailure) : AppRoutingOperationState
}

internal sealed interface AppRoutingModeState {
    data object Unavailable : AppRoutingModeState
    data class Available(
        val mode: AndroidAppRoutingMode,
        val freshness: AppRoutingSnapshotFreshness,
    ) : AppRoutingModeState
}

internal enum class AppRoutingSnapshotFreshness {
    Current,
    Retained,
}

internal enum class AppRoutingSnapshotAdmission {
    Accepted,
    Stale,
    Conflict,
}

internal data class AppRoutingRequestId(val value: ULong)

internal enum class AppRoutingRequestAdmission {
    Current,
    Superseded,
}

internal enum class AppRoutingRequestKind {
    Refresh,
    Mutation,
}

internal fun AppRoutingSnapshotState.lastAccepted(): AcceptedAppRoutingSnapshot = when (this) {
    is AppRoutingSnapshotState.Loading -> lastAccepted
    is AppRoutingSnapshotState.Observed -> AcceptedAppRoutingSnapshot.Present(observation)
    is AppRoutingSnapshotState.Unavailable -> lastAccepted
}

internal fun AppRoutingSnapshotState.markUnavailable(): AppRoutingSnapshotState =
    AppRoutingSnapshotState.Unavailable(lastAccepted())

internal fun InstalledAppsState.lastAccepted(): AcceptedInstalledApps = when (this) {
    is InstalledAppsState.Loading -> lastAccepted
    is InstalledAppsState.Available -> AcceptedInstalledApps.Present(apps)
    is InstalledAppsState.Unavailable -> lastAccepted
}

internal fun admitAppRoutingSnapshot(
    current: AcceptedAppRoutingSnapshot,
    incoming: AppRoutingObservation,
): AppRoutingSnapshotAdmission = when (current) {
    AcceptedAppRoutingSnapshot.Missing -> AppRoutingSnapshotAdmission.Accepted
    is AcceptedAppRoutingSnapshot.Present -> {
        val previous = current.observation
        if (previous.ownerSessionId != incoming.ownerSessionId ||
            previous.revision.generation != incoming.revision.generation
        ) {
            AppRoutingSnapshotAdmission.Accepted
        } else {
            when {
                incoming.revision.revision < previous.revision.revision -> AppRoutingSnapshotAdmission.Stale
                incoming.revision.revision > previous.revision.revision -> AppRoutingSnapshotAdmission.Accepted
                previous.mode == incoming.mode && previous.selectedPackages == incoming.selectedPackages -> {
                    AppRoutingSnapshotAdmission.Accepted
                }
                else -> AppRoutingSnapshotAdmission.Conflict
            }
        }
    }
}

internal fun appRoutingModeState(state: AppRoutingSnapshotState): AppRoutingModeState = when (state) {
    is AppRoutingSnapshotState.Observed -> AppRoutingModeState.Available(
        state.observation.mode,
        AppRoutingSnapshotFreshness.Current,
    )
    is AppRoutingSnapshotState.Loading -> state.lastAccepted.toModeState()
    is AppRoutingSnapshotState.Unavailable -> state.lastAccepted.toModeState()
}

private fun AcceptedAppRoutingSnapshot.toModeState(): AppRoutingModeState = when (this) {
    AcceptedAppRoutingSnapshot.Missing -> AppRoutingModeState.Unavailable
    is AcceptedAppRoutingSnapshot.Present -> AppRoutingModeState.Available(
        observation.mode,
        AppRoutingSnapshotFreshness.Retained,
    )
}

internal fun visibleAppItems(
    installedApps: InstalledAppsState,
    query: String,
    snapshot: AppRoutingSnapshotState,
): List<AppItem> {
    val apps = when (installedApps) {
        is InstalledAppsState.Loading -> installedApps.lastAccepted.appsOrEmpty()
        is InstalledAppsState.Available -> installedApps.apps
        is InstalledAppsState.Unavailable -> installedApps.lastAccepted.appsOrEmpty()
    }
    val selectedPackages = when (val retained = snapshot.lastAccepted()) {
        AcceptedAppRoutingSnapshot.Missing -> emptySet()
        is AcceptedAppRoutingSnapshot.Present -> retained.observation.selectedPackages
    }
    return apps.asSequence()
        .map { app ->
            app.copy(
                selection = if (app.packageName in selectedPackages) {
                    AppPackageSelection.Selected
                } else {
                    AppPackageSelection.Unselected
                },
            )
        }
        .filter { app ->
            query.isBlank() || app.name.contains(query, ignoreCase = true) ||
                app.packageName.contains(query, ignoreCase = true)
        }
        .toList()
}

private fun AcceptedInstalledApps.appsOrEmpty(): List<AppItem> = when (this) {
    AcceptedInstalledApps.Missing -> emptyList()
    is AcceptedInstalledApps.Present -> apps
}
