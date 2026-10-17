//! test-intent: real_process

use async_trait::async_trait;
use infiltrator_application::profile_application::ProfileApplication;
use infiltrator_application::rule_tracer_application::RuleTracerApplication;
use infiltrator_contract::rule_trace_run::{
    RuleTraceOperation, RuleTraceOperationId, RuleTraceOutboundFacts, RuleTraceOwnerBinding,
    RuleTraceOwnerObservation, RuleTraceRequest, ScopedRuleTraceRequest,
};
use infiltrator_contract::rule_tracer::{
    RuleTraceOutboundResolution, RuleTraceRuntimeMode, RuleTracerStatus, TrafficContextSnapshot,
};
use infiltrator_domain::rules::RuleEntry;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::rule_tracer::{RuleOverridePort, RuleWorkspace};
use infiltrator_ports::secure_store::SecureStore;
use lurker_core_contract::error::Failure;
use lurker_core_contract::identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
};
use lurker_core_contract::outbound::inventory::OutboundInventoryState;
use lurker_core_contract::profile_document_identity::ProfileId;
use lurker_core_contract::session::SessionToken;
use lurker_core_ports::core_scope::{CoreScopeObservation, CoreScopeSource};
use lurker_core_ports::error::PortError;
use lurker_core_ports::outbound::OutboundPort;
use mihomo_config::manager::ConfigManager;
use mihomo_config::profile_document_codec::MihomoProfileDocumentCodec;
use mihomo_driver::api::client::MihomoClient;
use mihomo_driver::outbound::MihomoOutboundAdapter;
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::Mutex;

struct FixedScope(CoreScope);

impl CoreScopeSource for FixedScope {
    fn scope_observation(&self) -> CoreScopeObservation {
        CoreScopeObservation::Observed(self.0.clone())
    }
}

#[derive(Default)]
struct SmokeSecureStore(Mutex<HashMap<(String, String), String>>);

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

struct ProfileRuleSource {
    profiles: ProfileApplication,
    profile_id: ProfileId,
}

#[async_trait]
impl RuleOverridePort for ProfileRuleSource {
    async fn load_rule_workspace(&self) -> Result<RuleWorkspace, PortError> {
        let workspace = self
            .profiles
            .load_workspace(&self.profile_id)
            .await
            .map_err(PortError::Rejected)?;
        self.profiles
            .read_profile_rule_workspace(&workspace)
            .map_err(PortError::Rejected)
    }

    async fn compare_and_apply_rules(
        &self,
        _expected: &RuleWorkspace,
        _entries: &[RuleEntry],
    ) -> Result<(), PortError> {
        Err(PortError::Rejected(Failure::unsupported(
            "the read-only rule-trace smoke does not compose rule override",
        )))
    }
}

fn required_environment(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("{name} must be set by the explicit Mihomo smoke"))
}

