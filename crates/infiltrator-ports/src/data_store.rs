use async_trait::async_trait;
use lurker_core_ports::error::PortError;

/// Byte-oriented persistence port. Serialization policy belongs to the
/// application/domain layer; filesystem/database details stay in adapters.
#[async_trait]
pub trait DataStore: Send + Sync {
    async fn read(&self, key: &str) -> Result<Option<Vec<u8>>, PortError>;
    async fn write(&self, key: &str, value: &[u8]) -> Result<(), PortError>;
    async fn delete(&self, key: &str) -> Result<(), PortError>;
}
