use crate::error::CoreError;
use crate::settings_io::app_config_manager_for_profile_reset;
use infiltrator_platform::defaults::DefaultCredentialStore;
use infiltrator_ports::profile_reset::ProfileResetPort;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_profile_store::profile_reset::{
    ActiveProfileReferenceResetOutcome, ActiveProfileReferenceResetRequest,
};
use lurker_core_contract::active_profile_reference::{
    ActiveProfileReferenceRevision, ActiveProfileReferenceState,
};
use lurker_core_contract::identity::EngineId;
use lurker_core_ports::error::PortError;
use mihomo_config::manager::ConfigManager;
use mihomo_config::port::find_available_port;

pub struct FileProfileReset;

impl FileProfileReset {
    pub fn current() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl ProfileResetPort for FileProfileReset {
    async fn reset_to_default(&self) -> Result<(), PortError> {
        reset_profiles_to_default_impl()
            .await
            .map_err(|error| PortError::Io(error.to_string()))
    }
}

pub async fn reset_profiles_to_default() -> Result<(), CoreError> {
    FileProfileReset::current()
        .reset_to_default()
        .await
        .map_err(|error| CoreError::other(error.to_string()))
}

async fn reset_profiles_to_default_impl() -> Result<(), CoreError> {
    let manager = app_config_manager_for_profile_reset().await?;
    reset_profiles_with_default_builder(&manager, build_default_config).await
}

async fn reset_profiles_with_default_builder(
    manager: &ConfigManager<DefaultCredentialStore>,
    build_default_config: impl FnOnce() -> Result<String, CoreError>,
) -> Result<(), CoreError> {
    let engine_id = manager.engine_id();
    match manager
        .reset_profiles_to_default(ActiveProfileReferenceResetRequest::ResumePendingIntent)
        .await?
    {
        ActiveProfileReferenceResetOutcome::NoPendingIntent => {}
        outcome => {
            finish_reset_outcome(outcome, &engine_id)?;
            return Ok(());
        }
    }

    // Only construct a fresh default after the manager has confirmed there is
    // no pending reset. A pending v2 intent already contains the exact bytes
    // needed for recovery and must not depend on port discovery succeeding.
    let default_config = build_default_config()?;
    let expected =
        ProfileStore::read_active_profile_reference(manager.engine_profile_store()).await?;
    let outcome = manager
        .reset_profiles_to_default(ActiveProfileReferenceResetRequest::Start {
            expected,
            default_document: default_config,
        })
        .await?;
    finish_reset_outcome(outcome, &engine_id)?;
    Ok(())
}

fn finish_reset_outcome(
    outcome: ActiveProfileReferenceResetOutcome,
    engine_id: &EngineId,
) -> Result<(), CoreError> {
    match outcome {
        ActiveProfileReferenceResetOutcome::NoPendingIntent => Err(CoreError::invalid(
            "profile reset did not start after receiving a new request",
        )),
        ActiveProfileReferenceResetOutcome::Committed { previous, current } => {
            let expected_next_revision = previous.revision().next().ok_or_else(|| {
                CoreError::invalid("active profile reference revision overflowed")
            })?;
            validate_reset_commit(
                &previous,
                expected_next_revision,
                engine_id,
                &previous,
                &current,
            )?;
            Ok(())
        }
        ActiveProfileReferenceResetOutcome::CleanupPending {
            previous,
            current,
            error,
        } => {
            let expected_next_revision = previous.revision().next().ok_or_else(|| {
                CoreError::invalid("active profile reference revision overflowed")
            })?;
            validate_reset_commit(
                &previous,
                expected_next_revision,
                engine_id,
                &previous,
                &current,
            )?;
            Err(CoreError::invalid(format!(
                "profile reset committed the default reference at revision {}, but cleanup is pending: {error}",
                current.revision().value()
            )))
        }
        ActiveProfileReferenceResetOutcome::Stale { observed, .. } => {
            Err(CoreError::invalid(format!(
                "profile reset lost a concurrent active-profile selection at revision {}",
                observed.revision().value()
            )))
        }
    }
}

fn validate_reset_commit(
    expected: &ActiveProfileReferenceState,
    expected_next_revision: ActiveProfileReferenceRevision,
    engine_id: &EngineId,
    previous: &ActiveProfileReferenceState,
    current: &ActiveProfileReferenceState,
) -> Result<(), CoreError> {
    if previous == expected
        && current.revision() == expected_next_revision
        && matches!(
            current,
            ActiveProfileReferenceState::Selected(reference)
                if &reference.engine_id == engine_id
                    && reference.profile_id.as_str() == "default"
        )
    {
        Ok(())
    } else {
        Err(CoreError::invalid(
            "profile reset committed an unexpected active reference",
        ))
    }
}

fn build_default_config() -> Result<String, CoreError> {
    let port = find_available_port(9090)
        .ok_or_else(|| CoreError::not_found("无法找到可用的控制接口端口（9090-9190）"))?;
    Ok(format!(
        r#"# mihomo configuration
port: 7890
socks-port: 7891
allow-lan: false
mode: rule
log-level: info
external-controller: 127.0.0.1:{}
"#,
        port
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use lurker_core_contract::active_profile_reference::ActiveProfileReference;
    use lurker_core_contract::profile_document_identity::ProfileId;
    use sha2::{Digest, Sha256};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use tokio::fs::{read, write};

    fn manager(directory: &tempfile::TempDir) -> ConfigManager<DefaultCredentialStore> {
        ConfigManager::with_home_and_store(
            directory.path().to_path_buf(),
            DefaultCredentialStore::default(),
        )
        .expect("isolated reset manager")
    }

    #[tokio::test]
    async fn pending_v2_reset_recovers_without_invoking_failing_port_builder() {
        let directory = tempfile::tempdir().unwrap();
        let manager = manager(&directory);
        let expected = manager.migrate_legacy_mihomo_selection().await.unwrap();
        let current = ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
            manager.engine_id(),
            ProfileId::new("default").unwrap(),
            expected.revision().next().unwrap(),
        ));
        let default_document = b"mode: rule\nmixed-port: 7890\n";
        let previous_default_path = manager.config_dir().join("default.yaml");
        let previous_default = read(&previous_default_path).await.unwrap();
        write(
            manager
                .config_dir()
                .join(".active-profile-reset-default.previous"),
            previous_default,
        )
        .await
        .unwrap();
        let digest = Sha256::digest(default_document)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let intent = serde_json::json!({
            "schema_version": 2,
            "expected_reference": expected.clone(),
            "target_reference": current.clone(),
            "default_document_sha256": digest,
            "default_document": default_document,
        });
        write(
            manager
                .config_dir()
                .join(".active-profile-reset-intent.json"),
            serde_json::to_vec(&intent).unwrap(),
        )
        .await
        .unwrap();

        let builder_called = Arc::new(AtomicBool::new(false));
        let called = Arc::clone(&builder_called);
        reset_profiles_with_default_builder(&manager, move || {
            called.store(true, Ordering::SeqCst);
            Err(CoreError::not_found("no control port available"))
        })
        .await
        .unwrap();

        assert!(!builder_called.load(Ordering::SeqCst));
        assert_eq!(read(previous_default_path).await.unwrap(), default_document);
        assert_eq!(
            ProfileStore::read_active_profile_reference(manager.engine_profile_store(),)
                .await
                .unwrap(),
            current
        );
    }

    #[tokio::test]
    async fn no_pending_reset_invokes_builder_and_starts_new_reset() {
        let directory = tempfile::tempdir().unwrap();
        let manager = manager(&directory);
        manager.migrate_legacy_mihomo_selection().await.unwrap();
        let default_document = "mode: direct\n";
        reset_profiles_with_default_builder(&manager, || Ok(default_document.to_owned()))
            .await
            .unwrap();

        assert_eq!(
            read(manager.config_dir().join("default.yaml"))
                .await
                .unwrap(),
            default_document.as_bytes()
        );
    }
}
