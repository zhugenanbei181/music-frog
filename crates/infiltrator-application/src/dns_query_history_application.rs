//! DNS query history tracking and ring buffer application service (FLPARITY-004).
//!
//! Maintains an in-memory bounded ring buffer of resolved DNS queries matching
//! FlClash v0.9.2, supporting live tracking, pause/resume, search filtering, and clearing.

use infiltrator_contract::dns_query::DnsRecordType;
use infiltrator_contract::dns_query_history::{
    DEFAULT_DNS_HISTORY_CAPACITY, DnsHistoryRecordingState, DnsQueryHistoryRecord,
    DnsQueryHistorySnapshot, DnsQueryStatus,
};
use infiltrator_ports::dns_query_history::DnsQueryHistoryPort;
use lurker_core_contract::error::{ErrorCode, Failure};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

struct BufferState {
    records: VecDeque<DnsQueryHistoryRecord>,
    capacity: usize,
    next_id: u64,
    total_queries_observed: u64,
    recording_state: DnsHistoryRecordingState,
    generation: u64,
    revision: u64,
}

/// Application service for observing, tracking, and buffering DNS query history.
#[derive(Clone)]
pub struct DnsQueryHistoryApplication {
    port: Option<Arc<dyn DnsQueryHistoryPort>>,
    state: Arc<Mutex<BufferState>>,
}

impl DnsQueryHistoryApplication {
    pub fn new(port: Option<Arc<dyn DnsQueryHistoryPort>>, capacity: usize) -> Self {
        let cap = if capacity == 0 {
            DEFAULT_DNS_HISTORY_CAPACITY
        } else {
            capacity
        };
        Self {
            port,
            state: Arc::new(Mutex::new(BufferState {
                records: VecDeque::with_capacity(cap),
                capacity: cap,
                next_id: 1,
                total_queries_observed: 0,
                recording_state: DnsHistoryRecordingState::Active,
                generation: 1,
                revision: 1,
            })),
        }
    }

    pub fn with_default_capacity(port: Option<Arc<dyn DnsQueryHistoryPort>>) -> Self {
        Self::new(port, DEFAULT_DNS_HISTORY_CAPACITY)
    }

    /// Record an incoming DNS query resolution into the bounded history buffer.
    pub fn record(&self, mut record: DnsQueryHistoryRecord) -> u64 {
        let Ok(mut guard) = self.state.lock() else {
            return 0;
        };
        if record.id == 0 {
            record.id = guard.next_id;
            guard.next_id = guard.next_id.saturating_add(1);
        }
        let assigned_id = record.id;
        guard.total_queries_observed = guard.total_queries_observed.saturating_add(1);

        if guard.recording_state.is_active() {
            if guard.records.len() >= guard.capacity {
                guard.records.pop_front();
            }
            guard.records.push_back(record);
            guard.revision = guard.revision.saturating_add(1);
        }

        assigned_id
    }

    /// Record a DNS query from granular facts.
    pub fn record_query_facts(
        &self,
        timestamp_epoch_ms: u64,
        name: impl Into<String>,
        record_type: DnsRecordType,
        status: DnsQueryStatus,
        latency_ms: u32,
        answers: Vec<String>,
        server: Option<String>,
    ) -> u64 {
        let mut rec = DnsQueryHistoryRecord::new(
            0,
            timestamp_epoch_ms,
            name,
            record_type,
            status,
            latency_ms,
        );
        rec.answers = answers;
        rec.server = server;
        self.record(rec)
    }

    /// Clear all stored historic queries in the ring buffer.
    pub async fn clear(&self) -> Result<(), Failure> {
        if let Ok(mut guard) = self.state.lock() {
            guard.records.clear();
            guard.revision = guard.revision.saturating_add(1);
        }
        if let Some(port) = &self.port {
            port.clear_query_stream()
                .await
                .map_err(|e| Failure::new(ErrorCode::Network, e.to_string(), true))?;
        }
        Ok(())
    }

    /// Pause tracking new query entries into the buffer.
    pub fn pause(&self) {
        if let Ok(mut guard) = self.state.lock() {
            guard.recording_state = DnsHistoryRecordingState::Paused;
            guard.revision = guard.revision.saturating_add(1);
        }
    }

    /// Resume tracking new query entries into the buffer.
    pub fn resume(&self) {
        if let Ok(mut guard) = self.state.lock() {
            guard.recording_state = DnsHistoryRecordingState::Active;
            guard.revision = guard.revision.saturating_add(1);
        }
    }

    /// Toggle the paused state, returning the new recording state.
    pub fn toggle_pause(&self) -> DnsHistoryRecordingState {
        if let Ok(mut guard) = self.state.lock() {
            guard.recording_state = match guard.recording_state {
                DnsHistoryRecordingState::Active => DnsHistoryRecordingState::Paused,
                DnsHistoryRecordingState::Paused => DnsHistoryRecordingState::Active,
            };
            guard.revision = guard.revision.saturating_add(1);
            guard.recording_state
        } else {
            DnsHistoryRecordingState::Active
        }
    }

    /// Poll upstream controller stream for newly resolved queries.
    pub async fn poll_upstream(&self) -> Result<usize, Failure> {
        let Some(port) = &self.port else {
            return Ok(0);
        };
        let fetched = port
            .fetch_recent_queries()
            .await
            .map_err(|e| Failure::new(ErrorCode::Network, e.to_string(), true))?;
        let count = fetched.len();
        for query in fetched {
            self.record(query);
        }
        Ok(count)
    }

