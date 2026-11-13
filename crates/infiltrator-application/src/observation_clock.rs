//! The Core's own clock.
//!
//! Exactly one function in the Core reads the wall clock for an observation.
//! Every owner that answers "when did the Core accept this fact" calls it, so
//! two owners can never publish two different instants for one observation,
//! and no surface has to read a clock of its own.

use chrono::{DateTime, Utc};

/// The instant the Core accepts a fact at.
pub fn accepted_at() -> DateTime<Utc> {
    Utc::now()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_core_clock_reads_one_instant() {
        let first = accepted_at();
        let second = accepted_at();
        assert!(second >= first);
    }
}
