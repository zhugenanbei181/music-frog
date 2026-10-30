//! Outbound port definition for user-owned local proxy storage (FLPARITY-037).

use async_trait::async_trait;
use infiltrator_contract::local_proxy::LocalProxyCollection;
use lurker_core_ports::error::PortError;

/// Port abstraction for reading and writing user-owned local proxy configurations.
#[async_trait]
pub trait LocalProxyStorePort: Send + Sync {
    /// Load the current local proxy collection from persistent storage.
    async fn load_collection(&self) -> Result<LocalProxyCollection, PortError>;

    /// Save the updated local proxy collection to persistent storage.
    async fn save_collection(&self, collection: &LocalProxyCollection) -> Result<(), PortError>;
}
