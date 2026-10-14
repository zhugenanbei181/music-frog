use super::*;
use infiltrator_contract::profile_display_name::{
    ProfileDisplayName, ProfileDisplayNameMutation, ProfileDisplayNameObservation,
    ProfileDisplayNameTarget,
};

pub(super) fn observe_fake_store(
    store: &FakeStore,
    profile: &ProfileId,
) -> Result<ProfileDisplayNameObservation, PortError> {
    if !store
        .profiles
        .lock()
        .unwrap()
        .contains_key(profile.as_str())
    {
        return Err(PortError::NotFound(profile.as_str().to_owned()));
    }
    Ok(store
        .display_names
        .lock()
        .unwrap()
        .get(profile.as_str())
        .cloned()
        .unwrap_or(ProfileDisplayNameObservation::Identity))
}

pub(super) fn set_fake_store(
    store: &FakeStore,
    profile: &ProfileId,
    target: ProfileDisplayNameTarget,
) -> Result<ProfileDisplayNameMutation, PortError> {
    let previous = observe_fake_store(store, profile)?;
    let current = match target {
        ProfileDisplayNameTarget::Identity => ProfileDisplayNameObservation::Identity,
        ProfileDisplayNameTarget::Custom(name) if name.as_str() == profile.as_str() => {
            ProfileDisplayNameObservation::Identity
        }
        ProfileDisplayNameTarget::Custom(name) => ProfileDisplayNameObservation::Custom(name),
    };
    if previous == current {
        return Ok(ProfileDisplayNameMutation::Unchanged { current });
    }
    let mut names = store.display_names.lock().unwrap();
    match &current {
        ProfileDisplayNameObservation::Identity => {
            names.remove(profile.as_str());
        }
        ProfileDisplayNameObservation::Custom(_) => {
            names.insert(profile.as_str().to_owned(), current.clone());
        }
    }
    Ok(ProfileDisplayNameMutation::Applied { previous, current })
}

#[tokio::test]
async fn display_name_use_case_keeps_profile_identity_and_returns_typed_readback() {
    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", true));
    let application = ProfileApplication::new(store.clone());
    let label = ProfileDisplayName::new("Primary route").unwrap();

    assert_eq!(
        application
            .set_profile_display_name("main", ProfileDisplayNameTarget::Custom(label.clone()),)
            .await
            .unwrap(),
        ProfileDisplayNameMutation::Applied {
            previous: ProfileDisplayNameObservation::Identity,
            current: ProfileDisplayNameObservation::Custom(label.clone()),
        }
    );
    assert_eq!(
        application.profile_display_name("main").await.unwrap(),
        ProfileDisplayNameObservation::Custom(label)
    );
    assert_eq!(application.current_profile().await.unwrap(), "main");

    assert_eq!(
        application
            .set_profile_display_name("main", ProfileDisplayNameTarget::Identity)
            .await
            .unwrap(),
        ProfileDisplayNameMutation::Applied {
            previous: ProfileDisplayNameObservation::Custom(
                ProfileDisplayName::new("Primary route").unwrap(),
            ),
            current: ProfileDisplayNameObservation::Identity,
        }
    );
}

#[tokio::test]
async fn invalid_profile_identity_is_rejected_before_display_name_port_call() {
    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", false));
    let application = ProfileApplication::new(store);

    let result = application
        .set_profile_display_name(
            "../main",
            ProfileDisplayNameTarget::Custom(ProfileDisplayName::new("Primary").unwrap()),
        )
        .await;

    assert!(result.is_err());
}
