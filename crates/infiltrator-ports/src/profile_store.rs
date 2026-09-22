//! Runtime-neutral profile/configuration persistence port.

use crate::error::PortError;
use async_trait::async_trait;
use infiltrator_contract::capability::Capability;
use infiltrator_domain::profile_options::ProfileOptions;
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
    fn config_dir(&self) -> PathBuf;
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
    /// DUAL-07-08: load a profile's option sidecar (filter + mixin). A missing
    /// sidecar is the default (empty) options, not an error.
    async fn load_options(&self, profile: &str) -> Result<ProfileOptions, PortError>;
    /// DUAL-07-08: persist a profile's option sidecar; empty options remove it.
    async fn save_options(&self, profile: &str, options: &ProfileOptions) -> Result<(), PortError>;
    async fn delete_options(&self, profile: &str) -> Result<(), PortError>;
    /// DUAL-08-13: load the saved aggregation templates, empty when none where
    /// stored. Stores without a template sidecar answer with a typed
    /// unsupported instead of pretending the library is empty.
    async fn load_aggregation_templates(
        &self,
    ) -> Result<Vec<infiltrator_contract::aggregator::AggregationTemplate>, PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "this profile store keeps no aggregation-template sidecar",
        ))
    }
    /// DUAL-08-13: persist the aggregation template library; an empty library
    /// removes the sidecar.
    async fn save_aggregation_templates(
        &self,
        _templates: &[infiltrator_contract::aggregator::AggregationTemplate],
    ) -> Result<(), PortError> {
        Err(PortError::unsupported(
            Capability::Profiles,
            "this profile store keeps no aggregation-template sidecar",
        ))
    }
    async fn clear_backup(&self, profile: &str) -> Result<(), PortError>;
    async fn restore_backup(&self, profile: &str) -> Result<bool, PortError>;
}
