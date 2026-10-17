//! Product namespace initialization: engine schema plus credential migration.
//!
//! The neutral kernel owns the schema marker, durable state recovery and the
//! pure legacy-state merge. This module composes them with the product
//! credential-store migration that rewrites subscription keys into the
//! engine-scoped namespace.

use crate::metadata::{SUBSCRIPTION_SERVICE, legacy_subscription_key, subscription_key};
use crate::store::EngineProfileStore;
use infiltrator_ports::secure_store::SecureStore;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::profile_name::validate_profile_name;
use lurker_core_ports::error::PortError;
use lurker_profile_store::namespace::{
    LegacyProfileNamespaceMigrationV1, PROFILE_NAMESPACE_SCHEMA_KEY,
    ProfileNamespaceInitialization, ProfileNamespaceSchema, has_legacy_profile_data_with_platform,
    initialize_engine_scoped_namespace_unlocked_with_platform, merge_profile_state,
    migrate_legacy_profile_files_with_platform, read_profile_state_with_platform,
    retire_legacy_sync_index_with_platform, write_legacy_settings_with_platform,
    write_profile_state_atomically_with_platform,
};
use std::future::Future;
use std::io;
use tokio::fs;
use toml::map::Map;

impl<S: SecureStore> EngineProfileStore<S> {
    /// Initialize this store's typed namespace policy once, under a
    /// caller-provided product mutation fence.
    pub async fn initialize_profile_namespace_with<G, F, Fut>(
        &self,
        acquire_fence: F,
    ) -> Result<(), PortError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<G, PortError>>,
    {
        self.initialize_namespace_with(|| async {
            let _fence = acquire_fence().await?;
            match self.namespace_initialization() {
                ProfileNamespaceInitialization::EngineScoped => {
                    initialize_engine_scoped_namespace_unlocked_with_platform(
                        self.platform(),
                        self.layout(),
                        self.config_dir(),
                        self.settings_file(),
                    )
                    .await
                }
                ProfileNamespaceInitialization::LegacyV1(migration) => {
                    initialize_legacy_namespace_v1_unlocked(self, migration).await
                }
            }
        })
        .await
    }

    pub async fn reset_engine_scoped_namespace_marker_unlocked(&self) -> Result<(), PortError> {
        let mut state = toml::Value::Table(Map::new());
        ProfileNamespaceSchema::write_engine_scoped_v1(&mut state)?;
        write_profile_state_atomically_with_platform(
            self.platform(),
            self.layout(),
            self.settings_file(),
            &state,
        )
        .await
    }
}

async fn initialize_legacy_namespace_v1_unlocked<S: SecureStore>(
    store: &EngineProfileStore<S>,
    migration: &LegacyProfileNamespaceMigrationV1,
) -> Result<(), PortError> {
    migration.validate()?;
    let mut scoped_state =
        read_profile_state_with_platform(store.platform(), store.layout(), store.settings_file())
            .await?;
    let schema = ProfileNamespaceSchema::parse(scoped_state.get(PROFILE_NAMESPACE_SCHEMA_KEY))?;
    match schema {
        ProfileNamespaceSchema::EngineScopedV1 => {
            migrate_legacy_profile_files_with_platform(
                store.platform(),
                store.config_root(),
                store.config_dir(),
                store.document_extension(),
                migration,
            )
            .await?;
            let mut legacy_state = read_profile_state_with_platform(
                store.platform(),
                store.layout(),
                migration.settings_file(),
            )
            .await?;
            let previous_legacy_state = legacy_state.clone();
            let mut retired = toml::Value::Table(Map::new());
            merge_profile_state(&mut retired, &mut legacy_state)?;
            if legacy_state != previous_legacy_state {
                write_legacy_settings_with_platform(
                    store.platform(),
                    store.layout(),
                    migration.settings_file(),
                    &legacy_state,
                )
                .await?;
            }
            retire_legacy_sync_index_with_platform(
                store.platform(),
                store.config_root(),
                migration,
            )
            .await?;
            return Ok(());
        }
        ProfileNamespaceSchema::LegacyShared => {}
    }

    if !has_legacy_profile_data_with_platform(
        store.platform(),
        store.layout(),
        store.config_root(),
        store.settings_file(),
        store.document_extension(),
        migration,
    )
    .await?
    {
        return Ok(());
    }
    fs::create_dir_all(store.config_dir())
        .await
        .map_err(map_io_error)?;
    migrate_legacy_profile_files_with_platform(
        store.platform(),
        store.config_root(),
        store.config_dir(),
        store.document_extension(),
        migration,
    )
    .await?;
    let mut legacy_state = read_profile_state_with_platform(
        store.platform(),
        store.layout(),
        migration.settings_file(),
    )
    .await?;
    let previous_legacy_state = legacy_state.clone();
    merge_profile_state(&mut scoped_state, &mut legacy_state)?;
    migrate_legacy_credentials(store, &mut scoped_state).await?;
    ProfileNamespaceSchema::write_engine_scoped_v1(&mut scoped_state)?;
    write_profile_state_atomically_with_platform(
        store.platform(),
        store.layout(),
        store.settings_file(),
        &scoped_state,
    )
    .await?;
    if legacy_state != previous_legacy_state {
        write_legacy_settings_with_platform(
            store.platform(),
            store.layout(),
            migration.settings_file(),
            &legacy_state,
        )
        .await?;
    }
    retire_legacy_sync_index_with_platform(store.platform(), store.config_root(), migration).await
}

