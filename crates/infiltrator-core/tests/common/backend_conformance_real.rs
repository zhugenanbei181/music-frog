use super::backend_conformance::{
    BackendConformanceCase, BackendConformancePorts, DetailsBehavior,
    NativeProfileFixtureApplyOutcome, NativeProfileFixturePort, ProbeBehavior, RoutingBehavior,
    run_backend_conformance,
};
use async_trait::async_trait;
use infiltrator_contract::core_capabilities::{AdapterRevision, EngineBinaryTarget};
use infiltrator_contract::core_identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
};
use infiltrator_contract::outbound::identity::OutboundId;
use infiltrator_contract::profile_document::{DocumentDiagnosticSnapshot, DocumentValidation};
use infiltrator_contract::routing_intent::{
    EngineDefinedRoutingIntent, RoutingIntent, RoutingIntentExtensionId,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::outbound::{
    OutboundPort, OutboundProbeRequest, OutboundScopeSource, ProbeTarget,
};
use infiltrator_ports::profile_document_codec::ProfileDocumentCodecPort;
use infiltrator_ports::routing_intent::RoutingIntentPort;
use mihomo_api::client::MihomoClient;
use mihomo_api::outbound::MihomoOutboundAdapter;
use mihomo_api::readiness::ControllerReadiness;
use mihomo_api::routing_intent::MihomoRoutingIntentAdapter;
use mihomo_config::profile_document_codec::MihomoProfileDocumentCodec;
use mihomo_platform::desktop::ProcessCoreController;
use mihomo_platform::paths::{clear_home_dir_override, set_home_dir_override};
use singbox_adapter::clash_api::SingBoxClashApiClient;
use singbox_adapter::clash_runtime::{SingBoxOutboundAdapter, SingBoxRoutingIntentAdapter};
use singbox_adapter::config::SingBoxProfileConfigurationAdapter;
use singbox_adapter::http::SingBoxControllerAuthorization;
use singbox_adapter::process::SingBoxProcess;
use singbox_adapter::profile_document_codec::SingBoxProfileDocumentCodec;
use singbox_adapter::readiness::SingBoxControllerReadiness;
use std::env;
use std::net::{SocketAddr, TcpListener};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener as TokioTcpListener;
use tokio::process::Command;
use tokio::task::JoinHandle;
use tokio::time::timeout;

const MIHOMO_LOCKED_VERSION: &str = "v1.19.32";
const SING_BOX_LOCKED_VERSION: &str = "v1.14.3";
static MIHOMO_NATIVE_VALIDATION_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct FixedScope(CoreScope);

impl OutboundScopeSource for FixedScope {
    fn current_scope(&self) -> Option<CoreScope> {
        Some(self.0.clone())
    }
}

/// Mihomo currently has a syntax codec and process adapter, but no typed
/// validate/apply/readback port. The real conformance harness performs the
/// locked binary's `-t` check and exact file readback without advertising a
/// production configuration capability.
struct MihomoNativeProfileFixture {
    engine: EngineId,
    binary: PathBuf,
    config: PathBuf,
}

#[async_trait]
impl NativeProfileFixturePort for MihomoNativeProfileFixture {
    fn engine(&self) -> EngineId {
        self.engine.clone()
    }

    async fn validate_native(&self, raw_bytes: &[u8]) -> Result<DocumentValidation, String> {
        let directory = self
            .config
            .parent()
            .ok_or_else(|| "Mihomo profile path has no parent directory".to_owned())?;
        let file_name = self
            .config
            .file_name()
            .map(|name| name.to_string_lossy())
            .unwrap_or_else(|| "profile.yaml".into());
        let sequence = MIHOMO_NATIVE_VALIDATION_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let candidate = directory.join(format!(
            ".{file_name}.native-check-{}-{sequence}.yaml",
            std::process::id()
        ));
        tokio::fs::write(&candidate, raw_bytes)
            .await
            .map_err(|error| format!("write isolated Mihomo validation candidate: {error}"))?;
        let check_result = Command::new(&self.binary)
            .arg("-t")
            .arg("-d")
            .arg(directory)
            .arg("-f")
            .arg(&candidate)
            .output()
            .await;
        let cleanup_result = tokio::fs::remove_file(&candidate).await;
        let check =
            check_result.map_err(|error| format!("run native Mihomo config check: {error}"))?;
        cleanup_result
            .map_err(|error| format!("remove isolated Mihomo validation candidate: {error}"))?;
        if check.status.success() {
            Ok(DocumentValidation::Valid)
        } else {
            Ok(DocumentValidation::Invalid {
                diagnostics: vec![DocumentDiagnosticSnapshot {
                    line: None,
                    column: None,
                    message: String::from_utf8_lossy(&check.stderr).into_owned(),
                }],
            })
        }
    }

    async fn apply_and_readback(
        &self,
        raw_bytes: &[u8],
    ) -> Result<NativeProfileFixtureApplyOutcome, String> {
        match self.validate_native(raw_bytes).await? {
            DocumentValidation::Valid => {}
            DocumentValidation::Invalid { diagnostics } => {
                return Ok(NativeProfileFixtureApplyOutcome::Rejected {
                    reason: format!(
                        "native Mihomo config check rejected the fixture: {diagnostics:?}"
                    ),
                });
            }
        }
        tokio::fs::write(&self.config, raw_bytes)
            .await
            .map_err(|error| format!("write isolated Mihomo profile: {error}"))?;
        Ok(NativeProfileFixtureApplyOutcome::Applied {
            readback: self.readback().await?,
        })
    }

    async fn readback(&self) -> Result<Vec<u8>, String> {
        tokio::fs::read(&self.config)
            .await
            .map_err(|error| format!("read back isolated Mihomo profile: {error}"))
    }
}

struct SingBoxNativeProfileFixture {
    configuration: SingBoxProfileConfigurationAdapter,
}

#[async_trait]
impl NativeProfileFixturePort for SingBoxNativeProfileFixture {
    fn engine(&self) -> EngineId {
        singbox_adapter::engine_id()
    }

    async fn validate_native(&self, raw_bytes: &[u8]) -> Result<DocumentValidation, String> {
        self.configuration
            .validate_native(raw_bytes)
            .await
            .map_err(|error| error.to_string())
    }

    async fn apply_and_readback(
        &self,
        raw_bytes: &[u8],
    ) -> Result<NativeProfileFixtureApplyOutcome, String> {
        match self.configuration.apply_and_readback(raw_bytes).await {
            Ok(_) => Ok(NativeProfileFixtureApplyOutcome::Applied {
                readback: self
                    .configuration
                    .readback()
                    .await
                    .map_err(|error| error.to_string())?,
            }),
            Err(error)
                if error.error_code() == infiltrator_contract::error::ErrorCode::Configuration =>
            {
                Ok(NativeProfileFixtureApplyOutcome::Rejected {
                    reason: error.to_string(),
                })
            }
            Err(error) => Err(error.to_string()),
        }
    }

    async fn readback(&self) -> Result<Vec<u8>, String> {
        self.configuration
            .readback()
            .await
            .map_err(|error| error.to_string())
    }
}

struct ProbeProxyFixture {
    address: SocketAddr,
    task: JoinHandle<Result<(), String>>,
}

impl ProbeProxyFixture {
    async fn start() -> Result<Self, String> {
        let listener = TokioTcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|error| format!("bind local Mihomo probe proxy: {error}"))?;
        let address = listener
            .local_addr()
            .map_err(|error| format!("read local probe proxy address: {error}"))?;
        let task = tokio::spawn(async move {
            let (mut stream, _) = listener
                .accept()
                .await
                .map_err(|error| format!("accept Mihomo probe connection: {error}"))?;
            let probe_request = read_http_headers(&mut stream).await?;
            let is_head = probe_request.starts_with(b"HEAD http://probe.test/health HTTP/1.1\r\n");
            let is_get = probe_request.starts_with(b"GET http://probe.test/health HTTP/1.1\r\n");
            if !is_head && !is_get {
                return Err(format!(
                    "unexpected Mihomo HTTP proxy request for configured HTTP target: {}",
                    String::from_utf8_lossy(&probe_request)
                ));
            }
            stream
                .write_all(
                    b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .await
                .map_err(|error| format!("answer Mihomo probe request: {error}"))?;
            stream
                .flush()
                .await
                .map_err(|error| format!("flush Mihomo probe response: {error}"))?;
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

async fn read_http_headers(stream: &mut tokio::net::TcpStream) -> Result<Vec<u8>, String> {
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
    let profile_dir = temp.path().join("data");
    tokio::fs::create_dir_all(&profile_dir)
        .await
        .map_err(|error| format!("create Mihomo profile directory: {error}"))?;
    let controller_reservation = TcpListener::bind("127.0.0.1:0")
        .map_err(|error| format!("reserve Mihomo controller port: {error}"))?;
    let controller_address = controller_reservation
        .local_addr()
        .map_err(|error| format!("read Mihomo controller port: {error}"))?;
    drop(controller_reservation);
    let mut probe_proxy = ProbeProxyFixture::start().await?;
    let secret = format!("music-frog-conformance-{}", std::process::id());
    let profile_path = profile_dir.join("profile.yaml");
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
    let process =
        ProcessCoreController::with_pid_file(binary.clone(), profile_path.clone(), pid_path);
    let readiness =
        ControllerReadiness::new(format!("http://{controller_address}"), Some(secret.clone()));
    let client = MihomoClient::new(&format!("http://{controller_address}"), Some(secret))
        .map_err(|error| format!("construct Mihomo controller client: {error}"))?;
    let scope = conformance_scope(mihomo.clone(), "desktop-active", expected_version.clone());
    let scope_source: Arc<dyn OutboundScopeSource> = Arc::new(FixedScope(scope));
    let outbound = MihomoOutboundAdapter::new(Arc::new(client.clone()), scope_source);
    let routing = MihomoRoutingIntentAdapter::new(client);
    let codec = MihomoProfileDocumentCodec;
    let native_profile = MihomoNativeProfileFixture {
        engine: mihomo.clone(),
        binary,
        config: profile_path,
    };
    let collection_id = outbound_id(&mihomo, "CORE-SMOKE-SELECTOR");
    let initial_member_id = outbound_id(&mihomo, "DIRECT");
    let alternate_member_id = outbound_id(&mihomo, "CORE-SMOKE-HTTP");
    let probe_outbound_id = alternate_member_id.clone();
    let case = BackendConformanceCase {
        engine: mihomo,
        expected_version,
        binary_target: EngineBinaryTarget::current(),
        adapter_revision: AdapterRevision::new(1),
        declared_operations: {
            let mut operations = super::backend_conformance_fakes::supported_engine_operations();
            operations.extend(
                super::backend_conformance_fakes::unsupported_profile_transaction_operations(),
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
        routing: RoutingBehavior::Writable {
            initial: RoutingIntent::FollowRules,
            requested: RoutingIntent::RouteAllThroughProxy,
        },
    };

    let _home_guard = mihomo_platform::TEST_LOCK.lock().await;
    if !set_home_dir_override(temp.path().to_path_buf()) {
        probe_proxy.task.abort();
        return Err("could not install isolated Mihomo home override".to_owned());
    }
    let declarations = super::backend_conformance_fakes::declarations_for_engine(&case.engine);
    let conformance_result = run_backend_conformance(
        &case,
        &declarations,
        &BackendConformancePorts {
            process: &process,
            readiness: &readiness,
            codec: &codec,
            native_profile: &native_profile,
            outbound: &outbound,
            routing: &routing,
        },
    )
    .await;
    clear_home_dir_override();
    let proxy_result = probe_proxy.finish().await;
    conformance_result?;
    proxy_result
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
    let engine = singbox_adapter::engine_id();
    let native_profile = SingBoxNativeProfileFixture {
        configuration: SingBoxProfileConfigurationAdapter::with_tokio(
            binary.clone(),
            config_path.clone(),
        ),
    };
    let process = SingBoxProcess::with_tokio(
        binary,
        config_path,
        CoreInstanceId::new("sing-box-conformance")
            .expect("fixed sing-box smoke instance ID is valid"),
        controller_address,
    )
    .map_err(|error| format!("construct sing-box process adapter: {error}"))?;
    let readiness =
        SingBoxControllerReadiness::with_reqwest(&format!("http://{controller_address}"))
            .map_err(|error| format!("construct sing-box readiness adapter: {error}"))?;
    let api = SingBoxClashApiClient::with_reqwest(
        &format!("http://{controller_address}"),
        SingBoxControllerAuthorization::NoSecret,
    )
    .map_err(|error| format!("construct sing-box Clash API client: {error}"))?;
    let scope = conformance_scope(
        engine.clone(),
        "sing-box-conformance",
        expected_version.clone(),
    );
    let scope_source: Arc<dyn OutboundScopeSource> = Arc::new(FixedScope(scope));
    let outbound = SingBoxOutboundAdapter::new(api.clone(), Arc::clone(&scope_source));
    let routing = SingBoxRoutingIntentAdapter::new(api, scope_source);
    let codec = SingBoxProfileDocumentCodec;
    let case = BackendConformanceCase {
        engine: engine.clone(),
        expected_version,
        binary_target: EngineBinaryTarget::current(),
        adapter_revision: AdapterRevision::new(1),
        declared_operations: super::backend_conformance_fakes::unsupported_engine_operations(),
        valid_profile,
        invalid_profile: b"{".to_vec(),
        native_invalid_profile:
            br#"{"outbounds":[{"type":"core-conformance-invalid","tag":"invalid"}]}"#.to_vec(),
        collection_id: outbound_id(&engine, "selector"),
        initial_member_id: outbound_id(&engine, "direct-a"),
        alternate_member_id: outbound_id(&engine, "direct-b"),
        probe_outbound_id: outbound_id(&engine, "direct-a"),
        probe_request: OutboundProbeRequest::new(
            ProbeTarget::http_url("http://probe.test/health")
                .map_err(|error| format!("construct sing-box probe URL: {error}"))?,
            5_000,
        )
        .map_err(|error| format!("construct sing-box probe request: {error}"))?,
        details: DetailsBehavior::Unsupported,
        probe: ProbeBehavior::Unsupported,
        routing: RoutingBehavior::ReadOnlyUnsupported {
            initial: RoutingIntent::EngineDefined(EngineDefinedRoutingIntent::new(
                engine,
                RoutingIntentExtensionId::new("mode.rule")
                    .expect("fixed sing-box mode extension ID is valid"),
            )),
            requested: RoutingIntent::FollowRules,
        },
    };
    let declarations = super::backend_conformance_fakes::declarations_for_engine(&case.engine);
    run_backend_conformance(
        &case,
        &declarations,
        &BackendConformancePorts {
            process: &process,
            readiness: &readiness,
            codec: &codec,
            native_profile: &native_profile,
            outbound: &outbound,
            routing: &routing,
        },
    )
    .await
}
