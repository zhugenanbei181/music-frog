package com.musicfrog.infiltrator.ui.settings.rules

import infiltrator_android.AndroidRuleEntry
import com.musicfrog.infiltrator.ui.common.FailureTextKey
import com.musicfrog.infiltrator.ui.common.localizedFailure
import com.musicfrog.infiltrator.ui.common.toAppFailure
import org.junit.Assert.assertEquals
import org.junit.Test

class RulesStateTest {
    @Test
    fun emptyDefaultsDoNotClaimTheOwnerHasNoRules() {
        val state = RulesUiState()

        assertEquals(RulesObservationAvailability.Unavailable, state.rulesAvailability)
        assertEquals(RulesObservationAvailability.Unavailable, state.providersAvailability)
    }

    @Test
    fun observedEmptySnapshotIsDistinctFromUnavailable() {
        val observed = RulesUiState(
            rules = RulesReadState.Observed(RulesSnapshot(emptyList(), revision(1u))),
            providers = RuleProvidersReadState.Observed(RuleProvidersSnapshot(emptyList(), "{}", revision(1u))),
        )
        val unavailable = RulesUiState(
            rules = RulesReadState.Unavailable(null, failure()),
            providers = RuleProvidersReadState.Unavailable(null, failure()),
        )

        assertEquals(RulesObservationAvailability.Available, observed.rulesAvailability)
        assertEquals(RulesObservationAvailability.Available, observed.providersAvailability)
        assertEquals(RulesObservationAvailability.Unavailable, unavailable.rulesAvailability)
        assertEquals(RulesObservationAvailability.Unavailable, unavailable.providersAvailability)
    }

    @Test
    fun revisionAdmissionRejectsStaleSnapshotsButAllowsNewOwnerGeneration() {
        val current = RulesSnapshot(listOf(AndroidRuleEntry("DOMAIN,example.com,DIRECT", true)), revision(8u))

        assertEquals(RulesSnapshotAdmission.Stale, admitsRulesRevision(current, revision(7u)))
        assertEquals(RulesSnapshotAdmission.Accepted, admitsRulesRevision(current, revision(9u)))
        assertEquals(
            RulesSnapshotAdmission.Accepted,
            admitsRulesRevision(current, revision(1u, generation = 5u)),
        )
        assertEquals(
            RulesSnapshotAdmission.Accepted,
            admitsRulesRevision(current, revision(1u, session = "owner-b")),
        )
    }

    @Test
    fun reloadIsBlockedWhileEitherDomainHasUnsavedChanges() {
        val rulesChanged = RulesUiState(
            rules = RulesReadState.Observed(RulesSnapshot(emptyList(), revision(1u))),
            rulesDraft = listOf(AndroidRuleEntry("MATCH,DIRECT", true)),
        )
        val providersChanged = RulesUiState(
            providers = RuleProvidersReadState.Observed(RuleProvidersSnapshot(emptyList(), "{}", revision(1u))),
            providersDraft = "{\"provider\":{}}",
        )

        assertEquals(RulesDraftStatus.Changed, rulesChanged.rulesDraftStatus)
        assertEquals(RulesReloadAvailability.UnsavedChanges, rulesChanged.reloadAvailability)
        assertEquals(RulesDraftStatus.Changed, providersChanged.providersDraftStatus)
        assertEquals(RulesReloadAvailability.UnsavedChanges, providersChanged.reloadAvailability)
    }

    private fun revision(
        dataRevision: ULong,
        generation: ULong = 2u,
        session: String = "owner-a",
    ) = RulesRevision(session, generation, dataRevision)

    private fun failure() = localizedFailure(FailureTextKey.NotReady).toAppFailure()
}
