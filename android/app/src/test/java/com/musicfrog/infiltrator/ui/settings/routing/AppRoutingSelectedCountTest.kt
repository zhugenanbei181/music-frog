package com.musicfrog.infiltrator.ui.settings.routing

import com.musicfrog.infiltrator.ui.common.AppFailure
import com.musicfrog.infiltrator.ui.common.AppFailureCode
import com.musicfrog.infiltrator.ui.common.FailureTextKey
import com.musicfrog.infiltrator.ui.common.ObservationRevision
import infiltrator_android.AndroidAppRoutingMode
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * 「已选 N」 is the installed-filtered selection, not the length of the set the
 * owner stores: a package that was selected and later uninstalled must not keep
 * inflating the number the bottom bar shows.
 */
class AppRoutingSelectedCountTest {
    private val inventory = listOf(
        AppItem("Installed", "org.example.installed", null, AppClassification.User),
        AppItem("System", "org.example.system", null, AppClassification.System),
    )

    private fun snapshot(packages: Set<String>) = AppRoutingSnapshotState.Observed(
        AppRoutingObservation(
            mode = AndroidAppRoutingMode.PROXY_SELECTED,
            selectedPackages = packages,
            ownerSessionId = "owner-session-1",
            revision = ObservationRevision.Owner(generation = 2uL, revision = 1uL),
        ),
    )

    @Test
    fun uninstalledSelectionIsNotCounted() {
        val count = selectedAppCount(
            snapshot(setOf("org.example.installed", "org.example.gone")),
            InstalledAppsState.Available(inventory),
        )

        assertEquals(SelectedAppCount.Known(installedSelected = 1), count)
    }

    @Test
    fun missingInventoryPublishesNoNumber() {
        val count = selectedAppCount(
            snapshot(setOf("org.example.installed")),
            InstalledAppsState.Loading(),
        )

        assertEquals(SelectedAppCount.InventoryUnknown, count)
    }

    @Test
    fun failedRefreshKeepsCountingAgainstTheLastAcceptedInventory() {
        val count = selectedAppCount(
            snapshot(setOf("org.example.installed", "org.example.system", "org.example.gone")),
            InstalledAppsState.Unavailable(
                AcceptedInstalledApps.Present(inventory),
                AppFailure(code = AppFailureCode.NotReady, localization = FailureTextKey.NotReady),
            ),
        )

        assertEquals(SelectedAppCount.Known(installedSelected = 2), count)
    }
}
