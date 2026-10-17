use super::*;
use crate::core_application::CoreApplication;
use crate::core_application::ReadinessPolicy;
use crate::core_owner_shutdown_application::CoreOwnerShutdownApplication;
use crate::vpn_application::VpnServiceApplication;
use infiltrator_contract::core_identity::{CoreGeneration, CoreIdentity, CoreInstanceId, EngineId};
use infiltrator_contract::core_process::CoreProcessStartContext;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::snapshot::CoreLifecycleSnapshot;
use infiltrator_contract::vpn::{
    PreparedVpnConfiguration, VpnSessionSnapshot, VpnSessionState, VpnStartRequest,
};
use infiltrator_ports::active_profile_reference_store::ActiveProfileOwnerLease;
use infiltrator_ports::core_lifecycle::CoreLifecyclePort;
use infiltrator_ports::core_process::CoreProcess;
use infiltrator_ports::error::PortError;
use infiltrator_ports::vpn_service::{VpnServicePort, VpnStartReceipt};
use std::collections::VecDeque;
use std::future::pending;
use std::mem;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

#[derive(Clone, Copy)]
enum StopDisposition {
    Confirmed,
    Failed,
}

struct ShutdownProcess {
    state: Mutex<CoreLifecycle>,
    events: Arc<Mutex<Vec<&'static str>>>,
    stop_outcomes: Mutex<VecDeque<StopDisposition>>,
}

#[async_trait::async_trait]
impl CoreProcess for ShutdownProcess {
    fn core_identity(&self) -> Option<CoreIdentity> {
        Some(CoreIdentity::new(
            EngineId::new("mihomo").ok()?,
            CoreInstanceId::new("owner-shutdown-test").ok()?,
            None,
        ))
    }

    async fn start(&self, _context: CoreProcessStartContext) -> Result<(), PortError> {
        self.events.lock().expect("event lock").push("core.start");
        *self.state.lock().expect("process state lock") = CoreLifecycle::Running;
        Ok(())
    }

    async fn stop(&self) -> Result<(), PortError> {
        self.events.lock().expect("event lock").push("core.stop");
        match self
            .stop_outcomes
            .lock()
            .expect("Core stop outcome lock")
            .pop_front()
            .unwrap_or(StopDisposition::Confirmed)
        {
            StopDisposition::Confirmed => {
                *self.state.lock().expect("process state lock") = CoreLifecycle::Stopped;
                Ok(())
            }
            StopDisposition::Failed => Err(PortError::Failed(
                "fake process did not confirm exit".to_owned(),
            )),
        }
    }

    async fn status(&self) -> Result<CoreLifecycle, PortError> {
        Ok(self.state.lock().expect("process state lock").clone())
    }

    fn controller_endpoint(&self) -> Option<String> {
        Some("http://127.0.0.1:9090".to_owned())
    }
}

struct ShutdownVpnPort {
    events: Arc<Mutex<Vec<&'static str>>>,
    stop_outcomes: Mutex<VecDeque<StopDisposition>>,
    first_stop_gate: Mutex<ShutdownVpnFirstStopGate>,
}

enum ShutdownVpnFirstStopGate {
    Open,
    SuspendFirst(Arc<Notify>),
}

struct ShutdownOwnerLease {
    identity: CoreIdentity,
    generation: CoreGeneration,
    releases: Arc<AtomicUsize>,
}

#[async_trait::async_trait]
impl ActiveProfileOwnerLease for ShutdownOwnerLease {
    fn identity(&self) -> &CoreIdentity {
        &self.identity
    }

    fn generation(&self) -> CoreGeneration {
        self.generation
    }

    async fn record_start_context(
        &self,
        _context: &CoreProcessStartContext,
    ) -> Result<(), PortError> {
        Ok(())
    }

