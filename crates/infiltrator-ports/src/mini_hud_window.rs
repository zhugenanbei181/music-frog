//! Host floating-window capability for the Mini HUD.
//!
//! The HUD's *placement* is a shared setting; projecting that placement onto
//! a real always-on-top frameless window is host work. Hosts that own such a
//! window implement this port; hosts that do not must answer
//! [`MiniHudHostOutcome::Unsupported`] with a reason instead of pretending the
//! window moved.

use crate::error::PortError;
use infiltrator_contract::mini_hud::{MiniHudHostOutcome, MiniHudPlacement};

#[async_trait::async_trait]
pub trait MiniHudWindowPort: Send + Sync {
    /// Move and pin the floating HUD window to a persisted placement.
    async fn apply_placement(
        &self,
        placement: MiniHudPlacement,
    ) -> Result<MiniHudHostOutcome, PortError>;

    /// Show or hide the floating HUD window.
    async fn set_visible(&self, visible: bool) -> Result<MiniHudHostOutcome, PortError>;
}
