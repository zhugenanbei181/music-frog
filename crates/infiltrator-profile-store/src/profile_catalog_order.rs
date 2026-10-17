//! Durable, engine-scoped ordering for profile catalog entries.

use crate::file_lock::lock_profile_writes_for_dir;
use crate::metadata::ensure_table;
use crate::reset_fence::initialize_namespace_with_reset_fence;
use crate::store::EngineProfileStore;
use infiltrator_contract::profile_catalog_order::{
    ProfileCatalogOrderMutation, ProfileCatalogOrderRequest, ProfileCatalogSnapshot,
};
use infiltrator_contract::profile_document_identity::ProfileId;
use infiltrator_domain::profiles::ProfileInfo;
use infiltrator_ports::active_profile_reference_store::ActiveProfileReferenceStore;
use infiltrator_ports::secure_store::SecureStore;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::EngineId;
use lurker_core_ports::error::PortError;
use std::collections::{BTreeMap, HashSet};

pub(crate) fn apply_catalog_order(
    profiles: &mut [ProfileInfo],
    settings: &toml::Value,
) -> Result<(), PortError> {
    let actual_names = profiles
        .iter()
        .map(|profile| {
            ProfileId::new(profile.name.clone())
                .map(|profile_id| profile_id.as_str().to_owned())
                .map_err(|error| catalog_configuration_error(error.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let actual_names = actual_names.into_iter().collect::<HashSet<_>>();

    let Some(stored_order) = settings.get("profile_catalog_order") else {
        profiles.sort_by(|left, right| left.name.cmp(&right.name));
        return Ok(());
    };
    let stored_order = stored_order.as_array().ok_or_else(|| {
        catalog_configuration_error("profile catalog order is not an array".to_owned())
    })?;

    let mut seen = HashSet::with_capacity(stored_order.len());
    let mut ordered_names = Vec::with_capacity(actual_names.len());
    for value in stored_order {
        let name = value.as_str().ok_or_else(|| {
            catalog_configuration_error("profile catalog order contains a non-text ID".to_owned())
        })?;
        let profile_id = ProfileId::new(name.to_owned())
            .map_err(|error| catalog_configuration_error(error.to_string()))?;
        if !seen.insert(profile_id.as_str().to_owned()) {
            return Err(catalog_configuration_error(format!(
                "profile catalog order repeats `{}`",
                profile_id.as_str()
            )));
        }
        if actual_names.contains(profile_id.as_str()) {
            ordered_names.push(profile_id.as_str().to_owned());
        }
    }

    let mut newly_created = actual_names
        .into_iter()
        .filter(|profile_id| !seen.contains(profile_id))
        .collect::<Vec<_>>();
    newly_created.sort();
    ordered_names.extend(newly_created);

    let ranks = ordered_names
        .into_iter()
        .enumerate()
        .map(|(rank, profile_id)| (profile_id, rank))
        .collect::<BTreeMap<_, _>>();
    profiles.sort_by_key(|profile| ranks.get(&profile.name).copied().unwrap_or(usize::MAX));
    Ok(())
}

pub(crate) fn snapshot_from_profile_info(
    engine_id: EngineId,
    profiles: Vec<ProfileInfo>,
) -> Result<ProfileCatalogSnapshot, PortError> {
    let ordered_profile_ids = profiles
        .into_iter()
        .map(|profile| {
            ProfileId::new(profile.name)
                .map_err(|error| catalog_configuration_error(error.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ProfileCatalogSnapshot::new(engine_id, ordered_profile_ids))
}

impl<S: SecureStore> EngineProfileStore<S> {
    pub async fn set_profile_catalog_order(
        &self,
        request: ProfileCatalogOrderRequest,
    ) -> Result<ProfileCatalogOrderMutation, PortError> {
        request
            .validate_for_engine(&self.engine_id())
            .map_err(|error| {
                PortError::Rejected(Failure::new(
                    ErrorCode::InvalidInput,
                    error.to_string(),
                    false,
                ))
            })?;
        initialize_namespace_with_reset_fence(self).await?;
        let selection = ActiveProfileReferenceStore::acquire_selection_lease(self).await?;
        let _profile_guard = lock_profile_writes_for_dir(self.config_dir()).await?;

        let previous = snapshot_from_profile_info(
            self.engine_id(),
            self.list_profile_info_locked(selection.state()).await?,
        )?;
        if previous != request.expected {
            return Ok(ProfileCatalogOrderMutation::Stale {
                expected: request.expected,
                observed: previous,
            });
        }
        if previous.ordered_profile_ids == request.target_order {
            return Ok(ProfileCatalogOrderMutation::Unchanged { current: previous });
        }

        let mut settings = self.read_settings_value().await?;
        let root = ensure_table(&mut settings)?;
        root.insert(
            "profile_catalog_order".to_owned(),
            toml::Value::Array(
                request
                    .target_order
                    .iter()
                    .map(|profile_id| toml::Value::String(profile_id.as_str().to_owned()))
                    .collect(),
            ),
        );
        self.write_settings_value(&settings).await?;

        let current = snapshot_from_profile_info(
            self.engine_id(),
            self.list_profile_info_locked(selection.state()).await?,
        )?;
        let expected_current =
            ProfileCatalogSnapshot::new(self.engine_id(), request.target_order.clone());
        if current != expected_current {
            return Err(PortError::Failed(
                "profile catalog order write returned a mismatched readback".to_owned(),
            ));
        }
        Ok(ProfileCatalogOrderMutation::Applied { previous, current })
    }
}

fn catalog_configuration_error(message: String) -> PortError {
    PortError::Rejected(Failure::new(ErrorCode::Configuration, message, false))
}

#[cfg(test)]
#[path = "profile_catalog_order_tests.rs"]
mod tests;
