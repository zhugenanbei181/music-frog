//! Mini HUD handlers (DUAL-15-03/04), split out of `update/ui.rs` so the UI
//! dispatcher keeps its line budget.
//!
//! The HUD is a single-window desktop mode on Iced: entering it shrinks and
//! moves the host window to the persisted placement, dragging moves the
//! mirror, and releasing runs the shared clamp/edge-snap pass through
//! `infiltrator_application::mini_hud_application`.

use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use iced::Task;

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
                        crate::mini_hud_window::enter(self.shell.window_id, placement),
                        // Resolve the live monitor rectangle for clamp/snap.
                        match self.shell.window_id {
                            Some(id) => {
                                iced::window::monitor_size(id).map(Message::MiniHudDisplayKnown)
                            }
                            None => Task::none(),
                        },
                    ])
                } else {
                    self.shell.always_on_top = false;
                    crate::mini_hud_window::exit(self.shell.window_id)
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
                    crate::mini_hud_window::set_level(self.shell.window_id, v && in_hud_mode),
                    Task::perform(
                        async move {
                            crate::mini_hud_store::set_pinned(current, v)
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
                let anchor =
                    self.shell
                        .mini_hud_drag_anchor
                        .unwrap_or(crate::state::MiniHudDragAnchor {
                            origin: (x, y),
                            placement: self.shell.mini_hud_placement,
                        });
                self.shell.mini_hud_drag_anchor = Some(anchor);
                let placement = infiltrator_contract::mini_hud::MiniHudPlacement {
                    x: anchor.placement.x + (x - anchor.origin.0).round() as i32,
                    y: anchor.placement.y + (y - anchor.origin.1).round() as i32,
                    ..anchor.placement
                };
                self.shell.mini_hud_placement = placement;
                crate::mini_hud_window::move_to(self.shell.window_id, placement)
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
                        crate::mini_hud_store::place(placement, placement.x, placement.y, display)
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
                    Task::none()
                }
                Err(error) => self.push_toast(error, ToastStatus::Error),
            },
            Message::MiniHudDisplayKnown(size) => {
                self.shell.mini_hud_display = size.map(|size| {
                    infiltrator_contract::mini_hud::MiniHudDisplay::new(
                        0,
                        0,
                        size.width as u32,
                        size.height as u32,
                    )
                });
                Task::none()
            }
            Message::WindowIdResolved(id) => {
                self.shell.window_id = id;
                Task::none()
            }

            _ => return None,
        };
        Some(task)
    }
}
