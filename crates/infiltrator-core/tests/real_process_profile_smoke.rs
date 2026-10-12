//! Real-process smoke for the typed Mihomo configuration transaction adapter.
//!
//! This test is deliberately ignored in ordinary nextest runs. The smoke
//! harness starts an exact SHA-locked Mihomo binary and supplies its active
//! profile path and controller credentials. The smoke drives
//! `MihomoProfileConfigurationTransactionAdapter` directly, including live
//! reload, exact workspace readback, and typed terminal outcomes. It does not
//! assert the product capability matrix or composition wiring.

use async_trait::async_trait;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReferenceCasOutcome, ActiveProfileReferenceState, ActiveProfileReferenceTarget,
    ActiveProfileReferenceWriteAuthorization,
};
use infiltrator_contract::capability::Capability;
use infiltrator_contract::core_identity::{CoreIdentity, CoreInstanceId, CoreVersion, EngineId};
use infiltrator_contract::profile_document_identity::ProfileId;
use infiltrator_contract::session::SessionToken;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_core::apply::{ConfigReloader, EndpointConfigReloader};
use infiltrator_core::profile_configuration_transaction::MihomoProfileConfigurationTransactionAdapter;
use infiltrator_domain::apply::ApplyStrategy;
use infiltrator_ports::core_lifecycle::CoreLifecyclePort;
use infiltrator_ports::endpoint::EndpointSource;
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_configuration_transaction::{
    ProfileConfigurationCandidateValidatorPort, ProfileConfigurationCommitEffect,
    ProfileConfigurationTransactionOutcome, ProfileConfigurationTransactionPort,
    ProfileConfigurationTransactionRequest, ProfileConfigurationTransactionScope,
    UnavailableProfileConfigurationCandidateValidator,
};
use infiltrator_ports::profile_document_codec::ProfileDocumentCodecPort;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::profile_workspace::{
    ProfileWorkspace, ProfileWorkspacePurpose, ProfileWorkspaceUpdate,
};
use infiltrator_ports::secure_store::SecureStore;
use mihomo_api::client::MihomoClient;
use mihomo_config::endpoint::ProfileEndpointSource;
use mihomo_config::manager::ConfigManager;
use mihomo_config::profile_document_codec::MihomoProfileDocumentCodec;
use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::fs;
use tokio::sync::Mutex;
use tokio::time::timeout;

struct SmokeSecureStore(Mutex<HashMap<(String, String), String>>);

impl Default for SmokeSecureStore {
    fn default() -> Self {
        Self(Mutex::new(HashMap::new()))
    }
}

#[async_trait]
impl SecureStore for SmokeSecureStore {
    async fn get(&self, namespace: &str, key: &str) -> Result<Option<String>, PortError> {
        Ok(self
            .0
            .lock()
            .await
            .get(&(namespace.to_owned(), key.to_owned()))
            .cloned())
    }

    async fn set(&self, namespace: &str, key: &str, value: &str) -> Result<(), PortError> {
        self.0
            .lock()
            .await
            .insert((namespace.to_owned(), key.to_owned()), value.to_owned());
        Ok(())
    }

    async fn delete(&self, namespace: &str, key: &str) -> Result<(), PortError> {
        self.0
            .lock()
            .await
            .remove(&(namespace.to_owned(), key.to_owned()));
        Ok(())
    }
}

/// Lifecycle view for the process owned by the outer smoke harness.
/// It permits the existing hot-reload transaction and checks readiness via
/// the live controller; it cannot start, stop, or restart that process.
struct ExternalMihomoSession {
    identity: CoreIdentity,
    client: MihomoClient,
}

#[async_trait]
impl CoreLifecyclePort for ExternalMihomoSession {
    fn core_identity(&self) -> Option<CoreIdentity> {
        Some(self.identity.clone())
    }

    fn lifecycle(&self) -> CoreLifecycle {
        CoreLifecycle::Running
    }

    fn generation(&self) -> u64 {
        1
    }

    fn session_token(&self) -> Option<SessionToken> {
        Some(SessionToken::new(1))
    }

    async fn start(&self) -> Result<u64, PortError> {
        Err(PortError::unsupported(
            Capability::CoreLifecycle,
            "the real-process smoke harness owns the Mihomo child process",
        ))
    }

    async fn stop(&self) -> Result<(), PortError> {
        Err(PortError::unsupported(
            Capability::CoreLifecycle,
            "the real-process smoke harness owns the Mihomo child process",
        ))
    }

    async fn restart(&self) -> Result<u64, PortError> {
        Err(PortError::unsupported(
            Capability::CoreLifecycle,
            "the real-process smoke harness owns the Mihomo child process",
        ))
    }

