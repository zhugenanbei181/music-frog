//! Mini HUD handlers (DUAL-15-03/04), split out of `update/ui.rs` so the UI
//! dispatcher keeps its line budget.
//!
//! The HUD is a single-window desktop mode on Iced: entering it shrinks and
//! moves the host window to the persisted placement, dragging moves the
//! mirror, and releasing runs the shared clamp/edge-snap pass through
//! `infiltrator_application::mini_hud_application`.

use crate::mini_hud_store::{place, set_pinned};
use crate::mini_hud_window::{
    IcedMiniHudWindowHandle, enter, exit, host_requests_task, move_to, set_level,
};
use crate::state::AppState;
use crate::state::shell::MiniHudDragAnchor;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use iced::Task;
use iced::window::monitor_size;
use infiltrator_contract::mini_hud::{MiniHudDisplay, MiniHudPlacement};

impl AppState {
    /// Handlers for the Mini HUD messages. Returns `None` when the message is
    /// not part of this domain, so the caller keeps its own arm chain
    /// exhaustive.
    pub(crate) fn update_mini_hud(&mut self, message: Message) -> Option<Task<Message>> {
        let task = match message {
            Message::ToggleMiniHudMode => {
                self.shell.mini_hud_mode = !self.shell.mini_hud_mode;
                if self.shell.mini_hud_mode {
                    let placement = self.shell.mini_hud_placement;
                    self.shell.always_on_top = placement.pinned;
                    Task::batch([
                        enter(self.shell.window_id, placement),
                        // Resolve the live monitor rectangle for clamp/snap.
                        match self.shell.window_id {
                            Some(id) => monitor_size(id).map(Message::MiniHudDisplayKnown),
                            None => Task::none(),
                        },
                    ])
                } else {
                    self.shell.always_on_top = false;
                    exit(self.shell.window_id)
                }
            }
            Message::SetAlwaysOnTop(v) => {
                self.shell.always_on_top = v;
                // Optimistic mirror so the pin button repaints immediately;
                // the persisted placement is the authoritative follow-up.
                self.shell.mini_hud_placement = self.shell.mini_hud_placement.with_pinned(v);
                let current = self.shell.mini_hud_placement;
                let in_hud_mode = self.shell.mini_hud_mode;
                Task::batch([
                    set_level(self.shell.window_id, v && in_hud_mode),
                    Task::perform(
                        async move {
                            set_pinned(current, v)
                                .await
                                .map_err(|error| error.to_string())
                        },
                        Message::MiniHudPlacementUpdated,
                    ),
                ])
            }
            Message::MiniHudMoved { x, y } => {
                // Track the pointer's widget-local anchor; the delta against
                // the placement captured at drag start moves the HUD mirror,
                // and the host window follows. Snapping/persisting happens on
                // release so a drag does not write the settings file per pixel.
                let anchor = self
                    .shell
                    .mini_hud_drag_anchor
                    .unwrap_or(MiniHudDragAnchor {
                        origin: (x, y),
                        placement: self.shell.mini_hud_placement,
                    });
                self.shell.mini_hud_drag_anchor = Some(anchor);
                let placement = MiniHudPlacement {
                    x: anchor.placement.x + (x - anchor.origin.0).round() as i32,
                    y: anchor.placement.y + (y - anchor.origin.1).round() as i32,
                    ..anchor.placement
                };
                self.shell.mini_hud_placement = placement;
                move_to(self.shell.window_id, placement)
            }
            Message::MiniHudDragReleased => {
                self.shell.mini_hud_drag_anchor = None;
                let placement = self.shell.mini_hud_placement;
                let display = self.shell.mini_hud_display;
                Task::perform(
                    async move {
                        // The host monitor rectangle is the real geometry; a
                        // host without one persists raw coordinates instead of
                        // inventing a display.
                        place(placement, placement.x, placement.y, display)
                            .await
                            .map_err(|error| error.to_string())
                    },
                    Message::MiniHudPlacementUpdated,
                )
            }
            Message::MiniHudPlacementUpdated(result) => match result {
                Ok(placement) => {
                    self.shell.mini_hud_placement = placement;
                    self.shell.always_on_top = placement.pinned;
                    // The desktop host port accepted (or refused) the placement
                    // while persisting; accepted requests become the real
                    // window tasks here, on the update path.
                    host_requests_task(self.shell.window_id)
                }
                Err(error) => self.push_toast(error, ToastStatus::Error),
            },
            Message::MiniHudDisplayKnown(size) => {
                self.shell.mini_hud_display = size
                    .map(|size| MiniHudDisplay::new(0, 0, size.width as u32, size.height as u32));
                Task::none()
            }
            Message::WindowIdResolved(id) => {
                self.shell.window_id = id;
                // The shared application's host port only accepts placement
                // requests while the host window is alive; the desktop adapter
                // answers typed unsupported until then.
                IcedMiniHudWindowHandle::host().mark_live(id.is_some());
                Task::none()
            }

            _ => return None,
        };
        Some(task)
    }
}
