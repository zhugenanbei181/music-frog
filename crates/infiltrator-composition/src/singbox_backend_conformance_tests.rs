//! test-intent: behavior

use async_trait::async_trait;
use infiltrator_contract::core_identity::{CoreIdentity, CoreInstanceId, EngineId};
use infiltrator_contract::profile_document::DocumentValidation;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_ports::core_process::{CoreProcess, CoreProcessStartContext, CoreReadiness};
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_document_codec::ProfileDocumentCodecPort;
use serde_json::Value;
use singbox_api::controller_configuration::SingBoxControllerConfiguration;
use singbox_api::http::{
    ReqwestSingBoxClashHttpPort, SingBoxClashHttpPort, SingBoxClashMethod, SingBoxClashResponse,
    SingBoxControllerAuthorization,
};
use singbox_api::readiness::SingBoxControllerReadiness;
use singbox_config::config::SingBoxProfileConfigurationAdapter;
use singbox_config::profile_document_codec::{PROFILE_FORMAT, SingBoxProfileDocumentCodec};
use singbox_process::command::{
    NativeConfigCheck, SingBoxChildPoll, SingBoxChildProcess, SingBoxCommandPort,
};
use singbox_process::process::SingBoxProcess;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use tempfile::TempDir;
use tokio::sync::{Mutex, Semaphore};
use url::Url;

fn controller_configuration(address: &str, secret: Option<&str>) -> SingBoxControllerConfiguration {
    let mut clash_api = serde_json::json!({ "external_controller": address });
    if let Some(secret) = secret {
        clash_api["secret"] = Value::String(secret.to_owned());
    }
    let profile = serde_json::json!({ "experimental": { "clash_api": clash_api } });
    SingBoxControllerConfiguration::from_profile_json(
        &serde_json::to_vec(&profile).expect("serialize controller test profile"),
    )
    .expect("controller test profile is loopback and fixed-port")
}

fn controller_configuration_for_endpoint(
    endpoint: &str,
    secret: Option<&str>,
) -> SingBoxControllerConfiguration {
    let endpoint = Url::parse(endpoint).expect("test controller URL is valid");
    let port = endpoint.port().expect("test controller URL has a port");
    let address = match endpoint.host().expect("test controller URL has a host") {
        url::Host::Ipv4(host) => format!("{host}:{port}"),
        url::Host::Ipv6(host) => format!("[{host}]:{port}"),
        url::Host::Domain(_) => panic!("test controller must use a numeric loopback host"),
    };
    controller_configuration(&address, secret)
}

#[derive(Clone, Copy)]
enum FakeChildPhase {
    Running,
    Exited,
}

struct FakeSingBoxCommandPort {
    validation: NativeConfigCheck,
    checked_documents: Arc<Mutex<Vec<Vec<u8>>>>,
    spawn_count: Arc<AtomicUsize>,
    child_phase: Arc<Mutex<FakeChildPhase>>,
}

impl FakeSingBoxCommandPort {
    fn new(validation: NativeConfigCheck) -> Self {
        Self {
            validation,
            checked_documents: Arc::new(Mutex::new(Vec::new())),
            spawn_count: Arc::new(AtomicUsize::new(0)),
            child_phase: Arc::new(Mutex::new(FakeChildPhase::Running)),
        }
    }
}

#[async_trait]
impl SingBoxCommandPort for FakeSingBoxCommandPort {
    async fn check_config(
        &self,
        _binary_path: &Path,
        config_path: &Path,
    ) -> Result<NativeConfigCheck, PortError> {
        let bytes = tokio::fs::read(config_path)
            .await
            .map_err(|error| PortError::Io(error.to_string()))?;
        self.checked_documents.lock().await.push(bytes);
        Ok(self.validation)
    }

