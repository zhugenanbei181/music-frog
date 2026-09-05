//! Iced adapter tests for the shared surface revision/status boundary.
//! test-intent: behavior

use crate::surface::SurfaceBridge;
use crate::surface::SurfaceModel;
use crate::state::AppState;
use async_trait::async_trait;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::{PageId, PageStatus, SurfaceSnapshot};
use infiltrator_contract::snapshot::{CoreWatchdogState, CoreWatchdogSnapshot};
use infiltrator_contract::version::{
    CoreArtifactVerification, CoreChannelSnapshot, CoreChannelStatus, CoreRelease,
    CoreReleaseChannel, CoreRollbackSnapshot, CoreVersionSnapshot,
};
use infiltrator_ports::application_runtime::{
    ApplicationFuture, ApplicationRuntime, ApplicationSleep,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::surface::SurfaceReader;
use std::sync::Arc;
use std::time::Duration;

fn snapshot(revision: u64) -> SurfaceSnapshot {
    let mut value = SurfaceSnapshot::unavailable(
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
        Failure::new(ErrorCode::NotReady, "not ready", true),
    );
    value.revision = revision;
    value.core.revision = revision;
    value
}

fn session_snapshot(revision: u64, generation: u64, token: u128) -> SurfaceSnapshot {
    let mut value = snapshot(revision);
    value.generation = generation;
    value.core.generation = generation;
    value.core.session_token = Some(infiltrator_contract::session::SessionToken::new(token));
    value
}

#[test]
fn stale_surface_events_are_rejected() {
    let mut model = SurfaceModel::default();
    assert!(model.apply(snapshot(2)));
    assert!(!model.apply(snapshot(1)));
    assert_eq!(model.revision(), 2);
    assert!(matches!(
        model.page_status(PageId::Overview),
        Some(PageStatus::Unavailable { .. })
    ));
}

#[test]
fn stale_session_snapshot_is_rejected_even_with_a_larger_revision() {
    let mut model = SurfaceModel::default();
    assert!(model.apply(session_snapshot(2, 4, 40)));
    assert!(!model.apply(session_snapshot(3, 4, 39)));
    assert_eq!(model.revision(), 2);
    assert_eq!(
        model
            .latest()
            .and_then(|snapshot| snapshot.core.session_token)
            .map(|token| token.value()),
        Some(40)
    );
    assert!(model.apply(session_snapshot(1, 5, 50)));
    assert_eq!(model.revision(), 1);
}

#[test]
fn hot_reload_snapshot_keeps_the_session_identity_and_generation() {
    let mut model = SurfaceModel::default();
    assert!(model.apply(session_snapshot(2, 4, 40)));
    assert!(model.apply(session_snapshot(3, 4, 40)));
    let latest = model.latest().expect("hot reload snapshot");
    assert_eq!(latest.revision, 3);
    assert_eq!(latest.generation, 4);
    assert_eq!(latest.core.session_token.map(|token| token.value()), Some(40));
}

#[test]
fn shared_watchdog_snapshot_updates_the_iced_diagnostics_projection() {
    let (mut state, _) = AppState::new();
    let mut snapshot = snapshot(4);
    snapshot.core.session_token = Some(infiltrator_contract::session::SessionToken::new(42));
    snapshot.core.watchdog = CoreWatchdogSnapshot {
        state: CoreWatchdogState::Waiting {
            attempt: 2,
            retry_in_ms: 200,
        },
        session_token: snapshot.core.session_token,
        consecutive_failures: 2,
        last_error: Some(Failure::new(ErrorCode::Internal, "core exited", true)),
    };

    assert!(state.apply_shared_surface_snapshot(snapshot));
    assert_eq!(
        state.diag.crash_watchdog.shared.state,
        CoreWatchdogState::Waiting {
            attempt: 2,
            retry_in_ms: 200,
        }
    );
    assert_eq!(state.diag.crash_watchdog.shared.consecutive_failures, 2);
    assert_eq!(
        state.diag.crash_watchdog.last_crash_summary.as_deref(),
        Some("core exited")
    );
}

#[test]
fn shared_core_channel_probe_updates_the_iced_kernel_projection() {
    let (mut state, _) = AppState::new();
    let mut snapshot = snapshot(5);
    snapshot.versions = CoreVersionSnapshot {
        revision: 1,
        channels: vec![CoreChannelSnapshot {
            channel: CoreReleaseChannel::MetaCore,
            status: CoreChannelStatus::Ready {
                release: CoreRelease {
                    version: "v1.19.30".to_owned(),
                    release_date: "2026-08-16".to_owned(),
                },
            },
        }],
        verification: CoreArtifactVerification::Verified {
            version: "v1.19.30".to_owned(),
        },
        rollback: CoreRollbackSnapshot {
            current: Some("v1.19.30".to_owned()),
            target: Some("v1.19.29".to_owned()),
            history: vec!["v1.19.29".to_owned()],
        },
    };

    assert!(state.apply_shared_surface_snapshot(snapshot));
    assert_eq!(state.runtime.core_versions.revision, 1);
    assert_eq!(
        state.runtime.core_versions.channels[0].channel,
        CoreReleaseChannel::MetaCore
    );
    assert_eq!(
        state.runtime.core_integrity,
        CoreArtifactVerification::Verified {
            version: "v1.19.30".to_owned(),
        }
    );
    assert_eq!(
        state.runtime.core_versions.rollback.target.as_deref(),
        Some("v1.19.29")
    );
}

struct TokioRuntime(tokio::runtime::Runtime);

impl ApplicationRuntime for TokioRuntime {
    fn block_on(&self, future: ApplicationFuture) {
        self.0.block_on(future);
    }

    fn sleep(&self, duration: Duration) -> ApplicationSleep<'_> {
        Box::pin(tokio::time::sleep(duration))
    }
}

struct StaticReader;

#[async_trait]
impl SurfaceReader for StaticReader {
    async fn read(&self) -> Result<SurfaceSnapshot, PortError> {
        Ok(snapshot(7))
    }
}

#[test]
fn surface_bridge_translates_the_application_snapshot_to_an_iced_message() {
    let runtime = Arc::new(TokioRuntime(
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime"),
    ));
    let initial = snapshot(1);
    let pump = infiltrator_application::surface_application::SurfacePump::spawn(
        Arc::new(StaticReader),
        Duration::ZERO,
        runtime,
        initial,
    );
    let bridge = SurfaceBridge::from_pump(pump);
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    loop {
        let messages = bridge.drain_messages();
        if let Some(message) = messages.into_iter().next() {
            match message {
                crate::types::message::Message::SurfaceSnapshotUpdated(snapshot) => {
                    assert_eq!(snapshot.revision, 7);
                    assert_eq!(snapshot.surface, SurfaceKind::IcedDesktop);
                }
                other => panic!("unexpected message: {other:?}"),
            }
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "bridge did not deliver"
        );
        std::thread::yield_now();
    }
}
