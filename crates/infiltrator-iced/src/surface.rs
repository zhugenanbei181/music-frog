//! Iced adapter for the shared 11-page surface read model.
//!
//! The existing Iced page state remains optimized for its Elm update loop,
//! but its canonical cross-surface input is now this contract projection.
//! Toolkit-specific fields may be derived from it; they must not replace it.

use iced::futures::stream::BoxStream;
use iced::{Subscription, stream};
use infiltrator_application::surface_application::{SurfacePump, SurfacePumpBridge};
use infiltrator_contract::surface_snapshot::{PageId, PageStatus, SurfaceSnapshot};
use std::hash::Hash;
use std::time::Duration;

#[derive(Clone, Debug, Default)]
pub struct SurfaceModel {
    latest: Option<SurfaceSnapshot>,
}

/// Iced's thin adapter over the application pump. It only translates owned
/// snapshots into the existing Elm message bus; it does not own a runtime or
/// perform any page/business work.
#[derive(Clone)]
pub struct SurfaceBridge {
    bridge: SurfacePumpBridge,
    /// Keeps the application worker alive for a composed Iced host. A
    /// pull-only embedder can use [`Self::new`] and own the pump separately.
    _pump: Option<SurfacePump>,
}

impl SurfaceBridge {
    pub fn new(bridge: SurfacePumpBridge) -> Self {
        Self {
            bridge,
            _pump: None,
        }
    }

    /// Create the Iced adapter and retain the application pump handle for the
    /// lifetime of the Iced state. The worker itself remains application-owned.
    pub fn from_pump(pump: SurfacePump) -> Self {
        Self {
            bridge: pump.bridge(),
            _pump: Some(pump),
        }
    }

    pub fn drain_messages(&self) -> Vec<crate::types::message::Message> {
        self.bridge
            .drain_events()
            .into_iter()
            .map(|event| match event {
                infiltrator_contract::surface_snapshot::SurfaceEvent::SnapshotUpdated(snapshot) => {
                    crate::types::message::Message::SurfaceSnapshotUpdated(Box::new(snapshot))
                }
            })
            .collect()
    }

    /// Poll the bounded application bridge from Iced's declarative
    /// subscription system. The bridge is the only cross-thread input; no
    /// executor, task handle, or toolkit state crosses the application port.
    pub fn subscription(&self) -> Subscription<crate::types::message::Message> {
        let input = SurfaceSubscriptionInput {
            identity: self.bridge.identity(),
            bridge: self.bridge.clone(),
        };
        Subscription::run_with(input, build_surface_stream)
    }
}

#[derive(Clone)]
struct SurfaceSubscriptionInput {
    identity: usize,
    bridge: SurfacePumpBridge,
}

impl Hash for SurfaceSubscriptionInput {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.identity.hash(state);
    }
}

fn build_surface_stream(
    input: &SurfaceSubscriptionInput,
) -> BoxStream<'static, crate::types::message::Message> {
    let bridge = input.bridge.clone();
    let channel = stream::channel(
        64,
        move |mut output: iced::futures::channel::mpsc::Sender<crate::types::message::Message>| async move {
            loop {
                for message in bridge.drain_events().into_iter().map(|event| match event {
                    infiltrator_contract::surface_snapshot::SurfaceEvent::SnapshotUpdated(
                        snapshot,
                    ) => crate::types::message::Message::SurfaceSnapshotUpdated(Box::new(snapshot)),
                }) {
                    if output.try_send(message).is_err() {
                        return;
                    }
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        },
    );
    Box::pin(channel)
}

impl SurfaceModel {
    /// Apply only newer revisions. A delayed task or stale host event can
    /// never overwrite a newer projection.
    pub fn apply(&mut self, snapshot: SurfaceSnapshot) -> bool {
        if self
            .latest
            .as_ref()
            .is_some_and(|current| snapshot.revision <= current.revision)
        {
            return false;
        }
        self.latest = Some(snapshot);
        true
    }

    pub fn latest(&self) -> Option<&SurfaceSnapshot> {
        self.latest.as_ref()
    }

    pub fn revision(&self) -> u64 {
        self.latest.as_ref().map_or(0, |snapshot| snapshot.revision)
    }

    pub fn page_status(&self, page: PageId) -> Option<&PageStatus> {
        self.latest
            .as_ref()
            .map(|snapshot| snapshot.page_status(page))
    }
}
