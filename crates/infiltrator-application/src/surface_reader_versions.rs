//! Engine-scoped version observation used by the complete surface reader.

use crate::surface_reader::ApplicationSurfaceReader;
use infiltrator_contract::version::{CoreChannelStatus, CoreVersionSnapshot};
use std::time::{Duration, Instant};

impl ApplicationSurfaceReader {
    pub(super) async fn read_versions(&self) -> CoreVersionSnapshot {
        let Some(versions) = &self.versions else {
            let Some(engine_id) = self
                .core
                .snapshot()
                .identity
                .map(|identity| identity.engine)
            else {
                return CoreVersionSnapshot::unavailable();
            };
            return CoreVersionSnapshot::for_engine(engine_id);
        };
        if let Some((probed_at, snapshot)) = self
            .version_cache
            .lock()
            .expect("version cache lock")
            .as_ref()
            .cloned()
        {
            let has_ready_channel = snapshot
                .channels
                .iter()
                .any(|channel| matches!(channel.status, CoreChannelStatus::Ready { .. }));
            let ttl = if has_ready_channel {
                Duration::from_secs(600)
            } else {
                Duration::from_secs(30)
            };
            if snapshot.engine_id == *versions.engine_id() && probed_at.elapsed() < ttl {
                return snapshot;
            }
        }

        let next_revision = self
            .version_cache
            .lock()
            .expect("version cache lock")
            .as_ref()
            .filter(|(_, snapshot)| snapshot.engine_id == *versions.engine_id())
            .map_or(1, |(_, snapshot)| snapshot.revision.saturating_add(1));
        let mut snapshot = versions.probe_channels().await;
        snapshot.revision = next_revision;
        *self.version_cache.lock().expect("version cache lock") =
            Some((Instant::now(), snapshot.clone()));
        snapshot
    }
}