/// Rewrite every legacy subscription key into the engine-scoped namespace.
async fn migrate_legacy_credentials<S: SecureStore>(
    store: &EngineProfileStore<S>,
    state: &mut toml::Value,
) -> Result<(), PortError> {
    let engine = store.engine_id();
    let Some(profiles) = state
        .as_table_mut()
        .and_then(|table| table.get_mut("profiles"))
        .and_then(toml::Value::as_table_mut)
    else {
        return Ok(());
    };

    for (profile, metadata) in profiles.iter_mut() {
        validate_profile_name(profile).map_err(|error| {
            namespace_configuration_error(format!(
                "legacy profile ID `{profile}` is invalid: {error}"
            ))
        })?;
        let metadata = metadata.as_table_mut().ok_or_else(|| {
            namespace_configuration_error(format!(
                "profile namespace migration found invalid metadata for `{profile}`"
            ))
        })?;
        let old_key = metadata
            .get("subscription_url_key")
            .and_then(toml::Value::as_str)
            .map(str::to_owned)
            .or_else(|| {
                metadata
                    .get("subscription_url")
                    .and_then(toml::Value::as_str)
                    .filter(|url| !url.trim().is_empty())
                    .map(|_| legacy_subscription_key(profile))
            });
        let Some(old_key) = old_key else {
            continue;
        };

        let new_key = subscription_key(&engine, profile);
        if old_key != new_key {
            let mut value = store
                .credential_store()
                .get(SUBSCRIPTION_SERVICE, &new_key)
                .await
                .map_err(credential_migration_error)?;
            if value.is_none() {
                value = store
                    .credential_store()
                    .get(SUBSCRIPTION_SERVICE, &old_key)
                    .await
                    .map_err(credential_migration_error)?;
                if value.is_none() {
                    value = metadata
                        .get("subscription_url")
                        .and_then(toml::Value::as_str)
                        .map(str::to_owned);
                }
                if let Some(value) = value.as_deref() {
                    store
                        .credential_store()
                        .set(SUBSCRIPTION_SERVICE, &new_key, value)
                        .await
                        .map_err(credential_migration_error)?;
                }
            }
            if value.is_some() {
                store
                    .credential_store()
                    .delete(SUBSCRIPTION_SERVICE, &old_key)
                    .await
                    .map_err(credential_migration_error)?;
            }
        }
        metadata.insert("subscription_url_key".into(), toml::Value::String(new_key));
    }
    Ok(())
}

fn namespace_configuration_error(message: impl Into<String>) -> PortError {
    PortError::Rejected(Failure::new(ErrorCode::Configuration, message, false))
}

fn credential_migration_error(error: PortError) -> PortError {
    namespace_configuration_error(format!(
        "profile credential namespace migration failed: {error}"
    ))
}

fn map_io_error(error: io::Error) -> PortError {
    if error.kind() == io::ErrorKind::PermissionDenied {
        PortError::PermissionDenied(error.to_string())
    } else {
        PortError::Io(error.to_string())
    }
}
