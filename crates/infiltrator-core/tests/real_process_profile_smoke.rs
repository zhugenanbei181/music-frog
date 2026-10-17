//! Real-process smoke for the typed Mihomo configuration transaction adapter.
//!
//! This test is deliberately ignored in ordinary nextest runs. The smoke
//! harness starts an exact SHA-locked Mihomo binary and supplies its active
//! profile path and controller credentials. The smoke drives
//! `MihomoProfileConfigurationTransactionAdapter` directly, including typed
//! DNS settings, live reload, exact workspace readback, and terminal outcomes.
//! It does not assert product capability matrix or composition wiring.

use async_trait::async_trait;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReferenceCasOutcome, ActiveProfileReferenceState, ActiveProfileReferenceTarget,
    ActiveProfileReferenceWriteAuthorization,
};
use infiltrator_contract::dns::DnsSettingsPatch;
use infiltrator_contract::dns_query::{DnsQueryRequest, DnsRecordType};
use infiltrator_contract::profile_document_identity::ProfileId;
use infiltrator_core::apply::{ConfigReloader, EndpointConfigReloader};
use infiltrator_core::profile_configuration_transaction::MihomoProfileConfigurationTransactionAdapter;
use infiltrator_domain::apply::ApplyStrategy;
use infiltrator_domain::rules::RuleEntry;
use infiltrator_ports::core_lifecycle::CoreLifecyclePort;
use infiltrator_ports::dns_query::DnsQueryPort;
use infiltrator_ports::endpoint::EndpointSource;
use infiltrator_ports::profile_configuration_transaction::{
    ProfileConfigurationCandidateValidatorPort, ProfileConfigurationCommitEffect,
    ProfileConfigurationTransactionOutcome, ProfileConfigurationTransactionPort,
    ProfileConfigurationTransactionRequest, ProfileConfigurationTransactionScope,
    UnavailableProfileConfigurationCandidateValidator,
};
use infiltrator_ports::profile_document_codec::ProfileConfigurationCodecPort;
use infiltrator_ports::profile_document_codec::{
    ProfileConfigurationQuery, ProfileConfigurationReadout, ProfileConfigurationUpdate,
};
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::profile_workspace::{
    ProfileWorkspace, ProfileWorkspacePurpose, ProfileWorkspaceUpdate,
};
use infiltrator_ports::secure_store::SecureStore;
use lurker_core_contract::capability::Capability;
use lurker_core_contract::identity::{CoreIdentity, CoreInstanceId, CoreVersion, EngineId};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::session::SessionToken;
use lurker_core_ports::error::PortError;
use mihomo_api::client::MihomoClient;
use mihomo_config::endpoint::ProfileEndpointSource;
use mihomo_config::manager::ConfigManager;
use mihomo_config::profile_document_codec::MihomoProfileDocumentCodec;
use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;
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

fn native_check_candidate(
    binary: &str,
    data_dir: &Path,
    filename: &str,
    document_bytes: &[u8],
    controller_secret: &str,
) {
    use std::fs;

    let candidate_path = data_dir.join(filename);
    fs::write(&candidate_path, document_bytes)
        .expect("write isolated profile candidate for native validation");
    let native_check = Command::new(binary)
        .arg("-t")
        .arg("-d")
        .arg(data_dir)
        .arg("-f")
        .arg(&candidate_path)
        .output()
        .expect("run the exact locked Mihomo native config check");
    let _ = fs::remove_file(&candidate_path);
    let native_stdout = String::from_utf8_lossy(&native_check.stdout);
    let native_stderr = String::from_utf8_lossy(&native_check.stderr);
    assert!(
        native_check.status.success(),
        "Mihomo native check rejected typed candidate ({}): stdout={} stderr={}",
        native_check.status,
        native_stdout.replace(controller_secret, "[redacted]"),
        native_stderr.replace(controller_secret, "[redacted]")
    );
}

