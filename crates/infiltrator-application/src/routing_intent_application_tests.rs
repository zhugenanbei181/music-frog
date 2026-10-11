use super::{RoutingIntentApplication, engine_defined_intent};
use crate::core_capability_application::CoreCapabilityApplication;
use infiltrator_contract::core_capabilities::{
    AdapterCapabilityDecision, AdapterRevision, CoreCapabilityEntry, CoreCapabilityMatrix,
    CoreCapabilityScope, CoreOperation, EngineCapabilityDecision, HostCapabilityDecision,
    ProxyControlOperation,
};
use infiltrator_contract::core_identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
};
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::routing_intent::RoutingIntent;
use infiltrator_contract::routing_intent::RoutingIntentStatus;
use infiltrator_contract::surface::HostKind;
use infiltrator_ports::core_capability_provider::{
    CoreCapabilityProvider, CoreCapabilityProviderError,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::routing_intent::RoutingIntentPort;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct FixedCapabilityProvider {
    matrix: CoreCapabilityMatrix,
}

impl CoreCapabilityProvider for FixedCapabilityProvider {
    fn matrix_for(
        &self,
        _engine_id: &EngineId,
        _core_version: Option<&CoreVersion>,
    ) -> Result<CoreCapabilityMatrix, CoreCapabilityProviderError> {
        Ok(self.matrix.clone())
    }
}

#[derive(Default)]
struct RecordingPort {
    calls: AtomicUsize,
    observe_calls: AtomicUsize,
    readback: Mutex<Option<RoutingIntent>>,
}

#[async_trait::async_trait]
impl RoutingIntentPort for RecordingPort {
    async fn observe(&self, _scope: &CoreScope) -> Result<RoutingIntent, PortError> {
        self.observe_calls.fetch_add(1, Ordering::SeqCst);
        Ok(self
            .readback
            .lock()
            .expect("readback lock")
            .clone()
            .unwrap_or(RoutingIntent::FollowRules))
    }

    async fn apply(
        &self,
        _scope: &CoreScope,
        intent: RoutingIntent,
    ) -> Result<RoutingIntent, PortError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self
            .readback
            .lock()
            .expect("readback lock")
            .clone()
            .unwrap_or(intent))
    }
}

fn scope(engine: &str) -> CoreScope {
    CoreScope {
        identity: CoreIdentity::new(
            EngineId::new(engine).expect("valid engine ID"),
            CoreInstanceId::new("core-a").expect("valid instance ID"),
            Some(CoreVersion::new("v1.19.32").expect("valid version")),
        ),
        generation: CoreGeneration::new(3),
        session_token: None,
    }
}

fn capabilities(register_write: bool, register_read: bool) -> CoreCapabilityApplication {
    let engine = EngineId::new("mihomo").expect("valid engine ID");
    let version = CoreVersion::new("v1.19.32").expect("valid version");
    let entries = [
        (ProxyControlOperation::SetRoutingIntent, register_write),
        (ProxyControlOperation::GetRoutingIntent, register_read),
    ]
    .into_iter()
    .filter(|(_, registered)| *registered)
    .map(|(operation, _)| {
        CoreCapabilityEntry::new(
            CoreOperation::ProxyControl(operation),
            EngineCapabilityDecision::Supported,
            HostCapabilityDecision::Supported,
            AdapterCapabilityDecision::Supported,
        )
    })
    .collect();
    let matrix = CoreCapabilityMatrix::new(
        CoreCapabilityScope::new(
            engine,
            Some(version),
            HostKind::Desktop,
            AdapterRevision::new(1),
        ),
        entries,
    )
    .expect("valid test capability matrix");
    CoreCapabilityApplication::new(Arc::new(FixedCapabilityProvider { matrix }))
}

#[tokio::test]
async fn missing_scope_and_unregistered_capability_refuse_without_calling_port() {
    let port = Arc::new(RecordingPort::default());
    let application = RoutingIntentApplication::new(port.clone(), capabilities(false, false));

    let missing_scope = application
        .apply(None, RoutingIntent::FollowRules)
        .await
        .expect_err("missing scope is denied");
    assert_eq!(missing_scope.code, ErrorCode::InvalidState);

    let unregistered = application
        .apply(Some(&scope("mihomo")), RoutingIntent::FollowRules)
        .await
        .expect_err("unregistered capability is denied");
    assert_eq!(unregistered.code, ErrorCode::Unsupported);
    assert_eq!(port.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn foreign_engine_scope_and_extension_owner_refuse_before_port_side_effects() {
    let port = Arc::new(RecordingPort::default());
    let application = RoutingIntentApplication::new(port.clone(), capabilities(true, true));

    let foreign_scope = application
        .apply(Some(&scope("sing-box")), RoutingIntent::FollowRules)
        .await
        .expect_err("foreign engine scope is denied");
    assert_eq!(foreign_scope.code, ErrorCode::InvalidState);

    let wrong_owner_intent = engine_defined_intent(
        EngineId::new("sing-box").expect("valid engine ID"),
        "script",
    )
    .expect("valid typed extension");
    let owner_mismatch = application
        .apply(Some(&scope("mihomo")), wrong_owner_intent)
        .await
        .expect_err("extension from another engine is denied");
    assert_eq!(owner_mismatch.code, ErrorCode::InvalidState);
    assert_eq!(port.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn application_requires_readback_to_equal_the_requested_intent() {
    let port = Arc::new(RecordingPort::default());
    *port.readback.lock().expect("readback lock") = Some(RoutingIntent::BypassProxy);
    let application = RoutingIntentApplication::new(port.clone(), capabilities(true, true));

    let failure = application
        .apply(Some(&scope("mihomo")), RoutingIntent::RouteAllThroughProxy)
        .await
        .expect_err("different adapter readback is rejected");

    assert_eq!(failure.code, ErrorCode::InvalidState);
    assert_eq!(port.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn supported_intent_reaches_port_and_returns_confirmed_value() {
    let port = Arc::new(RecordingPort::default());
    let application = RoutingIntentApplication::new(port.clone(), capabilities(true, true));

    let actual = application
        .apply(Some(&scope("mihomo")), RoutingIntent::FollowRules)
        .await
        .expect("registered intent is applied");

    assert_eq!(actual, RoutingIntent::FollowRules);
    assert_eq!(port.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn typed_observation_requires_scope_and_the_exact_read_capability() {
    let port = Arc::new(RecordingPort::default());
    let no_read_capability = RoutingIntentApplication::new(port.clone(), capabilities(true, false));
    let denied = no_read_capability.observe(Some(&scope("mihomo"))).await;
    assert_eq!(denied.status, RoutingIntentStatus::Unsupported);
    assert_eq!(port.observe_calls.load(Ordering::SeqCst), 0);

    let application = RoutingIntentApplication::new(port.clone(), capabilities(true, true));
    let missing_scope = application.observe(None).await;
    assert_eq!(missing_scope.status, RoutingIntentStatus::Failed);
    assert_eq!(port.observe_calls.load(Ordering::SeqCst), 0);

    let observed = application.observe(Some(&scope("mihomo"))).await;
    assert_eq!(observed.current, Some(RoutingIntent::FollowRules));
    assert_eq!(observed.status, RoutingIntentStatus::Ready);
    assert_eq!(port.observe_calls.load(Ordering::SeqCst), 1);
}