    async fn release_after_terminal(
        &self,
        terminal: &CoreLifecycleSnapshot,
    ) -> Result<(), PortError> {
        if !terminal
            .identity
            .as_ref()
            .is_some_and(|identity| identity.same_instance(&self.identity))
            || terminal.lifecycle != CoreLifecycle::Stopped
            || terminal.session_token.is_some()
            || terminal.generation < self.generation.value()
        {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "shutdown test owner lease received a nonterminal snapshot",
                true,
            )));
        }
        self.releases.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[async_trait::async_trait]
impl VpnServicePort for ShutdownVpnPort {
    async fn request_start(&self) -> Result<VpnSessionSnapshot, PortError> {
        Err(PortError::Failed("not used by owner shutdown test".into()))
    }

    async fn prepare(
        &self,
        _prepared: PreparedVpnConfiguration,
    ) -> Result<VpnSessionSnapshot, PortError> {
        Err(PortError::Failed("not used by owner shutdown test".into()))
    }

    async fn start(&self, _request: VpnStartRequest) -> Result<VpnStartReceipt, PortError> {
        Err(PortError::Failed("not used by owner shutdown test".into()))
    }

    async fn stop(&self) -> Result<VpnSessionSnapshot, PortError> {
        self.events.lock().expect("event lock").push("vpn.stop");
        let first_stop_gate = {
            let mut gate = self.first_stop_gate.lock().expect("VPN stop gate lock");
            mem::replace(&mut *gate, ShutdownVpnFirstStopGate::Open)
        };
        if let ShutdownVpnFirstStopGate::SuspendFirst(started) = first_stop_gate {
            started.notify_one();
            pending::<()>().await;
        }
        match self
            .stop_outcomes
            .lock()
            .expect("VPN stop outcome lock")
            .pop_front()
            .unwrap_or(StopDisposition::Confirmed)
        {
            StopDisposition::Confirmed => Ok(VpnSessionSnapshot {
                state: VpnSessionState::Stopped,
                ..VpnSessionSnapshot::default()
            }),
            StopDisposition::Failed => Err(PortError::Failed(
                "fake VPN owner did not confirm terminal readback".to_owned(),
            )),
        }
    }

    async fn revoke(&self) -> Result<VpnSessionSnapshot, PortError> {
        Err(PortError::Failed("not used by owner shutdown test".into()))
    }

    async fn snapshot(&self) -> Result<VpnSessionSnapshot, PortError> {
        Ok(VpnSessionSnapshot::default())
    }
}

fn shutdown_application(
    vpn_stop: StopDisposition,
    core_stop: StopDisposition,
    events: Arc<Mutex<Vec<&'static str>>>,
) -> (CoreApplication, CoreOwnerShutdownApplication) {
    shutdown_application_with_outcomes(
        VecDeque::from([vpn_stop]),
        VecDeque::from([core_stop]),
        ShutdownVpnFirstStopGate::Open,
        events,
    )
}

fn shutdown_application_with_outcomes(
    vpn_stops: VecDeque<StopDisposition>,
    core_stops: VecDeque<StopDisposition>,
    first_stop_gate: ShutdownVpnFirstStopGate,
    events: Arc<Mutex<Vec<&'static str>>>,
) -> (CoreApplication, CoreOwnerShutdownApplication) {
    let core = CoreApplication::new_with_policy(
        Arc::new(ShutdownProcess {
            state: Mutex::new(CoreLifecycle::Stopped),
            events: Arc::clone(&events),
            stop_outcomes: Mutex::new(core_stops),
        }),
        Arc::new(FakeReadiness {
            endpoint: Ok("http://127.0.0.1:9090".to_owned()),
        }),
        ReadinessPolicy {
            timeout: time::Duration::from_millis(100),
            poll_interval: time::Duration::from_millis(1),
        },
        runtime(),
    );
    install_lifecycle_capabilities(&core);
    let vpn = VpnServiceApplication::new(Arc::new(ShutdownVpnPort {
        events,
        stop_outcomes: Mutex::new(vpn_stops),
        first_stop_gate: Mutex::new(first_stop_gate),
    }));
    let shutdown = CoreOwnerShutdownApplication::new(core.clone(), vpn);
    (core, shutdown)
}

