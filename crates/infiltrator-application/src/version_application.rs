//! Core-version use-cases over a host-provided version port.

use infiltrator_contract::error::Failure;
use infiltrator_contract::version::{
    CoreChannelSnapshot, CoreChannelStatus, CoreRelease, CoreReleaseChannel, CoreReleaseSummary,
    CoreVersionSnapshot, InstalledCoreVersion, VersionDownloadProgress,
};
use infiltrator_ports::version::{VersionPort, VersionProgressSink};
use std::sync::Arc;

#[derive(Clone)]
pub struct VersionApplication {
    port: Arc<dyn VersionPort>,
}

impl VersionApplication {
    pub fn new(port: Arc<dyn VersionPort>) -> Self {
        Self { port }
    }

    pub async fn list_installed(&self) -> Result<Vec<InstalledCoreVersion>, Failure> {
        self.port.list_installed().await.map_err(Failure::from)
    }

    pub async fn latest(&self, channel: CoreReleaseChannel) -> Result<CoreRelease, Failure> {
        self.port.latest(channel).await.map_err(Failure::from)
    }

    /// Probe all official channels independently. A failure in Alpha must not
    /// hide a usable Stable or Meta-Core result from either UI surface.
    pub async fn probe_channels(&self) -> CoreVersionSnapshot {
        let probes = futures_util::future::join_all(CoreReleaseChannel::ALL.into_iter().map(
            |channel| async move {
                let status = match self.latest(channel).await {
                    Ok(release) => CoreChannelStatus::Ready { release },
                    Err(failure) => CoreChannelStatus::Failed { failure },
                };
                CoreChannelSnapshot { channel, status }
            },
        ));
        let (probes, rollback) = futures_util::join!(probes, self.port.rollback_snapshot());
        CoreVersionSnapshot {
            revision: 1,
            channels: probes,
            verification: self.port.verification(),
            rollback: rollback.unwrap_or_default(),
        }
    }

    pub async fn list_releases(&self, limit: usize) -> Result<Vec<CoreReleaseSummary>, Failure> {
        self.port.list_releases(limit).await.map_err(Failure::from)
    }

    pub async fn install(
        &self,
        version: String,
        progress: Arc<dyn VersionProgressSink>,
    ) -> Result<(), Failure> {
        self.port
            .install(version, progress)
            .await
            .map_err(Failure::from)
    }

    pub async fn activate(&self, version: &str) -> Result<(), Failure> {
        self.port.activate(version).await.map_err(Failure::from)
    }

    pub async fn uninstall(&self, version: &str) -> Result<(), Failure> {
        self.port.uninstall(version).await.map_err(Failure::from)
    }

    pub async fn rollback(&self) -> Result<String, Failure> {
        self.port.rollback().await.map_err(Failure::from)
    }
}

/// No-op progress sink for one-shot CLI/API calls that do not render a live
/// progress stream.
#[derive(Default)]
pub struct QuietVersionProgress;

impl VersionProgressSink for QuietVersionProgress {
    fn progress(&self, _progress: VersionDownloadProgress) {}

    fn is_cancelled(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use infiltrator_contract::version::CoreChannelStatus;
    use infiltrator_ports::error::PortError;

    struct FakeVersionPort;

    #[async_trait]
    impl VersionPort for FakeVersionPort {
        async fn list_installed(&self) -> Result<Vec<InstalledCoreVersion>, PortError> {
            Ok(Vec::new())
        }

        async fn latest(&self, channel: CoreReleaseChannel) -> Result<CoreRelease, PortError> {
            match channel {
                CoreReleaseChannel::Stable => Ok(CoreRelease {
                    version: "v1.19.30".to_owned(),
                    release_date: "2026-08-16".to_owned(),
                }),
                CoreReleaseChannel::Alpha => Err(PortError::Network(
                    "Prerelease-Alpha temporarily unavailable".to_owned(),
                )),
                CoreReleaseChannel::MetaCore => Ok(CoreRelease {
                    version: "v1.19.29".to_owned(),
                    release_date: "2026-07-18".to_owned(),
                }),
            }
        }

        async fn list_releases(&self, _limit: usize) -> Result<Vec<CoreReleaseSummary>, PortError> {
            Ok(Vec::new())
        }

        async fn install(
            &self,
            _version: String,
            _progress: Arc<dyn VersionProgressSink>,
        ) -> Result<(), PortError> {
            Ok(())
        }

        async fn activate(&self, _version: &str) -> Result<(), PortError> {
            Ok(())
        }

        async fn uninstall(&self, _version: &str) -> Result<(), PortError> {
            Ok(())
        }

        async fn rollback(&self) -> Result<String, PortError> {
            Ok("v1.19.29".to_owned())
        }
    }

    #[tokio::test]
    async fn channel_probe_keeps_successes_when_one_channel_fails() {
        let application = VersionApplication::new(Arc::new(FakeVersionPort));
        let snapshot = application.probe_channels().await;

        assert_eq!(snapshot.revision, 1);
        assert_eq!(snapshot.channels.len(), 3);
        assert_eq!(snapshot.channels[0].channel, CoreReleaseChannel::Stable);
        assert!(matches!(
            snapshot.channels[0].status,
            CoreChannelStatus::Ready { .. }
        ));
        assert_eq!(snapshot.channels[1].channel, CoreReleaseChannel::Alpha);
        assert!(matches!(
            snapshot.channels[1].status,
            CoreChannelStatus::Failed { .. }
        ));
        assert_eq!(snapshot.channels[2].channel, CoreReleaseChannel::MetaCore);
        assert!(matches!(
            snapshot.channels[2].status,
            CoreChannelStatus::Ready { .. }
        ));
    }

    #[tokio::test]
    async fn rollback_is_forwarded_through_the_application_facade() {
        let application = VersionApplication::new(Arc::new(FakeVersionPort));
        assert_eq!(application.rollback().await.unwrap(), "v1.19.29");
    }
}
