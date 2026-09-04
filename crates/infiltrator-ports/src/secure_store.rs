use crate::error::PortError;
use async_trait::async_trait;

/// Secure credential storage supplied by the selected host.
#[async_trait]
pub trait SecureStore: Send + Sync {
    async fn get(&self, namespace: &str, key: &str) -> Result<Option<String>, PortError>;
    async fn set(&self, namespace: &str, key: &str, value: &str) -> Result<(), PortError>;
    async fn delete(&self, namespace: &str, key: &str) -> Result<(), PortError>;
}
