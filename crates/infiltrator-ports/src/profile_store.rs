//! Runtime-neutral profile/configuration persistence port.

use crate::error::PortError;
use crate::profile_workspace::{ProfileWorkspace, ProfileWorkspaceUpdate};
use async_trait::async_trait;
use infiltrator_contract::aggregator::AggregationTemplate;
use infiltrator_contract::apply_transaction::ApplyTransactionSnapshot;
use infiltrator_contract::capability::Capability;
use infiltrator_contract::core_identity::EngineId;
use infiltrator_contract::profile_document_identity::{ProfileId, ProfileWorkspaceIdentity};
use infiltrator_domain::profiles::{ProfileInfo, ProfileMetadata};
use std::path::PathBuf;

/// Profile persistence operations needed by application/inbound surfaces.
///
/// The implementation may use a filesystem, database, or platform sync store;
/// callers receive only domain values and owned strings. No `ConfigManager`,
/// keyring implementation, Tokio channel, or controller type crosses this
/// boundary.
#[async_trait]
pub trait ProfileStore: Send + Sync {
    /// Namespace of every operation in this store, including string-keyed
    /// legacy domain metadata, the active pointer, credentials and backups.
    fn engine_id(&self) -> EngineId;
    /// Filesystem root already scoped to `engine_id()`.
    fn config_dir(&self) -> PathBuf;
    /// A prior actual transaction from this host instance for this profile, if observed.
    fn apply_transaction(&self, _profile: &str) -> Option<ApplyTransactionSnapshot> {
        None
    }

    async fn list_profiles(&self) -> Result<Vec<ProfileInfo>, PortError>;
    async fn get_current(&self) -> Result<String, PortError>;
    async fn set_current(&self, profile: &str) -> Result<(), PortError>;
    async fn load(&self, profile: &str) -> Result<String, PortError>;
    async fn save(&self, profile: &str, content: &str) -> Result<(), PortError>;
    async fn delete_profile(&self, profile: &str) -> Result<(), PortError>;
    async fn get_profile_metadata(&self, profile: &str) -> Result<ProfileMetadata, PortError>;
    async fn update_profile_metadata(
        &self,
        profile: &str,
        metadata: &ProfileMetadata,
    ) -> Result<(), PortError>;
    async fn delete_subscription_credential(&self, profile: &str) -> Result<(), PortError>;
    /// Both documents must be read under the same persistence boundary.
    async fn load_workspace(&self, _profile: &ProfileId) -> Result<ProfileWorkspace, PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "coherent profile editing is unavailable",
        ))
    }
    async fn load_active_workspace(&self) -> Result<ProfileWorkspace, PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "coherent active profile editing is unavailable",
        ))
    }
    /// Compare before any write. Failed publication restores prior bytes or
    /// surfaces a typed rollback failure. This persists files only; applying a running core is a
    /// separate managed-runtime transaction.
    async fn compare_and_save_workspace(
        &self,
        _expected: &ProfileWorkspaceIdentity,
        _update: &ProfileWorkspaceUpdate,
    ) -> Result<ProfileWorkspace, PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "source-bound profile editing is unavailable",
        ))
    }
    /// Active-profile check belongs to the same comparison/publication
    /// boundary. Managed hosts must use this for a running-core edit.
    async fn compare_and_save_active_workspace(
        &self,
        _expected: &ProfileWorkspaceIdentity,
        _update: &ProfileWorkspaceUpdate,
    ) -> Result<ProfileWorkspace, PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "source-bound active profile editing is unavailable",
        ))
    }
    async fn compare_and_save_inactive_workspace(
        &self,
        _expected: &ProfileWorkspaceIdentity,
        _update: &ProfileWorkspaceUpdate,
    ) -> Result<ProfileWorkspace, PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "source-bound inactive profile editing is unavailable",
        ))
    }
    /// Restore exact prior bytes only while this transaction's published
    /// source is still current. Refuse to overwrite later edits.
    async fn restore_workspace(
        &self,
        _expected: &ProfileWorkspaceIdentity,
        _previous: &ProfileWorkspace,
    ) -> Result<ProfileWorkspace, PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "source-bound profile recovery is unavailable",
        ))
    }
    /// DUAL-08-13: load the saved aggregation templates, empty when none where
    /// stored. Stores without a template sidecar answer with a typed
    /// unsupported instead of pretending the library is empty.
    async fn load_aggregation_templates(&self) -> Result<Vec<AggregationTemplate>, PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "this profile store keeps no aggregation-template sidecar",
        ))
    }
    /// DUAL-08-13: persist the aggregation template library; an empty library
    /// removes the sidecar.
    async fn save_aggregation_templates(
        &self,
        _templates: &[AggregationTemplate],
    ) -> Result<(), PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "this profile store keeps no aggregation-template sidecar",
        ))
    }
    async fn clear_backup(&self, profile: &str) -> Result<(), PortError>;
    async fn restore_backup(&self, profile: &str) -> Result<bool, PortError>;
}
