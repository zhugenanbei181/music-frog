//! Source-bound two-document persistence under the store's write boundary.
use crate::manager::ConfigManager;
use crate::manager::paths::sanitized_profile_key;
use crate::manager::profiles::backup_path;
use crate::sidecar_store::{read_optional, remove_optional, write_document};
use crate::yaml::validate;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_source::ProfileSourceIdentity;
use infiltrator_domain::profile_options::{ProfileOptions, options_path};
use infiltrator_domain::profile_source::identify_profile_source;
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_workspace::{
    ProfileWorkspace, ProfileWorkspacePurpose, ProfileWorkspaceUpdate,
};
use infiltrator_ports::secure_store::SecureStore;
use mihomo_api::error::MihomoError;
use std::fmt::Display;
use std::future::Future;
use std::io::ErrorKind;
use std::path::Path;

#[derive(Clone, Copy)]
pub(crate) enum WorkspaceTarget {
    Any,
    Active,
    Inactive,
}

impl<S: SecureStore> ConfigManager<S> {
    pub(crate) async fn read_active_workspace(&self) -> Result<ProfileWorkspace, PortError> {
        let _guard = self.lock_profile_writes().await;
        let current = self.get_current().await.map_err(config_error)?;
        self.read_workspace_locked(&current).await
    }
    pub(crate) async fn read_workspace(
        &self,
        profile: &str,
    ) -> Result<ProfileWorkspace, PortError> {
        let _guard = self.lock_profile_writes().await;
        self.read_workspace_locked(profile).await
    }

    async fn read_workspace_locked(&self, profile: &str) -> Result<ProfileWorkspace, PortError> {
        let profile = sanitized_profile_key(profile).map_err(config_error)?;
        let content = self.load(&profile).await.map_err(config_error)?;
        let raw_options = read_optional(&options_path(self.config_dir(), &profile)).await?;
        let options = match &raw_options {
            Some(text) => serde_yaml_ng::from_str(text).map_err(storage_error)?,
            None => Default::default(),
        };
        let write_protection = self.workspace_protection(&profile).await?;
        Ok(ProfileWorkspace {
            write_protection,
            source: identify_profile_source(profile, &content, raw_options.as_deref()),
            content,
            options,
            options_document: raw_options,
        })
    }

    pub(crate) async fn save_workspace(
        &self,
        expected: &ProfileSourceIdentity,
        update: &ProfileWorkspaceUpdate,
        target: WorkspaceTarget,
    ) -> Result<ProfileWorkspace, PortError> {
        validate(&update.content).map_err(|error| {
            PortError::Rejected(Failure::new(
                ErrorCode::Configuration,
                error.to_string(),
                false,
            ))
        })?;
        let new_options = if update.options.is_empty() {
            None
        } else {
            Some(serde_yaml_ng::to_string(&update.options).map_err(storage_error)?)
        };
        self.publish_workspace(expected, update, new_options, target, true)
            .await
    }

