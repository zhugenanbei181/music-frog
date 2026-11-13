//! The owner's instant for one observed connection list.
//!
//! A connection list is a fact about the Core at one instant. The kernel
//! publishes the rows; the Core publishes when it accepted them. Without that
//! instant, every consumer that wants to answer "how long has this connection
//! been alive" has to read a clock of its own, and the answer then depends on
//! who renders it. The instant therefore travels with the observation, and a
//! surface only ever reads it.
//!
//! The same rule holds for the duration itself: a row publishes either a
//! duration of its own (a closed record carries the length it lived) or
//! nothing at all, and the honest unknown is a typed variant rather than a
//! zero that would read like "instantaneous".

use chrono::{DateTime, NaiveDateTime, Utc};
use crate::runtime::ConnectionSnapshot;

/// When the Core observed a connection list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ConnectionObservationInstant {
    /// The owner published the instant of this observation.
    Observed { at: DateTime<Utc> },
    /// Nobody published an instant, so no age can be derived from it.
    Unobserved,
}

impl ConnectionObservationInstant {
    /// The instant the owner published.
    pub const fn observed_at(at: DateTime<Utc>) -> Self {
        Self::Observed { at }
    }

    /// An observation that carries no instant.
    pub const fn unobserved() -> Self {
        Self::Unobserved
    }

    /// The published instant, when there is one.
    pub const fn at(self) -> Option<DateTime<Utc>> {
        match self {
            Self::Observed { at } => Some(at),
            Self::Unobserved => None,
        }
    }

    /// The age of a row that started at `start`.
    ///
    /// A start the controller did not report, or one that lies in the future,
    /// is an unknown age rather than a zero: a zero would read like a
    /// connection that just opened.
    pub fn age_of(self, start: &str) -> Option<u64> {
        let at = self.at()?;
        let started_at = parse_connection_start(start)?;
        u64::try_from(at.signed_duration_since(started_at).num_milliseconds()).ok()
    }
}

/// The duration one row published.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ConnectionDuration {
    /// The owner published how long this row lived.
    Observed { duration_ms: u64 },
    /// Nobody published a duration for this row.
    Unobserved,
}

/// One observed connection list and the instant the Core accepted it.
///
/// The rows and their instant are one fact: a list without its instant would
/// let every surface answer "how long has this connection been alive" from a
/// clock of its own, and the answers would drift apart.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObservedConnections {
    snapshot: ConnectionSnapshot,
    instant: ConnectionObservationInstant,
}

impl ObservedConnections {
    /// Pair one observed list with the instant the Core accepted it.
    pub const fn new(snapshot: ConnectionSnapshot, instant: ConnectionObservationInstant) -> Self {
        Self { snapshot, instant }
    }

    /// The observed rows.
    pub const fn snapshot(&self) -> &ConnectionSnapshot {
        &self.snapshot
    }

    /// The instant the Core accepted this list.
    pub const fn instant(&self) -> ConnectionObservationInstant {
        self.instant
    }
}

impl ConnectionDuration {
    /// The published duration in milliseconds, when there is one.
    pub const fn duration_ms(self) -> Option<u64> {
        match self {
            Self::Observed { duration_ms } => Some(duration_ms),
            Self::Unobserved => None,
        }
    }
}

/// Parse a controller start timestamp into an instant.
///
/// Mihomo reports RFC3339; the legacy `%Y-%m-%d %H:%M:%S` form is also read.
/// An unparseable stamp is an unknown start, never an invented one.
pub fn parse_connection_start(start: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(start)
        .map(|value| value.with_timezone(&Utc))
        .ok()
        .or_else(|| {
            NaiveDateTime::parse_from_str(start, "%Y-%m-%d %H:%M:%S")
                .or_else(|_| NaiveDateTime::parse_from_str(start, "%Y-%m-%d %H:%M"))
                .ok()
                .map(|value| value.and_utc())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(text)
            .expect("instant")
            .with_timezone(&Utc)
    }

    #[test]
    fn an_observed_instant_measures_the_age_of_a_started_row() {
        let instant = ConnectionObservationInstant::observed_at(at("2026-10-06T00:05:00Z"));
        assert_eq!(instant.age_of("2026-10-06T00:00:00Z"), Some(300_000));
        assert_eq!(instant.age_of("2026-10-06 00:00:00"), Some(300_000));
    }

    #[test]
    fn an_unobserved_instant_never_invents_an_age() {
        let instant = ConnectionObservationInstant::unobserved();
        assert_eq!(instant.at(), None);
        assert_eq!(instant.age_of("2026-10-06T00:00:00Z"), None);
    }

    #[test]
    fn an_unparseable_or_future_start_is_an_unknown_age() {
        let instant = ConnectionObservationInstant::observed_at(at("2026-10-06T00:05:00Z"));
        assert_eq!(instant.age_of(""), None);
        assert_eq!(instant.age_of("not a start"), None);
        assert_eq!(instant.age_of("2026-10-06T00:10:00Z"), None);
    }

    #[test]
    fn the_duration_fact_is_typed() {
        assert_eq!(
            ConnectionDuration::Observed { duration_ms: 42 }.duration_ms(),
            Some(42)
        );
        assert_eq!(ConnectionDuration::Unobserved.duration_ms(), None);
    }
}
