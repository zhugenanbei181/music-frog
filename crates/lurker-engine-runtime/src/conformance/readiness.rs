//! Bounded, executor-neutral readiness waiting shared by the conformance run.

use futures_timer::Delay;
use lurker_core_contract::identity::CoreVersion;
use lurker_core_ports::core_process::CoreReadiness;
use lurker_core_ports::error::PortError;
use std::time::{Duration, Instant};

const READINESS_RETRY_INTERVAL: Duration = Duration::from_millis(100);

/// Probe until the adapter reports readiness or the bounded deadline passes.
///
/// A zero timeout still permits one probe; every later retry is bounded by the
/// remaining budget and the fixed retry interval.
pub(super) async fn wait_for_readiness(
    readiness: &dyn CoreReadiness,
    timeout: Duration,
) -> Result<Option<CoreVersion>, PortError> {
    let now = Instant::now();
    let deadline = now.checked_add(timeout).unwrap_or(now);
    loop {
        let error = match readiness.probe().await {
            Ok(_) => return readiness.active_core_version().await,
            Err(error) => error,
        };
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(error);
        }
        Delay::new(remaining.min(READINESS_RETRY_INTERVAL)).await;
    }
}
