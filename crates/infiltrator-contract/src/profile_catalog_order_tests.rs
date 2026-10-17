use super::{
    ProfileCatalogOrderMutation, ProfileCatalogOrderRequest, ProfileCatalogOrderValidationError,
    ProfileCatalogSnapshot,
};
use lurker_core_contract::identity::EngineId;
use lurker_core_contract::profile_document_identity::ProfileId;

fn profile(name: &str) -> ProfileId {
    ProfileId::new(name).unwrap()
}

fn snapshot(ids: &[&str]) -> ProfileCatalogSnapshot {
    ProfileCatalogSnapshot::new(
        EngineId::new("mihomo").unwrap(),
        ids.iter().map(|id| profile(id)).collect(),
    )
}

#[test]
fn order_request_requires_an_exact_profile_permutation() {
    let expected = snapshot(&["alpha", "beta", "gamma"]);

    assert_eq!(
        ProfileCatalogOrderRequest::new(
            expected.clone(),
            vec![profile("gamma"), profile("alpha"), profile("beta"),]
        )
        .validate_for_engine(&EngineId::new("mihomo").unwrap()),
        Ok(())
    );
    assert_eq!(
        ProfileCatalogOrderRequest::new(
            expected.clone(),
            vec![profile("alpha"), profile("alpha"), profile("gamma"),]
        )
        .validate_for_engine(&EngineId::new("mihomo").unwrap()),
        Err(ProfileCatalogOrderValidationError::DuplicateTargetProfile(
            profile("alpha"),
        ))
    );
    assert_eq!(
        ProfileCatalogOrderRequest::new(expected.clone(), vec![profile("alpha"), profile("beta")])
            .validate_for_engine(&EngineId::new("mihomo").unwrap()),
        Err(ProfileCatalogOrderValidationError::MissingTargetProfile(
            profile("gamma"),
        ))
    );
    assert_eq!(
        ProfileCatalogOrderRequest::new(
            expected.clone(),
            vec![profile("alpha"), profile("beta"), profile("other")],
        )
        .validate_for_engine(&EngineId::new("mihomo").unwrap()),
        Err(ProfileCatalogOrderValidationError::ForeignTargetProfile(
            profile("other"),
        ))
    );
    assert_eq!(
        ProfileCatalogOrderRequest::new(snapshot(&["alpha", "alpha"]), vec![profile("alpha")])
            .validate_for_engine(&EngineId::new("mihomo").unwrap()),
        Err(ProfileCatalogOrderValidationError::DuplicateExpectedProfile(profile("alpha"),))
    );
    assert_eq!(
        ProfileCatalogOrderRequest::new(expected, vec![])
            .validate_for_engine(&EngineId::new("sing-box").unwrap()),
        Err(ProfileCatalogOrderValidationError::ForeignEngine {
            request_engine: EngineId::new("mihomo").unwrap(),
            owner_engine: EngineId::new("sing-box").unwrap(),
        })
    );
}

#[test]
fn stale_catalog_mutation_round_trips_with_typed_snapshots() {
    let outcome = ProfileCatalogOrderMutation::Stale {
        expected: snapshot(&["alpha", "beta"]),
        observed: snapshot(&["beta"]),
    };
    let encoded = serde_json::to_string(&outcome).unwrap();
    assert_eq!(
        serde_json::from_str::<ProfileCatalogOrderMutation>(&encoded).unwrap(),
        outcome
    );
}
