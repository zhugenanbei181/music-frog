//! Shared idle-connection activity tracker (DUAL-13-11).
//!
//! The core's `/connections` payload carries cumulative byte counters but no
//! per-connection last-activity timestamp, so "idle" can only be observed by
//! watching a connection's counters stop moving between two feeds. This tracker
//! is the one seam that does that for both surfaces: each surface feeds its
//! live connection slice in and reads back the ids whose counters have not
//! moved for longer than the configured timeout.
//!
//! It never invents activity: a first observation always counts as active now,
//! and an id is only reported idle after genuinely unchanged counters across a
//! full timeout window. Ids that disappear from the feed are forgotten.

use crate::connection_view::ConnectionView;
use std::collections::{HashMap, HashSet};

/// Default idle timeout: 10 minutes without a byte change.
pub const DEFAULT_IDLE_TIMEOUT_SECS: u64 = 600;

/// The timeout choices both surfaces expose identically (5 / 10 / 30 min).
pub const IDLE_TIMEOUT_CHOICES: [u64; 3] = [300, 600, 1800];

/// Formats a timeout in seconds as the compact minutes label both surfaces show.
pub fn idle_timeout_minutes_label(timeout_secs: u64) -> String {
    format!("{}m", timeout_secs / 60)
}

/// Tracks the last time each connection's cumulative counters changed.
#[derive(Clone, Debug, Default)]
pub struct ConnectionActivityTracker {
    last_active_secs: HashMap<String, u64>,
    last_totals: HashMap<String, (u64, u64)>,
}

impl ConnectionActivityTracker {
    /// A tracker with no observations.
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed the current connection slice at `now_secs`. A connection whose
    /// cumulative counters changed, or that is seen for the first time, counts
    /// as active at `now_secs`. Ids absent from this slice are forgotten.
    pub fn observe<C: ConnectionView>(&mut self, conns: &[C], now_secs: u64) {
        let mut seen: HashSet<String> = HashSet::with_capacity(conns.len());
        for conn in conns {
            let id = conn.view_id().to_string();
            let totals = (conn.view_upload_total(), conn.view_download_total());
            let changed = self.last_totals.get(&id) != Some(&totals);
            if changed || !self.last_active_secs.contains_key(&id) {
                self.last_active_secs.insert(id.clone(), now_secs);
            }
            self.last_totals.insert(id.clone(), totals);
            seen.insert(id);
        }
        self.last_active_secs.retain(|id, _| seen.contains(id));
        self.last_totals.retain(|id, _| seen.contains(id));
    }

    /// Ids whose counters have not moved for **more than** `timeout_secs`,
    /// sorted for deterministic UI/test ordering.
    pub fn idle_ids(&self, now_secs: u64, timeout_secs: u64) -> Vec<String> {
        let mut ids: Vec<String> = self
            .last_active_secs
            .iter()
            .filter(|(_, last)| now_secs.saturating_sub(**last) > timeout_secs)
            .map(|(id, _)| id.clone())
            .collect();
        ids.sort();
        ids
    }

    /// Last observed activity instant for one connection.
    pub fn last_active_secs(&self, id: &str) -> Option<u64> {
        self.last_active_secs.get(id).copied()
    }

    /// Number of connections currently tracked.
    pub fn tracked(&self) -> usize {
        self.last_active_secs.len()
    }

    /// Whether nothing is tracked.
    pub fn is_empty(&self) -> bool {
        self.last_active_secs.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{Connection, ConnectionMetadata};

    fn connection(id: &str, up: u64, down: u64) -> Connection {
        Connection {
            id: id.to_string(),
            metadata: ConnectionMetadata::default(),
            upload: up,
            download: down,
            start: String::new(),
            rule: String::new(),
            rule_payload: String::new(),
            chains: Vec::new(),
        }
    }

    #[test]
    fn first_observation_counts_as_active() {
        let mut tracker = ConnectionActivityTracker::new();
        tracker.observe(&[connection("c1", 0, 0)], 100);
        assert_eq!(tracker.last_active_secs("c1"), Some(100));
        assert!(tracker.idle_ids(100, 600).is_empty());
        assert_eq!(tracker.tracked(), 1);
        assert!(!tracker.is_empty());
    }

    #[test]
    fn unchanged_counters_become_idle_only_after_timeout() {
        let mut tracker = ConnectionActivityTracker::new();
        tracker.observe(&[connection("c1", 10, 20)], 0);
        // Counter moved → active again at 500.
        tracker.observe(&[connection("c1", 30, 40)], 500);
        assert_eq!(tracker.last_active_secs("c1"), Some(500));
        // Not yet past the timeout.
        assert!(tracker.idle_ids(1000, 600).is_empty());
        // 500 + 600 = 1100; strictly greater than the window.
        assert!(tracker.idle_ids(1100, 600).is_empty());
        assert_eq!(tracker.idle_ids(1101, 600), ["c1"]);
    }

    #[test]
    fn disappeared_ids_are_forgotten() {
        let mut tracker = ConnectionActivityTracker::new();
        tracker.observe(&[connection("c1", 1, 1), connection("c2", 1, 1)], 10);
        tracker.observe(&[connection("c2", 1, 1)], 20);
        assert_eq!(tracker.last_active_secs("c1"), None);
        assert_eq!(tracker.tracked(), 1);
        assert_eq!(tracker.idle_ids(10_000, 600), ["c2"]);
    }

    #[test]
    fn timeout_choices_and_labels_are_shared() {
        assert_eq!(IDLE_TIMEOUT_CHOICES, [300, 600, 1800]);
        assert_eq!(DEFAULT_IDLE_TIMEOUT_SECS, 600);
        assert_eq!(idle_timeout_minutes_label(300), "5m");
        assert_eq!(idle_timeout_minutes_label(1800), "30m");
    }
}
