//! Desktop system-tray host seam (FLPARITY-006).
//!
//! The tray is a *host* capability: the desktop host owns the OS tray, the
//! surface never does. This module owns that seam:
//!
//! * [`spawn_tray`] starts the platform backend (StatusNotifierItem on Linux,
//!   muda + tray-icon on Windows/macOS) behind one neutral contract;
//! * [`DesktopTrayHost`] carries the controller, the event channel and the
//!   typed capability report a surface is allowed to publish;
//! * a failed spawn degrades to [`TrayUnavailable`] — a tray is a pure
//!   enhancement, so any failure leaves a window-only app, never a panic.
//!
//! The menu tree is built by the shared presentation projection
//! (`infiltrator_presentation::tray_projection`); this module only renders
//! whatever spec it is pushed.
//!
//! `!Send` on purpose: the Windows/macOS backend owns a native menu handle
//! that must stay on the thread that created it, so hosts carry the
//! controller on their main thread (Bevy non-send resource, Slint host
//! struct) instead of moving it across threads.

use infiltrator_contract::tray_menu::TraySpec;
use infiltrator_contract::tray_status::{TrayEvent, TrayIconColorMode, TraySupport};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::sync::mpsc::Receiver;

#[cfg(all(unix, not(target_os = "macos"), not(target_os = "android")))]
pub mod ksni_backend;
#[cfg(any(windows, target_os = "macos"))]
pub mod native;

/// The tray backend handle: push a full menu refresh, then release the OS
/// resources on shutdown. Best-effort by contract — errors are swallowed the
/// same way they were before the seam existed.
pub trait TrayController {
    fn update_spec(&self, spec: TraySpec);
    fn shutdown(&mut self);
}

/// Typed startup result of a tray spawn.
pub enum TrayStartup {
    Ready {
        controller: Box<dyn TrayController>,
        events: Receiver<TrayEvent>,
    },
    Unavailable {
        reason: String,
    },
}

/// The typed degradation a failed tray spawn produces.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrayUnavailable {
    pub reason: String,
}

impl Display for TrayUnavailable {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "system tray unavailable: {}", self.reason)
    }
}

impl Error for TrayUnavailable {}

/// Spawn the platform tray with the given spec. Never panics and never
/// fails the host.
pub fn spawn_tray(spec: TraySpec) -> TrayStartup {
    #[cfg(target_os = "android")]
    {
        let _ = spec;
        return TrayStartup::Unavailable {
            reason: "android-host-has-no-system-tray".to_string(),
        };
    }
    #[cfg(all(unix, not(target_os = "macos"), not(target_os = "android")))]
    {
        ksni_backend::spawn_ksni(spec)
    }
    #[cfg(any(windows, target_os = "macos"))]
    {
        return native::spawn_native(spec);
    }
}

/// The host-owned system tray. One instance per desktop host.
pub struct DesktopTrayHost {
    controller: Option<Box<dyn TrayController>>,
    events: Receiver<TrayEvent>,
    support: TraySupport,
}

impl DesktopTrayHost {
    /// Start the platform tray with an initial spec. A failed spawn returns
    /// the typed degradation so the host stays window-only.
    pub fn start(spec: TraySpec, color_mode: TrayIconColorMode) -> Result<Self, TrayUnavailable> {
        match spawn_tray(spec) {
            TrayStartup::Ready { controller, events } => Ok(Self {
                controller: Some(controller),
                events,
                support: TraySupport::hosted_with_mode(true, color_mode),
            }),
            TrayStartup::Unavailable { reason } => Err(TrayUnavailable { reason }),
        }
    }

    /// The capability this host honestly offers, published as-is.
    pub fn support(&self) -> TraySupport {
        self.support
    }

    /// Push a full menu refresh to the backend. Best-effort.
    pub fn push(&self, spec: TraySpec) {
        if let Some(controller) = self.controller.as_ref() {
            controller.update_spec(spec);
        }
    }

    /// Drain every pending activation without blocking.
    pub fn drain(&self) -> Vec<TrayEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.events.try_recv() {
            events.push(event);
        }
        events
    }
}

impl Drop for DesktopTrayHost {
    fn drop(&mut self) {
        if let Some(mut controller) = self.controller.take() {
            controller.shutdown();
        }
    }
}

#[cfg(test)]
#[path = "tray/host_tests.rs"]
mod host_tests;
