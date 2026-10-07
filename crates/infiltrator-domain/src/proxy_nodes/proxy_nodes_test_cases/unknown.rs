//! Behavior cases for unknown.
//! test-intent: behavior

use super::*;

#[test]
fn test_unknown_field_survives_repeated_roundtrips() {
    let nodes1 = parse_profile_yaml(TUIC_YAML).expect("parse");
    let yaml1 = nodes_to_profile_yaml(&nodes1).expect("serialize 1");
    assert!(yaml1.contains("fake-field: 123"));

    let nodes2 = parse_profile_yaml(&yaml1).expect("re-parse 1");
    let yaml2 = nodes_to_profile_yaml(&nodes2).expect("serialize 2");
    assert_eq!(yaml1, yaml2, "serialization must be a fixed point");
    assert_eq!(nodes1, nodes2);

    let extra = nodes2[0].extra();
    assert!(
        extra.contains_key("fake-field"),
        "unknown field must still be captured after roundtrips"
    );
}
