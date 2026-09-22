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

/// The window-owner half of the Mini HUD window capability.
///
/// The port above is owned by the host runtime; the OS window it drives is
/// owned by whichever surface actually opened it (the Iced desktop shell in
/// this product). That surface implements this handle and registers it with
/// the host adapter, so a placement accepted by the shared application reaches
/// a real window instead of a bookkeeping field.
///
/// Both methods return `false` when the owner has no live window to drive —
/// for example before the host resolved a window id, or on a surface whose HUD
/// is an in-window overlay (Bevy). The adapter turns that into a typed
/// [`MiniHudHostOutcome::Unsupported`]; it must never report `Applied` on a
/// `false`.
///
/// A handle may enqueue the request for its own event loop: `true` means
/// "accepted by a live window", not "already painted".
pub trait MiniHudWindowHandle: Send + Sync {
    /// Accept a placement (move and always-on-top level) for the live window.
    fn apply_placement(&self, placement: MiniHudPlacement) -> bool;

    /// Accept a show/hide request for the live floating window. Owners without
    /// an independent window answer `false`.
    fn set_visible(&self, visible: bool) -> bool;
}
