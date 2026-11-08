//! The traffic-history command, on the shared waveform owner.
//!
//! `ClearTrafficHistory` used to acknowledge success without touching any
//! history owner at all. The curve this command clears is owned by ONE
//! `TrafficWaveformApplication` inside `CoreApplication`: the surface reader
//! records into that instance and the command handler resets it, so the
//! reader sees the empty curve afterwards. A host that never composed that
//! owner reports an honest typed refusal instead of pretending to have
//! cleared anything.

use super::CommandApplication;
use lurker_core_contract::error::{ErrorCode, Failure};

impl CommandApplication {
    /// Clear the live traffic history the shared waveform owner holds.
    ///
    /// Upstream `traffic.reset` clears the cumulative counters; this
    /// repository's history is an application-owned sample history rather
    /// than an engine counter, so the route is local to this owner.
    pub(super) fn clear_traffic_history(&self) -> Result<(), Failure> {
        let Some(waveform) = self.traffic_waveform.as_ref() else {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "clear traffic history has no composed waveform owner",
                true,
            ));
        };
        waveform.reset();
        Ok(())
    }
}
