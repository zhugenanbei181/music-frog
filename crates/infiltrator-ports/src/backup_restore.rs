//! Outbound port definition for encrypted backup export, local/WebDAV delivery, and atomic restore (FLPARITY-013).

use async_trait::async_trait;
use infiltrator_contract::backup_restore::{BackupDeliveryTarget, BackupRestoreScope};
use infiltrator_domain::backup::BackupBundle;
use lurker_core_ports::error::PortError;

/// Port abstraction for reading/writing encrypted backup archives and applying restored state.
#[async_trait]
pub trait BackupRestorePort: Send + Sync {
    /// Collect the current application settings, mixin overlay, and local profiles into a `BackupBundle`.
    async fn capture_current_bundle(&self) -> Result<BackupBundle, PortError>;

    /// Deliver encrypted backup bytes to the specified target (`LocalFile` or `WebDav`)
    /// and return the canonical destination URI/path.
    async fn deliver_encrypted_archive(
        &self,
        target: BackupDeliveryTarget,
        destination_hint: &str,
        encrypted_bytes: &[u8],
    ) -> Result<String, PortError>;

    /// Fetch encrypted backup bytes from the specified target (`LocalFile` or `WebDav`).
    async fn fetch_encrypted_archive(
        &self,
        target: BackupDeliveryTarget,
        source_uri: &str,
    ) -> Result<Vec<u8>, PortError>;

    /// Atomically apply the restored `BackupBundle` according to `scope` (`ConfigOnly` or `FullData`),
    /// returning the count of restored profile documents.
    ///
    /// Implementations must guarantee that on error, no partial profile or setting writes remain committed.
    async fn apply_restored_bundle(
        &self,
        bundle: &BackupBundle,
        scope: BackupRestoreScope,
    ) -> Result<usize, PortError>;
}