#[tokio::test]
async fn owner_shutdown_confirms_vpn_before_core_and_returns_both_terminals() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let (core, shutdown) = shutdown_application(
        StopDisposition::Confirmed,
        StopDisposition::Confirmed,
        Arc::clone(&events),
    );
    CoreLifecyclePort::start(&core).await.unwrap();
    events.lock().expect("event lock").clear();

    let receipt = shutdown.close().await.unwrap();

    assert_eq!(
        *events.lock().expect("event lock"),
        vec!["vpn.stop", "core.stop"]
    );
    assert_eq!(receipt.vpn_terminal.state, VpnSessionState::Stopped);
    assert_eq!(receipt.core_terminal.lifecycle, CoreLifecycle::Stopped);
    assert!(receipt.core_terminal.session_token.is_none());
}

#[tokio::test]
async fn owner_shutdown_checks_host_process_even_when_core_was_never_started() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let (_core, shutdown) = shutdown_application(
        StopDisposition::Confirmed,
        StopDisposition::Confirmed,
        Arc::clone(&events),
    );

    let receipt = shutdown.close().await.unwrap();

    assert_eq!(
        *events.lock().expect("event lock"),
        vec!["vpn.stop", "core.stop"]
    );
    assert_eq!(receipt.core_terminal.lifecycle, CoreLifecycle::Stopped);
    assert!(receipt.core_terminal.session_token.is_none());
}

#[tokio::test]
async fn owner_shutdown_failure_to_stop_vpn_does_not_stop_core() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let (core, shutdown) = shutdown_application(
        StopDisposition::Failed,
        StopDisposition::Confirmed,
        Arc::clone(&events),
    );
    CoreLifecyclePort::start(&core).await.unwrap();
    events.lock().expect("event lock").clear();

    assert!(shutdown.close().await.is_err());

    assert_eq!(*events.lock().expect("event lock"), vec!["vpn.stop"]);
    assert_eq!(core.snapshot().lifecycle, CoreLifecycle::Running);
}

#[tokio::test]
async fn owner_shutdown_core_failure_keeps_the_terminal_receipt_unavailable() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let (core, shutdown) = shutdown_application(
        StopDisposition::Confirmed,
        StopDisposition::Failed,
        Arc::clone(&events),
    );
    CoreLifecyclePort::start(&core).await.unwrap();
    events.lock().expect("event lock").clear();

    assert!(shutdown.close().await.is_err());

    assert_eq!(
        *events.lock().expect("event lock"),
        vec!["vpn.stop", "core.stop"]
    );
    assert_ne!(core.snapshot().lifecycle, CoreLifecycle::Stopped);
}

#[tokio::test]
async fn owner_shutdown_retries_vpn_failure_before_stopping_core() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let (_core, shutdown) = shutdown_application_with_outcomes(
        VecDeque::from([StopDisposition::Failed, StopDisposition::Confirmed]),
        VecDeque::from([StopDisposition::Confirmed]),
        ShutdownVpnFirstStopGate::Open,
        Arc::clone(&events),
    );

    assert!(shutdown.close().await.is_err());
    assert_eq!(*events.lock().expect("event lock"), vec!["vpn.stop"]);

    let receipt = shutdown.close().await.expect("retry close");
    assert_eq!(receipt.vpn_terminal.state, VpnSessionState::Stopped);
    assert_eq!(
        *events.lock().expect("event lock"),
        vec!["vpn.stop", "vpn.stop", "core.stop"]
    );
}