fn require_committed(
    context: &str,
    outcome: ProfileConfigurationTransactionOutcome,
) -> ProfileWorkspace {
    match outcome {
        ProfileConfigurationTransactionOutcome::Committed {
            workspace,
            effect: ProfileConfigurationCommitEffect::AppliedToRunningCore,
        } => workspace,
        ProfileConfigurationTransactionOutcome::Committed { effect, .. } => {
            panic!("{context}: expected a live apply, got {effect:?}")
        }
        ProfileConfigurationTransactionOutcome::RolledBack { cause, .. } => {
            panic!("{context}: Mihomo transaction rolled back: {cause:?}")
        }
        ProfileConfigurationTransactionOutcome::RollbackFailed { cause, rollback } => {
            panic!(
                "{context}: Mihomo transaction rollback failed: cause={cause:?}, rollback={rollback:?}"
            )
        }
    }
}

#[tokio::test]
#[ignore = "requires the explicit SHA-locked Mihomo real-process smoke"]
async fn profile_dns_rules_apply_readback_and_restore_use_the_live_mihomo_transaction() {
    let controller_url = required_environment("MIHOMO_REAL_CONTROLLER_URL");
    let controller_secret = required_environment("MIHOMO_REAL_CONTROLLER_SECRET");
    let expected_version = required_environment("MIHOMO_REAL_VERSION");
    let mihomo_binary = required_environment("MIHOMO_REAL_BINARY");
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

    let expected = ProfileStore::read_active_profile_reference(config.engine_profile_store())
        .await
        .expect("read selection before profile CAS");
    let target = ActiveProfileReferenceTarget::selected(
        ProfileStore::engine_id(config.engine_profile_store()),
        ProfileId::new(profile_name).expect("valid smoke profile ID"),
    );
    match ProfileStore::compare_and_set_active_profile_reference(
        config.engine_profile_store(),
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
    let codec: Arc<dyn ProfileConfigurationCodecPort> = Arc::new(MihomoProfileDocumentCodec);
    let stopped_validator: Arc<dyn ProfileConfigurationCandidateValidatorPort> =
        Arc::new(UnavailableProfileConfigurationCandidateValidator::new(
            ProfileConfigurationTransactionScope::new(codec.scope()),
            "the real-process smoke exercises the running-controller validation path",
        ));
    let transaction = MihomoProfileConfigurationTransactionAdapter::new(
        Arc::clone(&codec),
        stopped_validator,
        session,
        config.clone(),
        Arc::clone(&reloader),
        Arc::new(Mutex::new(())),
    )
    .expect("compose the Mihomo codec and transaction owner for the same engine");

    let before = ProfileStore::load_active_workspace(config.engine_profile_store())
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
    assert_eq!(initial_config.log_level, "debug");
    let ProfileConfigurationReadout::Dns(original_dns) = codec
        .read_configuration(
            &before.document.identity,
            &before.document.raw_bytes,
            ProfileConfigurationQuery::Dns,
        )
        .expect("read original typed Mihomo DNS profile config")
    else {
        panic!("Mihomo DNS query returned a non-DNS typed readout")
    };
    let original_runtime_dns_nameservers = original_dns
        .nameserver
        .expect("smoke profile has configured nameservers");
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

    let changed_text = replace_one(&before_text, "log-level: debug", "log-level: error");
    let changed_bytes = changed_text.as_bytes().to_vec();
    let expected_reference =
        match ProfileStore::read_active_profile_reference(config.engine_profile_store())
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
    let changed = require_committed("log-level change", changed);
    assert_eq!(changed.document.utf8_text().unwrap(), changed_text);
    assert_eq!(changed.sidecar_bytes, before.sidecar_bytes);
    assert_eq!(
        ProfileStore::load_active_workspace(config.engine_profile_store())
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
    assert_eq!(applied_config.log_level, "error");

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
    let restored = require_committed("log-level restore", restored);
    assert_eq!(restored.document.raw_bytes, before.document.raw_bytes);
    assert_eq!(restored.sidecar_bytes, before.sidecar_bytes);
    assert_eq!(
        ProfileStore::load_active_workspace(config.engine_profile_store())
            .await
            .expect("read restored workspace"),
        restored
    );
    let restored_config = client
        .get_config()
        .await
        .expect("read controller config after fixture restore");
    assert_eq!(restored_config.mode, "rule");
    assert_eq!(restored_config.log_level, "debug");

    let dns_before = ProfileStore::load_active_workspace(config.engine_profile_store())
        .await
        .expect("read the original workspace before typed DNS apply");
    let dns_reference =
        match ProfileStore::read_active_profile_reference(config.engine_profile_store())
            .await
            .expect("read active selection before DNS transaction")
        {
            ActiveProfileReferenceState::Selected(reference) => reference,
            ActiveProfileReferenceState::Unselected { .. } => {
                panic!("real-process DNS smoke requires a selected profile")
            }
        };
    let updated_nameservers = vec![required_environment("MIHOMO_REAL_DNS_ALT_SERVER")];
    let dns_mutation = codec
        .apply_configuration(
            &dns_before.document.identity,
            &dns_before.document.raw_bytes,
            ProfileConfigurationUpdate::DnsSettings(DnsSettingsPatch {
                nameserver: Some(updated_nameservers.clone()),
                ..Default::default()
            }),
        )
        .expect("apply the shared typed DnsSettingsPatch through Mihomo codec");
    let ProfileConfigurationReadout::Dns(dns_codec_readback) = dns_mutation.readback else {
        panic!("Mihomo DNS patch returned a non-DNS typed readback")
    };
    assert_eq!(
        dns_codec_readback.nameserver.as_deref(),
        Some(updated_nameservers.as_slice())
    );
    native_check_candidate(
        &mihomo_binary,
        config.config_dir(),
        "core-smoke-dns-candidate.yaml",
        &dns_mutation.document_bytes,
        &controller_secret,
    );
    let dns_applied = transaction
        .transact(ProfileConfigurationTransactionRequest {
            expected_reference: dns_reference.clone(),
            expected: dns_before.identity(),
            update: workspace_update(&dns_before, dns_mutation.document_bytes),
            authorization: ActiveProfileReferenceWriteAuthorization::Unowned,
            strategy: ApplyStrategy::PreferReload,
        })
        .await
        .expect("apply typed DNS profile update through the live Mihomo transaction");
    let dns_applied = require_committed("typed DNS apply", dns_applied);
    assert_eq!(
        ProfileStore::load_active_workspace(config.engine_profile_store())
            .await
            .expect("read back the DNS-updated workspace"),
        dns_applied
    );
    let dns_runtime_readback = DnsQueryPort::query(
        &client,
        &DnsQueryRequest {
            name: "core-smoke-dns-updated.test".to_owned(),
            record_type: DnsRecordType::A,
        },
    )
    .await
    .expect("query through the updated live Mihomo DNS nameserver");
    assert!(dns_runtime_readback.answers.iter().any(|answer| {
        answer.record_type == DnsRecordType::A.number() && answer.data == "192.0.2.53"
    }));

    let dns_restored = transaction
        .transact(ProfileConfigurationTransactionRequest {
            expected_reference: dns_reference,
            expected: dns_applied.identity(),
            update: workspace_update(&dns_applied, dns_before.document.raw_bytes.clone()),
            authorization: ActiveProfileReferenceWriteAuthorization::Unowned,
            strategy: ApplyStrategy::PreferReload,
        })
        .await
        .expect("restore exact original workspace after DNS conformance");
    let dns_restored = require_committed("typed DNS restore", dns_restored);
    assert_eq!(
        dns_restored.document.raw_bytes,
        dns_before.document.raw_bytes
    );
    assert_eq!(
        ProfileStore::load_active_workspace(config.engine_profile_store())
            .await
            .expect("read restored original DNS workspace"),
        dns_restored
    );
    let restored_dns = DnsQueryPort::query(
        &client,
        &DnsQueryRequest {
            name: "core-smoke-dns-restored.test".to_owned(),
            record_type: DnsRecordType::A,
        },
    )
    .await
    .expect("query through the restored live Mihomo DNS nameserver");
    assert!(!original_runtime_dns_nameservers.is_empty());
    assert!(restored_dns.answers.iter().any(|answer| {
        answer.record_type == DnsRecordType::A.number() && answer.data == "192.0.2.42"
    }));

    let rules_before = ProfileStore::load_active_workspace(config.engine_profile_store())
        .await
        .expect("read the original workspace before typed rule apply");
    let rules_reference =
        match ProfileStore::read_active_profile_reference(config.engine_profile_store())
            .await
            .expect("read active selection before rule transaction")
        {
            ActiveProfileReferenceState::Selected(reference) => reference,
            ActiveProfileReferenceState::Unselected { .. } => {
                panic!("real-process rule smoke requires a selected profile")
            }
        };
    let ProfileConfigurationReadout::Rules(original_rules) = codec
        .read_configuration(
            &rules_before.document.identity,
            &rules_before.document.raw_bytes,
            ProfileConfigurationQuery::Rules,
        )
        .expect("read original typed Mihomo rules")
    else {
        panic!("Mihomo rules query returned a non-rules typed readout")
    };
    let new_rule = "DOMAIN,core-smoke-rule-apply.test,REJECT";
    let mut updated_rules = original_rules.clone();
    updated_rules.push(RuleEntry {
        rule: new_rule.to_owned(),
        enabled: true,
    });
    let rules_mutation = codec
        .apply_configuration(
            &rules_before.document.identity,
            &rules_before.document.raw_bytes,
            ProfileConfigurationUpdate::Rules(updated_rules),
        )
        .expect("apply typed Mihomo rules update through profile codec");
    let ProfileConfigurationReadout::Rules(rules_codec_readback) = rules_mutation.readback else {
        panic!("Mihomo rules patch returned a non-rules typed readback")
    };
    assert!(
        rules_codec_readback
            .iter()
            .any(|entry| entry.rule == new_rule)
    );
    native_check_candidate(
        &mihomo_binary,
        config.config_dir(),
        "core-smoke-rules-candidate.yaml",
        &rules_mutation.document_bytes,
        &controller_secret,
    );
    let rules_applied = transaction
        .transact(ProfileConfigurationTransactionRequest {
            expected_reference: rules_reference.clone(),
            expected: rules_before.identity(),
            update: workspace_update(&rules_before, rules_mutation.document_bytes),
            authorization: ActiveProfileReferenceWriteAuthorization::Unowned,
            strategy: ApplyStrategy::PreferReload,
        })
        .await
        .expect("apply typed rule profile update through the live Mihomo transaction");
    let rules_applied = require_committed("typed rules apply", rules_applied);
    assert_eq!(
        ProfileStore::load_active_workspace(config.engine_profile_store())
            .await
            .expect("read back the rule-updated workspace"),
        rules_applied
    );
    let live_rules = client
        .get_rules()
        .await
        .expect("read live Mihomo rules after ApplyConfiguration");
    assert!(live_rules.iter().any(|rule| {
        rule.rule_type == "Domain"
            && rule.payload == "core-smoke-rule-apply.test"
            && rule.proxy == "REJECT"
    }));

    let rules_restored = transaction
        .transact(ProfileConfigurationTransactionRequest {
            expected_reference: rules_reference,
            expected: rules_applied.identity(),
            update: workspace_update(&rules_applied, rules_before.document.raw_bytes.clone()),
            authorization: ActiveProfileReferenceWriteAuthorization::Unowned,
            strategy: ApplyStrategy::PreferReload,
        })
        .await
        .expect("restore exact original workspace after rule conformance");
    let rules_restored = require_committed("typed rules restore", rules_restored);
    assert_eq!(
        rules_restored.document.raw_bytes,
        rules_before.document.raw_bytes
    );
    assert_eq!(
        ProfileStore::load_active_workspace(config.engine_profile_store())
            .await
            .expect("read restored original rule workspace"),
        rules_restored
    );
    let restored_rules = client
        .get_rules()
        .await
        .expect("read Mihomo rules after rule restore");
    assert!(!restored_rules.iter().any(|rule| {
        rule.rule_type == "Domain" && rule.payload == "core-smoke-rule-apply.test"
    }));
}
