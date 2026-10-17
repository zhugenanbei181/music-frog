use super::*;

#[test]
fn outbound_id_validates_and_preserves_opaque_values() {
    assert!(matches!(
        OutboundId::new(EngineId::new("mihomo").expect("engine ID"), " \t "),
        Err(OutboundContractError::EmptyId)
    ));
    assert!(matches!(
        OutboundId::new(EngineId::new("mihomo").expect("engine ID"), "x".repeat(513)),
        Err(OutboundContractError::OversizedId)
    ));
    assert!(EngineId::new("Mihomo").is_err());

    let id = OutboundId::new(
        EngineId::new("sing-box").expect("engine ID"),
        "出口 / 节点 A",
    )
    .expect("opaque ID");
    assert_eq!(id.value(), "出口 / 节点 A");
    assert_eq!(id.engine_id().as_str(), "sing-box");
    let encoded = serde_json::to_string(&id).expect("serialize ID");
    assert_eq!(
        serde_json::from_str::<OutboundId>(&encoded).expect("round trip"),
        id
    );
    assert!(
        serde_json::from_str::<OutboundId>(r#"{"engine_id":"Mihomo","value":"node"}"#).is_err()
    );
    assert!(serde_json::from_str::<OutboundId>(r#"{"engine_id":"mihomo","value":"   "}"#).is_err());
}

#[test]
fn revision_is_a_typed_serializable_counter() {
    let revision = SnapshotRevision::new(18);
    assert_eq!(revision.value(), 18);
    assert_eq!(serde_json::to_string(&revision).expect("serialize"), "18");
    assert_eq!(
        serde_json::from_str::<SnapshotRevision>("18").expect("deserialize"),
        revision
    );
}

#[test]
fn probe_observations_keep_distinct_states_and_scope_fences() {
    let scope = scope("sing-box");
    let states = [
        OutboundProbeState::Unknown,
        OutboundProbeState::Probing,
        OutboundProbeState::Measured { delay_ms: 0 },
        OutboundProbeState::TimedOut,
        OutboundProbeState::Unsupported,
        OutboundProbeState::Failed {
            failure: Failure::new(ErrorCode::Network, "probe failed", true),
        },
    ];

    for state in states {
        let snapshot = OutboundProbeSnapshot::new(
            scope.clone(),
            SnapshotRevision::new(12),
            SnapshotRevision::new(24),
            outbound_id("sing-box", "proxy-a"),
            state,
        )
        .expect("probe snapshot matches its scope");
        let encoded = serde_json::to_string(&snapshot).expect("serialize probe snapshot");
        let decoded: OutboundProbeSnapshot =
            serde_json::from_str(&encoded).expect("deserialize probe snapshot");
        assert_eq!(decoded, snapshot);
        assert_eq!(
            decoded.observed_inventory_revision(),
            SnapshotRevision::new(12)
        );
        assert_eq!(decoded.detail_revision(), SnapshotRevision::new(24));
    }

    assert!(matches!(
        OutboundProbeSnapshot::new(
            scope,
            SnapshotRevision::new(12),
            SnapshotRevision::new(24),
            outbound_id("mihomo", "proxy-a"),
            OutboundProbeState::Measured { delay_ms: 21 },
        ),
        Err(OutboundContractError::EngineScopeMismatch)
    ));
}

#[test]
fn batch_probe_contract_validates_bounds_and_round_trips_typed_results() {
    let scope = scope("mihomo");
    let request = ProbeOutboundBatchRequest::new(
        OutboundProbeBatchScope::Collection(outbound_id("mihomo", "selector/main")),
        SnapshotRevision::new(12),
        "https://health.example.test/check",
        5_000,
        OutboundProbeConcurrency::new(4).expect("bounded concurrency"),
    )
    .expect("valid batch probe request");
    assert_eq!(request.test_url(), "https://health.example.test/check");
    assert_eq!(request.concurrency().limit(), 4);
    assert!(request.validate_scope(&scope).is_ok());

    let items = vec![
        OutboundProbeBatchItem {
            outbound_id: outbound_id("mihomo", "proxy-a"),
            outcome: OutboundProbeBatchOutcome::Measured { delay_ms: 0 },
        },
        OutboundProbeBatchItem {
            outbound_id: outbound_id("mihomo", "proxy-b"),
            outcome: OutboundProbeBatchOutcome::Unsupported,
        },
    ];
    let snapshot =
        OutboundProbeBatchSnapshot::new(scope, request.clone(), SnapshotRevision::new(8), items)
            .expect("valid batch snapshot");
    assert!(snapshot.is_confirmed_by(&request));
    let encoded = serde_json::to_string(&snapshot).expect("serialize batch snapshot");
    let decoded: OutboundProbeBatchSnapshot =
        serde_json::from_str(&encoded).expect("deserialize batch snapshot");
    assert_eq!(decoded, snapshot);

    assert!(matches!(
        OutboundProbeConcurrency::new(0),
        Err(OutboundContractError::InvalidProbeBatchConcurrency)
    ));
    assert!(matches!(
        OutboundProbeConcurrency::new(MAX_OUTBOUND_PROBE_BATCH_CONCURRENCY + 1),
        Err(OutboundContractError::InvalidProbeBatchConcurrency)
    ));
    assert!(matches!(
        ProbeOutboundBatchRequest::new(
            OutboundProbeBatchScope::All,
            SnapshotRevision::new(12),
            "https://user:pass@health.example.test/check",
            5_000,
            OutboundProbeConcurrency::new(1).expect("valid concurrency"),
        ),
        Err(OutboundContractError::ProbeCredentialsNotAllowed)
    ));
    assert!(matches!(
        ProbeOutboundBatchRequest::new(
            OutboundProbeBatchScope::All,
            SnapshotRevision::new(12),
            "https://health.example.test/check",
            0,
            OutboundProbeConcurrency::new(1).expect("valid concurrency"),
        ),
        Err(OutboundContractError::ZeroProbeTimeout)
    ));
}

#[test]
fn batch_probe_snapshot_rejects_duplicate_and_wrong_single_member_results() {
    let scope = scope("mihomo");
    let request = ProbeOutboundBatchRequest::new(
        OutboundProbeBatchScope::One(outbound_id("mihomo", "proxy-a")),
        SnapshotRevision::new(4),
        "https://health.example.test/check",
        2_000,
        OutboundProbeConcurrency::new(1).expect("valid concurrency"),
    )
    .expect("valid one-member request");
    let wrong_member = OutboundProbeBatchItem {
        outbound_id: outbound_id("mihomo", "proxy-b"),
        outcome: OutboundProbeBatchOutcome::TimedOut,
    };
    assert!(matches!(
        OutboundProbeBatchSnapshot::new(
            scope.clone(),
            request.clone(),
            SnapshotRevision::new(3),
            vec![wrong_member.clone()],
        ),
        Err(OutboundContractError::ProbeBatchResultMismatch)
    ));

    let collection_request = ProbeOutboundBatchRequest::new(
        OutboundProbeBatchScope::All,
        SnapshotRevision::new(4),
        "https://health.example.test/check",
        2_000,
        OutboundProbeConcurrency::new(1).expect("valid concurrency"),
    )
    .expect("valid all request");
    assert!(matches!(
        OutboundProbeBatchSnapshot::new(
            scope,
            collection_request,
            SnapshotRevision::new(3),
            vec![wrong_member.clone(), wrong_member],
        ),
        Err(OutboundContractError::DuplicateOutboundProbeResultId)
    ));
}

#[test]
fn selected_probe_batch_validates_targets_and_preserves_result_order() {
    let core_scope = scope("mihomo");
    let concurrency = OutboundProbeConcurrency::new(2).expect("valid concurrency");
    assert!(matches!(
        ProbeOutboundBatchRequest::new(
            OutboundProbeBatchScope::Selected(Vec::new()),
            SnapshotRevision::new(4),
            "https://health.example.test/check",
            2_000,
            concurrency,
        ),
        Err(OutboundContractError::EmptySelectedProbeBatch)
    ));
    assert!(matches!(
        ProbeOutboundBatchRequest::new(
            OutboundProbeBatchScope::Selected(vec![
                outbound_id("mihomo", "proxy-a"),
                outbound_id("mihomo", "proxy-a"),
            ]),
            SnapshotRevision::new(4),
            "https://health.example.test/check",
            2_000,
            concurrency,
        ),
        Err(OutboundContractError::DuplicateSelectedProbeTarget)
    ));
    let invalid_wire = json!({
        "target_scope": { "selected": [] },
        "expected_inventory_revision": 4,
        "test_url": "https://health.example.test/check",
        "timeout_ms": 2000,
        "concurrency": 2,
    });
    assert!(serde_json::from_value::<ProbeOutboundBatchRequest>(invalid_wire).is_err());

    let targets = vec![
        outbound_id("mihomo", "proxy-b"),
        outbound_id("mihomo", "proxy-a"),
    ];
    let request = ProbeOutboundBatchRequest::new(
        OutboundProbeBatchScope::Selected(targets.clone()),
        SnapshotRevision::new(4),
        "https://health.example.test/check",
        2_000,
        concurrency,
    )
    .expect("ordered selected targets are valid");
    assert!(request.validate_scope(&core_scope).is_ok());
    assert!(matches!(
        request.validate_scope(&scope("sing-box")),
        Err(OutboundContractError::EngineScopeMismatch)
    ));
    let valid_items = targets
        .iter()
        .cloned()
        .map(|outbound_id| OutboundProbeBatchItem {
            outbound_id,
            outcome: OutboundProbeBatchOutcome::TimedOut,
        })
        .collect();
    let snapshot = OutboundProbeBatchSnapshot::new(
        core_scope.clone(),
        request.clone(),
        SnapshotRevision::new(8),
        valid_items,
    )
    .expect("results keep selected request order");
    assert_eq!(snapshot.items()[0].outbound_id, targets[0]);
    assert_eq!(snapshot.items()[1].outbound_id, targets[1]);

    let reversed_items = targets
        .iter()
        .rev()
        .cloned()
        .map(|outbound_id| OutboundProbeBatchItem {
            outbound_id,
            outcome: OutboundProbeBatchOutcome::TimedOut,
        })
        .collect();
    assert!(matches!(
        OutboundProbeBatchSnapshot::new(
            core_scope,
            request,
            SnapshotRevision::new(8),
            reversed_items,
        ),
        Err(OutboundContractError::ProbeBatchResultMismatch)
    ));
}