#[tokio::test]
#[ignore = "requires the explicit SHA-locked Mihomo real-process smoke"]
async fn rule_trace_uses_the_live_profile_workspace_and_mihomo_outbound_observation() {
    let controller_url = required_environment("MIHOMO_REAL_CONTROLLER_URL");
    let controller_secret = required_environment("MIHOMO_REAL_CONTROLLER_SECRET");
    let expected_version = required_environment("MIHOMO_REAL_VERSION");
    let configs_root = PathBuf::from(required_environment("MIHOMO_REAL_CONFIGS_ROOT"));
    let profile_path = PathBuf::from(required_environment("MIHOMO_REAL_PROFILE_PATH"));
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
        .expect("construct Mihomo profile store around the isolated live profile"),
    );
    let profile_id = ProfileId::new("profile").expect("fixture profile ID");
    assert_eq!(
        fs::canonicalize(
            config
                .existing_profile_document_path(profile_id.as_str())
                .await
                .expect("isolated profile path")
        )
        .await
        .expect("canonical stored profile path"),
        fs::canonicalize(profile_path)
            .await
            .expect("canonical profile used by live Mihomo"),
        "the traced profile is the exact YAML loaded by the live process"
    );

    let store: Arc<dyn ProfileStore> = config.shared_engine_profile_store();
    let profiles =
        ProfileApplication::new(store).with_document_codec(Arc::new(MihomoProfileDocumentCodec));
    let workspace = profiles
        .load_workspace(&profile_id)
        .await
        .expect("load the exact live Mihomo profile workspace");
    let rule_workspace = profiles
        .read_profile_rule_workspace(&workspace)
        .expect("decode typed rules from the live Mihomo profile");

    let client = MihomoClient::new(&controller_url, Some(controller_secret))
        .expect("construct production Mihomo controller client");
    let version = client
        .get_version()
        .await
        .expect("read live Mihomo version");
    assert_eq!(version.version, expected_version);
    let scope = CoreScope {
        identity: CoreIdentity::new(
            EngineId::new("mihomo").expect("Mihomo engine ID"),
            CoreInstanceId::new("core-008-rule-trace-real-process").expect("instance ID"),
            Some(CoreVersion::new(expected_version).expect("locked Mihomo version")),
        ),
        generation: CoreGeneration::new(1),
        session_token: Some(SessionToken::new(1)),
    };
    let outbound =
        MihomoOutboundAdapter::new(Arc::new(client), Arc::new(FixedScope(scope.clone())));
    let observation = OutboundPort::observation(&outbound, scope.clone())
        .await
        .expect("read one scoped live Mihomo outbound observation");
    assert_eq!(observation.inventory().scope(), &scope);
    assert_eq!(observation.details().scope(), &scope);
    assert!(matches!(
        observation.inventory().inventory(),
        OutboundInventoryState::Ready { .. }
    ));

    let observed_workspace = workspace.identity();
    let owner = RuleTraceOwnerBinding::new(scope.clone(), observed_workspace.clone())
        .expect("profile and runtime have the same engine identity");
    let query = "core-smoke-active.test".to_owned();
    let request = ScopedRuleTraceRequest::new(
        owner,
        RuleTraceRequest {
            query: query.clone(),
            context: TrafficContextSnapshot {
                domain: Some(query),
                network: Some("tcp".to_owned()),
                ..Default::default()
            },
        },
    )
    .expect("typed rule trace request is valid");
    let application = RuleTracerApplication::new();
    application.set_override_port(Arc::new(ProfileRuleSource {
        profiles,
        profile_id,
    }));
    application
        .simulate_scoped(
            &scope,
            &observed_workspace,
            RuleTraceOperationId(1),
            request,
            &RuleTraceOutboundFacts::Observed(Box::new(observation)),
        )
        .await
        .expect("simulate the current profile against live Mihomo outbounds");

    let execution = application.execution();
    assert_eq!(execution.operation, RuleTraceOperation::Completed);
    let RuleTraceOwnerObservation::Observed(owner) = execution.owner else {
        panic!("completed trace retains its exact Core/profile owner binding")
    };
    assert_eq!(owner.scope, scope);
    assert_eq!(owner.workspace, observed_workspace);
    let report = execution.report.expect("completed trace has a report");
    assert_eq!(report.status, RuleTracerStatus::Ready);
    assert_eq!(report.runtime_mode, RuleTraceRuntimeMode::Live);
    let chain = report
        .decision_chain
        .expect("live profile query matches its explicit DOMAIN rule");
    assert_eq!(chain.matched_rule_type, "DOMAIN");
    assert_eq!(chain.target_proxy, "CORE-SMOKE-HTTP");
    assert!(matches!(
        report.outbound_resolution,
        RuleTraceOutboundResolution::Resolved { ref outbound_id, .. }
            if outbound_id.value() == "CORE-SMOKE-HTTP"
    ));
    assert_eq!(rule_workspace.profile_identity, observed_workspace);
}
