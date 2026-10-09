//! BEVY-020: the bounded in-memory window behind the Logs page.
//!
//! The surface may stream an ever-growing log set, but the UI layer must never
//! keep it all. [`LogsRing`] ingests each projection into the shared
//! [`FixedRingBuffer`], merging by the stable entry id, so the mounted rows,
//! the regex search and the highlight replay all read the same bounded tail.
//! [`LastLogsProjection`](crate::pages::logs::LastLogsProjection) is rebuilt
//! from the ring, never from the raw event, so a continuously growing stream
//! cannot grow the UI heap.

use crate::pages::logs::{LogEntry, LogsProjection};
use bevy::ecs::resource::Resource;
use infiltrator_bevy_widgets::chart::ring_buffer::FixedRingBuffer;
use infiltrator_contract::logs::LogLevel;
use infiltrator_contract::session::SessionToken;
use infiltrator_contract::surface_snapshot::PageStatus;

/// Maximum number of log entries the UI layer keeps in memory. The shared
/// charter names a 500-entry ring (BEVY-020); the mounted row topology is
/// bounded further by the virtual window.
pub const LOGS_RING_CAPACITY: usize = 500;

/// Keep only the newest [`LOGS_RING_CAPACITY`] entries of a projection. The
/// full stream may arrive once; the UI only ever stores the tail.
pub fn bounded_tail(mut entries: Vec<LogEntry>) -> Vec<LogEntry> {
    if entries.len() > LOGS_RING_CAPACITY {
        let drop_count = entries.len() - LOGS_RING_CAPACITY;
        entries.drain(..drop_count);
    }
    entries
}

/// The bounded log window. One resource holds the newest entries plus the
/// projection metadata the page renders (status, level filter, generation).
#[derive(Resource, Clone, Debug)]
pub struct LogsRing {
    entries: FixedRingBuffer<LogEntry, LOGS_RING_CAPACITY>,
    total_entries: usize,
    generation: u64,
    session_token: Option<SessionToken>,
    status: PageStatus,
    active_level: Option<LogLevel>,
}

impl Default for LogsRing {
    fn default() -> Self {
        Self::new()
    }
}

impl LogsRing {
    /// An empty window waiting for its first projection.
    pub fn new() -> Self {
        Self {
            entries: FixedRingBuffer::new(),
            total_entries: 0,
            generation: 0,
            session_token: None,
            status: PageStatus::Loading,
            active_level: None,
        }
    }

    /// Ingest one projection into the bounded window.
    ///
    /// The incoming entries are merged by the stable entry id (existing rows
    /// take the fresh copy, new ids append), then only the newest
    /// [`LOGS_RING_CAPACITY`] survive. A session change, a severity-filter
    /// change or a shrinking source total (the buffer was cleared) starts a
    /// fresh window instead of merging stale rows.
    pub fn ingest(&mut self, projection: &LogsProjection) {
        if self.session_token != projection.session_token
            || projection.total_entries < self.total_entries
            || self.active_level != projection.active_level
        {
            self.entries.clear();
            self.total_entries = 0;
        }
        self.session_token = projection.session_token;
        self.generation = projection.generation;
        self.status = projection.status.clone();
        self.active_level = projection.active_level;
        // `total_entries` is the source's current buffer length; it is
        // authoritative and may shrink when the buffer is cleared.
        self.total_entries = self.total_entries.max(projection.total_entries);

        let mut merged = self.entries.to_vec();
        for entry in &projection.entries {
            match merged.iter_mut().find(|held| held.id == entry.id) {
                Some(held) => *held = entry.clone(),
                None => merged.push(entry.clone()),
            }
        }
        merged.sort_by_key(|entry| entry.id);
        if merged.len() > LOGS_RING_CAPACITY {
            let drop_count = merged.len() - LOGS_RING_CAPACITY;
            merged.drain(..drop_count);
        }
        self.total_entries = self.total_entries.max(merged.len());
        self.entries.clear();
        for entry in merged {
            self.entries.push(entry);
        }
    }

    /// The bounded projection the page, search and recycler all consume.
    pub fn snapshot(&self) -> LogsProjection {
        LogsProjection {
            status: self.status.clone(),
            generation: self.generation,
            session_token: self.session_token,
            total_entries: self.total_entries,
            active_level: self.active_level,
            entries: self.entries.to_vec(),
        }
    }

    /// Number of entries currently held (never above [`LOGS_RING_CAPACITY`]).
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the window holds no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The fixed capacity of the window.
    pub const fn capacity(&self) -> usize {
        LOGS_RING_CAPACITY
    }

    /// The newest entry, when the window is not empty.
    pub fn newest(&self) -> Option<&LogEntry> {
        self.entries.newest()
    }

