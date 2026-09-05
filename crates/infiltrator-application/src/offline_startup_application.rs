//! Offline-first startup use-case over a host validation port.

use infiltrator_contract::error::Failure;
use infiltrator_contract::offline_startup::OfflineStartupSnapshot;
use infiltrator_ports::offline_startup::OfflineStartupPort;
use std::sync::Arc;

#[derive(Clone)]
pub struct OfflineStartupApplication {
    port: Arc<dyn OfflineStartupPort>,
}

impl OfflineStartupApplication {
    pub fn new(port: Arc<dyn OfflineStartupPort>) -> Self {
        Self { port }
    }

    /// Return a typed result for every local preflight outcome.  Adapter I/O
    /// failures become a blocked snapshot, so neither UI invents readiness.
    pub async fn snapshot(&self) -> OfflineStartupSnapshot {
        self.port
            .validate_offline_startup()
            .await
            .unwrap_or_else(|error| OfflineStartupSnapshot::blocked(Failure::from(error)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use infiltrator_contract::offline_startup::{
        LocalAssetStatus, OfflineStartupState, StartupRemoteDependency,
    };
    use infiltrator_ports::error::PortError;

    struct ReadyPort;

    #[async_trait]
    impl OfflineStartupPort for ReadyPort {
        async fn validate_offline_startup(&self) -> Result<OfflineStartupSnapshot, PortError> {
            Ok(OfflineStartupSnapshot::ready(LocalAssetStatus::Available))
        }
    }

    struct BrokenPort;

    #[async_trait]
    impl OfflineStartupPort for BrokenPort {
        async fn validate_offline_startup(&self) -> Result<OfflineStartupSnapshot, PortError> {
            Err(PortError::Io("profile unreadable".to_owned()))
        }
    }

    #[tokio::test]
    async fn application_preserves_local_ready_and_optional_remote_policy() {
        let snapshot = OfflineStartupApplication::new(Arc::new(ReadyPort))
            .snapshot()
            .await;
        assert_eq!(snapshot.state, OfflineStartupState::Ready);
        assert!(snapshot.is_offline_startable());
        assert_eq!(
            snapshot.remote_dependency,
            StartupRemoteDependency::Optional
        );
    }

    #[tokio::test]
    async fn application_fails_closed_on_adapter_io() {
        let snapshot = OfflineStartupApplication::new(Arc::new(BrokenPort))
            .snapshot()
            .await;
        assert_eq!(snapshot.state, OfflineStartupState::Blocked);
        assert!(!snapshot.is_offline_startable());
        assert!(snapshot.failure.is_some());
    }
}
