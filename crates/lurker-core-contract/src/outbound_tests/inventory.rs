use super::*;
use std::collections::HashSet;

#[test]
fn sing_box_selector_inventory_needs_no_proxy_group() {
    let snapshot = OutboundInventorySnapshot::new(
        scope("sing-box"),
        SnapshotRevision::new(3),
        OutboundInventoryState::Ready {
            entries: vec![
                outbound("sing-box", "proxy-a"),
                outbound("sing-box", "proxy-b"),
            ],
            collections: vec![selection_set(
                "sing-box",
                "selector/main",
                &["proxy-a", "proxy-b"],
                SelectionAvailability::Selectable,
                OutboundSelectionState::Ready {
                    selected: outbound_id("sing-box", "proxy-b"),
                },
            )],
        },
    )
    .expect("valid engine-scoped snapshot");
    let encoded = serde_json::to_string(&snapshot).expect("serialize snapshot");
    assert!(!encoded.contains("group"));
    let decoded: OutboundInventorySnapshot =
        serde_json::from_str(&encoded).expect("deserialize snapshot");
    assert_eq!(decoded.scope().identity.engine.as_str(), "sing-box");
    assert_eq!(decoded.revision(), SnapshotRevision::new(3));
    let OutboundInventoryState::Ready { collections, .. } = decoded.inventory() else {
        panic!("inventory remains ready");
    };
    assert_eq!(collections.len(), 1);
    assert_eq!(collections[0].id.value(), "selector/main");
    assert_eq!(
        collections[0].selection,
        OutboundSelectionState::Ready {
            selected: outbound_id("sing-box", "proxy-b")
        }
    );
}

#[test]
fn mihomo_style_collections_keep_independent_selections() {
    let snapshot = valid_mihomo_snapshot();
    let OutboundInventoryState::Ready { collections, .. } = snapshot.inventory() else {
        panic!("inventory remains ready");
    };
    assert_eq!(collections.len(), 2);
    assert_eq!(collections[0].id.value(), "collection/primary");
    assert_eq!(collections[0].member_ids.len(), 2);
    assert_eq!(
        collections[0].selection,
        OutboundSelectionState::Ready {
            selected: outbound_id("mihomo", "node-a")
        }
    );
    assert_eq!(collections[1].id.value(), "collection/streaming");
    assert_eq!(
        collections[1].selection,
        OutboundSelectionState::Ready {
            selected: outbound_id("mihomo", "node-c")
        }
    );
}

#[test]
fn inventory_and_selection_keep_unknown_empty_unsupported_and_failed_distinct() {
    let failure = Failure::new(ErrorCode::Unsupported, "controller unavailable", false);
    let inventory_states = [
        OutboundInventoryState::Unknown,
        OutboundInventoryState::Ready {
            entries: vec![outbound("mihomo", "relay/primary")],
            collections: vec![],
        },
        OutboundInventoryState::Empty,
        OutboundInventoryState::Unsupported,
        OutboundInventoryState::Failed {
            failure: failure.clone(),
        },
    ];
    let selection_states = [
        OutboundSelectionState::Unknown,
        OutboundSelectionState::Ready {
            selected: outbound_id("mihomo", "relay/primary"),
        },
        OutboundSelectionState::Empty,
        OutboundSelectionState::Unsupported,
        OutboundSelectionState::Failed { failure },
    ];

    for state in inventory_states {
        let encoded = serde_json::to_string(&state).expect("serialize inventory state");
        let decoded: OutboundInventoryState =
            serde_json::from_str(&encoded).expect("deserialize inventory state");
        assert_eq!(decoded, state);
    }
    for state in selection_states {
        let encoded = serde_json::to_string(&state).expect("serialize selection state");
        let decoded: OutboundSelectionState =
            serde_json::from_str(&encoded).expect("deserialize selection state");
        assert_eq!(decoded, state);
    }

    let availability = [
        SelectionAvailability::Selectable,
        SelectionAvailability::ReadOnly,
        SelectionAvailability::Automatic,
        SelectionAvailability::Unsupported,
        SelectionAvailability::Unknown,
    ];
    let encoded: Vec<String> = availability
        .iter()
        .map(|state| serde_json::to_string(state).expect("serialize availability"))
        .collect();
    let unique: HashSet<&str> = encoded.iter().map(String::as_str).collect();
    assert_eq!(unique.len(), availability.len());
}

#[test]
fn snapshot_rejects_ready_inventory_without_entries() {
    assert!(matches!(
        OutboundInventorySnapshot::new(
            scope("mihomo"),
            SnapshotRevision::new(1),
            OutboundInventoryState::Ready {
                entries: vec![],
                collections: vec![],
            },
        ),
        Err(OutboundContractError::EmptyReadyInventory)
    ));
    let empty = OutboundInventorySnapshot::new(
        scope("mihomo"),
        SnapshotRevision::new(1),
        OutboundInventoryState::Empty,
    )
    .expect("known empty inventory is valid");
    assert_eq!(empty.inventory(), &OutboundInventoryState::Empty);
}

