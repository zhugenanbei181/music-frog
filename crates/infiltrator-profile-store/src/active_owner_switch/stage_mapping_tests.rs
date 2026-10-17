use super::{
    active_reference_revision, engine_switch_revision, persisted_owner_state, persisted_wire,
    switch_snapshot,
};
use crate::active_reference_owner::{
    ACTIVE_PROFILE_REFERENCE_SCHEMA_VERSION, ActiveProfileReferenceRecord, LegacySelectionMigration,
};
use lurker_core_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceRevision, ActiveProfileReferenceState,
};
use lurker_core_contract::engine_switch::{
    EngineOwnerState, EngineSwitchRevision, EngineSwitchStage,
};
use lurker_core_contract::identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreVersion, EngineId,
};
use lurker_core_contract::profile_document_identity::ProfileId;
use lurker_profile_store::owner_switch::owner::{
    ActiveCoreOwnerState, ActiveOwnerSwitchStage, ActiveOwnerToken,
};
use serde_json::{Value, json};

fn engine(value: &str) -> EngineId {
    EngineId::new(value).expect("test engine ID")
}

fn identity(engine_id: &str, instance: &str, version: Option<&str>) -> CoreIdentity {
    CoreIdentity::new(
        engine(engine_id),
        CoreInstanceId::new(instance).expect("test instance ID"),
        version.map(|value| CoreVersion::new(value).expect("test version")),
    )
}

fn token(value: &str) -> ActiveOwnerToken {
    ActiveOwnerToken(value.to_owned())
}

fn reference_state(engine_id: &str, profile: &str, revision: u64) -> ActiveProfileReferenceState {
    ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
        engine(engine_id),
        ProfileId::new(profile).expect("test profile ID"),
        ActiveProfileReferenceRevision::new(revision),
    ))
}

fn switching(
    previous_reference: ActiveProfileReferenceState,
    stage: ActiveOwnerSwitchStage,
) -> ActiveCoreOwnerState<ActiveProfileReferenceState> {
    ActiveCoreOwnerState::Switching {
        transition_id: token("transition-1"),
        previous_reference,
        owner_identity: identity("mihomo", "old-owner", Some("1.19.32")),
        owner_generation: CoreGeneration::new(3),
        owner_token: token("owner-token-1"),
        stage,
    }
}

/// The exact wire JSON this repository already persists. These literals are
/// the contract: renaming any tag or field is a schema migration.
fn existing_wire_json(stage: &str) -> Value {
    match stage {
        "closing_old" => json!({
            "schema_version": 1,
            "selection": {
                "state": "selected",
                "reference": {
                    "engine_id": "mihomo",
                    "profile_id": "main",
                    "revision": 7
                }
            },
            "legacy_migration": {"state": "explicit_selection"},
            "owner": {
                "state": "switching",
                "owner": {
                    "transition_id": "transition-1",
                    "previous_reference": {
                        "state": "selected",
                        "reference": {
                            "engine_id": "mihomo",
                            "profile_id": "main",
                            "revision": 7
                        }
                    },
                    "owner_identity": {
                        "engine": "mihomo",
                        "instance": "old-owner",
                        "version": "1.19.32"
                    },
                    "owner_generation": 3,
                    "owner_token": "owner-token-1",
                    "stage": {"stage": "closing_old"}
                }
            }
        }),
        "old_stopped" => json!({
            "schema_version": 1,
            "selection": {
                "state": "selected",
                "reference": {
                    "engine_id": "mihomo",
                    "profile_id": "main",
                    "revision": 7
                }
            },
            "legacy_migration": {"state": "explicit_selection"},
            "owner": {
                "state": "switching",
                "owner": {
                    "transition_id": "transition-1",
                    "previous_reference": {
                        "state": "selected",
                        "reference": {
                            "engine_id": "mihomo",
                            "profile_id": "main",
                            "revision": 7
                        }
                    },
                    "owner_identity": {
                        "engine": "mihomo",
                        "instance": "old-owner",
                        "version": "1.19.32"
                    },
                    "owner_generation": 3,
                    "owner_token": "owner-token-1",
                    "stage": {
                        "stage": "old_stopped",
                        "details": {"terminal_generation": 4}
                    }
                }
            }
        }),
        "reference_changed" => json!({
            "schema_version": 1,
            "selection": {
                "state": "selected",
                "reference": {
                    "engine_id": "sing-box",
                    "profile_id": "replacement",
                    "revision": 8
                }
            },
            "legacy_migration": {"state": "explicit_selection"},
            "owner": {
                "state": "switching",
                "owner": {
                    "transition_id": "transition-1",
                    "previous_reference": {
                        "state": "selected",
                        "reference": {
                            "engine_id": "mihomo",
                            "profile_id": "main",
                            "revision": 7
                        }
                    },
                    "owner_identity": {
                        "engine": "mihomo",
                        "instance": "old-owner",
                        "version": "1.19.32"
                    },
                    "owner_generation": 3,
                    "owner_token": "owner-token-1",
                    "stage": {
                        "stage": "reference_changed",
                        "details": {"terminal_generation": 4}
                    }
                }
            }
        }),
        "binding_new" => json!({
            "schema_version": 1,
            "selection": {
                "state": "selected",
                "reference": {
                    "engine_id": "sing-box",
                    "profile_id": "replacement",
                    "revision": 8
                }
            },
            "legacy_migration": {"state": "explicit_selection"},
            "owner": {
                "state": "switching",
                "owner": {
                    "transition_id": "transition-1",
                    "previous_reference": {
                        "state": "selected",
                        "reference": {
                            "engine_id": "mihomo",
                            "profile_id": "main",
                            "revision": 7
                        }
                    },
                    "owner_identity": {
                        "engine": "mihomo",
                        "instance": "old-owner",
                        "version": "1.19.32"
                    },
                    "owner_generation": 3,
                    "owner_token": "owner-token-1",
                    "stage": {
                        "stage": "binding_new",
                        "details": {
                            "terminal_generation": 4,
                            "engine_id": "sing-box",
                            "token": "candidate-token-1"
                        }
                    }
                }
            }
        }),
        other => panic!("unknown fixture {other}"),
    }
}

