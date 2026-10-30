//! Outbound port definition for user-owned general override storage (FLPARITY-038).

use async_trait::async_trait;
use infiltrator_contract::general_override::GeneralOverrideCollection;
use lurker_core_ports::error::PortError;

/// Port abstraction for reading and writing user-owned general override configurations.
#[async_trait]
pub trait GeneralOverrideStorePort: Send + Sync {
    /// Load the current general override collection from persistent storage.
    async fn load_collection(&self) -> Result<GeneralOverrideCollection, PortError>;

    /// Save the updated general override collection to persistent storage.
    async fn save_collection(
        &self,
        collection: &GeneralOverrideCollection,
    ) -> Result<(), PortError>;
}