    async fn spawn_core(
        &self,
        _binary_path: &Path,
        _config_path: &Path,
    ) -> Result<Box<dyn SingBoxChildProcess>, PortError> {
        self.spawn_count.fetch_add(1, Ordering::SeqCst);
        *self.child_phase.lock().await = FakeChildPhase::Running;
        Ok(Box::new(FakeSingBoxChildProcess {
            phase: self.child_phase.clone(),
        }))
    }
}

struct FakeSingBoxChildProcess {
    phase: Arc<Mutex<FakeChildPhase>>,
}

#[async_trait]
impl SingBoxChildProcess for FakeSingBoxChildProcess {
    fn pid(&self) -> u32 {
        9001
    }

    async fn poll(&mut self) -> Result<SingBoxChildPoll, PortError> {
        match *self.phase.lock().await {
            FakeChildPhase::Running => Ok(SingBoxChildPoll::Running),
            FakeChildPhase::Exited => Ok(SingBoxChildPoll::Exited),
        }
    }

    async fn terminate(&mut self) -> Result<(), PortError> {
        *self.phase.lock().await = FakeChildPhase::Exited;
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum FakeHttpOutcome {
    Ready,
    Unavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FakeObservedAuthorization {
    NoSecret,
    BearerToken,
}

struct FakeReadinessHttpPort {
    outcome: FakeHttpOutcome,
    requests: StdMutex<
        Vec<(
            String,
            SingBoxClashMethod,
            Option<Vec<u8>>,
            std::time::Duration,
            FakeObservedAuthorization,
        )>,
    >,
}

#[async_trait]
impl SingBoxClashHttpPort for FakeReadinessHttpPort {
    async fn request(
        &self,
        url: &Url,
        method: SingBoxClashMethod,
        authorization: &SingBoxControllerAuthorization,
        body: Option<Vec<u8>>,
        timeout: std::time::Duration,
    ) -> Result<SingBoxClashResponse, PortError> {
        let observed_authorization = match authorization {
            SingBoxControllerAuthorization::NoSecret => FakeObservedAuthorization::NoSecret,
            SingBoxControllerAuthorization::BearerToken(_) => {
                FakeObservedAuthorization::BearerToken
            }
        };
        self.requests
            .lock()
            .expect("fake readiness request list is not poisoned")
            .push((
                url.to_string(),
                method,
                body,
                timeout,
                observed_authorization,
            ));
        match self.outcome {
            FakeHttpOutcome::Ready => Ok(SingBoxClashResponse {
                status: 200,
                body: br#"{"version":"sing-box 1.14.3"}"#.to_vec(),
            }),
            FakeHttpOutcome::Unavailable => Ok(SingBoxClashResponse {
                status: 503,
                body: br#"{"error":"fixture controller is unavailable"}"#.to_vec(),
            }),
        }
    }
}

#[test]
fn sing_box_codec_is_scoped_to_json_and_requires_an_object_root() {
    let codec = SingBoxProfileDocumentCodec;
    let scope = codec.scope();
    assert_eq!(scope.engine_id.as_str(), "sing-box");
    assert_eq!(scope.format_id.as_str(), PROFILE_FORMAT);
    assert_eq!(scope.schema_version.value(), 1);
    assert_eq!(
        codec.inspect(br#"{"log":{"level":"warn"}}"#).unwrap(),
        DocumentValidation::Valid
    );
    match codec.inspect(br#"{"outbounds":[}"#).unwrap() {
        DocumentValidation::Invalid { diagnostics } => {
            assert_eq!(diagnostics.len(), 1);
            assert!(diagnostics[0].line.is_some());
            assert!(diagnostics[0].column.is_some());
        }
        DocumentValidation::Valid => panic!("malformed JSON was accepted"),
    }
    match codec.inspect(b"[]").unwrap() {
        DocumentValidation::Invalid { diagnostics } => {
            assert!(diagnostics[0].line.is_none());
        }
        DocumentValidation::Valid => panic!("non-object config root was accepted"),
    }
}

#[tokio::test]
async fn native_validation_precedes_atomic_config_apply_and_exact_readback() {
    let home = TempDir::new().unwrap();
    let config_path = home.path().join("config.json");
    tokio::fs::write(&config_path, br#"{"log":{"level":"error"}}"#)
        .await
        .unwrap();
    let command = Arc::new(FakeSingBoxCommandPort::new(NativeConfigCheck::Accepted));
    let adapter = SingBoxProfileConfigurationAdapter::new(
        PathBuf::from("sing-box"),
        config_path.clone(),
        command.clone(),
    );
    let proposed = br#"{"log":{"level":"warn"},"outbounds":[]}"#;

    let receipt = adapter.apply_and_readback(proposed).await.unwrap();

    assert_eq!(receipt.persisted_byte_count, proposed.len());
    assert_eq!(adapter.readback().await.unwrap().as_slice(), &proposed[..]);
    let checked_documents = command.checked_documents.lock().await;
    assert_eq!(checked_documents.len(), 1);
    assert_eq!(checked_documents[0].as_slice(), &proposed[..]);
    let mut files = tokio::fs::read_dir(home.path()).await.unwrap();
    assert_eq!(
        files.next_entry().await.unwrap().unwrap().file_name(),
        "config.json"
    );
    assert!(files.next_entry().await.unwrap().is_none());
}

#[tokio::test]
async fn invalid_or_natively_rejected_json_never_replaces_the_active_file() {
    let home = TempDir::new().unwrap();
    let config_path = home.path().join("config.json");
    let previous = br#"{"log":{"level":"error"}}"#;
    tokio::fs::write(&config_path, previous).await.unwrap();
    let command = Arc::new(FakeSingBoxCommandPort::new(NativeConfigCheck::Rejected));
    let adapter = SingBoxProfileConfigurationAdapter::new(
        PathBuf::from("sing-box"),
        config_path.clone(),
        command.clone(),
    );

    let rejected = adapter.apply_and_readback(br#"{"inbounds":[]}"#).await;
    assert!(matches!(rejected, Err(PortError::Rejected(_))));
    assert_eq!(adapter.readback().await.unwrap().as_slice(), &previous[..]);
    assert_eq!(command.checked_documents.lock().await.len(), 1);

    let malformed = adapter.apply_and_readback(b"{").await;
    assert!(matches!(malformed, Err(PortError::Rejected(_))));
    assert_eq!(adapter.readback().await.unwrap().as_slice(), &previous[..]);
    assert_eq!(command.checked_documents.lock().await.len(), 1);
}

struct SerializedApplyCommandPort {
    first_validation_started: Semaphore,
    second_validation_started: Semaphore,
    release_first_validation: Semaphore,
    checked_documents: Mutex<Vec<Vec<u8>>>,
}

impl SerializedApplyCommandPort {
    fn new() -> Self {
        Self {
            first_validation_started: Semaphore::new(0),
            second_validation_started: Semaphore::new(0),
            release_first_validation: Semaphore::new(0),
            checked_documents: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl SingBoxCommandPort for SerializedApplyCommandPort {
    async fn check_config(
        &self,
        _binary_path: &Path,
        config_path: &Path,
    ) -> Result<NativeConfigCheck, PortError> {
        let document = tokio::fs::read(config_path)
            .await
            .map_err(|error| PortError::Io(error.to_string()))?;
        self.checked_documents.lock().await.push(document.clone());
        match document.as_slice() {
            b"{\"transaction\":\"first\"}" => {
                self.first_validation_started.add_permits(1);
                self.release_first_validation
                    .acquire()
                    .await
                    .map_err(|_| PortError::Failed("validation gate closed".to_owned()))?
                    .forget();
            }
            b"{\"transaction\":\"second\"}" => {
                self.second_validation_started.add_permits(1);
            }
            _ => return Err(PortError::Failed("unexpected staged config".to_owned())),
        }
        Ok(NativeConfigCheck::Accepted)
    }

    async fn spawn_core(
        &self,
        _binary_path: &Path,
        _config_path: &Path,
    ) -> Result<Box<dyn SingBoxChildProcess>, PortError> {
        Err(PortError::Failed(
            "spawn is not used by this test".to_owned(),
        ))
    }
}

#[tokio::test]
async fn concurrent_apply_transactions_on_one_adapter_serialize_validation_through_readback() {
    let home = TempDir::new().unwrap();
    let config_path = home.path().join("config.json");
    tokio::fs::write(&config_path, br#"{"transaction":"original"}"#)
        .await
        .unwrap();
    let command = Arc::new(SerializedApplyCommandPort::new());
    let adapter = Arc::new(SingBoxProfileConfigurationAdapter::new(
        PathBuf::from("sing-box"),
        config_path,
        command.clone(),
    ));

    let first_adapter = adapter.clone();
    let first = tokio::spawn(async move {
        first_adapter
            .apply_and_readback(br#"{"transaction":"first"}"#)
            .await
    });
    tokio::time::timeout(
        std::time::Duration::from_secs(2),
        command.first_validation_started.acquire(),
    )
    .await
    .expect("first transaction reaches fake native validation")
    .unwrap()
    .forget();

    let second_adapter = adapter.clone();
    let second = tokio::spawn(async move {
        second_adapter
            .apply_and_readback(br#"{"transaction":"second"}"#)
            .await
    });
    let second_started_while_first_was_validating = tokio::time::timeout(
        std::time::Duration::from_millis(50),
        command.second_validation_started.acquire(),
    )
    .await;
    assert!(
        second_started_while_first_was_validating.is_err(),
        "second apply must wait until the first readback transaction releases the adapter lock"
    );

    command.release_first_validation.add_permits(1);
    first
        .await
        .unwrap()
        .expect("first transaction validates and reads back before second starts");
    second
        .await
        .unwrap()
        .expect("second transaction validates and reads back after first completes");
    command
        .second_validation_started
        .acquire()
        .await
        .unwrap()
        .forget();

    let checked_documents = command.checked_documents.lock().await;
    assert_eq!(
        checked_documents.as_slice(),
        &[
            br#"{"transaction":"first"}"#.to_vec(),
            br#"{"transaction":"second"}"#.to_vec(),
        ]
    );
    assert_eq!(
        adapter.readback().await.unwrap(),
        br#"{"transaction":"second"}"#
    );
}

#[tokio::test]
async fn process_lifecycle_tracks_a_single_child_and_fails_closed_on_duplicate_start() {
    let home = TempDir::new().unwrap();
    let config_path = home.path().join("config.json");
    let raw_config = br#"{"outbounds":[],"experimental":{"clash_api":{"external_controller":"127.0.0.1:9090"}}}"#;
    tokio::fs::write(&config_path, raw_config).await.unwrap();
    let command = Arc::new(FakeSingBoxCommandPort::new(NativeConfigCheck::Accepted));
    let core = SingBoxProcess::new(
        PathBuf::from("sing-box"),
        config_path,
        CoreInstanceId::new("test-sing-box").unwrap(),
        controller_configuration("127.0.0.1:9090", None).address(),
        command.clone(),
    )
    .unwrap();

    assert_eq!(core.status().await.unwrap(), CoreLifecycle::Stopped);
    let foreign_context = CoreProcessStartContext::standalone(CoreIdentity::new(
        EngineId::new("sing-box").expect("sing-box engine"),
        CoreInstanceId::new("different-sing-box").expect("foreign instance"),
        None,
    ));
    assert!(core.start(foreign_context).await.is_err());
    assert_eq!(command.spawn_count.load(Ordering::SeqCst), 0);
    let start_context = CoreProcessStartContext::standalone(
        core.core_identity().expect("stable sing-box test identity"),
    );
    core.start(start_context.clone()).await.unwrap();
    assert_eq!(core.status().await.unwrap(), CoreLifecycle::Running);
    assert_eq!(core.pid().await, Some(9001));
    assert!(core.start(start_context.clone()).await.is_err());
    assert_eq!(command.spawn_count.load(Ordering::SeqCst), 1);
    core.stop().await.unwrap();
    assert_eq!(core.status().await.unwrap(), CoreLifecycle::Stopped);
    assert_eq!(core.pid().await, None);
    core.start(start_context).await.unwrap();
    assert_eq!(core.status().await.unwrap(), CoreLifecycle::Running);
    assert_eq!(command.spawn_count.load(Ordering::SeqCst), 2);
    core.stop().await.unwrap();
}

#[tokio::test]
async fn readiness_uses_the_injected_http_port_and_rejects_non_loopback_endpoints() {
    let http = Arc::new(FakeReadinessHttpPort {
        outcome: FakeHttpOutcome::Ready,
        requests: StdMutex::new(Vec::new()),
    });
    let readiness = SingBoxControllerReadiness::new(
        controller_configuration("127.0.0.1:9090", None),
        http.clone(),
    )
    .expect("loopback endpoint is accepted");

    assert_eq!(readiness.probe().await.unwrap(), "http://127.0.0.1:9090");
    let requests = http
        .requests
        .lock()
        .expect("fake readiness request list is not poisoned");
    assert_eq!(requests[0].0, "http://127.0.0.1:9090/version");
    assert_eq!(requests[0].1, SingBoxClashMethod::Get);
    assert_eq!(requests[0].3, std::time::Duration::from_secs(5));
    assert_eq!(requests[0].4, FakeObservedAuthorization::NoSecret);
    assert_eq!(
        SingBoxControllerConfiguration::from_profile_json(
            br#"{"experimental":{"clash_api":{"external_controller":"192.0.2.1:9090"}}}"#,
        ),
        Err(singbox_api::controller_configuration::SingBoxControllerConfigurationError::ExternalControllerNotLoopback)
    );
}

#[tokio::test]
async fn readiness_rejects_an_unavailable_loopback_controller() {
    let readiness = SingBoxControllerReadiness::new(
        controller_configuration("127.0.0.1:9090", None),
        Arc::new(FakeReadinessHttpPort {
            outcome: FakeHttpOutcome::Unavailable,
            requests: StdMutex::new(Vec::new()),
        }),
    )
    .expect("loopback endpoint is accepted");

    assert!(readiness.probe().await.is_err());
}

#[tokio::test]
async fn readiness_reads_a_successful_loopback_http_version_response() {
    let mut server = mockito::Server::new_async().await;
    let route = server
        .mock("GET", "/version")
        .match_header("authorization", "Bearer fixture-token")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"version":"sing-box 1.14.3","meta":true}"#)
        .expect(2)
        .create_async()
        .await;
    let readiness = SingBoxControllerReadiness::new(
        controller_configuration_for_endpoint(&server.url(), Some("fixture-token")),
        Arc::new(ReqwestSingBoxClashHttpPort),
    )
    .unwrap();

    readiness
        .wait_until_ready(std::time::Duration::from_secs(2))
        .await
        .unwrap();
    assert_eq!(
        readiness
            .active_core_version()
            .await
            .unwrap()
            .expect("live sing-box version")
            .raw(),
        "v1.14.3"
    );

    route.assert_async().await;
}

#[tokio::test]
async fn readiness_does_not_accept_http_errors_or_unstructured_version_bodies() {
    let mut server = mockito::Server::new_async().await;
    let route = server
        .mock("GET", "/version")
        .with_status(503)
        .with_body(r#"{"error":"not ready"}"#)
        .create_async()
        .await;
    let readiness = SingBoxControllerReadiness::with_reqwest(
        controller_configuration_for_endpoint(&server.url(), None),
    )
    .unwrap();

    assert!(readiness.probe().await.is_err());

    route.assert_async().await;
}
