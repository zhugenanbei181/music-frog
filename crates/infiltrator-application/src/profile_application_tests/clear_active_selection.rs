use super::*;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReferenceRevision, ActiveProfileReferenceWriteAuthorization,
};
use infiltrator_contract::profile_selection_clear::{
    ClearActiveProfileSelectionOutcome, ClearActiveProfileSelectionRequest,
    UnselectedProfileSelectionReadback,
};
use lurker_core_contract::identity::CoreInstanceId;

fn clear_request(expected: ActiveProfileReferenceState) -> ClearActiveProfileSelectionRequest {
    ClearActiveProfileSelectionRequest::new(
        expected,
        ActiveProfileReferenceWriteAuthorization::Unowned,
    )
}

#[tokio::test]
async fn clear_selection_returns_exact_unselected_readback_and_is_idempotent() {
    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", true));
    let application = ProfileApplication::new(store.clone());
    let expected = store.active_reference();
    let ActiveProfileReferenceState::Selected(previous) = expected.clone() else {
        panic!("fixture starts with a selected profile")
    };

    assert_eq!(
        application
            .clear_active_profile_selection(clear_request(expected.clone()))
            .await
            .unwrap(),
        ClearActiveProfileSelectionOutcome::Cleared {
            previous,
            current: UnselectedProfileSelectionReadback {
                revision: ActiveProfileReferenceRevision::new(2),
            },
        }
    );
    let current = store.active_reference();
    assert_eq!(
        current,
        ActiveProfileReferenceState::Unselected {
            revision: ActiveProfileReferenceRevision::new(2),
        }
    );
    assert_eq!(store.reference_write_count.load(Ordering::SeqCst), 1);

    assert_eq!(
        application
            .clear_active_profile_selection(clear_request(expected.clone()))
            .await
            .unwrap(),
        ClearActiveProfileSelectionOutcome::Stale {
            expected,
            observed: current.clone(),
        }
    );
    assert_eq!(
        application
            .clear_active_profile_selection(clear_request(current.clone()))
            .await
            .unwrap(),
        ClearActiveProfileSelectionOutcome::AlreadyUnselected {
            current: UnselectedProfileSelectionReadback {
                revision: ActiveProfileReferenceRevision::new(2),
            },
        }
    );
    assert_eq!(store.reference_write_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn clear_selection_rejects_stale_and_foreign_engine_requests_without_writes() {
    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", true));
    let application = ProfileApplication::new(store.clone());
    let expected = store.active_reference();
    let observed = ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
        EngineId::mihomo(),
        ProfileId::new("main").unwrap(),
        ActiveProfileReferenceRevision::new(2),
    ));
    *store.active_reference.lock().expect("active reference") = observed.clone();
    assert_eq!(
        application
            .clear_active_profile_selection(clear_request(expected.clone()))
            .await
            .unwrap(),
        ClearActiveProfileSelectionOutcome::Stale { expected, observed }
    );
    assert_eq!(store.reference_write_count.load(Ordering::SeqCst), 0);

    let foreign = ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
        EngineId::sing_box(),
        ProfileId::new("main").unwrap(),
        ActiveProfileReferenceRevision::new(2),
    ));
    *store.active_reference.lock().expect("active reference") = foreign.clone();
    let failure = application
        .clear_active_profile_selection(clear_request(foreign.clone()))
        .await
        .unwrap_err();
    assert_eq!(failure.code, ErrorCode::InvalidInput);
    assert_eq!(store.active_reference(), foreign);
    assert_eq!(store.reference_write_count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn exact_bound_owner_authorization_clears_selection_without_unbinding_core() {
    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", true));
    let application = ProfileApplication::new(store.clone());
    let identity = CoreIdentity::new(
        EngineId::mihomo(),
        CoreInstanceId::new("core-1").unwrap(),
        None,
    );
    let generation = CoreGeneration::new(8);
    store.bind_owner_for_test(identity.clone(), generation);
    let expected = store.active_reference();

    let wrong_generation = ClearActiveProfileSelectionRequest::new(
        expected.clone(),
        ActiveProfileReferenceWriteAuthorization::BoundOwner {
            identity: identity.clone(),
            generation: CoreGeneration::new(7),
        },
    );
    assert_eq!(
        application
            .clear_active_profile_selection(wrong_generation)
            .await
            .unwrap_err()
            .code,
        ErrorCode::NotReady
    );
    assert_eq!(store.active_reference(), expected.clone());
    assert_eq!(store.reference_write_count.load(Ordering::SeqCst), 0);

    let request = ClearActiveProfileSelectionRequest::new(
        expected,
        ActiveProfileReferenceWriteAuthorization::BoundOwner {
            identity: identity.clone(),
            generation,
        },
    );
    let outcome = application
        .clear_active_profile_selection(request)
        .await
        .unwrap();
    let ClearActiveProfileSelectionOutcome::Cleared { current, .. } = outcome else {
        panic!("an authorized bound owner clear should commit")
    };
    assert_eq!(
        current,
        UnselectedProfileSelectionReadback {
            revision: ActiveProfileReferenceRevision::new(2),
        }
    );
    assert_eq!(
        store.active_reference(),
        ActiveProfileReferenceState::Unselected {
            revision: ActiveProfileReferenceRevision::new(2),
        }
    );
    assert_eq!(
        *store.bound_owner.lock().expect("test active owner"),
        Some((identity, generation))
    );
    assert_eq!(store.reference_write_count.load(Ordering::SeqCst), 1);
}
