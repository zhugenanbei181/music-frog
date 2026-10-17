//! Host port for Android VpnService permission, foreground and tunnel state.

use async_trait::async_trait;
use infiltrator_contract::android_control::VpnAuthorizationState;
use infiltrator_contract::vpn::{
    PreparedVpnConfiguration, VpnSessionSnapshot, VpnStartRequest, VpnSurfaceObservation,
};
use lurker_core_ports::error::PortError;

/// Typed acknowledgement that this specific TUN data-plane start operation
/// completed. Android publishes its service-level `Running` state only after
/// the callback carrying this receipt returns successfully.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VpnStartReceipt {
    Started(VpnSessionSnapshot),
    AlreadyRunning(VpnSessionSnapshot),
}

#[async_trait]
pub trait VpnServicePort: Send + Sync {
    /// Return the source-authored surface read model. Android adapters expose
    /// the immutable `:vpn` owner snapshot; other hosts state their disposition.
    async fn owner_observation(&self) -> VpnSurfaceObservation {
        VpnSurfaceObservation::Unsupported {
            reason: "the host has no VpnService owner snapshot adapter".to_owned(),
        }
    }
    async fn authorization_state(&self) -> Result<VpnAuthorizationState, PortError> {
        Ok(VpnAuthorizationState::Required)
    }
    async fn request_start(&self) -> Result<VpnSessionSnapshot, PortError>;
    /// Transfer the exact Builder configuration and its VPN/Core activation
    /// binding to the Android host.
    async fn prepare(
        &self,
        prepared: PreparedVpnConfiguration,
    ) -> Result<VpnSessionSnapshot, PortError>;
    async fn start(&self, request: VpnStartRequest) -> Result<VpnStartReceipt, PortError>;
    async fn stop(&self) -> Result<VpnSessionSnapshot, PortError>;
    async fn revoke(&self) -> Result<VpnSessionSnapshot, PortError>;
    async fn snapshot(&self) -> Result<VpnSessionSnapshot, PortError>;
}
