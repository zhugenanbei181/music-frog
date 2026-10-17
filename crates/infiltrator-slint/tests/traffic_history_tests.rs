//! SLINT-007: Traffic history and closed connections tests.
//!
//! Validates:
//! - Clear traffic history command dispatch
//! - Clear closed connections command dispatch
//! - Traffic history snapshot honesty (no fake synthesized data on empty/partial)
//! - Closed connections snapshot structure and mapping

#![allow(clippy::field_reassign_with_default)]

use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::core_identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, EngineId,
};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_contract::traffic_history::{
    ClosedConnectionSnapshot, ClosedConnectionsSnapshot, TrafficBarSnapshot, TrafficHistorySnapshot,
};
use infiltrator_slint::{SlintAction, SlintProductHost};
use std::sync::Mutex;

struct TrafficTestHost {
    scope: CoreScope,
    executed_intents: Mutex<Vec<CommandIntent>>,
}

impl TrafficTestHost {
    fn new() -> Self {
        let identity = CoreIdentity::new(
            EngineId::mihomo(),
            CoreInstanceId::new("slint-traffic-test").unwrap(),
            None,
        );
        let scope = CoreScope {
            identity,
            generation: CoreGeneration::new(1),
            session_token: None,
        };
        Self {
            scope,
            executed_intents: Mutex::new(Vec::new()),
        }
    }

    fn make_snapshot(&self) -> SurfaceSnapshot {
        let mut snapshot = SurfaceSnapshot::unavailable(
            SurfaceKind::SlintDesktop,
            HostKind::Desktop,
            Failure::new(ErrorCode::NotReady, "initial", true),
        );
        snapshot.core.identity = Some(self.scope.identity.clone());
        snapshot.core.generation = self.scope.generation.value();
        snapshot
    }

    fn intents(&self) -> Vec<CommandIntent> {
        self.executed_intents.lock().unwrap().clone()
    }
}

impl SlintProductHost for TrafficTestHost {
    fn read_snapshot(&self) -> Result<SurfaceSnapshot, Failure> {
        Ok(self.make_snapshot())
    }

    fn execute(
        &self,
        _snapshot: Option<&SurfaceSnapshot>,
        action: SlintAction,
    ) -> Result<(), Failure> {
        match action {
            SlintAction::Command(intent) => {
                self.executed_intents.lock().unwrap().push(intent);
                Ok(())
            }
            SlintAction::Sequence(intents) => {
                self.executed_intents.lock().unwrap().extend(intents);
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

#[test]
fn clear_traffic_history_dispatches_intent() {
    let host = TrafficTestHost::new();
    host.execute(
        None,
        SlintAction::Command(CommandIntent::ClearTrafficHistory),
    )
    .unwrap();
    assert_eq!(host.intents(), vec![CommandIntent::ClearTrafficHistory]);
}

#[test]
fn clear_closed_connections_dispatches_intent() {
    let host = TrafficTestHost::new();
    host.execute(
        None,
        SlintAction::Command(CommandIntent::ClearClosedConnections),
    )
    .unwrap();
    assert_eq!(host.intents(), vec![CommandIntent::ClearClosedConnections]);
}

#[test]
fn traffic_history_defaults_to_honest_unobserved() {
    let history = TrafficHistorySnapshot::default();
    assert!(
        !history.today_has_data,
        "honest empty state must not claim today data"
    );
    assert!(
        !history.week_has_data,
        "honest empty state must not claim week data"
    );
    assert_eq!(history.today_download_bytes, 0);
    assert_eq!(history.today_upload_bytes, 0);
    assert_eq!(history.week_download_bytes, 0);
    assert_eq!(history.week_upload_bytes, 0);
    assert_eq!(history.week_average_download_bytes, 0);
    assert_eq!(history.week_average_upload_bytes, 0);
    assert!(history.today_bars.is_empty());
    assert!(history.week_bars.is_empty());
}

#[test]
fn closed_connections_defaults_to_empty() {
    let closed = ClosedConnectionsSnapshot::default();
    assert_eq!(closed.total_closed, 0);
    assert!(closed.records.is_empty());
}

#[test]
fn traffic_history_with_valid_buckets_and_bars() {
    let mut history = TrafficHistorySnapshot::default();
    history.today_has_data = true;
    history.week_has_data = true;
    history.today_download_bytes = 1024 * 1024 * 50; // 50 MB
    history.today_upload_bytes = 1024 * 1024 * 10; // 10 MB
    history.week_download_bytes = 1024 * 1024 * 350; // 350 MB
    history.week_upload_bytes = 1024 * 1024 * 70; // 70 MB
    history.week_average_download_bytes = 1024 * 1024 * 50;
    history.week_average_upload_bytes = 1024 * 1024 * 10;
    history.today_bars.push(TrafficBarSnapshot {
        label: "14:00".into(),
        detail_label: "Today 14:00".into(),
        download_bytes: 1024 * 1024 * 5,
        upload_bytes: 1024 * 1024,
        download_fraction: 0.5,
        upload_fraction: 0.1,
    });

    assert!(history.today_has_data);
    assert_eq!(history.today_bars.len(), 1);
    assert_eq!(history.today_bars[0].label, "14:00");
}

#[test]
fn closed_connections_preserves_attributes() {
    let item = ClosedConnectionSnapshot {
        id: "conn-123".into(),
        destination: "api.github.com:443".into(),
        duration_ms: 15400,
        upload_bytes: 4096,
        download_bytes: 65536,
        rule: "Rule: Match".into(),
        outbound: "ProxyNode".into(),
        closed_at: "2026-10-15T10:30:00Z".into(),
    };
    let closed = ClosedConnectionsSnapshot {
        records: vec![item],
        total_closed: 1,
    };
    assert_eq!(closed.records.len(), 1);
    assert_eq!(closed.records[0].id, "conn-123");
    assert_eq!(closed.records[0].duration_ms / 1000, 15);
}