    async fn wait_for_ready(&self, _generation: u64, budget: Duration) -> Result<(), PortError> {
        let response = timeout(budget, self.client.get_version())
            .await
            .map_err(|_| PortError::Failed("live Mihomo readiness check timed out".into()))?
            .map_err(|error| PortError::Network(error.to_string()))?;
        let expected = self
            .identity
            .version
            .as_ref()
            .expect("the real-process session has an observed version")
            .raw();
        if response.version != expected {
            return Err(PortError::Failed(format!(
                "live Mihomo version changed during profile apply: expected {expected}, got {}",
                response.version
            )));
        }
        Ok(())
    }
}

fn required_environment(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("{name} must be set by the explicit Mihomo smoke"))
}

fn profile_name_and_directory(profile_path: &Path) -> (&str, &Path) {
    let profile_name = profile_path
        .file_stem()
        .and_then(|name| name.to_str())
        .expect("smoke profile path must have a UTF-8 filename");
    let config_directory = profile_path
        .parent()
        .expect("smoke profile path must have a parent directory");
    (profile_name, config_directory)
}

fn replace_one(source: &str, before: &str, after: &str) -> String {
    assert_eq!(
        source.matches(before).count(),
        1,
        "isolated smoke profile must contain exactly one {before:?} field"
    );
    source.replacen(before, after, 1)
}

fn workspace_update(
    workspace: &ProfileWorkspace,
    document_bytes: Vec<u8>,
) -> ProfileWorkspaceUpdate {
    ProfileWorkspaceUpdate {
        purpose: ProfileWorkspacePurpose::Derived,
        document_bytes,
        sidecar_bytes: workspace.sidecar_bytes.clone(),
    }
}

fn require_committed(outcome: ProfileConfigurationTransactionOutcome) -> ProfileWorkspace {
    match outcome {
        ProfileConfigurationTransactionOutcome::Committed {
            workspace,
            effect: ProfileConfigurationCommitEffect::AppliedToRunningCore,
        } => workspace,
        ProfileConfigurationTransactionOutcome::Committed { effect, .. } => {
            panic!("real-process smoke expected a live apply, got {effect:?}")
        }
        ProfileConfigurationTransactionOutcome::RolledBack { cause, .. } => {
            panic!("Mihomo transaction rolled back: {cause:?}")
        }
        ProfileConfigurationTransactionOutcome::RollbackFailed { cause, rollback } => {
            panic!("Mihomo transaction rollback failed: cause={cause:?}, rollback={rollback:?}")
        }
    }
}