    pub(crate) async fn recover_workspace(
        &self,
        expected: &ProfileSourceIdentity,
        previous: &ProfileWorkspace,
    ) -> Result<ProfileWorkspace, PortError> {
        let decoded_options: ProfileOptions = match previous.options_document.as_deref() {
            Some(text) => serde_yaml_ng::from_str(text).map_err(storage_error)?,
            None => Default::default(),
        };
        if previous.source.profile != expected.profile
            || previous.source
                != identify_profile_source(
                    previous.source.profile.clone(),
                    &previous.content,
                    previous.options_document.as_deref(),
                )
            || decoded_options != previous.options
        {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidInput,
                "Recovery documents do not match their observed source",
                false,
            )));
        }
        validate(&previous.content).map_err(config_error)?;
        self.publish_workspace(
            expected,
            &ProfileWorkspaceUpdate {
                purpose: ProfileWorkspacePurpose::Derived,
                content: previous.content.clone(),
                options: previous.options.clone(),
            },
            previous.options_document.clone(),
            WorkspaceTarget::Any,
            false,
        )
        .await
    }

    async fn publish_workspace(
        &self,
        expected: &ProfileSourceIdentity,
        update: &ProfileWorkspaceUpdate,
        new_options: Option<String>,
        target: WorkspaceTarget,
        preserve_unchanged_options: bool,
    ) -> Result<ProfileWorkspace, PortError> {
        let _guard = self.lock_profile_writes().await;
        let workspace = self.read_workspace_locked(&expected.profile).await?;
        let new_options = if preserve_unchanged_options && workspace.options == update.options {
            workspace.options_document.clone()
        } else {
            new_options
        };
        let target_matches = match target {
            WorkspaceTarget::Any => true,
            WorkspaceTarget::Active => {
                self.get_current().await.map_err(config_error)? == expected.profile
            }
            WorkspaceTarget::Inactive => {
                self.get_current().await.map_err(config_error)? != expected.profile
            }
        };
        if workspace.source != *expected || !target_matches {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "The profile document or options changed; reload and inspect before applying",
                true,
            )));
        }
        if matches!(
            update.purpose,
            ProfileWorkspacePurpose::DirectEdit {
                allow_protected: false
            }
        ) && workspace.write_protection.is_protected()
        {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::Configuration,
                "Direct editing of this remote subscription requires an explicit unlock",
                false,
            )));
        }
        // Pin both paths before publishing either document. Never consult the
        // active-profile pointer to redirect an already prepared edit.
        let profile_path = self
            .existing_profile_yaml_path(&expected.profile)
            .await
            .map_err(config_error)?;
        let sidecar_path = options_path(self.config_dir(), &expected.profile);
        publish_pair(
            &sidecar_path,
            workspace.options_document.as_deref(),
            new_options.as_deref(),
            write_document(&profile_path, &update.content),
        )
        .await?;
        if let Err(error) = remove_optional(&backup_path(&profile_path)).await {
            // The pair is already committed. Backup cleanup cannot turn a
            // completed apply into a false failure, and remains under this
            // write boundary so it cannot delete a later writer's backup.
            log::warn!("committed workspace backup cleanup failed: {error}");
        }
        Ok(ProfileWorkspace {
            write_protection: workspace.write_protection,
            source: identify_profile_source(
                expected.profile.clone(),
                &update.content,
                new_options.as_deref(),
            ),
            content: update.content.clone(),
            options: update.options.clone(),
            options_document: new_options,
        })
    }
}

/// The future carries one bounded profile-file publication and starts only
/// after the options have been published.
async fn publish_pair(
    sidecar: &Path,
    previous: Option<&str>,
    proposed: Option<&str>,
    profile_publication: impl Future<Output = Result<(), PortError>>,
) -> Result<(), PortError> {
    publish_options(sidecar, proposed).await?;
    if let Err(cause) = profile_publication.await {
        if let Err(rollback) = publish_options(sidecar, previous).await {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                format!(
                    "profile publication failed ({cause}); options rollback failed ({rollback})"
                ),
                false,
            )));
        }
        return Err(cause);
    }
    Ok(())
}

async fn publish_options(path: &Path, text: Option<&str>) -> Result<(), PortError> {
    match text {
        Some(text) => write_document(path, text).await,
        None => remove_optional(path).await,
    }
}

fn storage_error(error: impl Display) -> PortError {
    PortError::Io(error.to_string())
}

pub(crate) fn config_error(error: MihomoError) -> PortError {
    match error {
        MihomoError::Io(error) if error.kind() == ErrorKind::PermissionDenied => {
            PortError::PermissionDenied(error.to_string())
        }
        MihomoError::Io(error) => PortError::Io(error.to_string()),
        MihomoError::NotFound(reason) => PortError::NotFound(reason),
        other => PortError::Rejected(Failure::new(
            ErrorCode::Configuration,
            other.to_string(),
            false,
        )),
    }
}

#[cfg(test)]
#[path = "profile_workspace_store_tests.rs"]
mod tests;