#[test]
fn snapshot_rejects_wrong_engines_unknown_members_and_nonmembers() {
    assert!(matches!(
        OutboundInventorySnapshot::new(
            scope("mihomo"),
            SnapshotRevision::new(1),
            OutboundInventoryState::Ready {
                entries: vec![outbound("sing-box", "proxy-a")],
                collections: vec![],
            },
        ),
        Err(OutboundContractError::EngineScopeMismatch)
    ));
    assert!(matches!(
        OutboundInventorySnapshot::new(
            scope("mihomo"),
            SnapshotRevision::new(1),
            OutboundInventoryState::Ready {
                entries: vec![outbound("mihomo", "proxy-a")],
                collections: vec![OutboundSelectionSet {
                    id: outbound_id("mihomo", "selector/main"),
                    member_ids: vec![outbound_id("sing-box", "proxy-a")],
                    availability: SelectionAvailability::Selectable,
                    selection: OutboundSelectionState::Unknown,
                }],
            },
        ),
        Err(OutboundContractError::EngineScopeMismatch)
    ));
    assert!(matches!(
        OutboundInventorySnapshot::new(
            scope("mihomo"),
            SnapshotRevision::new(1),
            OutboundInventoryState::Ready {
                entries: vec![outbound("mihomo", "proxy-a")],
                collections: vec![selection_set(
                    "mihomo",
                    "selector/main",
                    &["missing"],
                    SelectionAvailability::Selectable,
                    OutboundSelectionState::Unknown,
                )],
            },
        ),
        Err(OutboundContractError::UnknownMember)
    ));
    assert!(matches!(
        OutboundInventorySnapshot::new(
            scope("mihomo"),
            SnapshotRevision::new(1),
            OutboundInventoryState::Ready {
                entries: vec![outbound("mihomo", "proxy-a"), outbound("mihomo", "proxy-b")],
                collections: vec![selection_set(
                    "mihomo",
                    "selector/main",
                    &["proxy-a"],
                    SelectionAvailability::Selectable,
                    OutboundSelectionState::Ready {
                        selected: outbound_id("mihomo", "proxy-b"),
                    },
                )],
            },
        ),
        Err(OutboundContractError::SelectedOutboundNotMember)
    ));
    assert!(matches!(
        OutboundInventorySnapshot::new(
            scope("mihomo"),
            SnapshotRevision::new(1),
            OutboundInventoryState::Ready {
                entries: vec![outbound("mihomo", "proxy-a")],
                collections: vec![selection_set(
                    "mihomo",
                    "selector/main",
                    &["proxy-a"],
                    SelectionAvailability::Selectable,
                    OutboundSelectionState::Ready {
                        selected: outbound_id("sing-box", "proxy-a"),
                    },
                )],
            },
        ),
        Err(OutboundContractError::EngineScopeMismatch)
    ));
    assert!(matches!(
        OutboundInventorySnapshot::new(
            scope("mihomo"),
            SnapshotRevision::new(1),
            OutboundInventoryState::Ready {
                entries: vec![outbound("mihomo", "proxy-a")],
                collections: vec![selection_set(
                    "sing-box",
                    "selector/main",
                    &["proxy-a"],
                    SelectionAvailability::Selectable,
                    OutboundSelectionState::Unknown,
                )],
            },
        ),
        Err(OutboundContractError::EngineScopeMismatch)
    ));
}

#[test]
fn snapshot_rejects_duplicate_entries_collections_and_members() {
    let duplicate_entry = OutboundInventorySnapshot::new(
        scope("mihomo"),
        SnapshotRevision::new(1),
        OutboundInventoryState::Ready {
            entries: vec![outbound("mihomo", "proxy-a"), outbound("mihomo", "proxy-a")],
            collections: vec![],
        },
    );
    assert!(matches!(
        duplicate_entry,
        Err(OutboundContractError::DuplicateInventoryEntryId)
    ));

    let duplicate_collection = OutboundInventorySnapshot::new(
        scope("mihomo"),
        SnapshotRevision::new(1),
        OutboundInventoryState::Ready {
            entries: vec![outbound("mihomo", "proxy-a")],
            collections: vec![
                selection_set(
                    "mihomo",
                    "selector/main",
                    &["proxy-a"],
                    SelectionAvailability::Selectable,
                    OutboundSelectionState::Unknown,
                ),
                selection_set(
                    "mihomo",
                    "selector/main",
                    &["proxy-a"],
                    SelectionAvailability::Selectable,
                    OutboundSelectionState::Unknown,
                ),
            ],
        },
    );
    assert!(matches!(
        duplicate_collection,
        Err(OutboundContractError::DuplicateSelectionCollectionId)
    ));

    let duplicate_member = OutboundInventorySnapshot::new(
        scope("mihomo"),
        SnapshotRevision::new(1),
        OutboundInventoryState::Ready {
            entries: vec![outbound("mihomo", "proxy-a")],
            collections: vec![selection_set(
                "mihomo",
                "selector/main",
                &["proxy-a", "proxy-a"],
                SelectionAvailability::Selectable,
                OutboundSelectionState::Unknown,
            )],
        },
    );
    assert!(matches!(
        duplicate_member,
        Err(OutboundContractError::DuplicateSelectionMemberId)
    ));
}