#[tokio::test]
async fn owner_shutdown_retries_core_failure_after_stopping_vpn() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let (core, shutdown) = shutdown_application_with_outcomes(
        VecDeque::from([StopDisposition::Confirmed, StopDisposition::Confirmed]),
        VecDeque::from([StopDisposition::Failed, StopDisposition::Confirmed]),
        ShutdownVpnFirstStopGate::Open,
        Arc::clone(&events),
    );
    CoreLifecyclePort::start(&core).await.unwrap();
    events.lock().expect("event lock").clear();

    assert!(shutdown.close().await.is_err());
    assert_eq!(
        *events.lock().expect("event lock"),
        vec!["vpn.stop", "core.stop"]
    );

    let receipt = shutdown.close().await.expect("retry close");
    assert_eq!(receipt.core_terminal.lifecycle, CoreLifecycle::Stopped);
    assert_eq!(
        *events.lock().expect("event lock"),
        vec!["vpn.stop", "core.stop", "vpn.stop", "core.stop"]
    );
}

#[tokio::test]
async fn canceling_owner_shutdown_during_vpn_stop_never_stops_core_and_can_retry() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let started = Arc::new(Notify::new());
    let (_core, shutdown) = shutdown_application_with_outcomes(
        VecDeque::from([StopDisposition::Confirmed]),
        VecDeque::from([StopDisposition::Confirmed]),
        ShutdownVpnFirstStopGate::SuspendFirst(Arc::clone(&started)),
        Arc::clone(&events),
    );
    let close = tokio::spawn({
        let shutdown = shutdown.clone();
        async move { shutdown.close().await }
    });

    started.notified().await;
    close.abort();
    assert!(close.await.unwrap_err().is_cancelled());
    assert_eq!(*events.lock().expect("event lock"), vec!["vpn.stop"]);

    shutdown
        .close()
        .await
        .expect("retry close after cancellation");
    assert_eq!(
        *events.lock().expect("event lock"),
        vec!["vpn.stop", "vpn.stop", "core.stop"]
    );
}

#[tokio::test]
async fn owner_shutdown_retains_binding_through_vpn_and_core_failures_then_releases_once() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let releases = Arc::new(AtomicUsize::new(0));
    let (core, shutdown) = shutdown_application_with_outcomes(
        VecDeque::from([
            StopDisposition::Failed,
            StopDisposition::Confirmed,
            StopDisposition::Confirmed,
        ]),
        VecDeque::from([StopDisposition::Failed, StopDisposition::Confirmed]),
        ShutdownVpnFirstStopGate::Open,
        Arc::clone(&events),
    );
    CoreLifecyclePort::start(&core).await.unwrap();
    let snapshot = core.snapshot().lifecycle_snapshot();
    core.install_active_profile_owner_binding(Box::new(ShutdownOwnerLease {
        identity: snapshot.identity.expect("owner identity"),
        generation: CoreGeneration::new(snapshot.generation),
        releases: Arc::clone(&releases),
    }))
    .expect("install owner binding");
    events.lock().expect("event lock").clear();

    assert!(shutdown.close().await.is_err());
    assert_eq!(releases.load(Ordering::SeqCst), 0);
    assert!(matches!(
        *core.inner.active_profile_owner_binding.lock().unwrap(),
        super::ActiveProfileOwnerBindingState::Bound(_)
    ));

    assert!(shutdown.close().await.is_err());
    assert_eq!(releases.load(Ordering::SeqCst), 0);
    assert!(matches!(
        *core.inner.active_profile_owner_binding.lock().unwrap(),
        super::ActiveProfileOwnerBindingState::Bound(_)
    ));

    shutdown.close().await.expect("terminal retry");
    assert_eq!(releases.load(Ordering::SeqCst), 1);
    assert!(matches!(
        *core.inner.active_profile_owner_binding.lock().unwrap(),
        super::ActiveProfileOwnerBindingState::Released
    ));
    assert_eq!(
        *events.lock().expect("event lock"),
        vec!["vpn.stop", "vpn.stop", "core.stop", "vpn.stop", "core.stop",]
    );
}
