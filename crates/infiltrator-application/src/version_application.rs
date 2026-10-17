//! Engine-scoped core-version use-cases over host-provided version ports.

use futures_util::future::join_all;
use infiltrator_contract::version::{
    CoreArtifactVerification, CoreChannelSnapshot, CoreChannelStatus, CoreRelease,
    CoreReleaseSummary, CoreRollbackSnapshot, CoreVersionSnapshot, EngineNeutralReleaseChannel,
    EngineReleaseChannel, InstalledCoreVersion, VersionDownloadProgress,
};
use infiltrator_ports::engine_version_backend_factory::EngineVersionBackendFactory;
use infiltrator_ports::version::{
    VersionActivationRequest, VersionInstallRequest, VersionPort, VersionProgressSink,
    VersionRollbackRequest, VersionUninstallRequest,
};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::EngineId;
use lurker_core_ports::error::PortError;
use std::sync::Arc;

#[derive(Clone)]
pub struct VersionApplication {
    engine_id: EngineId,
    port: Arc<dyn VersionPort>,
}

impl VersionApplication {
    pub fn from_factory(
        engine_id: EngineId,
        factory: &dyn EngineVersionBackendFactory,
    ) -> Result<Self, Failure> {
        let port = factory.version_port(&engine_id).map_err(Failure::from)?;
        Self::new(engine_id, port)
    }

    pub fn new(engine_id: EngineId, port: Arc<dyn VersionPort>) -> Result<Self, Failure> {
        let port_id = port.engine_id();
        if port_id != engine_id {
            return Err(engine_scope_failure(&engine_id, &port_id));
        }
        Ok(Self { engine_id, port })
    }

    pub fn engine_id(&self) -> &EngineId {
        &self.engine_id
    }

    pub async fn list_installed(&self) -> Result<Vec<InstalledCoreVersion>, Failure> {
        self.ensure_port_engine()?;
        let installed = self
            .port
            .list_installed(&self.engine_id)
            .await
            .map_err(Failure::from)?;
        if let Some(version) = installed
            .iter()
            .find(|version| version.engine_id != self.engine_id)
        {
            return Err(engine_scope_failure(&self.engine_id, &version.engine_id));
        }
        Ok(installed)
    }

    /// Probe only channels advertised by this engine's version backend.
    pub async fn probe_channels(&self) -> CoreVersionSnapshot {
        let channel_result = if self.ensure_port_engine().is_ok() {
            self.port.release_channels(&self.engine_id).await
        } else {
            Err(PortError::Failed(
                "version port engine identity changed".to_owned(),
            ))
        };
        let (channels, channel_failure) = match channel_result {
            Ok(channels)
                if channels
                    .iter()
                    .all(|channel| channel.engine_id == self.engine_id) =>
            {
                (channels, None)
            }
            Ok(channels) => {
                let wrong_engine = channels
                    .iter()
                    .find(|channel| channel.engine_id != self.engine_id)
                    .map(|channel| channel.engine_id.clone())
                    .expect("a rejected channel list contains a mismatched engine");
                (
                    Vec::new(),
                    Some(engine_scope_failure(&self.engine_id, &wrong_engine)),
                )
            }
            Err(error) => (Vec::new(), Some(Failure::from(error))),
        };
        let channel_probes = join_all(channels.into_iter().map(|channel| async move {
            let status = match self.latest(channel.clone()).await {
                Ok(release) => CoreChannelStatus::Ready { release },
                Err(failure) => CoreChannelStatus::Failed { failure },
            };
            CoreChannelSnapshot {
                engine_id: self.engine_id.clone(),
                channel,
                status,
            }
        }));
        let rollback = async {
            if self.ensure_port_engine().is_err() {
                return None;
            }
            self.port.rollback_snapshot(&self.engine_id).await.ok()
        };
        let (channels, rollback) = futures_util::join!(channel_probes, rollback);
        let rollback = rollback
            .filter(|snapshot| snapshot.engine_id == self.engine_id)
            .unwrap_or_else(|| CoreRollbackSnapshot::for_engine(self.engine_id.clone()));
        CoreVersionSnapshot {
            engine_id: self.engine_id.clone(),
            revision: 1,
            channels,
            channel_failure,
            verification: if self.ensure_port_engine().is_ok() {
                scoped_verification(self.port.verification(&self.engine_id), &self.engine_id)
            } else {
                CoreArtifactVerification::Unknown
            },
            rollback,
        }
    }

    pub async fn latest_core(
        &self,
        channel: EngineNeutralReleaseChannel,
    ) -> Result<CoreRelease, Failure> {
        self.latest(EngineReleaseChannel::common(
            self.engine_id.clone(),
            channel,
        ))
        .await
    }

