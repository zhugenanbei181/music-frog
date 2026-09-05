//! Iced adapter tests for the shared surface revision/status boundary.
//! test-intent: behavior

use crate::surface::SurfaceBridge;
use crate::surface::SurfaceModel;
use async_trait::async_trait;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::{PageId, PageStatus, SurfaceSnapshot};
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
