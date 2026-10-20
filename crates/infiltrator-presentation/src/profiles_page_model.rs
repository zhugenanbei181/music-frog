//! The one typed page model of the profiles page, shared by every surface.
//!
//! The profiles page owns ten owner operations: loading the list, importing
//! a subscription, saving and updating a subscription, synchronising and
//! testing a WebDAV account, saving the application settings, loading and
//! applying a sync diff, and computing an aggregation. Iced wrote one phase
//! enum per operation, Bevy wrote its own shape for the same operations, and
//! Slint wrote no phase at all.
//!
//! This module owns those ten phases. Each operation is one typed phase
//! machine with its own token, so a late answer moves exactly one phase and
//! the page can answer "what is the profiles page doing" the same way on
//! every surface.

use crate::operation_phase::OperationPhase;
use lurker_core_contract::error::Failure;

/// The profiles page operations the shared model tracks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfilesOperation {
    /// Load the profile list.
    LoadList,
    /// Import a subscription from a URL or from a local file.
    Import,
    /// Save the subscription schedule and fetch settings.
    SaveSubscription,
    /// Refresh a subscription profile.
    UpdateSubscription,
    /// Synchronise the profile store with the WebDAV account.
    SyncWebDav,
    /// Test the WebDAV credentials and the connection.
    TestWebDav,
    /// Save the application-level settings.
    SaveAppSettings,
    /// Load the WebDAV sync diff.
    LoadSyncDiff,
    /// Resolve and commit the WebDAV sync diff.
    ApplySyncDiff,
    /// Compute an aggregation template preview.
    Aggregate,
}

impl ProfilesOperation {
    /// Every operation, in presentation order.
    pub const ALL: [Self; 10] = [
        Self::LoadList,
        Self::Import,
        Self::SaveSubscription,
        Self::UpdateSubscription,
        Self::SyncWebDav,
        Self::TestWebDav,
        Self::SaveAppSettings,
        Self::LoadSyncDiff,
        Self::ApplySyncDiff,
        Self::Aggregate,
    ];

    /// Position of the operation in [`Self::ALL`].
    pub const fn index(self) -> usize {
        match self {
            Self::LoadList => 0,
            Self::Import => 1,
            Self::SaveSubscription => 2,
            Self::UpdateSubscription => 3,
            Self::SyncWebDav => 4,
            Self::TestWebDav => 5,
            Self::SaveAppSettings => 6,
            Self::LoadSyncDiff => 7,
            Self::ApplySyncDiff => 8,
            Self::Aggregate => 9,
        }
    }
}

/// The one page model of the profiles page.
///
/// A surface renders the profiles page from this model alone: the list
/// filter, the visible profiles and every operation phase come from here,
/// and the surface only dispatches typed intents back.
#[derive(Clone, Debug, Default)]
pub struct ProfilesPageModel {
    filter: String,
    load_list: OperationPhase,
    import: OperationPhase,
    save_subscription: OperationPhase,
    update_subscription: OperationPhase,
    sync_web_dav: OperationPhase,
    test_web_dav: OperationPhase,
    save_app_settings: OperationPhase,
    load_sync_diff: OperationPhase,
    apply_sync_diff: OperationPhase,
    aggregate: OperationPhase,
}

impl ProfilesPageModel {
    /// The filter the page applies to the profile list.
    pub fn filter(&self) -> &str {
        &self.filter
    }

    /// Edit the filter the page applies to the profile list.
    pub fn edit_filter(&mut self, filter: &str) {
        self.filter = filter.trim().to_owned();
    }

    /// The profiles the page shows, in the order the owner published them.
    ///
    /// The filter matches the profile name and its kind; a profile the filter
    /// does not match is not shown.
    pub fn visible_profiles<'a, P: ProfileFilterTarget>(&self, profiles: &'a [P]) -> Vec<&'a P> {
        let query = self.filter.to_ascii_lowercase();
        if query.is_empty() {
            return profiles.iter().collect();
        }
        profiles
            .iter()
            .filter(|profile| {
                profile.profile_name().to_ascii_lowercase().contains(&query)
                    || profile.profile_kind().to_ascii_lowercase().contains(&query)
            })
            .collect()
    }

    /// The number of profiles the filter hides.
    pub fn hidden_profile_count<P: ProfileFilterTarget>(&self, profiles: &[P]) -> usize {
        profiles.len() - self.visible_profiles(profiles).len()
    }

    /// The phase of one operation.
    pub fn operation(&self, operation: ProfilesOperation) -> &OperationPhase {
        match operation {
            ProfilesOperation::LoadList => &self.load_list,
            ProfilesOperation::Import => &self.import,
            ProfilesOperation::SaveSubscription => &self.save_subscription,
            ProfilesOperation::UpdateSubscription => &self.update_subscription,
            ProfilesOperation::SyncWebDav => &self.sync_web_dav,
            ProfilesOperation::TestWebDav => &self.test_web_dav,
            ProfilesOperation::SaveAppSettings => &self.save_app_settings,
            ProfilesOperation::LoadSyncDiff => &self.load_sync_diff,
            ProfilesOperation::ApplySyncDiff => &self.apply_sync_diff,
            ProfilesOperation::Aggregate => &self.aggregate,
        }
    }

    /// The phase of one operation, for the fold that changes it.
    pub fn operation_mut(&mut self, operation: ProfilesOperation) -> &mut OperationPhase {
        match operation {
            ProfilesOperation::LoadList => &mut self.load_list,
            ProfilesOperation::Import => &mut self.import,
            ProfilesOperation::SaveSubscription => &mut self.save_subscription,
            ProfilesOperation::UpdateSubscription => &mut self.update_subscription,
            ProfilesOperation::SyncWebDav => &mut self.sync_web_dav,
            ProfilesOperation::TestWebDav => &mut self.test_web_dav,
            ProfilesOperation::SaveAppSettings => &mut self.save_app_settings,
            ProfilesOperation::LoadSyncDiff => &mut self.load_sync_diff,
            ProfilesOperation::ApplySyncDiff => &mut self.apply_sync_diff,
            ProfilesOperation::Aggregate => &mut self.aggregate,
        }
    }

    /// Whether any operation of the page is still in flight.
    pub fn is_busy(&self) -> bool {
        ProfilesOperation::ALL
            .iter()
            .any(|operation| self.operation(*operation).is_running())
    }

    /// The operations of the page that are still in flight.
    pub fn running_operations(&self) -> Vec<ProfilesOperation> {
        ProfilesOperation::ALL
            .into_iter()
            .filter(|operation| self.operation(*operation).is_running())
            .collect()
    }

    /// The failure of one operation, when it failed.
    pub fn operation_failure(&self, operation: ProfilesOperation) -> Option<&Failure> {
        self.operation(operation).failure()
    }
}

/// The facts one profile exposes to the shared list filter.
///
/// A surface implements this for its own profile view; the filter itself
/// stays in the shared model.
pub trait ProfileFilterTarget {
    /// The profile's display name.
    fn profile_name(&self) -> &str;

    /// The profile's kind, as the owner published it.
    fn profile_kind(&self) -> &str;
}

#[cfg(test)]
#[path = "profiles_page_model_tests.rs"]
mod tests;
