use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::error::Failure;
use super::*;
use crate::resource_application::ResourceApplication;
use async_trait::async_trait;
use infiltrator_contract::command::CoreLogLevel;
use infiltrator_contract::core_identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
};
use infiltrator_contract::resources::{
    CORE_MEMORY_SOFT_LIMIT_BYTES, CoreGcStatus, CoreResourceSnapshot, ResourceAvailability,
};
use infiltrator_domain::runtime::TrafficData;
use infiltrator_ports::core_log_level::CoreLogLevelPort;
use infiltrator_ports::core_scope::{CoreScopeObservation, CoreScopeSource};
use infiltrator_ports::error::PortError;
use infiltrator_ports::resources::{
    CoreResourcePort, GarbageCollectionOutcome, ScopedGarbageCollection, ScopedResourceObservation,
};
use infiltrator_ports::runtime_stream::{
    RuntimeStreamPort, ScopedRuntimeStream, UnsupportedRuntimeStreamPort,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

struct TestCoreOwner {
    level: Mutex<String>,
    ignore_log_level_write: bool,
    scope: CoreScope,
}

struct TestResourcePort {
    scope: Mutex<CoreScope>,
    observe_calls: AtomicUsize,
    memory: ResourceAvailability<u64>,
    cpu: ResourceAvailability<f32>,
    gc_outcome: GarbageCollectionOutcome,
    gc_calls: AtomicUsize,
}

impl CoreScopeSource for TestCoreOwner {
    fn scope_observation(&self) -> CoreScopeObservation {
        CoreScopeObservation::Observed(self.scope.clone())
    }
}

#[async_trait]
impl CoreResourcePort for TestResourcePort {
    async fn observe(&self, _scope: CoreScope) -> Result<ScopedResourceObservation, PortError> {
        self.observe_calls.fetch_add(1, Ordering::SeqCst);
        Ok(ScopedResourceObservation {
            scope: self.scope.lock().expect("resource scope lock").clone(),
            memory_bytes: self.memory.clone(),
            cpu_percent: self.cpu.clone(),
        })
    }

    async fn collect_garbage(
        &self,
        _scope: CoreScope,
        _before_bytes: u64,
    ) -> Result<ScopedGarbageCollection, PortError> {
        self.gc_calls.fetch_add(1, Ordering::SeqCst);
        Ok(ScopedGarbageCollection {
            scope: self.scope.lock().expect("resource scope lock").clone(),
            outcome: self.gc_outcome.clone(),
        })
    }
}

#[async_trait]
impl CoreLogLevelPort for TestCoreOwner {
    async fn set_core_log_level(&self, level: CoreLogLevel) -> Result<CoreLogLevel, PortError> {
        if !self.ignore_log_level_write {
            *self.level.lock().expect("level lock") = level.as_key().to_owned();
        }
        let observed = self.level.lock().expect("level lock").clone();
        CoreLogLevel::from_key(&observed)
            .ok_or_else(|| PortError::Failed(format!("unknown test log level: {observed}")))
    }
}

fn gateway(ignore_log_level_write: bool) -> Arc<TestCoreOwner> {
    Arc::new(TestCoreOwner {
        level: Mutex::new("info".to_owned()),
        ignore_log_level_write,
        scope: test_scope(1),
    })
}

fn test_scope(generation: u64) -> CoreScope {
    CoreScope {
        identity: CoreIdentity::new(
            EngineId::mihomo(),
            CoreInstanceId::new("resource-tests").expect("instance"),
            Some(CoreVersion::new("test").expect("version")),
        ),
        generation: CoreGeneration::new(generation),
        session_token: None,
    }
}

fn resource_port(
    scope: CoreScope,
    memory: ResourceAvailability<u64>,
    outcome: GarbageCollectionOutcome,
) -> Arc<TestResourcePort> {
    Arc::new(TestResourcePort {
        scope: Mutex::new(scope),
        observe_calls: AtomicUsize::new(0),
        memory,
        cpu: ResourceAvailability::Unsupported,
        gc_outcome: outcome,
        gc_calls: AtomicUsize::new(0),
    })
}

struct UnavailableCoreScope;

impl CoreScopeSource for UnavailableCoreScope {
    fn scope_observation(&self) -> CoreScopeObservation {
        CoreScopeObservation::Unavailable
    }
}

struct CountingRuntimeStreamPort(AtomicUsize);

#[async_trait]
impl RuntimeStreamPort for CountingRuntimeStreamPort {
    async fn logs(
        &self,
        _scope: CoreScope,
        _level: Option<String>,
    ) -> Result<ScopedRuntimeStream<String>, PortError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(PortError::Failed("unexpected log stream open".to_owned()))
    }

    async fn traffic(
        &self,
        _scope: CoreScope,
    ) -> Result<ScopedRuntimeStream<TrafficData>, PortError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(PortError::Failed(
            "unexpected traffic stream open".to_owned(),
        ))
    }
}

