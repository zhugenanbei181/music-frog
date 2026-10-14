package com.musicfrog.infiltrator.ui.sync

import infiltrator_android.WebDavSettings
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class SyncStateTest {
    @Test
    fun acceptedInventoryRejectsOlderRevisionAndConflictingEqualRevision() {
        val current = form(settings = settings(), revision = SyncSettingsRevision("owner-1", 3u, 8u), sequence = 4u)

        assertEquals(
            SyncSnapshotAdmission.Stale,
            admitSyncSettings(current, snapshot(settings(), SyncSettingsRevision("owner-1", 3u, 7u)), 5u),
        )
        assertEquals(
            SyncSnapshotAdmission.Stale,
            admitSyncSettings(current, snapshot(settings(), SyncSettingsRevision("owner-1", 3u, 9u)), 3u),
        )
        assertEquals(
            SyncSnapshotAdmission.Conflicting,
            admitSyncSettings(
                current,
                snapshot(settings(url = "https://other.invalid"), SyncSettingsRevision("owner-1", 3u, 8u)),
                5u,
            ),
        )
    }

    @Test
    fun newOwnerSessionOrNewCoreGenerationCanBeAdoptedButOldGenerationCannot() {
        val current = form(settings = settings(), revision = SyncSettingsRevision("owner-1", 3u, 8u), sequence = 4u)

        assertEquals(
            SyncSnapshotAdmission.Accepted,
            admitSyncSettings(current, snapshot(settings(), SyncSettingsRevision("owner-2", 1u, 1u)), 5u),
        )
        assertEquals(
            SyncSnapshotAdmission.Accepted,
            admitSyncSettings(current, snapshot(settings(), SyncSettingsRevision("owner-1", 4u, 1u)), 5u),
        )
        assertEquals(
            SyncSnapshotAdmission.Stale,
            admitSyncSettings(current, snapshot(settings(), SyncSettingsRevision("owner-1", 2u, 99u)), 5u),
        )
    }

    @Test
    fun requestSequenceAndIntervalHaveExplicitTerminalValues() {
        assertEquals(
            SyncRequestAllocation.Allocated(SyncRequestSequence(5u)),
            allocateSyncRequest(4u),
        )
        assertEquals(SyncRequestAllocation.Exhausted, allocateSyncRequest(ULong.MAX_VALUE))
        assertEquals(SyncIntervalParse.Valid(60u), parseSyncInterval(" 60 "))
        assertTrue(parseSyncInterval("not a number") is SyncIntervalParse.Invalid)
    }

    @Test
    fun exactReadbackAndWorkflowActivityAreTyped() {
        val accepted = snapshot(settings(), SyncSettingsRevision("owner-1", 3u, 8u))
        assertEquals(SyncReadbackConfirmation.Confirmed, accepted.isReadbackOf(settings()))
        assertEquals(
            SyncReadbackConfirmation.Mismatch,
            accepted.isReadbackOf(settings(url = "https://different.invalid")),
        )
        assertEquals(
            SyncScreenActivity.Busy,
            SyncUiState(
                settings = SyncSettingsState.Loading(),
                action = SyncActionState.Working(SyncOperation.LoadSettings),
            ).screenActivity(),
        )
        assertEquals(
            SyncInputAvailability.Disabled,
            SyncUiState(settings = SyncSettingsState.Unavailable(null)).inputAvailability(),
        )
    }

    private fun form(
        settings: WebDavSettings,
        revision: SyncSettingsRevision,
        sequence: ULong,
    ) = SyncForm(
        snapshot(settings, revision),
        snapshot(settings, revision).toDraft(),
        sequence,
    )

    private fun snapshot(
        settings: WebDavSettings,
        revision: SyncSettingsRevision,
    ) = SyncSettingsSnapshot(settings, revision)

    private fun settings(url: String = "https://dav.example.invalid") = WebDavSettings(
        enabled = true,
        url = url,
        username = "alice",
        password = "secret",
        syncIntervalMins = 30u,
        syncOnStartup = false,
    )
}
