use super::backend_conformance::profile_support::{
    ConformanceLifecycleSession, ConformanceProfileStore,
};
use super::backend_conformance::{
    BackendConformanceCase, BackendConformancePorts, RunningProfileTransactionBehavior,
    conformance_engine_backend, run_backend_conformance,
};
use super::backend_conformance_declarations;
use infiltrator_composition::mihomo_engine_backend_provider::MihomoEngineBackendProvider;
use infiltrator_composition::singbox_engine_backend_provider::SingBoxEngineBackendProvider;
use infiltrator_contract::core_capabilities::{AdapterRevision, EngineBinaryTarget};
use infiltrator_core::apply::EndpointConfigReloader;
use infiltrator_core::profile_configuration_transaction::{
    MihomoBinaryConfigurationCandidateValidator, MihomoProfileConfigurationTransactionAdapter,
};
use infiltrator_platform::paths::{clear_home_dir_override, set_home_dir_override};
use infiltrator_ports::core_lifecycle::CoreLifecyclePort;
use infiltrator_ports::engine_backend::ProductEngineBackendPorts;
use infiltrator_ports::profile_configuration_transaction::{
    ProfileConfigurationCandidateValidatorPort, ProfileConfigurationTransactionScope,
};
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::secure_store::SecureStore;
use lurker_core_contract::active_profile_reference::{
    ActiveProfileReferenceCasOutcome, ActiveProfileReferenceTarget,
    ActiveProfileReferenceWriteAuthorization,
};
use lurker_core_contract::identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
};
use lurker_core_contract::outbound::identity::OutboundId;
use lurker_core_contract::profile_document_identity::ProfileId;
use lurker_core_contract::routing_intent::{
    EngineDefinedRoutingIntent, RoutingIntent, RoutingIntentExtensionId,
};
use lurker_core_ports::core_process::{CoreProcess, CoreReadiness};
use lurker_core_ports::core_scope::{CoreScopeObservation, CoreScopeSource};
use lurker_core_ports::engine_backend::EngineBackendProvider;
use lurker_core_ports::error::PortError;
use lurker_core_ports::outbound::{OutboundProbeRequest, ProbeTarget};
use lurker_engine_runtime::conformance::case::{DetailsBehavior, ProbeBehavior, RoutingBehavior};
use lurker_engine_runtime::registry::EngineBackendProviderRegistry;
use mihomo_config::endpoint::ProfileEndpointSource;
use mihomo_config::manager::ConfigManager;
use mihomo_driver::api::client::MihomoClient;
use mihomo_driver::driver::ControllerReadiness;
use mihomo_driver::driver::ProcessCoreController;
use singbox_driver::api::controller_configuration::SingBoxControllerConfiguration;
use std::collections::HashMap;
use std::env;
use std::net::{SocketAddr, TcpListener};
use std::path::PathBuf;
use std::process;
use std::str;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tempfile::TempDir;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync;
use tokio::task::JoinHandle;
use tokio::time::{sleep, timeout};

const MIHOMO_LOCKED_VERSION: &str = "v1.19.33";
const SING_BOX_LOCKED_VERSION: &str = "v1.14.3";

struct SmokeSecureStore(Mutex<HashMap<(String, String), String>>);

impl Default for SmokeSecureStore {
    fn default() -> Self {
        Self(Mutex::new(HashMap::new()))
    }
}

#[async_trait::async_trait]
impl SecureStore for SmokeSecureStore {
    async fn get(&self, namespace: &str, key: &str) -> Result<Option<String>, PortError> {
        Ok(self
            .0
            .lock()
            .expect("conformance secure-store lock")
            .get(&(namespace.to_owned(), key.to_owned()))
            .cloned())
    }

    async fn set(&self, namespace: &str, key: &str, value: &str) -> Result<(), PortError> {
        self.0
            .lock()
            .expect("conformance secure-store lock")
            .insert((namespace.to_owned(), key.to_owned()), value.to_owned());
        Ok(())
    }

    async fn delete(&self, namespace: &str, key: &str) -> Result<(), PortError> {
        self.0
            .lock()
            .expect("conformance secure-store lock")
            .remove(&(namespace.to_owned(), key.to_owned()));
        Ok(())
    }
}

struct FixedScope(CoreScope);

impl CoreScopeSource for FixedScope {
    fn scope_observation(&self) -> CoreScopeObservation {
        CoreScopeObservation::Observed(self.0.clone())
    }
}

