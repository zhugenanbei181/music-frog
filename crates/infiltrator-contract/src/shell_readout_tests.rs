use super::ShellObservation;
use serde_json::json;

#[test]
fn shell_observation_wire_has_an_explicit_state_discriminant() {
    assert_eq!(
        serde_json::to_value(ShellObservation::<u64>::Unobserved).unwrap(),
        json!({"state": "unobserved"})
    );
    assert_eq!(
        serde_json::to_value(ShellObservation::Current(7_u64)).unwrap(),
        json!({"state": "current", "value": 7})
    );
    assert_eq!(
        serde_json::to_value(ShellObservation::<u64>::CurrentEmpty).unwrap(),
        json!({"state": "current_empty"})
    );
    assert_eq!(
        serde_json::to_value(ShellObservation::Stale(7_u64)).unwrap(),
        json!({"state": "stale", "value": 7})
    );
}

#[test]
fn value_access_and_stale_retention_preserve_the_typed_observation() {
    assert_eq!(ShellObservation::<u64>::Unobserved.value(), None);
    assert_eq!(ShellObservation::<u64>::CurrentEmpty.value(), None);
    assert_eq!(ShellObservation::Current(3_u64).current_value(), Some(&3));
    assert_eq!(ShellObservation::Stale(4_u64).current_value(), None);
    assert_eq!(
        ShellObservation::Current(5_u64).into_stale(),
        ShellObservation::Stale(5)
    );
    assert_eq!(
        ShellObservation::<u64>::CurrentEmpty.into_stale(),
        ShellObservation::Unobserved
    );
}
