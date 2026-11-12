use super::*;
use async_trait::async_trait;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::public_ip_probe_application::PublicIpProbeApplication;
use infiltrator_contract::capability::{Availability, CapabilitySnapshot, CapabilityStatus};
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::public_ip::{PublicIpProbeSnapshot, PublicIpProbeStatus};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_ports::application_runtime::{
    ApplicationFuture, ApplicationRuntime, ApplicationSleep,
};
use infiltrator_ports::capability_provider::CapabilityProvider;
use infiltrator_ports::surface::SurfaceReader;
use lurker_core_contract::capability::Capability;
use lurker_core_contract::core_process::CoreProcessStartContext;
use lurker_core_contract::identity::{CoreIdentity, CoreInstanceId, EngineId};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_ports::core_process::{CoreProcess, CoreReadiness};
use lurker_core_ports::error::PortError;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Builder;
use tokio::time::sleep;

struct PublicIpTestRuntime;

impl ApplicationRuntime for PublicIpTestRuntime {
    fn block_on(&self, future: ApplicationFuture) {
        Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime")
            .block_on(future);
    }

    fn sleep(&self, duration: Duration) -> ApplicationSleep<'_> {
        Box::pin(sleep(duration))
    }
}

struct ScopedProcess;

struct SupportedCapabilities;

impl CapabilityProvider for SupportedCapabilities {
    fn host_kind(&self) -> HostKind {
        HostKind::Desktop
    }

    fn capabilities(&self) -> CapabilitySnapshot {
        CapabilitySnapshot::new(
            HostKind::Desktop,
            1,
            vec![CapabilityStatus {
                capability: Capability::Speedtest,
                availability: Availability::Supported,
            }],
        )
    }
}

#[async_trait]
impl CoreProcess for ScopedProcess {
    fn core_identity(&self) -> Option<CoreIdentity> {
        Some(CoreIdentity::new(
            EngineId::new("mihomo").expect("test engine ID"),
            CoreInstanceId::new("public-ip-readback").expect("test instance ID"),
            None,
        ))
    }

    async fn start(&self, _context: CoreProcessStartContext) -> Result<(), PortError> {
        Ok(())
    }

    async fn stop(&self) -> Result<(), PortError> {
        Ok(())
    }

    async fn status(&self) -> Result<CoreLifecycle, PortError> {
        Ok(CoreLifecycle::Ready)
    }

    fn controller_endpoint(&self) -> Option<String> {
        None
    }
}

struct ReadyReadiness;

#[async_trait]
impl CoreReadiness for ReadyReadiness {
    async fn probe(&self) -> Result<String, PortError> {
        Ok("ready".to_owned())
    }
}

#[tokio::test]
async fn surface_reader_publishes_the_public_ip_probe_owner_readback() {
    let core = Arc::new(CoreApplication::new(
        Arc::new(ScopedProcess),
        Arc::new(ReadyReadiness),
        Arc::new(PublicIpTestRuntime),
    ));
    let owner = PublicIpProbeApplication::unconfigured();
    core.install_public_ip_probe_application(owner.clone())
        .expect("install public IP probe owner");
    install_lifecycle_capabilities(&core);
    core.install_command_handler(Arc::new(
        CommandApplication::new()
            .with_host_capabilities(Arc::new(SupportedCapabilities))
            .with_public_ip_probe(owner.clone()),
    ));
    let reader =
        ProductSurfaceReader::new(core.clone(), SurfaceKind::BevyDesktop, HostKind::Desktop)
            .with_public_ip_probe(owner.clone());

    let before = reader.read().await.expect("initial surface read");
    assert_eq!(before.public_ip.status, PublicIpProbeStatus::Unknown);

    test_execute_result_at_current_scope(&core, CommandIntent::RefreshPublicIpProbe)
        .await
        .into_unit()
        .expect("typed refresh through Core command route");

    let after = reader.read().await.expect("terminal surface read");
    assert_eq!(after.public_ip.status, PublicIpProbeStatus::Failed);
    assert_eq!(after.public_ip, owner.snapshot());
    assert_ne!(after.public_ip, PublicIpProbeSnapshot::default());
}