struct ProbeProxyFixture {
    address: SocketAddr,
    task: JoinHandle<Result<(), String>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProbeRequestMethod {
    Head,
    Get,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InitialProxyRequest {
    ConnectTunnel,
    ForwardedHealth(ProbeRequestMethod),
}

impl ProbeProxyFixture {
    async fn start() -> Result<Self, String> {
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|error| format!("bind local Mihomo probe proxy: {error}"))?;
        let address = listener
            .local_addr()
            .map_err(|error| format!("read local probe proxy address: {error}"))?;
        let task =
            tokio::spawn(async move {
                let (mut stream, _) = listener
                    .accept()
                    .await
                    .map_err(|error| format!("accept Mihomo probe connection: {error}"))?;
                let request = read_http_headers(&mut stream).await?;
                let method = match classify_initial_proxy_request(&request)? {
                    InitialProxyRequest::ConnectTunnel => {
                        stream
                            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
                            .await
                            .map_err(|error| format!("accept Mihomo CONNECT tunnel: {error}"))?;
                        stream
                            .flush()
                            .await
                            .map_err(|error| format!("flush Mihomo CONNECT response: {error}"))?;
                        let tunneled_request = read_http_headers(&mut stream).await?;
                        parse_origin_health_request(&tunneled_request)?
                    }
                    InitialProxyRequest::ForwardedHealth(method) => method,
                };
                sleep(Duration::from_millis(3)).await;
                let response = match method {
                    ProbeRequestMethod::Head => {
                        &b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"[..]
                    }
                    ProbeRequestMethod::Get => {
                        &b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok"[..]
                    }
                };
                stream.write_all(response).await.map_err(|error| {
                    format!("answer Mihomo tunneled HTTP health request: {error}")
                })?;
                stream.flush().await.map_err(|error| {
                    format!("flush Mihomo tunneled HTTP health response: {error}")
                })?;
                Ok(())
            });
        Ok(Self { address, task })
    }

    async fn finish(&mut self) -> Result<(), String> {
        match timeout(Duration::from_secs(2), &mut self.task).await {
            Ok(Ok(result)) => result,
            Ok(Err(error)) => Err(format!("Mihomo probe fixture task failed: {error}")),
            Err(_) => {
                self.task.abort();
                Err("Mihomo probe fixture received no probe request".to_owned())
            }
        }
    }
}

impl Drop for ProbeProxyFixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn request_start_line(request: &[u8]) -> Result<&str, String> {
    let request = str::from_utf8(request)
        .map_err(|error| format!("Mihomo probe request headers are not UTF-8: {error}"))?;
    request
        .lines()
        .next()
        .ok_or_else(|| "Mihomo probe request has no start line".to_owned())
}

fn classify_initial_proxy_request(request: &[u8]) -> Result<InitialProxyRequest, String> {
    match request_start_line(request)? {
        "CONNECT probe.test:80 HTTP/1.1" => Ok(InitialProxyRequest::ConnectTunnel),
        "HEAD http://probe.test/health HTTP/1.1" => Ok(InitialProxyRequest::ForwardedHealth(
            ProbeRequestMethod::Head,
        )),
        "GET http://probe.test/health HTTP/1.1" => Ok(InitialProxyRequest::ForwardedHealth(
            ProbeRequestMethod::Get,
        )),
        start_line => Err(format!(
            "unexpected Mihomo HTTP proxy request for configured HTTP target: {start_line}\n{}",
            String::from_utf8_lossy(request)
        )),
    }
}

fn parse_origin_health_request(request: &[u8]) -> Result<ProbeRequestMethod, String> {
    match request_start_line(request)? {
        "HEAD /health HTTP/1.1" => Ok(ProbeRequestMethod::Head),
        "GET /health HTTP/1.1" => Ok(ProbeRequestMethod::Get),
        start_line => Err(format!(
            "unexpected HTTP request inside Mihomo CONNECT tunnel: {start_line}\n{}",
            String::from_utf8_lossy(request)
        )),
    }
}

async fn read_http_headers(stream: &mut TcpStream) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    loop {
        let mut byte = [0_u8; 1];
        let count = stream
            .read(&mut byte)
            .await
            .map_err(|error| format!("read local HTTP proxy request: {error}"))?;
        if count == 0 {
            return Err("local HTTP proxy connection closed before headers ended".to_owned());
        }
        bytes.push(byte[0]);
        if bytes.ends_with(b"\r\n\r\n") {
            return Ok(bytes);
        }
        if bytes.len() > 16 * 1024 {
            return Err("local HTTP proxy request headers exceeded 16 KiB".to_owned());
        }
    }
}