    /// Produce a read-only immutable snapshot of the DNS query history.
    pub fn snapshot(&self) -> DnsQueryHistorySnapshot {
        let Ok(guard) = self.state.lock() else {
            return DnsQueryHistorySnapshot::default();
        };
        DnsQueryHistorySnapshot {
            generation: guard.generation,
            revision: guard.revision,
            records: guard.records.iter().cloned().collect(),
            capacity: guard.capacity,
            total_queries_observed: guard.total_queries_observed,
            recording_state: guard.recording_state,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use lurker_core_ports::error::PortError;

    struct MockDnsHistoryPort {
        cleared: Arc<Mutex<bool>>,
    }

    #[async_trait]
    impl DnsQueryHistoryPort for MockDnsHistoryPort {
        async fn fetch_recent_queries(&self) -> Result<Vec<DnsQueryHistoryRecord>, PortError> {
            Ok(vec![DnsQueryHistoryRecord::new(
                0,
                1000,
                "polled.example.com",
                DnsRecordType::A,
                DnsQueryStatus::Success,
                10,
            )])
        }

        async fn clear_query_stream(&self) -> Result<(), PortError> {
            if let Ok(mut guard) = self.cleared.lock() {
                *guard = true;
            }
            Ok(())
        }
    }

    #[test]
    fn ring_buffer_fifo_eviction() {
        let app = DnsQueryHistoryApplication::new(None, 3);

        app.record_query_facts(
            100,
            "host1.test",
            DnsRecordType::A,
            DnsQueryStatus::Success,
            10,
            vec![],
            None,
        );
        app.record_query_facts(
            200,
            "host2.test",
            DnsRecordType::A,
            DnsQueryStatus::Success,
            10,
            vec![],
            None,
        );
        app.record_query_facts(
            300,
            "host3.test",
            DnsRecordType::A,
            DnsQueryStatus::Success,
            10,
            vec![],
            None,
        );

        let snap1 = app.snapshot();
        assert_eq!(snap1.records.len(), 3);
        assert_eq!(snap1.records[0].name, "host1.test");
        assert_eq!(snap1.records[2].name, "host3.test");

        // Pushing a 4th item evicts host1.test
        app.record_query_facts(
            400,
            "host4.test",
            DnsRecordType::A,
            DnsQueryStatus::Success,
            10,
            vec![],
            None,
        );

        let snap2 = app.snapshot();
        assert_eq!(snap2.records.len(), 3);
        assert_eq!(snap2.records[0].name, "host2.test");
        assert_eq!(snap2.records[1].name, "host3.test");
        assert_eq!(snap2.records[2].name, "host4.test");
        assert_eq!(snap2.total_queries_observed, 4);
    }

    #[test]
    fn pause_and_resume_tracking() {
        let app = DnsQueryHistoryApplication::new(None, 10);

        app.record_query_facts(
            100,
            "active1.test",
            DnsRecordType::A,
            DnsQueryStatus::Success,
            5,
            vec![],
            None,
        );
        assert_eq!(app.snapshot().records.len(), 1);

        app.pause();
        assert!(app.snapshot().is_recording_paused());
        assert_eq!(
            app.snapshot().recording_state,
            DnsHistoryRecordingState::Paused
        );

        // While paused, records are observed in counter but not stored in active records
        app.record_query_facts(
            200,
            "paused.test",
            DnsRecordType::A,
            DnsQueryStatus::Success,
            5,
            vec![],
            None,
        );
        let snap_paused = app.snapshot();
        assert_eq!(snap_paused.records.len(), 1);
        assert_eq!(snap_paused.total_queries_observed, 2);

        app.resume();
        assert!(!app.snapshot().is_recording_paused());
        assert_eq!(
            app.snapshot().recording_state,
            DnsHistoryRecordingState::Active
        );

        app.record_query_facts(
            300,
            "active2.test",
            DnsRecordType::A,
            DnsQueryStatus::Success,
            5,
            vec![],
            None,
        );
        assert_eq!(app.snapshot().records.len(), 2);

        let toggled = app.toggle_pause();
        assert_eq!(toggled, DnsHistoryRecordingState::Paused);
        assert!(app.snapshot().is_recording_paused());
    }

    #[tokio::test]
    async fn clear_history_and_poll() {
        let cleared_flag = Arc::new(Mutex::new(false));
        let port = Arc::new(MockDnsHistoryPort {
            cleared: cleared_flag.clone(),
        });
        let app = DnsQueryHistoryApplication::new(Some(port), 10);

        app.record_query_facts(
            100,
            "test.local",
            DnsRecordType::A,
            DnsQueryStatus::Success,
            5,
            vec![],
            None,
        );
        assert_eq!(app.snapshot().records.len(), 1);

        app.clear().await.unwrap();
        assert_eq!(app.snapshot().records.len(), 0);
        assert!(*cleared_flag.lock().unwrap());

        let polled = app.poll_upstream().await.unwrap();
        assert_eq!(polled, 1);
        assert_eq!(app.snapshot().records.len(), 1);
        assert_eq!(app.snapshot().records[0].name, "polled.example.com");
    }
}