    /// The oldest entry still held, when the window is not empty.
    pub fn oldest(&self) -> Option<&LogEntry> {
        self.entries.oldest()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pages::logs::LogsProjection;

    fn entry(id: u64) -> LogEntry {
        LogEntry {
            id,
            timestamp: format!("10:00:{:02}.000", id % 60),
            level: LogLevel::Info,
            tag: "TCP".to_owned(),
            message: format!("log line {id}"),
        }
    }

    fn delta(id: u64, total: usize) -> LogsProjection {
        LogsProjection {
            status: PageStatus::Ready,
            generation: id,
            session_token: None,
            total_entries: total,
            active_level: None,
            entries: vec![entry(id)],
        }
    }

    #[test]
    fn ring_stays_bounded_under_a_growing_stream_and_keeps_the_tail() {
        let mut ring = LogsRing::new();
        for id in 0..20_000u64 {
            ring.ingest(&delta(id, id as usize + 1));
        }
        assert_eq!(ring.len(), LOGS_RING_CAPACITY);
        assert_eq!(ring.newest().map(|entry| entry.id), Some(19_999));
        assert_eq!(
            ring.oldest().map(|entry| entry.id),
            Some(19_999 - (LOGS_RING_CAPACITY as u64 - 1))
        );

        let snapshot = ring.snapshot();
        assert!(snapshot.entries.len() <= LOGS_RING_CAPACITY);
        assert_eq!(snapshot.total_entries, 20_000);
        assert_eq!(snapshot.entries.last().map(|entry| entry.id), Some(19_999));
    }

    #[test]
    fn full_snapshot_resend_does_not_duplicate_entries() {
        let mut ring = LogsRing::new();
        let projection = LogsProjection {
            status: PageStatus::Ready,
            generation: 1,
            session_token: None,
            total_entries: 5,
            active_level: None,
            entries: (0..5).map(entry).collect(),
        };
        ring.ingest(&projection);
        ring.ingest(&projection);
        assert_eq!(ring.len(), 5);
        assert_eq!(ring.snapshot().entries.len(), 5);
    }

    #[test]
    fn bounded_tail_keeps_only_the_newest_capacity() {
        let entries: Vec<_> = (0..(LOGS_RING_CAPACITY as u64 + 120)).map(entry).collect();
        let bounded = bounded_tail(entries);
        assert_eq!(bounded.len(), LOGS_RING_CAPACITY);
        assert_eq!(bounded.first().map(|entry| entry.id), Some(120));
        assert_eq!(
            bounded.last().map(|entry| entry.id),
            Some(LOGS_RING_CAPACITY as u64 + 119)
        );
    }

    #[test]
    fn session_change_resets_the_window() {
        let mut ring = LogsRing::new();
        ring.ingest(&LogsProjection {
            status: PageStatus::Ready,
            generation: 1,
            session_token: None,
            total_entries: 5,
            active_level: None,
            entries: (0..5).map(entry).collect(),
        });
        assert_eq!(ring.len(), 5);

        let mut next = delta(900, 1);
        next.session_token = Some(SessionToken::new(7));
        ring.ingest(&next);
        assert_eq!(ring.len(), 1);
        assert_eq!(ring.newest().map(|entry| entry.id), Some(900));
        assert_eq!(ring.snapshot().total_entries, 1);
    }

    #[test]
    fn shrinking_source_total_resets_the_window_on_clear() {
        let mut ring = LogsRing::new();
        ring.ingest(&LogsProjection {
            status: PageStatus::Ready,
            generation: 1,
            session_token: None,
            total_entries: 5,
            active_level: None,
            entries: (0..5).map(entry).collect(),
        });
        assert_eq!(ring.len(), 5);

        // Clear: the source buffer is empty and the total drops to zero.
        ring.ingest(&LogsProjection {
            status: PageStatus::Empty,
            generation: 1,
            session_token: None,
            total_entries: 0,
            active_level: None,
            entries: Vec::new(),
        });
        assert!(ring.is_empty());
        assert_eq!(ring.snapshot().total_entries, 0);
    }

    #[test]
    fn severity_filter_change_replaces_the_window_view() {
        let mut ring = LogsRing::new();
        ring.ingest(&LogsProjection {
            status: PageStatus::Ready,
            generation: 1,
            session_token: None,
            total_entries: 3,
            active_level: None,
            entries: (0..3).map(entry).collect(),
        });
        assert_eq!(ring.len(), 3);

        // Filtering to a severity narrows the visible set even though the
        // source total stays the same; the window must follow the filter, not
        // union the previous view with the new one.
        ring.ingest(&LogsProjection {
            status: PageStatus::Ready,
            generation: 1,
            session_token: None,
            total_entries: 3,
            active_level: Some(LogLevel::Warn),
            entries: vec![entry(1)],
        });
        assert_eq!(ring.len(), 1);
        assert_eq!(ring.newest().map(|entry| entry.id), Some(1));
        assert_eq!(ring.snapshot().total_entries, 3);
    }
}