fn required_environment(name: &str) -> Result<String, String> {
    env::var(name).map_err(|_| format!("{name} must be set by the unified real smoke script"))
}

fn conformance_scope(engine: EngineId, instance_id: &str, version: CoreVersion) -> CoreScope {
    CoreScope {
        identity: CoreIdentity::new(
            engine,
            CoreInstanceId::new(instance_id).expect("fixed smoke instance ID is valid"),
            Some(version),
        ),
        generation: CoreGeneration::new(1),
        session_token: None,
    }
}

fn outbound_id(engine: &EngineId, value: &str) -> OutboundId {
    OutboundId::new(engine.clone(), value).expect("fixed smoke outbound ID is valid")
}

pub async fn locked_real_mihomo_runs_the_shared_core_conformance_suite() -> Result<(), String> {
    let binary = PathBuf::from(required_environment("MIHOMO_REAL_BINARY")?);
    let reported_version = required_environment("MIHOMO_REAL_VERSION")?;
    if reported_version != MIHOMO_LOCKED_VERSION {
        return Err(format!(
            "unified conformance is locked to {MIHOMO_LOCKED_VERSION}, got {reported_version}"
        ));
    }
    let expected_version = CoreVersion::new(reported_version.clone())
        .map_err(|error| format!("invalid Mihomo version lock: {error}"))?;
    let temp =
        TempDir::new().map_err(|error| format!("create Mihomo conformance home: {error}"))?;
    let controller_reservation = TcpListener::bind("127.0.0.1:0")
        .map_err(|error| format!("reserve Mihomo controller port: {error}"))?;
    let controller_address = controller_reservation
        .local_addr()
        .map_err(|error| format!("read Mihomo controller port: {error}"))?;
    drop(controller_reservation);
    let mut probe_proxy = ProbeProxyFixture::start().await?;
    let secret = format!("music-frog-conformance-{}", process::id());
    let pid_path = temp.path().join("mihomo.pid");
    let valid_profile = format!(
        r#"port: 0
socks-port: 0
redir-port: 0
tproxy-port: 0
mixed-port: 0
allow-lan: false
mode: rule
log-level: error
ipv6: false
external-controller: {controller_address}
secret: "{secret}"
tun:
  enable: false
proxies:
  - name: CORE-SMOKE-HTTP
    type: http
    server: 127.0.0.1
    port: {}
proxy-groups:
  - name: CORE-SMOKE-SELECTOR
    type: select
    proxies:
      - DIRECT
      - CORE-SMOKE-HTTP
      - REJECT
rules:
  - MATCH,DIRECT
"#,
        probe_proxy.address.port()
    )
    .into_bytes();
    let mihomo = EngineId::new("mihomo").expect("static engine ID is valid");
    let collection_id = outbound_id(&mihomo, "CORE-SMOKE-SELECTOR");
    let initial_member_id = outbound_id(&mihomo, "DIRECT");
    let alternate_member_id = outbound_id(&mihomo, "CORE-SMOKE-HTTP");
    let probe_outbound_id = alternate_member_id.clone();
    let case = BackendConformanceCase {
        engine: mihomo,
        expected_version,
        binary_target: EngineBinaryTarget::current(),
        adapter_revision: AdapterRevision::new(2),
        declared_operations: {
            let mut operations = backend_conformance_declarations::supported_engine_operations();
            operations.extend(
                backend_conformance_declarations::unsupported_profile_transaction_operations(),
            );
            operations
        },
        valid_profile,
        invalid_profile: b"invalid: [".to_vec(),
        native_invalid_profile: b"proxies:\n  - name: CORE-SMOKE-INVALID\n    type: not-a-mihomo-proxy\n    server: 127.0.0.1\n    port: 1\nrules:\n  - MATCH,DIRECT\n".to_vec(),
        collection_id,
        initial_member_id,
        alternate_member_id,
        probe_outbound_id,
        probe_request: OutboundProbeRequest::new(
            ProbeTarget::http_url("http://probe.test/health")
                .map_err(|error| format!("construct Mihomo probe URL: {error}"))?,
            5_000,
        )
        .map_err(|error| format!("construct Mihomo probe request: {error}"))?,
        details: DetailsBehavior::Ready,
        probe: ProbeBehavior::Measured,
        running_profile_transaction: RunningProfileTransactionBehavior::NotExercised,
        routing: RoutingBehavior::Writable {
            initial: RoutingIntent::FollowRules,
            requested: RoutingIntent::RouteAllThroughProxy,
        },
    };

    let profile_root = temp.path().join("configs");
    let profile_root_string = profile_root
        .to_str()
        .ok_or_else(|| "Mihomo profile root is not UTF-8".to_owned())?;
    let profile_store = Arc::new(
        ConfigManager::with_home_configs_dir_and_store(
            temp.path().join("settings"),
            Some(profile_root_string),
            SmokeSecureStore::default(),
        )
        .map_err(|error| format!("construct Mihomo conformance profile store: {error}"))?,
    );
    let valid_text = String::from_utf8(case.valid_profile.clone())
        .map_err(|error| format!("Mihomo valid fixture is not UTF-8: {error}"))?;
    ProfileStore::save_inactive_profile(
        profile_store.engine_profile_store(),
        "profile",
        &valid_text,
    )
    .await
    .map_err(|error| format!("seed Mihomo profile workspace: {error}"))?;
    let expected =
        ProfileStore::read_active_profile_reference(profile_store.engine_profile_store())
            .await
            .map_err(|error| format!("read Mihomo active profile reference: {error}"))?;
    let target = ActiveProfileReferenceTarget::selected(
        ProfileStore::engine_id(profile_store.engine_profile_store()),
        ProfileId::new("profile").expect("valid conformance profile ID"),
    );
    match ProfileStore::compare_and_set_active_profile_reference(
        profile_store.engine_profile_store(),
        expected,
        target,
        ActiveProfileReferenceWriteAuthorization::Unowned,
    )
    .await
    .map_err(|error| format!("select Mihomo profile workspace: {error}"))?
    {
        ActiveProfileReferenceCasOutcome::Committed { .. } => {}
        ActiveProfileReferenceCasOutcome::Stale { expected, observed } => {
            return Err(format!(
                "conformance profile selection stale: {expected:?} -> {observed:?}"
            ));
        }
    }
    let profile_path = profile_store.config_dir().join("profile.yaml");
    let host_process: Arc<dyn CoreProcess> = Arc::new(ProcessCoreController::with_pid_file(
        binary.clone(),
        profile_path,
        pid_path,
    ));
    let readiness: Arc<dyn CoreReadiness> = Arc::new(ControllerReadiness::new(
        format!("http://{controller_address}"),
        Some(secret.clone()),
    ));
    let client = MihomoClient::new(&format!("http://{controller_address}"), Some(secret))
        .map_err(|error| format!("construct Mihomo controller client: {error}"))?;
    let provider = Arc::new(
        MihomoEngineBackendProvider::new(Arc::clone(&host_process), Arc::clone(&readiness), client)
            .map_err(|error| format!("construct production Mihomo backend provider: {error}"))?,
    );
    let registry_provider: Arc<dyn EngineBackendProvider<ProductEngineBackendPorts>> = provider;
    let registry = EngineBackendProviderRegistry::new([registry_provider])
        .map_err(|error| format!("register production Mihomo backend provider: {error}"))?;
    let backend = registry
        .backend(&case.engine)
        .map_err(|error| format!("create production Mihomo backend ports: {error}"))?;
    let process = backend.process();
    let scope = conformance_scope(
        case.engine.clone(),
        "desktop-active",
        case.expected_version.clone(),
    );
    let scope_source: Arc<dyn CoreScopeSource> = Arc::new(FixedScope(scope));
    backend
        .bind_owner_core_scope(scope_source)
        .map_err(|error| format!("bind Mihomo production Core scope: {error}"))?;
    let transaction_owner = Arc::new(ConformanceLifecycleSession::for_process(
        process.as_ref(),
        case.expected_version.clone(),
    )?);
    let session: Arc<dyn CoreLifecyclePort> = transaction_owner.clone();
    let codec = backend.configuration();
    let transaction_scope = ProfileConfigurationTransactionScope::new(codec.scope());
    let stopped_validator: Arc<dyn ProfileConfigurationCandidateValidatorPort> = Arc::new(
        MihomoBinaryConfigurationCandidateValidator::new(
            transaction_scope,
            binary,
            profile_store.config_dir().to_path_buf(),
        )
        .map_err(|error| format!("construct production Mihomo native validator: {error}"))?,
    );
    let endpoint_source = Arc::new(ProfileEndpointSource::new(Arc::clone(&profile_store)));
    let reloader = Arc::new(EndpointConfigReloader::new(endpoint_source));
    let transaction = Arc::new(
        MihomoProfileConfigurationTransactionAdapter::new(
            codec.clone(),
            stopped_validator,
            session,
            Arc::clone(&profile_store),
            reloader,
            Arc::new(sync::Mutex::new(())),
        )
        .map_err(|error| format!("construct production Mihomo profile transaction: {error}"))?,
    );
    backend
        .configuration_transaction_binding()
        .bind(transaction)
        .map_err(|error| format!("bind production Mihomo profile transaction: {error}"))?;
    let transaction = backend.configuration_transaction();

    let _home_guard = infiltrator_platform::TEST_LOCK.lock().await;
    if !set_home_dir_override(temp.path().to_path_buf()) {
        probe_proxy.task.abort();
        return Err("could not install isolated Mihomo home override".to_owned());
    }
    let declarations = backend_conformance_declarations::declarations_for_engine(&case.engine);
    let conformance_backend = conformance_engine_backend(
        &case.engine,
        backend.process(),
        backend.readiness(),
        backend.configuration(),
        backend.core_operations(),
    )?;
    let conformance_result = run_backend_conformance(
        &case,
        &declarations,
        &BackendConformancePorts {
            backend: &conformance_backend,
            configuration_transaction: transaction.as_ref(),
            profile_store: profile_store.engine_profile_store(),
            transaction_owner: transaction_owner.as_ref(),
        },
    )
    .await;
    clear_home_dir_override();
    let proxy_result = probe_proxy.finish().await;
    match (conformance_result, proxy_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(conformance_error), Ok(())) => Err(conformance_error),
        (Ok(()), Err(proxy_error)) => Err(proxy_error),
        (Err(conformance_error), Err(proxy_error)) => Err(format!(
            "{conformance_error}; probe proxy fixture also failed: {proxy_error}"
        )),
    }
}

