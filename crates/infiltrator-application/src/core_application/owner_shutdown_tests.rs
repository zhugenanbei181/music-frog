use super::*;
use crate::core_application::ReadinessPolicy;
use crate::core_owner_shutdown_application::CoreOwnerShutdownApplication;
use crate::vpn_application::VpnServiceApplication;
use infiltrator_contract::core_identity::{CoreIdentity, CoreInstanceId, EngineId};
use infiltrator_contract::vpn::{
    PreparedVpnConfiguration, VpnSessionSnapshot, VpnSessionState, VpnStartRequest,
};
use infiltrator_ports::core_lifecycle::CoreLifecyclePort;
use infiltrator_ports::core_process::CoreProcess;
use infiltrator_ports::error::PortError;
use infiltrator_ports::vpn_service::{VpnServicePort, VpnStartReceipt};
use std::result::Result as StdResult;
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy)]
enum StopDisposition {
    Confirmed,
    Failed,
}

struct ShutdownProcess {
    state: Mutex<CoreLifecycle>,
    events: Arc<Mutex<Vec<&'static str>>>,
    stop: StopDisposition,
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

    async fn start(&self) -> StdResult<(), PortError> {
        self.events.lock().expect("event lock").push("core.start");
        *self.state.lock().expect("process state lock") = CoreLifecycle::Running;
        Ok(())
    }

    async fn stop(&self) -> StdResult<(), PortError> {
        self.events.lock().expect("event lock").push("core.stop");
        match self.stop {
            StopDisposition::Confirmed => {
                *self.state.lock().expect("process state lock") = CoreLifecycle::Stopped;
                Ok(())
            }
            StopDisposition::Failed => Err(PortError::Failed(
                "fake process did not confirm exit".to_owned(),
            )),
        }
    }

    async fn status(&self) -> StdResult<CoreLifecycle, PortError> {
        Ok(self.state.lock().expect("process state lock").clone())
    }

    fn controller_endpoint(&self) -> Option<String> {
        Some("http://127.0.0.1:9090".to_owned())
    }
}

struct ShutdownVpnPort {
    events: Arc<Mutex<Vec<&'static str>>>,
    stop: StopDisposition,
}

#[async_trait::async_trait]
impl VpnServicePort for ShutdownVpnPort {
    async fn request_start(&self) -> StdResult<VpnSessionSnapshot, PortError> {
        Err(PortError::Failed("not used by owner shutdown test".into()))
    }

    async fn prepare(
        &self,
        _prepared: PreparedVpnConfiguration,
    ) -> StdResult<VpnSessionSnapshot, PortError> {
        Err(PortError::Failed("not used by owner shutdown test".into()))
    }

    async fn start(&self, _request: VpnStartRequest) -> StdResult<VpnStartReceipt, PortError> {
        Err(PortError::Failed("not used by owner shutdown test".into()))
    }

    async fn stop(&self) -> StdResult<VpnSessionSnapshot, PortError> {
        self.events.lock().expect("event lock").push("vpn.stop");
        match self.stop {
            StopDisposition::Confirmed => Ok(VpnSessionSnapshot {
                state: VpnSessionState::Stopped,
                ..VpnSessionSnapshot::default()
            }),
            StopDisposition::Failed => Err(PortError::Failed(
                "fake VPN owner did not confirm terminal readback".to_owned(),
            )),
        }
    }

    async fn revoke(&self) -> StdResult<VpnSessionSnapshot, PortError> {
        Err(PortError::Failed("not used by owner shutdown test".into()))
    }

    async fn snapshot(&self) -> StdResult<VpnSessionSnapshot, PortError> {
        Ok(VpnSessionSnapshot::default())
    }
}

fn shutdown_application(
    vpn_stop: StopDisposition,
    core_stop: StopDisposition,
    events: Arc<Mutex<Vec<&'static str>>>,
) -> (CoreApplication, CoreOwnerShutdownApplication) {
    let core = crate::core_application::CoreApplication::new_with_policy(
        Arc::new(ShutdownProcess {
            state: Mutex::new(CoreLifecycle::Stopped),
            events: Arc::clone(&events),
            stop: core_stop,
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
        stop: vpn_stop,
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