fn parsed(json: &Value) -> ActiveProfileReferenceRecord {
    serde_json::from_value(json.clone()).expect("parse existing wire JSON")
}

#[test]
fn persisted_stage_tags_and_fields_are_unchanged() {
    let expectations = [
        (
            "closing_old",
            vec![("stage", "closing_old")],
            Vec::<&str>::new(),
        ),
        (
            "old_stopped",
            vec![("stage", "old_stopped")],
            vec!["terminal_generation"],
        ),
        (
            "reference_changed",
            vec![("stage", "reference_changed")],
            vec!["terminal_generation"],
        ),
        (
            "binding_new",
            vec![("stage", "binding_new")],
            vec!["terminal_generation", "engine_id", "token"],
        ),
    ];
    for (fixture, tags, fields) in expectations {
        let json = existing_wire_json(fixture);
        let record = parsed(&json);
        let reserialized = serde_json::to_value(&record).expect("serialize record");
        let stage = &reserialized["owner"]["owner"]["stage"];
        assert_eq!(stage["stage"], tags[0].1, "{fixture} tag drifted");
        if fields.is_empty() {
            assert!(
                stage.get("details").is_none(),
                "{fixture} must stay a unit wire stage"
            );
        }
        for field in fields {
            assert!(
                stage["details"].get(field).is_some(),
                "{fixture} lost field {field}"
            );
        }
        for key in [
            "transition_id",
            "previous_reference",
            "owner_identity",
            "owner_generation",
            "owner_token",
        ] {
            assert!(
                reserialized["owner"]["owner"].get(key).is_some(),
                "{fixture} lost wrapper field {key}"
            );
        }
        assert_eq!(json, reserialized, "{fixture} wire bytes changed");
    }
}

#[test]
fn every_persisted_stage_round_trips_through_the_neutral_mapping() {
    for fixture in [
        "closing_old",
        "old_stopped",
        "reference_changed",
        "binding_new",
    ] {
        let json = existing_wire_json(fixture);
        let record = parsed(&json);
        let snapshot = switch_snapshot(&record).expect("map snapshot");
        assert_eq!(
            snapshot.revision,
            engine_switch_revision(record.selection.revision())
        );
        assert_eq!(snapshot.selection, record.selection);
        assert!(matches!(snapshot.owner, EngineOwnerState::Switching { .. }));
        let wire = persisted_wire(&record).expect("read wire fields");
        let owner = persisted_owner_state(&snapshot.owner, &wire).expect("map owner back");
        assert_eq!(
            owner, record.owner,
            "{fixture} neutral mapping is not total"
        );
        assert_eq!(
            serde_json::to_value(&owner).expect("serialize owner"),
            json["owner"],
            "{fixture} round trip changed the wire bytes"
        );
    }
}

