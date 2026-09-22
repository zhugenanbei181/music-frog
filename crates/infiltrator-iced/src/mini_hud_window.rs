//! Iced window-system projection for the Mini HUD (DUAL-15-03/04).
//!
//! The placement *contract* is shared; this module is the Iced desktop host
//! half: it resizes the single window down to the 260x90 HUD, moves it to the
//! persisted coordinates, and levels it always-on-top. On expand it restores
//! the main window geometry. Every task is a no-op while the host window id
//! has not been resolved yet.

use crate::types::message::Message;
use iced::window::{Id, Level};
use iced::{Point, Size, Task};
use infiltrator_contract::mini_hud::MiniHudPlacement;

/// The floating HUD's logical size (shared 260x90 contract).
pub const HUD_SIZE: (f32, f32) = (
    MiniHudPlacement::WINDOW_WIDTH as f32,
    MiniHudPlacement::WINDOW_HEIGHT as f32,
);

/// The main window's logical size restored on expand.
pub const MAIN_SIZE: (f32, f32) = (1180.0, 780.0);

/// The main window's minimum size restored on expand.
pub const MAIN_MIN_SIZE: (f32, f32) = (420.0, 560.0);

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
