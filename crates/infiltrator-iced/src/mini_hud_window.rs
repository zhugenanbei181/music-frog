//! Iced window-system projection for the Mini HUD (DUAL-15-03/04).
//!
//! The placement *contract* is shared; this module is the Iced desktop host
//! half: it resizes the single window down to the 260x90 HUD, moves it to the
//! persisted coordinates, and levels it always-on-top. On expand it restores
//! the main window geometry. Every task is a no-op while the host window id
//! has not been resolved yet.
//!
//! Since DUAL-15-04 the same window is also exposed to the desktop host port
//! as a [`MiniHudWindowHandle`]: the shared application facade hands the final
//! placement to the host, the handle accepts it only while the window is live,
//! and [`host_requests_task`] turns the accepted requests into real Iced window
//! tasks on the update path. A host without a live window stays typed
//! unsupported instead of claiming the window moved.

use crate::types::message::Message;
use iced::window::{Id, Level};
use iced::{Point, Size, Task};
use infiltrator_contract::mini_hud::MiniHudPlacement;
use infiltrator_ports::mini_hud_window::MiniHudWindowHandle;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

/// The floating HUD's logical size (shared 260x90 contract).
pub const HUD_SIZE: (f32, f32) = (
    MiniHudPlacement::WINDOW_WIDTH as f32,
    MiniHudPlacement::WINDOW_HEIGHT as f32,
);

/// The main window's logical size restored on expand.
pub const MAIN_SIZE: (f32, f32) = (1180.0, 780.0);

/// The main window's minimum size restored on expand.
pub const MAIN_MIN_SIZE: (f32, f32) = (420.0, 560.0);

#[derive(Default)]
struct HandleState {
    live: AtomicBool,
    pending: Mutex<Vec<MiniHudPlacement>>,
}

/// The Iced shell's [`MiniHudWindowHandle`]: it knows whether the host window
/// is live and queues accepted placements for the update path.
pub struct IcedMiniHudWindowHandle {
    state: Arc<HandleState>,
}

static HOST_HANDLE: OnceLock<Arc<IcedMiniHudWindowHandle>> = OnceLock::new();

impl IcedMiniHudWindowHandle {
    /// The process-wide handle the desktop port adapter is bound to.
    pub fn host() -> Arc<Self> {
        Arc::clone(HOST_HANDLE.get_or_init(|| Arc::new(Self::default())))
    }

    /// Record whether the host window is resolved and alive.
    pub fn mark_live(&self, live: bool) {
        self.state.live.store(live, Ordering::SeqCst);
    }

    /// Take the placements the host port accepted while the window was live.
    pub fn take_pending(&self) -> Vec<MiniHudPlacement> {
        match self.state.pending.lock() {
            Ok(mut pending) => std::mem::take(&mut *pending),
            Err(_) => Vec::new(),
        }
    }
}

impl Default for IcedMiniHudWindowHandle {
    fn default() -> Self {
        Self {
            state: Arc::new(HandleState::default()),
        }
    }
}

impl MiniHudWindowHandle for IcedMiniHudWindowHandle {
    fn apply_placement(&self, placement: MiniHudPlacement) -> bool {
        if !self.state.live.load(Ordering::SeqCst) {
            return false;
        }
        match self.state.pending.lock() {
            Ok(mut pending) => {
                pending.push(placement);
                true
            }
            Err(_) => false,
        }
    }

    fn set_visible(&self, _visible: bool) -> bool {
        // Single-window host: the HUD is the main window in HUD mode, so there
        // is no independent floating window to show or hide. Stay honest.
        false
    }
}

/// Bind the Iced handle into the desktop host port adapter. Called by the
/// composition constructor; binding is idempotent.
pub fn install_host_handle() -> Arc<IcedMiniHudWindowHandle> {
    let handle = IcedMiniHudWindowHandle::host();
    crate::host::mini_hud::bind_window_handle(handle.clone());
    handle
}

/// Enter HUD mode: shrink, move to the persisted placement, level on top.
pub fn enter(window: Option<Id>, placement: MiniHudPlacement) -> Task<Message> {
    let Some(id) = window else {
        return Task::none();
    };
    Task::batch(vec![
        iced::window::set_resizable(id, false),
        iced::window::set_min_size(id, Some(Size::new(HUD_SIZE.0, HUD_SIZE.1))),
        iced::window::resize(id, Size::new(HUD_SIZE.0, HUD_SIZE.1)),
        move_to(window, placement),
        set_level(window, placement.pinned),
    ])
}

/// Leave HUD mode: restore the main window's geometry and normal level.
pub fn exit(window: Option<Id>) -> Task<Message> {
    let Some(id) = window else {
        return Task::none();
    };
    Task::batch(vec![
        iced::window::set_min_size(id, Some(Size::new(MAIN_MIN_SIZE.0, MAIN_MIN_SIZE.1))),
        iced::window::resize(id, Size::new(MAIN_SIZE.0, MAIN_SIZE.1)),
        iced::window::set_resizable(id, true),
        iced::window::set_level(id, Level::Normal),
    ])
}

/// Move the window to the persisted placement.
pub fn move_to(window: Option<Id>, placement: MiniHudPlacement) -> Task<Message> {
    match window {
        Some(id) => iced::window::move_to(id, Point::new(placement.x as f32, placement.y as f32)),
        None => Task::none(),
    }
}

/// Apply the always-on-top level (only meaningful while the HUD is mounted).
pub fn set_level(window: Option<Id>, pinned: bool) -> Task<Message> {
    match window {
        Some(id) => iced::window::set_level(
            id,
            if pinned {
                Level::AlwaysOnTop
            } else {
                Level::Normal
            },
        ),
        None => Task::none(),
    }
}

/// Turn every placement the desktop host port accepted into the real Iced
/// window tasks (move + always-on-top level). Called after a placement write
/// completes, so the host window follows the authoritative persisted value.
pub fn host_requests_task(window: Option<Id>) -> Task<Message> {
    // Drain unconditionally: a request that cannot reach a window (id not
    // resolved) is dropped, never replayed later.
    let pending = IcedMiniHudWindowHandle::host().take_pending();
    let Some(id) = window else {
        return Task::none();
    };
    if pending.is_empty() {
        return Task::none();
    }
    Task::batch(
        pending
            .into_iter()
            .flat_map(|placement| {
                [
                    iced::window::move_to(id, Point::new(placement.x as f32, placement.y as f32)),
                    iced::window::set_level(
                        id,
                        if placement.pinned {
                            Level::AlwaysOnTop
                        } else {
                            Level::Normal
                        },
                    ),
                ]
            })
            .collect::<Vec<_>>(),
    )
}

#[cfg(test)]
#[path = "../tests/gui/mini_hud_window_tests.rs"]
mod mini_hud_window_tests;
