//! Ordered terminal shutdown for an Android Core owner session.

use crate::core_application::CoreApplication;
use crate::vpn_application::VpnServiceApplication;
use infiltrator_contract::core_identity::CoreIdentity;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::snapshot::{CoreLifecycle, CoreLifecycleSnapshot};
use infiltrator_contract::vpn::VpnSessionSnapshot;

#[derive(Clone)]
pub struct CoreOwnerShutdownApplication {
    core: CoreApplication,
    vpn: VpnServiceApplication,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoreOwnerShutdownReceipt {
    pub vpn_terminal: VpnSessionSnapshot,
    pub core_terminal: CoreLifecycleSnapshot,
}

impl CoreOwnerShutdownApplication {
    pub fn new(core: CoreApplication, vpn: VpnServiceApplication) -> Self {
        Self { core, vpn }
    }

    /// Stop and verify the host VPN before stopping Core and releasing its
    /// active-profile owner binding.
    pub async fn close(&self) -> Result<CoreOwnerShutdownReceipt, Failure> {
        let vpn_terminal = self.vpn.stop().await?;
        let bound_identity = self.core.snapshot().identity.ok_or_else(|| {
            Failure::new(
                ErrorCode::NotReady,
                "Core owner shutdown has no typed Core identity",
                true,
            )
        })?;
        self.core.close().await?;
        let core_terminal = self.core.snapshot().lifecycle_snapshot();
        validate_core_terminal(&bound_identity, &core_terminal)?;

        Ok(CoreOwnerShutdownReceipt {
            vpn_terminal,
            core_terminal,
        })
    }
}

fn validate_core_terminal(
    expected_identity: &CoreIdentity,
    terminal: &CoreLifecycleSnapshot,
) -> Result<(), Failure> {
    if !terminal
        .identity
        .as_ref()
        .is_some_and(|identity| identity.same_instance(expected_identity))
        || terminal.lifecycle != CoreLifecycle::Stopped
        || terminal.session_token.is_some()
    {
        return Err(Failure::new(
            ErrorCode::NotReady,
            "Core owner shutdown readback did not match its exact Stopped terminal",
            true,
        ));
    }
    Ok(())
}