#[tokio::test]
async fn unavailable_scope_rejects_runtime_stream_before_port_call() {
    let stream_port = Arc::new(CountingRuntimeStreamPort(AtomicUsize::new(0)));
    let application =
        RuntimeQueryApplication::new(Arc::new(UnavailableCoreScope), stream_port.clone());

    let failure = match application.logs(None).await {
        Err(failure) => failure,
        Ok(_) => panic!("inactive Core unexpectedly opened a runtime stream"),
    };

    assert_eq!(
        failure.code,
        ErrorCode::NotReady
    );
    assert_eq!(stream_port.0.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn core_log_level_uses_the_typed_port_and_reports_its_readback() {
    let gateway = gateway(false);
    RuntimeQueryApplication::new(gateway.clone(), Arc::new(UnsupportedRuntimeStreamPort))
        .with_core_log_level_port(gateway.clone())
        .set_core_log_level(CoreLogLevel::Debug)
        .await
        .expect("log-level readback should match");
    assert_eq!(gateway.level.lock().expect("level lock").as_str(), "debug");
}

#[tokio::test]
async fn core_log_level_readback_mismatch_is_not_reported_as_success() {
    let gateway = gateway(true);
    let failure =
        RuntimeQueryApplication::new(gateway.clone(), Arc::new(UnsupportedRuntimeStreamPort))
            .with_core_log_level_port(gateway)
            .set_core_log_level(CoreLogLevel::Debug)
            .await
            .expect_err("ignored update must not report success");
    assert_eq!(
        failure.code,
        ErrorCode::InvalidState
    );
}

#[tokio::test]
async fn resource_poll_triggers_gc_once_when_memory_exceeds_soft_limit() {
    let gateway = gateway(false);
    let resources = resource_port(
        gateway.scope.clone(),
        ResourceAvailability::Available(CORE_MEMORY_SOFT_LIMIT_BYTES + 1),
        GarbageCollectionOutcome::Collected {
            before_bytes: CORE_MEMORY_SOFT_LIMIT_BYTES + 1,
            after_bytes: 400 * 1024 * 1024,
        },
    );
    let application = ResourceApplication::new(gateway.clone(), resources.clone());
    let first = application.poll().await.expect("first poll");
    assert!(matches!(
        first,
        CoreResourceSnapshot::Observed { gc, .. }
            if matches!(gc.as_ref(), CoreGcStatus::Collected { after_bytes, .. } if *after_bytes == 400 * 1024 * 1024)
    ));
    assert_eq!(resources.gc_calls.load(Ordering::SeqCst), 1);

    let second = application.poll().await.expect("second poll");
    assert!(matches!(
        second,
        CoreResourceSnapshot::Observed { gc, .. }
            if matches!(gc.as_ref(), CoreGcStatus::Collected { .. })
    ));
    assert_eq!(resources.gc_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn resource_observation_preserves_unsupported_and_failed_values() {
    let gateway = gateway(false);
    let resources = resource_port(
        gateway.scope.clone(),
        ResourceAvailability::Failed(Failure::new(
            ErrorCode::Network,
            "memory probe failed",
            true,
        )),
        GarbageCollectionOutcome::Unsupported,
    );
    let snapshot = ResourceApplication::new(gateway, resources.clone())
        .poll()
        .await
        .expect("partial resource observations remain publishable");
    assert!(matches!(
        snapshot,
        CoreResourceSnapshot::Observed {
            memory: ResourceAvailability::Failed(_),
            cpu_percent: ResourceAvailability::Unsupported,
            gc,
            ..
        } if matches!(gc.as_ref(), CoreGcStatus::NotAttempted)
    ));
    assert_eq!(resources.gc_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn unsupported_memory_does_not_trigger_garbage_collection() {
    let gateway = gateway(false);
    let resources = resource_port(
        gateway.scope.clone(),
        ResourceAvailability::Unsupported,
        GarbageCollectionOutcome::Unsupported,
    );
    let snapshot = ResourceApplication::new(gateway, resources.clone())
        .poll()
        .await
        .expect("unsupported memory is a typed observation");
    assert!(matches!(
        snapshot,
        CoreResourceSnapshot::Observed {
            memory: ResourceAvailability::Unsupported,
            gc,
            ..
        } if matches!(gc.as_ref(), CoreGcStatus::NotAttempted)
    ));
    assert_eq!(resources.gc_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn stale_resource_observation_is_rejected_before_gc() {
    let gateway = gateway(false);
    let resources = resource_port(
        test_scope(2),
        ResourceAvailability::Available(CORE_MEMORY_SOFT_LIMIT_BYTES + 1),
        GarbageCollectionOutcome::Unsupported,
    );
    let failure = ResourceApplication::new(gateway, resources.clone())
        .poll()
        .await
        .expect_err("foreign resource scope must be rejected");
    assert_eq!(
        failure.code,
        ErrorCode::InvalidState
    );
    assert_eq!(resources.gc_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn failed_gc_is_typed_and_does_not_claim_memory_reclamation() {
    let gateway = gateway(false);
    let resources = resource_port(
        gateway.scope.clone(),
        ResourceAvailability::Available(CORE_MEMORY_SOFT_LIMIT_BYTES + 1),
        GarbageCollectionOutcome::Failed(Failure::new(
            ErrorCode::Network,
            "collector failed",
            true,
        )),
    );
    let snapshot = ResourceApplication::new(gateway, resources)
        .poll()
        .await
        .expect("GC failure is a typed resource outcome");
    assert!(matches!(
        snapshot,
        CoreResourceSnapshot::Observed { gc, .. }
            if matches!(gc.as_ref(), CoreGcStatus::Failed { .. })
    ));
}

#[tokio::test]
async fn unsupported_gc_is_never_reported_as_collection() {
    let gateway = gateway(false);
    let resources = resource_port(
        gateway.scope.clone(),
        ResourceAvailability::Available(CORE_MEMORY_SOFT_LIMIT_BYTES + 1),
        GarbageCollectionOutcome::Unsupported,
    );
    let snapshot = ResourceApplication::new(gateway, resources.clone())
        .poll()
        .await
        .expect("unsupported GC is a typed outcome");
    assert!(matches!(
        snapshot,
        CoreResourceSnapshot::Observed { gc, .. }
            if matches!(gc.as_ref(), CoreGcStatus::Unsupported)
    ));
    assert_eq!(resources.gc_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn unavailable_owner_scope_rejects_resource_reads_before_adapter_access() {
    let resources = resource_port(
        test_scope(1),
        ResourceAvailability::Available(1),
        GarbageCollectionOutcome::Unsupported,
    );
    let scope_source: Arc<dyn CoreScopeSource> = Arc::new(UnavailableCoreScope);
    let failure = ResourceApplication::new(scope_source, resources.clone())
        .observe()
        .await
        .expect_err("resource observation needs an active Core owner scope");

    assert_eq!(
        failure.code,
        ErrorCode::NotReady
    );
    assert_eq!(resources.observe_calls.load(Ordering::SeqCst), 0);
    assert_eq!(resources.gc_calls.load(Ordering::SeqCst), 0);
}