pub async fn locked_real_sing_box_runs_the_shared_core_conformance_suite() -> Result<(), String> {
    let binary = PathBuf::from(required_environment("SINGBOX_REAL_BINARY")?);
    let version_without_prefix = required_environment("SINGBOX_REAL_VERSION")?;
    if version_without_prefix != SING_BOX_LOCKED_VERSION.trim_start_matches('v') {
        return Err(format!(
            "unified conformance is locked to {SING_BOX_LOCKED_VERSION}, got {version_without_prefix}"
        ));
    }
    let expected_version = CoreVersion::new(SING_BOX_LOCKED_VERSION)
        .map_err(|error| format!("invalid sing-box version lock: {error}"))?;
    let temp =
        TempDir::new().map_err(|error| format!("create sing-box conformance home: {error}"))?;
    let config_path = temp.path().join("config.json");
    let controller_reservation = TcpListener::bind("127.0.0.1:0")
        .map_err(|error| format!("reserve sing-box controller port: {error}"))?;
    let controller_address = controller_reservation
        .local_addr()
        .map_err(|error| format!("read sing-box controller port: {error}"))?;
    drop(controller_reservation);
    let valid_profile = serde_json::to_vec(&serde_json::json!({
        "log": {"level": "error"},
        "inbounds": [],
        "outbounds": [
            {"type": "direct", "tag": "direct-a"},
            {"type": "direct", "tag": "direct-b"},
            {
                "type": "selector",
                "tag": "selector",
                "outbounds": ["direct-a", "direct-b"],
                "default": "direct-a"
            }
        ],
        "experimental": {
            "clash_api": {
                "external_controller": controller_address.to_string()
            }
        }
    }))
    .map_err(|error| format!("serialize sing-box conformance profile: {error}"))?;
    let engine = EngineId::new("sing-box").expect("static sing-box engine ID is valid");
    let controller = SingBoxControllerConfiguration::from_profile_json(&valid_profile)
        .map_err(|error| format!("sing-box controller preflight rejected the fixture: {error}"))?;
    let case = BackendConformanceCase {
        engine: engine.clone(),
        expected_version,
        binary_target: EngineBinaryTarget::current(),
        adapter_revision: AdapterRevision::new(1),
        declared_operations: backend_conformance_declarations::unsupported_engine_operations(),
        valid_profile,
        invalid_profile: b"{".to_vec(),
        native_invalid_profile:
            br#"{"outbounds":[{"type":"core-conformance-invalid","tag":"invalid"}]}"#.to_vec(),
        collection_id: outbound_id(&engine, "selector"),
        initial_member_id: outbound_id(&engine, "direct-a"),
        alternate_member_id: outbound_id(&engine, "direct-b"),
        probe_outbound_id: outbound_id(&engine, "direct-a"),
        // sing-box measures HTTPS targets only: an http target is silently
        // replaced by the engine's own URL test target, so the case asks for
        // the engine's HTTPS test target and expects a real measurement.
        probe_request: OutboundProbeRequest::new(
            ProbeTarget::http_url("https://www.gstatic.com/generate_204")
                .map_err(|error| format!("construct sing-box probe URL: {error}"))?,
            5_000,
        )
        .map_err(|error| format!("construct sing-box probe request: {error}"))?,
        details: DetailsBehavior::Unsupported,
        probe: ProbeBehavior::Measured,
        running_profile_transaction: RunningProfileTransactionBehavior::Unsupported,
        routing: RoutingBehavior::ReadOnlyUnsupported {
            initial: RoutingIntent::EngineDefined(EngineDefinedRoutingIntent::new(
                engine,
                RoutingIntentExtensionId::new("mode.rule")
                    .expect("fixed sing-box mode extension ID is valid"),
            )),
            requested: RoutingIntent::FollowRules,
        },
    };

    let instance = CoreInstanceId::new("sing-box-conformance")
        .expect("fixed sing-box smoke instance ID is valid");
    let provider = Arc::new(
        SingBoxEngineBackendProvider::with_tokio(
            binary,
            config_path.clone(),
            instance.clone(),
            controller,
        )
        .map_err(|error| format!("construct production sing-box backend provider: {error}"))?,
    );
    let registry_provider: Arc<dyn EngineBackendProvider<ProductEngineBackendPorts>> =
        provider.clone();
    let registry = EngineBackendProviderRegistry::new([registry_provider])
        .map_err(|error| format!("register production sing-box backend provider: {error}"))?;
    let backend = registry
        .backend(&case.engine)
        .map_err(|error| format!("create production sing-box backend ports: {error}"))?;
    let process = backend.process();
    let codec = backend.configuration();
    let profile_store = Arc::new(ConformanceProfileStore::new(
        &codec.scope(),
        &case.valid_profile,
    ));
    let transaction_owner = Arc::new(ConformanceLifecycleSession::for_process(
        process.as_ref(),
        case.expected_version.clone(),
    )?);
    let session: Arc<dyn CoreLifecyclePort> = transaction_owner.clone();
    provider
        .bind_profile_configuration_transaction(
            &backend,
            profile_store.clone() as Arc<dyn ProfileStore>,
            Arc::clone(&session),
        )
        .map_err(|error| format!("bind production sing-box profile transaction: {error}"))?;
    let scope = conformance_scope(
        case.engine.clone(),
        "sing-box-conformance",
        case.expected_version.clone(),
    );
    let scope_source: Arc<dyn CoreScopeSource> = Arc::new(FixedScope(scope));
    provider
        .bind_core_owner_scope(&backend, scope_source)
        .map_err(|error| format!("bind production sing-box Core scope: {error}"))?;
    let transaction = backend.configuration_transaction();
    let declarations = backend_conformance_declarations::declarations_for_engine(&case.engine);
    let conformance_backend = conformance_engine_backend(
        &case.engine,
        backend.process(),
        backend.readiness(),
        backend.configuration(),
        backend.core_operations(),
    )?;
    let conformance_result = run_backend_conformance(
        &case,
        &declarations,
        &BackendConformancePorts {
            backend: &conformance_backend,
            configuration_transaction: transaction.as_ref(),
            profile_store: profile_store.as_ref(),
            transaction_owner: transaction_owner.as_ref(),
        },
    )
    .await;
    let native_readback = fs::read(config_path)
        .await
        .map_err(|error| format!("read sing-box configuration after conformance: {error}"))?;
    if native_readback != case.valid_profile {
        return Err(
            "production sing-box transaction did not preserve exact native JSON bytes".to_owned(),
        );
    }
    conformance_result
}