#[test]
fn collection_id_may_also_be_an_inventory_entry() {
    let snapshot = OutboundInventorySnapshot::new(
        scope("mihomo"),
        SnapshotRevision::new(1),
        OutboundInventoryState::Ready {
            entries: vec![
                outbound("mihomo", "selector/main"),
                outbound("mihomo", "proxy-a"),
            ],
            collections: vec![selection_set(
                "mihomo",
                "selector/main",
                &["proxy-a"],
                SelectionAvailability::Selectable,
                OutboundSelectionState::Ready {
                    selected: outbound_id("mihomo", "proxy-a"),
                },
            )],
        },
    );
    let snapshot = snapshot.expect("collection may also appear as an outbound entry");
    let OutboundInventoryState::Ready {
        entries,
        collections,
    } = snapshot.inventory()
    else {
        panic!("inventory remains ready");
    };
    assert_eq!(entries[0].id, collections[0].id);
}

#[test]
fn deserialization_repeats_snapshot_scope_and_membership_validation() {
    let valid = serde_json::to_value(valid_mihomo_snapshot()).expect("serialize snapshot");

    let mut wrong_engine = valid.clone();
    wrong_engine["inventory"]["ready"]["collections"][0]["id"]["engine_id"] = json!("sing-box");
    assert!(serde_json::from_value::<OutboundInventorySnapshot>(wrong_engine).is_err());

    let mut wrong_member_engine = valid.clone();
    wrong_member_engine["inventory"]["ready"]["collections"][0]["member_ids"][0]["engine_id"] =
        json!("sing-box");
    assert!(serde_json::from_value::<OutboundInventorySnapshot>(wrong_member_engine).is_err());

    let mut missing_member = valid.clone();
    missing_member["inventory"]["ready"]["collections"][0]["member_ids"][0]["value"] =
        json!("missing");
    assert!(serde_json::from_value::<OutboundInventorySnapshot>(missing_member).is_err());

    let mut nonmember_selection = valid.clone();
    nonmember_selection["inventory"]["ready"]["collections"][0]["selection"]["ready"]["selected"]
        ["value"] = json!("node-c");
    assert!(serde_json::from_value::<OutboundInventorySnapshot>(nonmember_selection).is_err());

    let mut empty_ready = valid;
    empty_ready["inventory"]["ready"]["entries"] = Value::Array(vec![]);
    assert!(serde_json::from_value::<OutboundInventorySnapshot>(empty_ready).is_err());
}

#[test]
fn deserialization_cannot_bypass_uniqueness_validation() {
    let valid = serde_json::to_value(valid_mihomo_snapshot()).expect("serialize snapshot");

    let mut duplicate_entry = valid.clone();
    duplicate_entry["inventory"]["ready"]["entries"][1]["id"]["value"] = json!("node-a");
    assert!(serde_json::from_value::<OutboundInventorySnapshot>(duplicate_entry).is_err());

    let mut duplicate_collection = valid.clone();
    duplicate_collection["inventory"]["ready"]["collections"][1]["id"]["value"] =
        json!("collection/primary");
    assert!(serde_json::from_value::<OutboundInventorySnapshot>(duplicate_collection).is_err());

    let mut duplicate_member = valid;
    let members = duplicate_member["inventory"]["ready"]["collections"][0]["member_ids"]
        .as_array_mut()
        .expect("member list");
    let duplicate = members[0].clone();
    members.push(duplicate);
    assert!(serde_json::from_value::<OutboundInventorySnapshot>(duplicate_member).is_err());
}

#[test]
fn selection_request_keeps_typed_ids_revision_and_rejects_namespace_mismatch() {
    let request = OutboundSelectionRequest::new(
        outbound_id("mihomo", "selector/main"),
        outbound_id("mihomo", "node-b"),
        SnapshotRevision::new(8),
    )
    .expect("same-engine selection request");
    assert_eq!(request.expected_revision(), SnapshotRevision::new(8));
    assert!(request.validate_scope(&scope("mihomo")).is_ok());
    assert!(matches!(
        request.validate_scope(&scope("sing-box")),
        Err(OutboundContractError::EngineScopeMismatch)
    ));
    assert!(matches!(
        OutboundSelectionRequest::new(
            outbound_id("mihomo", "selector/main"),
            outbound_id("sing-box", "node-b"),
            SnapshotRevision::new(8),
        ),
        Err(OutboundContractError::EngineScopeMismatch)
    ));

    let serialized = serde_json::to_value(&request).expect("serialize request");
    let mut invalid = serialized;
    invalid["outbound_id"]["engine_id"] = json!("sing-box");
    assert!(serde_json::from_value::<OutboundSelectionRequest>(invalid).is_err());
}
