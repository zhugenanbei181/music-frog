//! Behavior tests for the application surface reader.

use super::*;
use async_trait::async_trait;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_contract::version::{CoreRelease, CoreReleaseChannel, CoreRollbackSnapshot};
use infiltrator_ports::application_runtime::{
    ApplicationFuture, ApplicationRuntime, ApplicationSleep,
};
use infiltrator_ports::core_process::{CoreProcess, CoreReadiness};
use infiltrator_ports::endpoint::{ControllerEndpoint, EndpointSource};
use infiltrator_ports::port_conflict::PortConflictPort;
use infiltrator_ports::offline_startup::OfflineStartupPort;
use infiltrator_ports::service_mode::ServiceModePort;
use infiltrator_ports::version::{VersionPort, VersionProgressSink};
use std::sync::atomic::{AtomicUsize, Ordering};

struct TestRuntime;

impl ApplicationRuntime for TestRuntime {
    fn block_on(&self, future: ApplicationFuture) {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime")
            .block_on(future);
    }

    fn sleep(&self, duration: Duration) -> ApplicationSleep<'_> {
        Box::pin(tokio::time::sleep(duration))
    }
}

struct TestProcess;

#[async_trait]
impl CoreProcess for TestProcess {
    async fn start(&self) -> Result<(), PortError> {
        Ok(())
    }

    async fn stop(&self) -> Result<(), PortError> {
        Ok(())
    }

    async fn status(&self) -> Result<CoreLifecycle, PortError> {
        Ok(CoreLifecycle::Stopped)
    }

    fn controller_endpoint(&self) -> Option<String> {
        Some("http://127.0.0.1:9090".to_owned())
    }
}

struct TestReadiness;

#[async_trait]
impl CoreReadiness for TestReadiness {
    async fn probe(&self) -> Result<String, PortError> {
        Ok("http://127.0.0.1:9090".to_owned())
    }
}

struct TestEndpoint;

#[async_trait]
impl EndpointSource for TestEndpoint {
    async fn resolve(&self) -> Result<ControllerEndpoint, PortError> {
        Ok(ControllerEndpoint {
            url: "http://127.0.0.1:9090".to_owned(),
            secret: Some("generated-by-host".to_owned()),
        })
    }
}

struct TestServiceMode;

#[async_trait]
impl ServiceModePort for TestServiceMode {
    async fn snapshot(
        &self,
    ) -> Result<infiltrator_contract::service_mode::ServiceModeSnapshot, PortError> {
        Ok(infiltrator_contract::service_mode::ServiceModeSnapshot {
            platform: infiltrator_contract::service_mode::ServiceModePlatform::LinuxPolkit,
            state: infiltrator_contract::service_mode::ServiceModeState::Ready,
        })
    }

    async fn prepare(
        &self,
    ) -> Result<infiltrator_contract::service_mode::ServiceModeSnapshot, PortError> {
        self.snapshot().await
    }
}

struct TestPortConflicts;

#[async_trait]
impl PortConflictPort for TestPortConflicts {
    async fn snapshot(
        &self,
    ) -> Result<infiltrator_contract::port_conflict::PortConflictSnapshot, PortError> {
        Ok(infiltrator_contract::port_conflict::PortConflictSnapshot {
            revision: 1,
            conflicts: vec![infiltrator_contract::port_conflict::PortConflict {
                binding: infiltrator_contract::port_conflict::PortBinding::Controller,
                port: 9090,
                available: false,
                owner_pid: Some(4242),
                owner_name: Some("mihomo".to_owned()),
                can_release: true,
            }],
        })
    }

    async fn repair(
        &self,
    ) -> Result<infiltrator_contract::port_conflict::PortConflictSnapshot, PortError> {
        Ok(infiltrator_contract::port_conflict::PortConflictSnapshot::default())
    }
}

struct TestOfflineStartup;

#[async_trait]
impl OfflineStartupPort for TestOfflineStartup {
    async fn validate_offline_startup(
        &self,
    ) -> Result<infiltrator_contract::offline_startup::OfflineStartupSnapshot, PortError> {
        Ok(infiltrator_contract::offline_startup::OfflineStartupSnapshot::ready(
            infiltrator_contract::offline_startup::LocalAssetStatus::Available,
        ))
    }
}

struct TestVersions {
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl VersionPort for TestVersions {
    async fn list_installed(
        &self,
    ) -> Result<Vec<infiltrator_contract::version::InstalledCoreVersion>, PortError> {
        Ok(Vec::new())
    }

    async fn latest(&self, channel: CoreReleaseChannel) -> Result<CoreRelease, PortError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(CoreRelease {
            version: format!("{}-v1.0.0", channel.as_str()),
            release_date: "2026-09-05".to_owned(),
        })
    }

    async fn list_releases(
        &self,
        _limit: usize,
    ) -> Result<Vec<infiltrator_contract::version::CoreReleaseSummary>, PortError> {
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

    async fn rollback_snapshot(&self) -> Result<CoreRollbackSnapshot, PortError> {
        Ok(CoreRollbackSnapshot {
            current: Some("v1.19.30".to_owned()),
            target: Some("v1.19.29".to_owned()),
            history: vec!["v1.19.29".to_owned()],
        })
    }
}

#[tokio::test]
async fn surface_reader_publishes_and_caches_all_core_channel_results() {
    let calls = Arc::new(AtomicUsize::new(0));
    let core = Arc::new(CoreApplication::new(
        Arc::new(TestProcess),
        Arc::new(TestReadiness),
        Arc::new(TestRuntime),
    ));
    let reader = ApplicationSurfaceReader::new(core, SurfaceKind::BevyDesktop, HostKind::Desktop)
        .with_versions(VersionApplication::new(Arc::new(TestVersions {
            calls: calls.clone(),
        })))
        .with_endpoint_source(Arc::new(TestEndpoint))
        .with_service_mode(ServiceModeApplication::new(Arc::new(TestServiceMode)))
        .with_port_conflicts(PortConflictApplication::new(Arc::new(TestPortConflicts)))
        .with_offline_startup(OfflineStartupApplication::new(Arc::new(TestOfflineStartup)));

    let first = reader.read().await.expect("first surface read");
    let second = reader.read().await.expect("cached surface read");
    assert_eq!(first.versions.channels.len(), 3);
    assert_eq!(first.versions.revision, 1);
    assert_eq!(second.versions.revision, 1);
    assert_eq!(calls.load(Ordering::SeqCst), 3);
    assert_eq!(first.versions.rollback.target.as_deref(), Some("v1.19.29"));
    assert_eq!(
        first.controller_auth.status,
        infiltrator_contract::controller::ControllerAuthStatus::Secured
    );
    assert_eq!(
        first.service_mode.state,
        infiltrator_contract::service_mode::ServiceModeState::Ready
    );
    assert!(first.port_conflicts.has_conflicts());
    assert_eq!(first.port_conflicts.conflicts[0].owner_pid, Some(4242));
    assert!(first.offline_startup.is_offline_startable());
    assert_eq!(
        first.offline_startup.geoip,
        infiltrator_contract::offline_startup::LocalAssetStatus::Available
    );
    assert!(first.versions.channels.iter().all(|channel| matches!(
        channel.status,
        infiltrator_contract::version::CoreChannelStatus::Ready { .. }
    )));
}