    pub async fn latest(&self, channel: EngineReleaseChannel) -> Result<CoreRelease, Failure> {
        self.ensure_port_engine()?;
        if channel.engine_id != self.engine_id {
            return Err(engine_scope_failure(&self.engine_id, &channel.engine_id));
        }
        let release = self.port.latest(channel).await.map_err(Failure::from)?;
        ensure_release_engine(&self.engine_id, &release)?;
        Ok(release)
    }

    pub async fn list_releases(&self, limit: usize) -> Result<Vec<CoreReleaseSummary>, Failure> {
        self.ensure_port_engine()?;
        let releases = self
            .port
            .list_releases(&self.engine_id, limit)
            .await
            .map_err(Failure::from)?;
        if let Some(release) = releases
            .iter()
            .find(|release| release.engine_id != self.engine_id)
        {
            return Err(engine_scope_failure(&self.engine_id, &release.engine_id));
        }
        Ok(releases)
    }

    pub async fn install(
        &self,
        version: String,
        progress: Arc<dyn VersionProgressSink>,
    ) -> Result<(), Failure> {
        self.ensure_port_engine()?;
        self.port
            .install(VersionInstallRequest {
                engine_id: self.engine_id.clone(),
                version,
                progress,
            })
            .await
            .map_err(Failure::from)
    }

    pub async fn activate(&self, version: &str) -> Result<(), Failure> {
        self.ensure_port_engine()?;
        self.port
            .activate(VersionActivationRequest {
                engine_id: self.engine_id.clone(),
                version: version.to_owned(),
            })
            .await
            .map_err(Failure::from)
    }

    pub async fn uninstall(&self, version: &str) -> Result<(), Failure> {
        self.ensure_port_engine()?;
        self.port
            .uninstall(VersionUninstallRequest {
                engine_id: self.engine_id.clone(),
                version: version.to_owned(),
            })
            .await
            .map_err(Failure::from)
    }

    pub async fn rollback(&self) -> Result<String, Failure> {
        self.ensure_port_engine()?;
        self.port
            .rollback(VersionRollbackRequest {
                engine_id: self.engine_id.clone(),
            })
            .await
            .map_err(Failure::from)
    }

    fn ensure_port_engine(&self) -> Result<(), Failure> {
        let actual_engine = self.port.engine_id();
        if actual_engine == self.engine_id {
            Ok(())
        } else {
            Err(engine_scope_failure(&self.engine_id, &actual_engine))
        }
    }
}

fn ensure_release_engine(engine_id: &EngineId, release: &CoreRelease) -> Result<(), Failure> {
    if &release.engine_id == engine_id {
        Ok(())
    } else {
        Err(engine_scope_failure(engine_id, &release.engine_id))
    }
}

fn scoped_verification(
    verification: CoreArtifactVerification,
    engine_id: &EngineId,
) -> CoreArtifactVerification {
    match verification {
        CoreArtifactVerification::Unknown => CoreArtifactVerification::Unknown,
        CoreArtifactVerification::Verified {
            engine_id: actual_engine,
            version,
        } if &actual_engine == engine_id => CoreArtifactVerification::Verified {
            engine_id: actual_engine,
            version,
        },
        CoreArtifactVerification::Rejected {
            engine_id: actual_engine,
            version,
            failure,
        } if &actual_engine == engine_id => CoreArtifactVerification::Rejected {
            engine_id: actual_engine,
            version,
            failure,
        },
        _ => CoreArtifactVerification::Unknown,
    }
}

