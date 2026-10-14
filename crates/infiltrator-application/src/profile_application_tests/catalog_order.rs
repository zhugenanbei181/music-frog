use super::*;
use infiltrator_contract::profile_catalog_order::{
    ProfileCatalogOrderMutation, ProfileCatalogOrderRequest, ProfileCatalogSnapshot,
};
use std::collections::BTreeMap;

pub(super) fn sort_fake_profiles(
    order: &Mutex<Option<Vec<ProfileId>>>,
    profiles: &mut [ProfileInfo],
) {
    let Some(order) = order.lock().unwrap().clone() else {
        return;
    };
    let ranks = order
        .into_iter()
        .enumerate()
        .map(|(index, profile_id)| (profile_id.as_str().to_owned(), index))
        .collect::<BTreeMap<_, _>>();
    profiles.sort_by_key(|profile| {
        (
            ranks.get(&profile.name).copied().unwrap_or(usize::MAX),
            profile.name.clone(),
        )
    });
}

pub(super) async fn set_fake_store_order(
    store: &FakeStore,
    request: ProfileCatalogOrderRequest,
) -> Result<ProfileCatalogOrderMutation, PortError> {
    request
        .validate_for_engine(&store.engine_id())
        .map_err(|error| {
            PortError::Rejected(Failure::new(
                ErrorCode::InvalidInput,
                error.to_string(),
                false,
            ))
        })?;
    let previous = snapshot_for_store(store).await?;
    if previous != request.expected {
        return Ok(ProfileCatalogOrderMutation::Stale {
            expected: request.expected,
            observed: previous,
        });
    }
    if previous.ordered_profile_ids == request.target_order {
        return Ok(ProfileCatalogOrderMutation::Unchanged { current: previous });
    }
    *store.catalog_order.lock().unwrap() = Some(request.target_order);
    let current = snapshot_for_store(store).await?;
    Ok(ProfileCatalogOrderMutation::Applied { previous, current })
}

async fn snapshot_for_store(store: &FakeStore) -> Result<ProfileCatalogSnapshot, PortError> {
    let ordered_profile_ids = store
        .list_profiles()
        .await?
        .into_iter()
        .map(|profile| {
            ProfileId::new(profile.name).map_err(|error| {
                PortError::Rejected(Failure::new(
                    ErrorCode::Configuration,
                    error.to_string(),
                    false,
                ))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ProfileCatalogSnapshot::new(
        store.engine_id(),
        ordered_profile_ids,
    ))
}

#[tokio::test]
async fn profile_catalog_order_is_engine_scoped_and_stale_requests_do_not_write() {
    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", false));
    for name in ["alpha", "zeta"] {
        store.profiles.lock().unwrap().insert(
            name.to_owned(),
            ("mode: rule\n".to_owned(), ProfileMetadata::default()),
        );
    }
    let application = ProfileApplication::new(store.clone());
    let expected = application.profile_catalog_order_snapshot().await.unwrap();
    assert_eq!(
        expected
            .ordered_profile_ids
            .iter()
            .map(ProfileId::as_str)
            .collect::<Vec<_>>(),
        vec!["alpha", "main", "zeta"]
    );

    let target_order = vec![
        ProfileId::new("zeta").unwrap(),
        ProfileId::new("main").unwrap(),
        ProfileId::new("alpha").unwrap(),
    ];
    assert_eq!(
        application
            .set_profile_catalog_order(ProfileCatalogOrderRequest::new(
                expected.clone(),
                target_order.clone(),
            ))
            .await
            .unwrap(),
        ProfileCatalogOrderMutation::Applied {
            previous: expected.clone(),
            current: ProfileCatalogSnapshot::new(store.engine_id(), target_order.clone()),
        }
    );

    let stale_target = vec![
        ProfileId::new("alpha").unwrap(),
        ProfileId::new("zeta").unwrap(),
        ProfileId::new("main").unwrap(),
    ];
    assert_eq!(
        application
            .set_profile_catalog_order(ProfileCatalogOrderRequest::new(
                expected.clone(),
                stale_target,
            ))
            .await
            .unwrap(),
        ProfileCatalogOrderMutation::Stale {
            expected,
            observed: ProfileCatalogSnapshot::new(store.engine_id(), target_order.clone()),
        }
    );
    assert_eq!(
        application
            .profile_catalog_order_snapshot()
            .await
            .unwrap()
            .ordered_profile_ids,
        target_order
    );
}

#[tokio::test]
async fn invalid_profile_catalog_order_is_rejected_before_port_write() {
    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", false));
    let application = ProfileApplication::new(store.clone());
    let expected = application.profile_catalog_order_snapshot().await.unwrap();
    let result = application
        .set_profile_catalog_order(ProfileCatalogOrderRequest::new(
            expected,
            vec![
                ProfileId::new("main").unwrap(),
                ProfileId::new("main").unwrap(),
            ],
        ))
        .await;

    assert_eq!(result.unwrap_err().code, ErrorCode::InvalidInput);
    assert_eq!(
        application
            .profile_catalog_order_snapshot()
            .await
            .unwrap()
            .ordered_profile_ids,
        vec![ProfileId::new("main").unwrap()]
    );
}
