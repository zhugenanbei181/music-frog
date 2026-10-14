use super::{
    ProfileDisplayName, ProfileDisplayNameMutation, ProfileDisplayNameObservation,
    ProfileDisplayNameTarget,
};

#[test]
fn display_name_validation_and_wire_decode_share_the_same_rules() {
    for invalid in ["", "   ", " leading", "trailing ", "line\nfeed"] {
        assert!(ProfileDisplayName::new(invalid).is_err(), "{invalid:?}");
        let wire = serde_json::to_string(invalid).expect("serialize invalid string");
        assert!(serde_json::from_str::<ProfileDisplayName>(&wire).is_err());
    }
    assert!(ProfileDisplayName::new("x".repeat(129)).is_err());
    assert_eq!(
        ProfileDisplayName::new("Work profile").unwrap().as_str(),
        "Work profile"
    );
}

#[test]
fn display_name_target_and_readback_round_trip_as_typed_variants() {
    let custom = ProfileDisplayName::new("Office").unwrap();
    let mutation = ProfileDisplayNameMutation::Applied {
        previous: ProfileDisplayNameObservation::Identity,
        current: ProfileDisplayNameObservation::Custom(custom.clone()),
    };
    let encoded = serde_json::to_string(&mutation).unwrap();
    assert_eq!(
        serde_json::from_str::<ProfileDisplayNameMutation>(&encoded).unwrap(),
        mutation
    );

    let target = ProfileDisplayNameTarget::Custom(custom);
    let encoded = serde_json::to_string(&target).unwrap();
    assert_eq!(
        serde_json::from_str::<ProfileDisplayNameTarget>(&encoded).unwrap(),
        target
    );
}