fn engine_scope_failure(expected: &EngineId, actual: &EngineId) -> Failure {
    Failure::new(
        ErrorCode::InvalidState,
        format!(
            "core version engine mismatch: expected {}, received {}",
            expected.as_str(),
            actual.as_str()
        ),
        false,
    )
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
    use infiltrator_contract::version::{CoreRelease, ReleaseChannelId};
    use infiltrator_ports::version::{
        VersionActivationRequest, VersionInstallRequest, VersionUninstallRequest,
    };
    use lurker_core_contract::capability::Capability;
    use std::sync::Mutex;

    struct FakeVersionPort {
        engine_id: EngineId,
        reported_engine_id: Mutex<Option<EngineId>>,
        rollback_engine_id: Mutex<Option<EngineId>>,
        channels: Mutex<Vec<EngineReleaseChannel>>,
        channels_called: Mutex<Vec<EngineReleaseChannel>>,
        verification: Mutex<CoreArtifactVerification>,
    }

    impl FakeVersionPort {
        fn new(engine_id: &str) -> Self {
            let engine_id = EngineId::new(engine_id).expect("valid engine ID");
            Self {
                channels: Mutex::new(
                    EngineNeutralReleaseChannel::ALL
                        .into_iter()
                        .map(|channel| EngineReleaseChannel::common(engine_id.clone(), channel))
                        .collect(),
                ),
                engine_id,
                reported_engine_id: Mutex::new(None),
                rollback_engine_id: Mutex::new(None),
                channels_called: Mutex::new(Vec::new()),
                verification: Mutex::new(CoreArtifactVerification::Unknown),
            }
        }

        fn include_channel(&self, channel_id: &str) {
            self.channels
                .lock()
                .unwrap()
                .push(EngineReleaseChannel::new(
                    self.engine_id.clone(),
                    ReleaseChannelId::new(channel_id).expect("valid release channel ID"),
                ));
        }

        fn release(&self, version: &str) -> CoreRelease {
            CoreRelease {
                engine_id: self.engine_id.clone(),
                version: version.to_owned(),
                release_date: "2026-08-16".to_owned(),
            }
        }
    }

    #[async_trait]
    impl VersionPort for FakeVersionPort {
        fn engine_id(&self) -> EngineId {
            self.reported_engine_id
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_else(|| self.engine_id.clone())
        }

        async fn list_installed(
            &self,
            engine_id: &EngineId,
        ) -> Result<Vec<InstalledCoreVersion>, PortError> {
            if engine_id != &self.engine_id {
                return Err(PortError::Failed("wrong engine context".to_owned()));
            }
            Ok(Vec::new())
        }

        async fn release_channels(
            &self,
            engine_id: &EngineId,
        ) -> Result<Vec<EngineReleaseChannel>, PortError> {
            if engine_id != &self.engine_id {
                return Err(PortError::Failed("wrong engine context".to_owned()));
            }
            Ok(self.channels.lock().unwrap().clone())
        }

        async fn latest(&self, channel: EngineReleaseChannel) -> Result<CoreRelease, PortError> {
            if channel.engine_id != self.engine_id {
                return Err(PortError::Failed("wrong engine context".to_owned()));
            }
            let advertised = self.channels.lock().unwrap().contains(&channel);
            if !advertised {
                return Err(PortError::unsupported(
                    Capability::CoreVersionInstall,
                    "release channel not advertised by fake backend",
                ));
            }
            let channel_id = channel.channel_id.as_str().to_owned();
            self.channels_called.lock().unwrap().push(channel);
            match channel_id.as_str() {
                "stable" => Ok(self.release("v1.19.30")),
                "alpha" => Err(PortError::Network(
                    "alpha temporarily unavailable".to_owned(),
                )),
                "edge" => Ok(self.release("v1.19.29-edge")),
                _ => Err(PortError::unsupported(
                    Capability::CoreVersionInstall,
                    "unknown fake release channel",
                )),
            }
        }

        async fn list_releases(
            &self,
            engine_id: &EngineId,
            _limit: usize,
        ) -> Result<Vec<CoreReleaseSummary>, PortError> {
            if engine_id != &self.engine_id {
                return Err(PortError::Failed("wrong engine context".to_owned()));
            }
            Ok(Vec::new())
        }

        async fn install(&self, request: VersionInstallRequest) -> Result<(), PortError> {
            if request.engine_id != self.engine_id {
                return Err(PortError::Failed("wrong engine context".to_owned()));
            }
            Ok(())
        }

        async fn activate(&self, request: VersionActivationRequest) -> Result<(), PortError> {
            if request.engine_id != self.engine_id {
                return Err(PortError::Failed("wrong engine context".to_owned()));
            }
            Ok(())
        }

        async fn uninstall(&self, request: VersionUninstallRequest) -> Result<(), PortError> {
            if request.engine_id != self.engine_id {
                return Err(PortError::Failed("wrong engine context".to_owned()));
            }
            Ok(())
        }

        async fn rollback(&self, request: VersionRollbackRequest) -> Result<String, PortError> {
            if request.engine_id != self.engine_id {
                return Err(PortError::Failed("wrong engine context".to_owned()));
            }
            Ok("v1.19.29".to_owned())
        }

        async fn rollback_snapshot(
            &self,
            engine_id: &EngineId,
        ) -> Result<CoreRollbackSnapshot, PortError> {
            if engine_id != &self.engine_id {
                return Err(PortError::Failed("wrong engine context".to_owned()));
            }
            Ok(CoreRollbackSnapshot {
                engine_id: self
                    .rollback_engine_id
                    .lock()
                    .unwrap()
                    .clone()
                    .unwrap_or_else(|| self.engine_id.clone()),
                current: Some("v1.19.30".to_owned()),
                target: Some("v1.19.29".to_owned()),
                history: vec!["v1.19.29".to_owned()],
            })
        }

        fn verification(&self, engine_id: &EngineId) -> CoreArtifactVerification {
            if engine_id != &self.engine_id {
                return CoreArtifactVerification::Unknown;
            }
            self.verification.lock().unwrap().clone()
        }
    }

    #[tokio::test]
    async fn snapshot_is_engine_scoped_and_contains_only_advertised_channels() {
        let engine_id = EngineId::new("engine-a").expect("engine ID");
        let port = Arc::new(FakeVersionPort::new("engine-a"));
        port.include_channel("edge");
        let application = VersionApplication::new(engine_id.clone(), port).unwrap();
        let snapshot = application.probe_channels().await;

        assert_eq!(snapshot.engine_id, engine_id);
        assert_eq!(snapshot.channels.len(), 3);
        assert_eq!(snapshot.channels[0].engine_id, engine_id);
        assert_eq!(
            snapshot.channels[0].channel,
            EngineReleaseChannel::common(engine_id.clone(), EngineNeutralReleaseChannel::Stable)
        );
        assert!(matches!(
            snapshot.channels[1].status,
            CoreChannelStatus::Failed { .. }
        ));
        assert_eq!(snapshot.channels[2].channel.channel_id.as_str(), "edge");
        assert!(matches!(
            snapshot.channels[2].status,
            CoreChannelStatus::Ready { .. }
        ));
        assert_eq!(snapshot.rollback.engine_id, engine_id);
    }

    #[test]
    fn application_rejects_a_port_bound_to_another_engine() {
        let expected = EngineId::new("engine-b").expect("engine ID");
        let port = Arc::new(FakeVersionPort::new("engine-a"));
        let result = VersionApplication::new(expected, port);
        assert!(matches!(
            result,
            Err(Failure {
                code: ErrorCode::InvalidState,
                ..
            })
        ));
    }

    #[tokio::test]
    async fn port_engine_identity_drift_fails_closed_before_calling_the_port() {
        let engine_id = EngineId::new("engine-a").expect("engine ID");
        let port = Arc::new(FakeVersionPort::new("engine-a"));
        let port_for_application: Arc<dyn VersionPort> = port.clone();
        let application = VersionApplication::new(engine_id, port_for_application).unwrap();
        *port.reported_engine_id.lock().unwrap() =
            Some(EngineId::new("engine-b").expect("engine ID"));

        let result = application.list_installed().await;

        assert!(matches!(
            result,
            Err(Failure {
                code: ErrorCode::InvalidState,
                ..
            })
        ));
    }

    #[tokio::test]
    async fn engine_scoped_custom_channel_is_consumed_by_its_port() {
        let engine_id = EngineId::new("engine-a").expect("engine ID");
        let port = Arc::new(FakeVersionPort::new("engine-a"));
        port.include_channel("edge");
        let application = VersionApplication::new(engine_id.clone(), port).unwrap();
        let release = application
            .latest(EngineReleaseChannel::new(
                engine_id,
                ReleaseChannelId::new("edge").unwrap(),
            ))
            .await
            .unwrap();
        assert_eq!(release.version, "v1.19.29-edge");
    }

    #[tokio::test]
    async fn unadvertised_engine_channel_is_unsupported() {
        let engine_id = EngineId::new("engine-a").expect("engine ID");
        let port: Arc<dyn VersionPort> = Arc::new(FakeVersionPort::new("engine-a"));
        let application = VersionApplication::new(engine_id.clone(), port).unwrap();

        let result = application
            .latest(EngineReleaseChannel::new(
                engine_id,
                ReleaseChannelId::new("edge").unwrap(),
            ))
            .await;

        assert!(matches!(
            result,
            Err(Failure {
                code: ErrorCode::Unsupported,
                ..
            })
        ));
    }

    #[tokio::test]
    async fn artifact_verification_from_another_engine_is_hidden() {
        let engine_id = EngineId::new("engine-a").expect("engine ID");
        let port = Arc::new(FakeVersionPort::new("engine-a"));
        *port.verification.lock().unwrap() = CoreArtifactVerification::Verified {
            engine_id: EngineId::new("engine-b").expect("engine ID"),
            version: "v1.2.3".to_owned(),
        };
        let application = VersionApplication::new(engine_id, port).unwrap();

        let snapshot = application.probe_channels().await;

        assert_eq!(snapshot.verification, CoreArtifactVerification::Unknown);
    }

    #[tokio::test]
    async fn rollback_journal_from_another_engine_is_not_published() {
        let engine_id = EngineId::new("engine-a").expect("engine ID");
        let port = Arc::new(FakeVersionPort::new("engine-a"));
        *port.rollback_engine_id.lock().unwrap() =
            Some(EngineId::new("engine-b").expect("engine ID"));
        let application = VersionApplication::new(engine_id.clone(), port).unwrap();

        let snapshot = application.probe_channels().await;

        assert_eq!(snapshot.rollback.engine_id, engine_id);
        assert_eq!(snapshot.rollback.current, None);
        assert_eq!(snapshot.rollback.target, None);
        assert!(snapshot.rollback.history.is_empty());
    }
}
