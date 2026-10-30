//! Request tracking and bounded ring buffer application service (FLPARITY-003).
//!
//! Maintains an in-memory bounded ring buffer of observed network requests matching
//! FlClash v0.9.2, supporting live tracking, pause/resume, lifecycle transitions,
//! search filtering, and clearing.

use infiltrator_contract::request_tracker::{
    DEFAULT_REQUEST_HISTORY_CAPACITY, RequestHistoryFilter, RequestRecord, RequestStatus,
    RequestTrackerSnapshot, RequestTrackingState,
};
use infiltrator_ports::request_tracker::RequestTrackerPort;
use lurker_core_contract::error::{ErrorCode, Failure};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

struct BufferState {
    records: VecDeque<RequestRecord>,
    capacity: usize,
    total_requests_observed: u64,
    tracking_state: RequestTrackingState,
    generation: u64,
    revision: u64,
}

/// Application service for observing, tracking, and buffering real-time network requests.
#[derive(Clone)]
pub struct RequestTrackerApplication {
    port: Option<Arc<dyn RequestTrackerPort>>,
    state: Arc<Mutex<BufferState>>,
}

impl RequestTrackerApplication {
    pub fn new(port: Option<Arc<dyn RequestTrackerPort>>, capacity: usize) -> Self {
        let cap = if capacity == 0 {
            DEFAULT_REQUEST_HISTORY_CAPACITY
        } else {
            capacity
        };
        Self {
            port,
            state: Arc::new(Mutex::new(BufferState {
                records: VecDeque::with_capacity(cap),
                capacity: cap,
                total_requests_observed: 0,
                tracking_state: RequestTrackingState::Active,
                generation: 1,
                revision: 1,
            })),
        }
    }

    pub fn with_default_capacity(port: Option<Arc<dyn RequestTrackerPort>>) -> Self {
        Self::new(port, DEFAULT_REQUEST_HISTORY_CAPACITY)
    }

    /// Record an incoming network request into the bounded history buffer.
    pub fn record(&self, record: RequestRecord) {
        let Ok(mut guard) = self.state.lock() else {
            return;
        };
        guard.total_requests_observed = guard.total_requests_observed.saturating_add(1);

        if guard.tracking_state.is_active() {
            if guard.records.len() >= guard.capacity {
                guard.records.pop_front();
            }
            guard.records.push_back(record);
            guard.revision = guard.revision.saturating_add(1);
        }
    }

    /// Mark an existing active request as closed with its final traffic metrics.
    pub fn record_closed(
        &self,
        id: &str,
        duration_ms: u64,
        upload_bytes: u64,
        download_bytes: u64,
    ) {
        let Ok(mut guard) = self.state.lock() else {
            return;
        };
        if let Some(record) = guard.records.iter_mut().find(|r| r.id == id) {
            record.status = RequestStatus::Closed;
            record.duration_ms = duration_ms;
            record.upload_bytes = upload_bytes;
            record.download_bytes = download_bytes;
            guard.revision = guard.revision.saturating_add(1);
        }
    }

    /// Clear all stored request records in the ring buffer.
    pub async fn clear(&self) -> Result<(), Failure> {
        if let Ok(mut guard) = self.state.lock() {
            guard.records.clear();
            guard.revision = guard.revision.saturating_add(1);
        }
        if let Some(port) = &self.port {
            port.clear_request_stream()
                .await
                .map_err(|e| Failure::new(ErrorCode::Network, e.to_string(), true))?;
        }
        Ok(())
    }

    /// Pause tracking new requests into the buffer.
    pub fn pause(&self) {
        if let Ok(mut guard) = self.state.lock() {
            guard.tracking_state = RequestTrackingState::Paused;
            guard.revision = guard.revision.saturating_add(1);
        }
    }

    /// Resume tracking new requests into the buffer.
    pub fn resume(&self) {
        if let Ok(mut guard) = self.state.lock() {
            guard.tracking_state = RequestTrackingState::Active;
            guard.revision = guard.revision.saturating_add(1);
        }
    }

    /// Toggle tracking state, returning the new typed tracking state.
    pub fn toggle_pause(&self) -> RequestTrackingState {
        if let Ok(mut guard) = self.state.lock() {
            guard.tracking_state = match guard.tracking_state {
                RequestTrackingState::Active => RequestTrackingState::Paused,
                RequestTrackingState::Paused => RequestTrackingState::Active,
            };
            guard.revision = guard.revision.saturating_add(1);
            guard.tracking_state
        } else {
            RequestTrackingState::Active
        }
    }

    /// Poll upstream controller stream for newly observed requests.
    pub async fn poll_upstream(&self) -> Result<usize, Failure> {
        let Some(port) = &self.port else {
            return Ok(0);
        };
        let fetched = port
            .fetch_recent_requests()
            .await
            .map_err(|e| Failure::new(ErrorCode::Network, e.to_string(), true))?;
        let count = fetched.len();
        for req in fetched {
            self.record(req);
        }
        Ok(count)
    }