#[tokio::test]
#[ignore = "requires the explicit SHA-locked Mihomo real-process smoke"]
async fn profile_apply_readback_and_restore_use_the_live_mihomo_transaction() {
    let controller_url = required_environment("MIHOMO_REAL_CONTROLLER_URL");
    let controller_secret = required_environment("MIHOMO_REAL_CONTROLLER_SECRET");
    let expected_version = required_environment("MIHOMO_REAL_VERSION");
    let configs_root = PathBuf::from(required_environment("MIHOMO_REAL_CONFIGS_ROOT"));
    let profile_path = PathBuf::from(required_environment("MIHOMO_REAL_PROFILE_PATH"));
    let (profile_name, config_directory) = profile_name_and_directory(&profile_path);
    let canonical_profile_path = fs::canonicalize(&profile_path)
        .await
        .expect("the smoke harness profile exists");

    let home = tempfile::tempdir().expect("temporary profile settings directory");
    let configs_root_string = configs_root
        .to_str()
        .expect("smoke configs root must be UTF-8");
    let config = Arc::new(
        ConfigManager::with_home_configs_dir_and_store(
            home.path().to_path_buf(),
            Some(configs_root_string),
            SmokeSecureStore::default(),
        )
        .expect("construct Mihomo profile store around the isolated smoke profile"),
    );
    assert_eq!(
        fs::canonicalize(config.config_dir())
            .await
            .expect("the profile config directory exists"),
        fs::canonicalize(config_directory)
            .await
            .expect("canonical smoke config directory"),
        "the integration must use the profile directory owned by the running Mihomo process"
    );

    let expected = ProfileStore::read_active_profile_reference(config.as_ref())
        .await
        .expect("read selection before profile CAS");
    let target = ActiveProfileReferenceTarget::selected(
        ProfileStore::engine_id(config.as_ref()),
        ProfileId::new(profile_name).expect("valid smoke profile ID"),
    );
    match ProfileStore::compare_and_set_active_profile_reference(
        config.as_ref(),
        expected,
        target,
        ActiveProfileReferenceWriteAuthorization::Unowned,
    )
    .await
    .expect("revisioned profile selection CAS")
    {
        ActiveProfileReferenceCasOutcome::Committed { .. } => {}
        ActiveProfileReferenceCasOutcome::Stale { expected, observed } => {
            panic!("real-process selection unexpectedly stale: {expected:?} -> {observed:?}")
        }
    }
    let endpoint_source: Arc<dyn EndpointSource> =
        Arc::new(ProfileEndpointSource::new(config.clone()));
    let endpoint = endpoint_source
        .resolve()
        .await
        .expect("resolve the controller from the active Mihomo profile");
    assert_eq!(endpoint.url, controller_url);
    let profile_secret = endpoint
        .secret
        .as_deref()
        .expect("the isolated controller profile carries its generated secret");
    assert!(
        profile_secret == controller_secret,
        "the active profile must resolve the secret used by the isolated process"
    );
    let client = MihomoClient::new(&endpoint.url, endpoint.secret.clone())
        .expect("construct the production Mihomo controller client");

    let live_version = client
        .get_version()
        .await
        .expect("read live Mihomo version");
    assert_eq!(live_version.version, expected_version);
    let engine_id = EngineId::new("mihomo").expect("static Mihomo engine ID");
    let core_version = CoreVersion::new(expected_version.clone()).expect("locked Mihomo version");
    let session: Arc<dyn CoreLifecyclePort> = Arc::new(ExternalMihomoSession {
        identity: CoreIdentity::new(
            engine_id,
            CoreInstanceId::new("core-008-profile-real-process").expect("test core instance ID"),
            Some(core_version),
        ),
        client: client.clone(),
    });
    let reloader: Arc<dyn ConfigReloader> = Arc::new(EndpointConfigReloader::new(endpoint_source));
    let codec: Arc<dyn ProfileDocumentCodecPort> = Arc::new(MihomoProfileDocumentCodec);
    let stopped_validator: Arc<dyn ProfileConfigurationCandidateValidatorPort> =
        Arc::new(UnavailableProfileConfigurationCandidateValidator::new(
            ProfileConfigurationTransactionScope::new(codec.scope()),
            "the real-process smoke exercises the running-controller validation path",
        ));
    let transaction = MihomoProfileConfigurationTransactionAdapter::new(
        codec,
        stopped_validator,
        session,
        config.clone(),
        Arc::clone(&reloader),
        Arc::new(Mutex::new(())),
    )
    .expect("compose the Mihomo codec and transaction owner for the same engine");

    let before = ProfileStore::load_active_workspace(config.as_ref())
        .await
        .expect("read the active profile workspace through ProfileStore");
    let before_text = before
        .document
        .utf8_text()
        .expect("isolated Mihomo profile is UTF-8")
        .to_owned();
    let initial_config = client
        .get_config()
        .await
        .expect("read initial controller config");
    assert_eq!(initial_config.mode, "rule");
    assert_eq!(initial_config.log_level, "info");
    assert_eq!(
        fs::canonicalize(
            config
                .existing_profile_document_path(profile_name)
                .await
                .expect("active profile path")
        )
        .await
        .expect("canonical active profile path"),
        canonical_profile_path,
        "the tested workspace must be the file loaded by this Mihomo process"
    );

    let changed_text = replace_one(&before_text, "log-level: info", "log-level: debug");
    let changed_bytes = changed_text.as_bytes().to_vec();
    let expected_reference = match ProfileStore::read_active_profile_reference(config.as_ref())
        .await
        .expect("read exact selected reference before transaction")
    {
        ActiveProfileReferenceState::Selected(reference) => reference,
        ActiveProfileReferenceState::Unselected { .. } => {
            panic!("real-process smoke requires selected active profile")
        }
    };
    let changed = transaction
        .transact(ProfileConfigurationTransactionRequest {
            expected_reference: expected_reference.clone(),
            expected: before.identity(),
            update: workspace_update(&before, changed_bytes),
            authorization: ActiveProfileReferenceWriteAuthorization::Unowned,
            strategy: ApplyStrategy::PreferReload,
        })
        .await
        .expect("pre-write checks accept the active workspace candidate");
    let changed = require_committed(changed);
    assert_eq!(changed.document.utf8_text().unwrap(), changed_text);
    assert_eq!(changed.sidecar_bytes, before.sidecar_bytes);
    assert_eq!(
        ProfileStore::load_active_workspace(config.as_ref())
            .await
            .expect("read persisted changed workspace"),
        changed,
        "the ProfileStore receipt and durable profile bytes must agree"
    );
    let applied_config = client
        .get_config()
        .await
        .expect("read controller config after the Rust profile transaction");
    assert_eq!(applied_config.mode, "rule");
    assert_eq!(applied_config.log_level, "debug");

    let restored = transaction
        .transact(ProfileConfigurationTransactionRequest {
            expected_reference,
            expected: changed.identity(),
            update: workspace_update(&changed, before.document.raw_bytes.clone()),
            authorization: ActiveProfileReferenceWriteAuthorization::Unowned,
            strategy: ApplyStrategy::PreferReload,
        })
        .await
        .expect("pre-write checks accept the original workspace fixture");
    let restored = require_committed(restored);
    assert_eq!(restored.document.raw_bytes, before.document.raw_bytes);
    assert_eq!(restored.sidecar_bytes, before.sidecar_bytes);
    assert_eq!(
        ProfileStore::load_active_workspace(config.as_ref())
            .await
            .expect("read restored workspace"),
        restored
    );
    let restored_config = client
        .get_config()
        .await
        .expect("read controller config after fixture restore");
    assert_eq!(restored_config.mode, "rule");
    assert_eq!(restored_config.log_level, "info");
}
