//! Shared connection-telemetry vocabulary for both UI surfaces (DUAL-13).
//!
//! The live-connection feed has a lifecycle that both Iced and Bevy must be
//! able to describe to the user: idle, connecting, live, reconnecting, or
//! unavailable. Iced observes the WebSocket subscription directly; Bevy reads
//! the polled surface snapshot. They express the phase with the same enum so a
//! stream badge on either surface means the same thing.

use crate::surface_snapshot::PageStatus;
use serde::{Deserialize, Serialize};

/// Lifecycle phase of the live connections telemetry feed (DUAL-13-01).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionStreamPhase {
    /// No feed attached and no observation yet.
    #[default]
    Idle,
    /// The feed is attaching for the first time.
    Connecting,
    /// Fresh connection rows are arriving.
    Live,
    /// A previously live feed dropped and is retrying.
    Reconnecting,
    /// The host does not provide the connections feed.
    Unavailable,
}

impl ConnectionStreamPhase {
    /// Canonical ordering used by the segmented badges on both surfaces.
    pub const ALL: [Self; 5] = [
        Self::Idle,
        Self::Connecting,
        Self::Live,
        Self::Reconnecting,
        Self::Unavailable,
    ];

    /// Stable wire/persistence identifier.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Connecting => "connecting",
            Self::Live => "live",
            Self::Reconnecting => "reconnecting",
            Self::Unavailable => "unavailable",
        }
    }

    /// Parse an identifier produced by [`Self::as_str`], defaulting to idle.
    pub fn from_identifier(value: &str) -> Self {
        match value {
            "connecting" => Self::Connecting,
            "live" => Self::Live,
            "reconnecting" => Self::Reconnecting,
            "unavailable" => Self::Unavailable,
            _ => Self::Idle,
        }
    }

    /// Whether the feed is actively delivering rows.
    pub const fn is_live(self) -> bool {
        matches!(self, Self::Live)
    }

    /// Derive the phase a polled surface can honestly claim from its page
    /// status. A ready or explicitly-empty page means rows were observed; a
    /// loading page is still connecting; an unavailable/failed page cannot be
    /// shown as live. Reconnecting is only observable by a streaming surface
    /// (Iced) and is never inferred from a single poll.
    pub fn from_page_status(status: &PageStatus) -> Self {
        match status {
            PageStatus::Loading => Self::Connecting,
            PageStatus::Ready | PageStatus::Empty => Self::Live,
            PageStatus::Unavailable { .. } | PageStatus::Failed { .. } => Self::Unavailable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Failure;

    #[test]
    fn stream_phase_round_trips_its_identifier() {
        for phase in ConnectionStreamPhase::ALL {
            assert_eq!(
                ConnectionStreamPhase::from_identifier(phase.as_str()),
                phase
            );
        }
        assert_eq!(
            ConnectionStreamPhase::from_identifier("bogus"),
            ConnectionStreamPhase::Idle
        );
        assert!(ConnectionStreamPhase::Live.is_live());
        assert!(!ConnectionStreamPhase::Idle.is_live());
    }

    #[test]
    fn page_status_maps_to_an_honest_phase() {
        assert_eq!(
            ConnectionStreamPhase::from_page_status(&PageStatus::Loading),
            ConnectionStreamPhase::Connecting
        );
        assert_eq!(
            ConnectionStreamPhase::from_page_status(&PageStatus::Ready),
            ConnectionStreamPhase::Live
        );
        assert_eq!(
            ConnectionStreamPhase::from_page_status(&PageStatus::Empty),
            ConnectionStreamPhase::Live
        );
        assert_eq!(
            ConnectionStreamPhase::from_page_status(&PageStatus::Unavailable {
                failure: Failure::unsupported("no gateway")
            }),
            ConnectionStreamPhase::Unavailable
        );
    }
}