    /// Produce a read-only immutable snapshot of the request tracker.
    pub fn snapshot(&self) -> RequestTrackerSnapshot {
        let Ok(guard) = self.state.lock() else {
            return RequestTrackerSnapshot::default();
        };
        let active_count = guard
            .records
            .iter()
            .filter(|r| r.status == RequestStatus::Active)
            .count();
        RequestTrackerSnapshot {
            generation: guard.generation,
            revision: guard.revision,
            records: guard.records.iter().cloned().collect(),
            capacity: guard.capacity,
            total_requests_observed: guard.total_requests_observed,
            active_count,
            tracking_state: guard.tracking_state,
        }
    }

    /// Filter historic records using the provided filter criteria.
    pub fn filter_records(&self, filter: &RequestHistoryFilter) -> Vec<RequestRecord> {
        let snap = self.snapshot();
        snap.filter(filter).into_iter().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use infiltrator_contract::request_tracker::RequestNetwork;
    use lurker_core_ports::error::PortError;

    struct MockRequestTrackerPort {
        cleared: Arc<Mutex<bool>>,
    }

    #[async_trait]
    impl RequestTrackerPort for MockRequestTrackerPort {
        async fn fetch_recent_requests(&self) -> Result<Vec<RequestRecord>, PortError> {
            Ok(vec![RequestRecord::new(
                "polled-1",
                2000,
                "polled.com:443",
                RequestNetwork::Tcp,
                "DIRECT",
            )])
        }

        async fn clear_request_stream(&self) -> Result<(), PortError> {
            if let Ok(mut guard) = self.cleared.lock() {
                *guard = true;
            }
            Ok(())
        }
    }

    #[test]
    fn ring_buffer_fifo_eviction() {
        let app = RequestTrackerApplication::new(None, 3);

        app.record(RequestRecord::new(
            "req-1",
            100,
            "host1:443",
            RequestNetwork::Tcp,
            "HK",
        ));
        app.record(RequestRecord::new(
            "req-2",
            200,
            "host2:443",
            RequestNetwork::Tcp,
            "US",
        ));
        app.record(RequestRecord::new(
            "req-3",
            300,
            "host3:443",
            RequestNetwork::Tcp,
            "JP",
        ));

        let snap1 = app.snapshot();
        assert_eq!(snap1.records.len(), 3);
        assert_eq!(snap1.records[0].id, "req-1");
        assert_eq!(snap1.records[2].id, "req-3");
        assert_eq!(snap1.active_count, 3);

        // 4th item evicts req-1
        app.record(RequestRecord::new(
            "req-4",
            400,
            "host4:443",
            RequestNetwork::Tcp,
            "SG",
        ));

        let snap2 = app.snapshot();
        assert_eq!(snap2.records.len(), 3);
        assert_eq!(snap2.records[0].id, "req-2");
        assert_eq!(snap2.records[2].id, "req-4");
        assert_eq!(snap2.total_requests_observed, 4);
    }

    #[test]
    fn update_active_request_to_closed() {
        let app = RequestTrackerApplication::new(None, 10);

        app.record(RequestRecord::new(
            "req-1",
            100,
            "api.github.com:443",
            RequestNetwork::Tcp,
            "HK",
        ));
        assert_eq!(app.snapshot().active_count, 1);

        app.record_closed("req-1", 350, 1024, 4096);

        let snap = app.snapshot();
        assert_eq!(snap.active_count, 0);
        assert_eq!(snap.records[0].status, RequestStatus::Closed);
        assert_eq!(snap.records[0].duration_ms, 350);
        assert_eq!(snap.records[0].download_bytes, 4096);
    }

    #[test]
    fn pause_resume_and_toggle() {
        let app = RequestTrackerApplication::new(None, 10);

        app.record(RequestRecord::new(
            "r-1",
            100,
            "h1",
            RequestNetwork::Tcp,
            "HK",
        ));
        assert_eq!(app.snapshot().records.len(), 1);

        app.pause();
        assert!(app.snapshot().is_tracking_paused());
        assert_eq!(app.snapshot().tracking_state, RequestTrackingState::Paused);

        // While paused, observation counter increments but buffer doesn't store
        app.record(RequestRecord::new(
            "r-2",
            200,
            "h2",
            RequestNetwork::Tcp,
            "HK",
        ));
        let snap_paused = app.snapshot();
        assert_eq!(snap_paused.records.len(), 1);
        assert_eq!(snap_paused.total_requests_observed, 2);

        app.resume();
        assert!(!app.snapshot().is_tracking_paused());
        assert_eq!(app.snapshot().tracking_state, RequestTrackingState::Active);

        let toggled = app.toggle_pause();
        assert_eq!(toggled, RequestTrackingState::Paused);
        assert!(app.snapshot().is_tracking_paused());
    }

    #[tokio::test]
    async fn clear_and_poll() {
        let cleared = Arc::new(Mutex::new(false));
        let port = Arc::new(MockRequestTrackerPort {
            cleared: cleared.clone(),
        });
        let app = RequestTrackerApplication::new(Some(port), 10);

        app.record(RequestRecord::new(
            "r-1",
            100,
            "h1",
            RequestNetwork::Tcp,
            "HK",
        ));
        assert_eq!(app.snapshot().records.len(), 1);

        app.clear().await.unwrap();
        assert_eq!(app.snapshot().records.len(), 0);
        assert!(*cleared.lock().unwrap());

        let polled = app.poll_upstream().await.unwrap();
        assert_eq!(polled, 1);
        assert_eq!(app.snapshot().records.len(), 1);
        assert_eq!(app.snapshot().records[0].id, "polled-1");
    }
}