#[test]
fn neutral_stage_names_match_the_persisted_transitions() {
    let previous = reference_state("mihomo", "main", 7);
    let current = reference_state("sing-box", "replacement", 8);
    let owner = |stage| switching(previous.clone(), stage);

    let closing = switch_snapshot(&ActiveProfileReferenceRecord {
        schema_version: ACTIVE_PROFILE_REFERENCE_SCHEMA_VERSION,
        selection: previous.clone(),
        legacy_migration: LegacySelectionMigration::ExplicitSelection,
        owner: owner(ActiveOwnerSwitchStage::ClosingOld),
    })
    .expect("closing snapshot");
    assert!(matches!(
        closing.owner,
        EngineOwnerState::Switching {
            stage: EngineSwitchStage::ClosingOld {
                owner_generation,
                ..
            }
        } if owner_generation == CoreGeneration::new(3)
    ));

    let changed = switch_snapshot(&ActiveProfileReferenceRecord {
        schema_version: ACTIVE_PROFILE_REFERENCE_SCHEMA_VERSION,
        selection: current.clone(),
        legacy_migration: LegacySelectionMigration::ExplicitSelection,
        owner: owner(ActiveOwnerSwitchStage::ReferenceChanged {
            terminal_generation: CoreGeneration::new(4),
        }),
    })
    .expect("reference-changed snapshot");
    assert!(matches!(
        changed.owner,
        EngineOwnerState::Switching {
            stage: EngineSwitchStage::SelectionChanged {
                previous,
                selection,
                terminal_generation,
                ..
            }
        } if previous == reference_state("mihomo", "main", 7)
            && selection == current
            && terminal_generation == CoreGeneration::new(4)
    ));

    let binding = switch_snapshot(&ActiveProfileReferenceRecord {
        schema_version: ACTIVE_PROFILE_REFERENCE_SCHEMA_VERSION,
        selection: current.clone(),
        legacy_migration: LegacySelectionMigration::ExplicitSelection,
        owner: owner(ActiveOwnerSwitchStage::BindingNew {
            terminal_generation: CoreGeneration::new(4),
            engine_id: engine("sing-box"),
            token: token("candidate-token-1"),
        }),
    })
    .expect("binding-new snapshot");
    assert!(matches!(
        binding.owner,
        EngineOwnerState::Switching {
            stage: EngineSwitchStage::BindingNew {
                selection,
                engine_id,
            }
        } if selection == current && engine_id == engine("sing-box")
    ));
}

#[test]
fn bound_and_unowned_owner_states_map_without_a_switch_stage() {
    let current = reference_state("mihomo", "main", 7);
    let bound = ActiveProfileReferenceRecord {
        schema_version: ACTIVE_PROFILE_REFERENCE_SCHEMA_VERSION,
        selection: current.clone(),
        legacy_migration: LegacySelectionMigration::ExplicitSelection,
        owner: ActiveCoreOwnerState::Bound {
            identity: identity("mihomo", "old-owner", None),
            generation: CoreGeneration::new(2),
            token: token("owner-token-1"),
        },
    };
    let snapshot = switch_snapshot(&bound).expect("bound snapshot");
    assert_eq!(
        snapshot.owner,
        EngineOwnerState::Bound {
            identity: identity("mihomo", "old-owner", None),
            generation: CoreGeneration::new(2),
        }
    );
    let wire = super::PersistedSwitchWire {
        transition_id: token("unused"),
        owner_token: token("owner-token-1"),
        previous_reference: current.clone(),
        owner_identity: identity("mihomo", "old-owner", None),
        owner_generation: CoreGeneration::new(2),
        binding_terminal_generation: CoreGeneration::new(0),
        binding_token: token("unused"),
    };
    assert_eq!(
        persisted_owner_state(&snapshot.owner, &wire).expect("map bound owner"),
        bound.owner
    );

    let unowned = ActiveProfileReferenceRecord {
        owner: ActiveCoreOwnerState::Unowned,
        ..bound
    };
    assert_eq!(
        switch_snapshot(&unowned).expect("unowned snapshot").owner,
        EngineOwnerState::Unowned
    );
}

#[test]
fn revisions_are_one_to_one() {
    let reference = ActiveProfileReferenceRevision::new(11);
    assert_eq!(
        active_reference_revision(engine_switch_revision(reference)),
        reference
    );
    assert_eq!(
        engine_switch_revision(reference),
        EngineSwitchRevision::new(11)
    );
}
