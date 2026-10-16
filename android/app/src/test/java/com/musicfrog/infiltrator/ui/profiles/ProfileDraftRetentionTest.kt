package com.musicfrog.infiltrator.ui.profiles

import infiltrator_android.AndroidOwnerOperation
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class ProfileDraftRetentionTest {

    private class NoOpProfilesRepository : ProfilesRepository {
        override suspend fun execute(
            operation: AndroidOwnerOperation,
            timeoutMs: Long,
        ): ProfilesRepositoryResult = error("Not invoked in draft test")
    }

    private fun createViewModel(): ProfilesViewModel {
        val testScope = CoroutineScope(Dispatchers.Unconfined + SupervisorJob())
        return ProfilesViewModel(NoOpProfilesRepository(), testScope)
    }

    @Test
    fun draftsAreRetainedInViewModelStateAcrossReconstruction() {
        val viewModel = createViewModel()

        // User opens add dialog with URL
        viewModel.openAddDialog("https://example.com/subscription.yaml")
        var drafts = viewModel.state.value.drafts
        assertTrue(drafts.showAddDialog)
        assertEquals("https://example.com/subscription.yaml", drafts.prefilledUrl)

        // User sets local import file draft with file content
        val sampleContent = "port: 7890\nproxies:\n  - name: test\n    type: ss"
        viewModel.setLocalImportDraft(LocalImportDraft("test-profile", sampleContent))

        // Activity recreation preserves ViewModel instance:
        // Screen re-attaches to the same viewModel and observes retained drafts
        drafts = viewModel.state.value.drafts
        assertTrue(drafts.showAddDialog)
        assertEquals("https://example.com/subscription.yaml", drafts.prefilledUrl)
        assertNotNull(drafts.localImportDraft)
        assertEquals("test-profile", drafts.localImportDraft?.suggestedName)
        assertEquals(sampleContent, drafts.localImportDraft?.content)

        // Dismissing add dialog clears prefilled url
        viewModel.closeAddDialog()
        assertFalse(viewModel.state.value.drafts.showAddDialog)
        assertNull(viewModel.state.value.drafts.prefilledUrl)
    }

    @Test
    fun deepLinkPrefillsUrlWithoutPersistingCredentials() {
        val viewModel = createViewModel()

        // Deep link provides URL
        val deepLinkUrl = "https://partner.example.com/clash/config"
        viewModel.openAddDialog(deepLinkUrl)

        assertEquals(deepLinkUrl, viewModel.state.value.drafts.prefilledUrl)
        assertTrue(viewModel.state.value.drafts.showAddDialog)

        // After import or dismiss, prefilled URL is cleared
        viewModel.closeAddDialog()
        assertNull(viewModel.state.value.drafts.prefilledUrl)
    }
}
