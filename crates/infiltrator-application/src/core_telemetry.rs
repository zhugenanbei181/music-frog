//! Session-fenced telemetry reads use the composed port and share the existing rate fold.
use super::CoreApplication;
use crate::overview::rates_from_totals;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::session::SessionToken;
use infiltrator_contract::snapshot::{CoreLifecycle, CoreSnapshot};
use infiltrator_ports::overview::OverviewSample;
use std::sync::atomic::Ordering;
use std::time::Instant;

#[derive(Default)]
pub(super) struct TelemetryState {
    session: Option<(u64, Option<SessionToken>)>,
    previous: Option<(u64, u64, Instant)>,
    epoch: Option<i64>,
}

#[cfg(test)]
#[path = "core_telemetry_test.rs"]
mod tests;
impl TelemetryState {
    fn apply(
        &mut self,
        mut core: CoreSnapshot,
        sample: OverviewSample,
        now: Instant,
    ) -> CoreSnapshot {
        let session = (core.generation, core.session_token);
        if self.session != Some(session) {
            *self = Self {
                session: Some(session),
                ..Default::default()
            };
        }
        if sample.sampled_at_epoch_ms.is_none()
            || sample
                .sampled_at_epoch_ms
                .zip(self.epoch)
                .is_some_and(|(next, old)| next <= old)
        {
            core.sampled_at_epoch_ms = None;
            return core;
        }
        let ordered = sample
            .sampled_at_epoch_ms
            .zip(self.epoch)
            .is_some_and(|(next, old)| next > old);
        let continuous = self.previous.is_some_and(|(up, down, before)| {
            sample.upload_total >= up && sample.download_total >= down && now > before
        });
        let rates = rates_from_totals(
            self.previous,
            sample.upload_total,
            sample.download_total,
            now,
        );
        core.sampled_at_epoch_ms = if ordered && continuous {
            sample.sampled_at_epoch_ms
        } else {
            None
        };
        core.upload_bps = rates.0;
        core.download_bps = rates.1;
        core.proxy_mode = sample.mode;
        core.core_version = sample.core_version;
        core.memory_bytes = sample.memory_bytes;
        core.active_connections = sample.active_connections;
        self.previous = Some((sample.upload_total, sample.download_total, now));
        self.epoch = sample.sampled_at_epoch_ms;
        core
    }
}

impl CoreApplication {
    pub async fn read_telemetry(&self) -> Result<CoreSnapshot, Failure> {
        let mut telemetry = self.inner.telemetry.lock().await;
        if self.inner.closed.load(Ordering::Acquire) {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "The product session is closed",
                false,
            ));
        }
        let before = self.snapshot();
        if !matches!(
            before.lifecycle,
            CoreLifecycle::Running | CoreLifecycle::Ready
        ) {
            return Ok(before);
        }
        let Some(reader) = self.inner.overview.as_ref() else {
            return Err(Failure::unsupported("Core telemetry is not composed"));
        };
        let sample = match reader.sample().await {
            Ok(sample) => sample,
            Err(error) => {
                *telemetry = TelemetryState::default();
                return Err(Failure::from(error));
            }
        };
        if !matches!(
            sample.lifecycle,
            CoreLifecycle::Running | CoreLifecycle::Ready
        ) {
            *telemetry = TelemetryState::default();
            return Err(Failure::new(
                ErrorCode::NotReady,
                "The telemetry source is not running",
                true,
            ));
        }
        let after = self.snapshot();
        if self.inner.closed.load(Ordering::Acquire)
            || before.generation != after.generation
            || before.session_token != after.session_token
            || !matches!(
                after.lifecycle,
                CoreLifecycle::Running | CoreLifecycle::Ready
            )
        {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "The core session changed during telemetry read",
                true,
            ));
        }
        Ok(telemetry.apply(after, sample, Instant::now()))
    }
}
