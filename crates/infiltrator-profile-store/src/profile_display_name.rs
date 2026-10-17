//! Stable profile identity with a separately persisted display label.

use crate::file_lock::lock_profile_writes_for_dir;
use crate::metadata::ensure_table;
use crate::reset_fence::{
    acquire_reset_fenced_profile_mutation_for_dir, initialize_namespace_with_reset_fence,
};
use crate::store::EngineProfileStore;
use infiltrator_contract::profile_display_name::{
    ProfileDisplayName, ProfileDisplayNameMutation, ProfileDisplayNameObservation,
    ProfileDisplayNameTarget,
};
use infiltrator_contract::profile_document_identity::ProfileId;
use infiltrator_ports::secure_store::SecureStore;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_ports::error::PortError;

impl<S: SecureStore> EngineProfileStore<S> {
    pub async fn profile_display_name(
        &self,
        profile: &ProfileId,
    ) -> Result<ProfileDisplayNameObservation, PortError> {
        initialize_namespace_with_reset_fence(self).await?;
        let _guard = lock_profile_writes_for_dir(self.config_dir()).await?;
        self.existing_profile_document_path(profile).await?;
        let settings = self.read_settings_value().await?;
        observation_from_settings(&settings, profile)
    }

    pub async fn set_profile_display_name(
        &self,
        profile: &ProfileId,
        target: ProfileDisplayNameTarget,
    ) -> Result<ProfileDisplayNameMutation, PortError> {
        initialize_namespace_with_reset_fence(self).await?;
        let _guard = acquire_reset_fenced_profile_mutation_for_dir(self.config_dir()).await?;
        self.existing_profile_document_path(profile).await?;

        let mut settings = self.read_settings_value().await?;
        let previous = observation_from_settings(&settings, profile)?;
        let current = canonicalize_target(profile, target);
        if previous == current {
            return Ok(ProfileDisplayNameMutation::Unchanged { current });
        }

        let root = ensure_table(&mut settings)?;
        let profiles = root
            .entry("profiles".to_owned())
            .or_insert_with(|| toml::Value::Table(Default::default()));
        let profiles = ensure_table(profiles)?;
        let profile_row = profiles
            .entry(profile.as_str().to_owned())
            .or_insert_with(|| toml::Value::Table(Default::default()));
        let profile_row = ensure_table(profile_row)?;
        match &current {
            ProfileDisplayNameObservation::Identity => {
                profile_row.remove("display_name");
            }
            ProfileDisplayNameObservation::Custom(name) => {
                profile_row.insert(
                    "display_name".to_owned(),
                    toml::Value::String(name.as_str().to_owned()),
                );
            }
        }
        self.write_settings_value(&settings).await?;

        let readback = observation_from_settings(&self.read_settings_value().await?, profile)?;
        if readback != current {
            return Err(PortError::Failed(
                "profile display-name write returned a mismatched readback".to_owned(),
            ));
        }
        Ok(ProfileDisplayNameMutation::Applied { previous, current })
    }
}

fn observation_from_settings(
    settings: &toml::Value,
    profile: &ProfileId,
) -> Result<ProfileDisplayNameObservation, PortError> {
    let raw = settings
        .get("profiles")
        .and_then(toml::Value::as_table)
        .and_then(|profiles| profiles.get(profile.as_str()))
        .and_then(toml::Value::as_table)
        .and_then(|row| row.get("display_name"));
    match raw {
        None => Ok(ProfileDisplayNameObservation::Identity),
        Some(value) => {
            let raw = value.as_str().ok_or_else(|| {
                PortError::Rejected(Failure::new(
                    ErrorCode::Configuration,
                    "profile display-name metadata is not text",
                    false,
                ))
            })?;
            ProfileDisplayName::new(raw.to_owned())
                .map(ProfileDisplayNameObservation::Custom)
                .map_err(|error| {
                    PortError::Rejected(Failure::new(
                        ErrorCode::Configuration,
                        format!("invalid stored profile display name: {error}"),
                        false,
                    ))
                })
        }
    }
}

fn canonicalize_target(
    profile: &ProfileId,
    target: ProfileDisplayNameTarget,
) -> ProfileDisplayNameObservation {
    match target {
        ProfileDisplayNameTarget::Identity => ProfileDisplayNameObservation::Identity,
        ProfileDisplayNameTarget::Custom(name) if name.as_str() == profile.as_str() => {
            ProfileDisplayNameObservation::Identity
        }
        ProfileDisplayNameTarget::Custom(name) => ProfileDisplayNameObservation::Custom(name),
    }
}

#[cfg(test)]
#[path = "profile_display_name_tests.rs"]
mod tests;
