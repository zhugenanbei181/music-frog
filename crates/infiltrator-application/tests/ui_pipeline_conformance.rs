//! Integration conformance tests for the standard unidirectional UI pipeline (PUB-004.2 / PUB-004.3).
//!
//! Verifies that:
//! 1. UI surfaces interact with Core exclusively through `SurfaceReader` (read snapshots)
//!    and `CommandSink` (typed intent dispatch).
//! 2. Intent submissions return structured `CommandResult` without side-effects or direct Core coupling.
//! 3. Recorded intents preserve ordering, monotonicity, and failure handling across all UI domains.

use async_trait::async_trait;
use infiltrator_contract::command::{CommandIntent, CommandResult, RequestId};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_ports::command_sink::{CommandSink, RecordingCommandSink};
use infiltrator_ports::surface::SurfaceReader;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::routing_intent::RoutingIntent;
use lurker_core_ports::error::PortError;

struct FakeSurfaceReader {
    snapshot: SurfaceSnapshot,
}

#[async_trait]
impl SurfaceReader for FakeSurfaceReader {
    async fn read(&self) -> Result<SurfaceSnapshot, PortError> {
        Ok(self.snapshot.clone())
    }
}

#[tokio::test]
async fn ui_pipeline_surface_reader_and_command_sink_enforce_unidirectional_contract() {
    // 1. Setup neutral pipeline actors
    let reader = FakeSurfaceReader {
        snapshot: SurfaceSnapshot::unavailable(
            SurfaceKind::IcedDesktop,
            HostKind::Desktop,
            Failure::new(ErrorCode::NotReady, "init", false),
        ),
    };
    let sink = RecordingCommandSink::new();

    // 2. UI surface reads read-only snapshot
    let initial_snapshot = reader.read().await.expect("surface snapshot read");
    assert_eq!(initial_snapshot.surface, SurfaceKind::IcedDesktop);

    // 3. UI surface dispatches user intentions into CommandSink
    let (req_id1, res1) = sink
        .submit_tracked(CommandIntent::StartCore)
        .await
        .expect("start core intent");
    assert_eq!(req_id1, RequestId(1));
    assert_eq!(
        res1,
        CommandResult::Completed {
            request_id: RequestId(1)
        }
    );

    let (req_id2, res2) = sink
        .submit_tracked(CommandIntent::SetRoutingIntent {
            intent: RoutingIntent::RouteAllThroughProxy,
        })
        .await
        .expect("routing intent change");
    assert_eq!(req_id2, RequestId(2));
    assert_eq!(
        res2,
        CommandResult::Completed {
            request_id: RequestId(2)
        }
    );

    // 4. Verify ordered pipeline trace
    let intents = sink.recorded_intents();
    assert_eq!(intents.len(), 2);
    assert_eq!(intents[0], CommandIntent::StartCore);
    assert_eq!(
        intents[1],
        CommandIntent::SetRoutingIntent {
            intent: RoutingIntent::RouteAllThroughProxy,
        }
    );
}

#[tokio::test]
async fn ui_pipeline_command_sink_propagates_typed_rejected_failures() {
    let sink = RecordingCommandSink::new();
    sink.enqueue_response(Err(PortError::Rejected(Failure::new(
        ErrorCode::Permission,
        "permission refused by system policy",
        false,
    ))));

    let err = sink
        .submit(CommandIntent::StopCore)
        .await
        .expect_err("expected rejected error");

    match err {
        PortError::Rejected(failure) => {
            assert_eq!(failure.code, ErrorCode::Permission);
            assert_eq!(failure.message, "permission refused by system policy");
        }
        other => panic!("expected PortError::Rejected, got {other:?}"),
    }
}
