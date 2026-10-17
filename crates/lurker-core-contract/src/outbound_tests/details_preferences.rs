use super::*;

#[test]
fn detail_snapshot_is_engine_scoped_and_has_an_independent_revision() {
    let details = OutboundDetailsSnapshot::new(
        scope("mihomo"),
        SnapshotRevision::new(77),
        OutboundDetailsState::Ready {
            details: ["node-a", "node-b", "node-c", "node-d"]
                .into_iter()
                .map(|id| detail("mihomo", id))
                .collect(),
        },
    )
    .expect("details are scoped and unique");
    let observation = OutboundObservationSnapshot::new(valid_mihomo_snapshot(), details)
        .expect("details line up with the inventory at an independent revision");
    assert_eq!(observation.inventory().revision(), SnapshotRevision::new(4));
    assert_eq!(
        observation.details().detail_revision(),
        SnapshotRevision::new(77)
    );
    assert_eq!(
        observation
            .details()
            .detail(&outbound_id("mihomo", "node-a"))
            .expect("keyed detail")
            .current_latency_ms,
        OutboundObservation::Observed(0)
    );

    let wrong_engine = OutboundDetailsSnapshot::new(
        scope("mihomo"),
        SnapshotRevision::new(1),
        OutboundDetailsState::Ready {
            details: vec![detail("sing-box", "node-a")],
        },
    );
    assert!(matches!(
        wrong_engine,
        Err(OutboundContractError::EngineScopeMismatch)
    ));
}

#[test]
fn outbound_preferences_validate_scope_and_duplicate_ids_and_encode_typed_states() {
    let preferences = OutboundUiPreferencesSnapshot::new(
        scope("mihomo"),
        SnapshotRevision::new(5),
        OutboundUiPreferencesValues {
            search_query: "edge".to_owned(),
            favorite_outbound_ids: vec![outbound_id("mihomo", "node-a")],
            group_order: vec![outbound_id("mihomo", "group-a")],
            collapsed_group_ids: vec![outbound_id("mihomo", "group-b")],
            sort_order: OutboundSortOrder::NameDescending,
            health_filter: OutboundHealthFilter::HealthyOnly,
            layout: OutboundLayout::Grid,
        },
    )
    .expect("typed preferences");
    let encoded = serde_json::to_value(&preferences).expect("serialize preferences");
    let mut serialized_fields = encoded
        .as_object()
        .expect("snapshot serializes as an object")
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    serialized_fields.sort_unstable();
    assert_eq!(
        serialized_fields,
        [
            "collapsed_group_ids",
            "favorite_outbound_ids",
            "group_order",
            "health_filter",
            "layout",
            "revision",
            "scope",
            "search_query",
            "sort_order",
        ]
    );
    assert_eq!(encoded["sort_order"], json!("name_descending"));
    assert_eq!(encoded["health_filter"], json!("healthy_only"));
    assert_eq!(encoded["layout"], json!("grid"));
    assert_eq!(
        serde_json::from_value::<OutboundUiPreferencesSnapshot>(encoded)
            .expect("deserialize preferences"),
        preferences
    );

    let foreign_id = outbound_id("sing-box", "outside");
    let scope_mismatches = [
        OutboundUiPreferencesValues {
            favorite_outbound_ids: vec![foreign_id.clone()],
            ..Default::default()
        },
        OutboundUiPreferencesValues {
            group_order: vec![foreign_id.clone()],
            ..Default::default()
        },
        OutboundUiPreferencesValues {
            collapsed_group_ids: vec![foreign_id],
            ..Default::default()
        },
    ];
    for values in scope_mismatches {
        assert!(matches!(
            OutboundUiPreferencesSnapshot::new(scope("mihomo"), SnapshotRevision::new(1), values,),
            Err(OutboundContractError::EngineScopeMismatch)
        ));
    }

    let duplicate_id = outbound_id("mihomo", "duplicate");
    let duplicate_ids = [
        OutboundUiPreferencesValues {
            favorite_outbound_ids: vec![duplicate_id.clone(), duplicate_id.clone()],
            ..Default::default()
        },
        OutboundUiPreferencesValues {
            group_order: vec![duplicate_id.clone(), duplicate_id.clone()],
            ..Default::default()
        },
        OutboundUiPreferencesValues {
            collapsed_group_ids: vec![duplicate_id.clone(), duplicate_id],
            ..Default::default()
        },
    ];
    for values in duplicate_ids {
        assert!(matches!(
            OutboundUiPreferencesSnapshot::new(scope("mihomo"), SnapshotRevision::new(1), values,),
            Err(OutboundContractError::DuplicateOutboundPreferenceId)
        ));
    }

    let request = OutboundUiPreferenceRequest::new(
        SnapshotRevision::new(5),
        OutboundUiPreferenceIntent::SetGroupExpansion {
            group_id: outbound_id("mihomo", "group-b"),
            expansion: OutboundGroupExpansion::Collapsed,
        },
    )
    .expect("typed request");
    assert!(request.validate_scope(&scope("mihomo")).is_ok());
    assert!(request.validate_scope(&scope("sing-box")).is_err());
    assert!(
        OutboundUiPreferenceRequest::new(
            SnapshotRevision::new(5),
            OutboundUiPreferenceIntent::SetFavorite {
                outbound_id: outbound_id("mihomo", "node-a"),
                state: OutboundFavoriteState::Pinned,
            },
        )
        .is_ok()
    );
    let _ = OutboundCollectionKind::ManualSelection;
}

#[test]
fn probe_outbound_request_fences_inventory_and_validates_target_before_dispatch() {
    let request = ProbeOutboundRequest::new(
        outbound_id("mihomo", "node-a"),
        SnapshotRevision::new(4),
        "https://health.example.test/check",
        5_000,
    )
    .expect("valid probe request");
    assert!(request.validate_scope(&scope("mihomo")).is_ok());
    assert!(request.validate_scope(&scope("sing-box")).is_err());
    assert!(
        ProbeOutboundRequest::new(
            outbound_id("mihomo", "node-a"),
            SnapshotRevision::new(4),
            "https://user:secret@health.example.test/check",
            5_000,
        )
        .is_err()
    );
    assert!(
        ProbeOutboundRequest::new(
            outbound_id("mihomo", "node-a"),
            SnapshotRevision::new(4),
            "https://health.example.test/check",
            0,
        )
        .is_err()
    );

    let readback = OutboundProbeSnapshot::new(
        scope("mihomo"),
        SnapshotRevision::new(4),
        SnapshotRevision::new(8),
        outbound_id("mihomo", "node-a"),
        OutboundProbeState::Measured { delay_ms: 0 },
    )
    .expect("fenced output");
    assert!(request.is_confirmed_by(&readback));
    assert!(
        !request.is_confirmed_by(
            &OutboundProbeSnapshot::new(
                scope("mihomo"),
                SnapshotRevision::new(5),
                SnapshotRevision::new(9),
                outbound_id("mihomo", "node-a"),
                OutboundProbeState::Measured { delay_ms: 2 },
            )
            .expect("newer inventory readback"),
        )
    );
}
