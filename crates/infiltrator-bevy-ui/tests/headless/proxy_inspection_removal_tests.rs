use super::*;

#[test]
fn failed_or_loading_typed_reads_keep_visible_last_details_until_successful_deletion() {
    let (mut app, sink) = setup();
    open(&mut app);
    let root = entity::<ProxyInspectionRoot>(&mut app);
    let title = entity::<ProxyInspectionTitle>(&mut app);
    let chart = entity::<ProxyInspectionChart>(&mut app);
    let probe = entity::<ProbeInspectedProxy>(&mut app);
    let original = inspection_read(app.world().resource::<ProxyInspectionState>())
        .expect("open inspection has a read state")
        .detail
        .clone();
    let mut snapshot = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
    let failure = Failure::new(ErrorCode::Network, "outbound detail read failed", true);
    for bad in [
        PageData::failed(failure.clone()),
        PageData::unavailable(failure.clone()),
        PageData::loading(),
    ] {
        snapshot.outbound_details = bad;
        publish(&mut app, snapshot.clone());
        let state = app.world().resource::<ProxyInspectionState>();
        let session = inspection_session(state).expect("read failures keep the inspection open");
        assert_eq!(session.selected.value(), INSPECTION_NODE);
        assert_eq!(session.read.detail, original);
        match &snapshot.outbound_details.status {
            PageStatus::Loading => {
                assert_eq!(session.read.phase, ProxyInspectionReadPhase::Loading);
            }
            PageStatus::Failed { .. } | PageStatus::Unavailable { .. } => {
                assert!(matches!(
                    &session.read.phase,
                    ProxyInspectionReadPhase::Failed(_)
                ));
            }
            PageStatus::Ready | PageStatus::Empty => unreachable!(),
        }
        assert_eq!(
            app.world().get::<Node>(root).unwrap().display,
            Display::Flex
        );
        assert_eq!(app.world().get::<Text>(title).unwrap().0, INSPECTION_NODE);
        assert!(app.world().get::<ButtonDisabled>(probe).unwrap().0);
        activate(&mut app, probe);
        assert!(
            sink.commands().is_empty(),
            "failed details cannot probe stale facts"
        );
        assert_eq!(entity::<ProxyInspectionChart>(&mut app), chart);
    }

    let mut recovered = snapshot.clone();
    let scope = recovered.core.scope().unwrap();
    replace_details(
        &mut recovered,
        vec![
            detail(
                &scope.identity.engine,
                OutboundId::new(scope.identity.engine.clone(), INSPECTION_NODE).unwrap(),
                91,
            ),
            detail(
                &scope.identity.engine,
                OutboundId::new(scope.identity.engine.clone(), OTHER_NODE).unwrap(),
                27,
            ),
        ],
        3,
    );
    publish(&mut app, recovered.clone());
    assert_eq!(field_text(&mut app, ProxyDetailField::Delay), "91 ms");
    assert!(!app.world().get::<ButtonDisabled>(probe).unwrap().0);
    assert_eq!(
        inspection_read(app.world().resource::<ProxyInspectionState>())
            .expect("recovered inspection remains open")
            .phase,
        ProxyInspectionReadPhase::Ready
    );

    let mut deleted = recovered.clone();
    deleted.outbound_inventory = PageData::ready(
        OutboundInventorySnapshot::new(
            scope.clone(),
            SnapshotRevision::new(8),
            OutboundInventoryState::Ready {
                entries: vec![OutboundSnapshot {
                    id: OutboundId::new(scope.identity.engine.clone(), OTHER_NODE).unwrap(),
                    display_name: Some(OTHER_NODE.to_owned()),
                }],
                collections: vec![OutboundSelectionSet {
                    id: OutboundId::new(scope.identity.engine.clone(), INSPECTION_GROUP).unwrap(),
                    member_ids: vec![
                        OutboundId::new(scope.identity.engine.clone(), OTHER_NODE).unwrap(),
                    ],
                    availability: SelectionAvailability::Selectable,
                    selection: OutboundSelectionState::Ready {
                        selected: OutboundId::new(scope.identity.engine.clone(), OTHER_NODE)
                            .unwrap(),
                    },
                }],
            },
        )
        .unwrap(),
    );
    deleted.outbound_details = PageData::empty(
        OutboundDetailsSnapshot::new(
            scope,
            SnapshotRevision::new(4),
            OutboundDetailsState::Ready {
                details: vec![detail(
                    &recovered.core.identity.as_ref().unwrap().engine,
                    OutboundId::new(
                        recovered.core.identity.as_ref().unwrap().engine.clone(),
                        OTHER_NODE,
                    )
                    .unwrap(),
                    27,
                )],
            },
        )
        .unwrap(),
    );
    publish(&mut app, deleted);
    assert!(selected_outbound(app.world().resource::<ProxyInspectionState>()).is_none());
    assert_eq!(
        app.world().get::<Node>(root).unwrap().display,
        Display::None
    );
}
