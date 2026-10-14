use super::*;

#[tokio::test]
async fn age_credential_readback_returns_the_selection_observed_by_the_application() {
    let store = Arc::new(FakeStore::with_profile("primary", "mode: rule\n", true));
    let expected_selection = store.active_reference();
    let application = ProfileApplication::new(store);

    let readback = application
        .profile_age_credential(
            profile_id("primary"),
            ActiveProfileReferenceWriteAuthorization::Unowned,
        )
        .await
        .expect("read owner credential state");

    assert_eq!(
        readback,
        ProfileAgeCredentialReadOutcome::Observed {
            current: ProfileAgeCredentialObservation::new(
                EngineId::mihomo(),
                profile_id("primary"),
                ProfileAgeCredentialRevision::new(4),
                ProfileAgeCredentialState::NotConfigured,
            ),
            selection: expected_selection,
        }
    );
}
