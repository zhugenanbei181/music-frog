//! BEVY-029: PiP / secondary-window session controls for the Mini HUD.
//!
//! The Mini HUD owns an OS-level picture-in-picture overlay. This module binds
//! the three placement controls (always-on-top pin, pointer click-through and
//! corner snap) to the shared [`PipOverlaySession`] state machine and its
//! bounded [`WindowRegistry`]. The scene carries typed markers only; the
//! observers mutate the session and mirror every flag onto the descriptor, so
//! the HUD and the native window can never disagree.

use crate::mini_hud::{
    MiniHudMode, MiniHudPipClickThroughButton, MiniHudPipPinButton, MiniHudPipReadout,
    MiniHudPipSnapButton,
};
use bevy::app::{App, Plugin, Update};
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::math::Vec2;
use bevy::ui::widget::Text;
use bevy::ui_widgets::Activate;
use infiltrator_bevy_widgets::windowing::{PipCorner, PipOverlaySession, WindowRegistry};

/// Title carried by the PiP secondary window descriptor.
pub const PIP_WINDOW_TITLE: &str = "MusicFrog PiP";

/// Logical screen size the PiP overlay snaps within; a host may update it.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct PipScreenSize(pub Vec2);

impl Default for PipScreenSize {
    fn default() -> Self {
        Self(Vec2::new(1920.0, 1080.0))
    }
}

/// Deterministic snap cycle used by the HUD snap control.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PipSnapCursor(pub PipCorner);

impl Default for PipSnapCursor {
    fn default() -> Self {
        Self(PipCorner::TopRight)
    }
}

impl PipSnapCursor {
    /// Advance to the next corner in a stable clockwise order.
    pub fn advance(&mut self) -> PipCorner {
        self.0 = match self.0 {
            PipCorner::TopRight => PipCorner::BottomRight,
            PipCorner::BottomRight => PipCorner::BottomLeft,
            PipCorner::BottomLeft => PipCorner::TopLeft,
            PipCorner::TopLeft => PipCorner::TopRight,
        };
        self.0
    }
}

fn corner_label(corner: PipCorner) -> &'static str {
    match corner {
        PipCorner::TopLeft => "top-left",
        PipCorner::TopRight => "top-right",
        PipCorner::BottomLeft => "bottom-left",
        PipCorner::BottomRight => "bottom-right",
    }
}

/// Open the PiP window while the HUD is visible and retire it when hidden.
///
/// The session is idempotent: an already-open window keeps its descriptor.
pub fn sync_pip_session(
    mode: Res<MiniHudMode>,
    mut session: ResMut<PipOverlaySession>,
    mut registry: ResMut<WindowRegistry>,
) {
    if mode.0 {
        let _ = session.open(&mut registry, PIP_WINDOW_TITLE);
    } else if session.window.is_some() {
        let _ = session.close(&mut registry);
    }
}

/// Observer: the PiP always-on-top control mirrors onto the descriptor.
pub fn on_pip_pin_activated(
    activate: On<Activate>,
    buttons: Query<(), With<MiniHudPipPinButton>>,
    mut session: ResMut<PipOverlaySession>,
    mut registry: ResMut<WindowRegistry>,
) {
    if buttons.get(activate.entity).is_ok() {
        session.toggle_pin(&mut registry);
    }
}

/// Observer: the PiP click-through control mirrors onto the descriptor.
pub fn on_pip_click_through_activated(
    activate: On<Activate>,
    buttons: Query<(), With<MiniHudPipClickThroughButton>>,
    mut session: ResMut<PipOverlaySession>,
    mut registry: ResMut<WindowRegistry>,
) {
    if buttons.get(activate.entity).is_ok() {
        session.toggle_click_through(&mut registry);
    }
}

/// Observer: the PiP snap control advances to the next corner and moves the
/// overlay (and its descriptor) to it.
pub fn on_pip_snap_activated(
    activate: On<Activate>,
    buttons: Query<(), With<MiniHudPipSnapButton>>,
    screen: Res<PipScreenSize>,
    mut cursor: ResMut<PipSnapCursor>,
    mut session: ResMut<PipOverlaySession>,
    mut registry: ResMut<WindowRegistry>,
) {
    if buttons.get(activate.entity).is_err() {
        return;
    }
    let corner = cursor.advance();
    session.snap(&mut registry, screen.0, corner);
}

/// Restamp the mounted PiP readout from the live session state.
pub fn refresh_pip_readout(
    session: Res<PipOverlaySession>,
    cursor: Res<PipSnapCursor>,
    readouts: Query<(&Children, &MiniHudPipReadout)>,
    mut texts: Query<&mut Text>,
) {
    let state = session.state;
    let value = format!(
        "PiP: {} · {} · {}",
        if state.is_pinned_top {
            "on-top"
        } else {
            "normal"
        },
        if state.is_click_through {
            "click-through"
        } else {
            "interactive"
        },
        corner_label(cursor.0),
    );
    for (children, _) in &readouts {
        for child in children {
            if let Ok(mut text) = texts.get_mut(*child)
                && text.0 != value
            {
                text.0 = value.clone();
            }
        }
    }
}

/// The Mini HUD PiP session plugin: state machine, descriptor registry and
/// the three placement controls.
pub struct PipHudPlugin;

impl Plugin for PipHudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PipOverlaySession>();
        app.init_resource::<WindowRegistry>();
        app.init_resource::<PipScreenSize>();
        app.init_resource::<PipSnapCursor>();
        app.add_observer(on_pip_pin_activated);
        app.add_observer(on_pip_click_through_activated);
        app.add_observer(on_pip_snap_activated);
        app.add_systems(Update, (sync_pip_session, refresh_pip_readout).chain());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_bevy_widgets::windowing::PipOverlayState;

    #[test]
    fn snap_cursor_cycles_all_four_corners() {
        let mut cursor = PipSnapCursor::default();
        assert_eq!(cursor.0, PipCorner::TopRight);
        assert_eq!(cursor.advance(), PipCorner::BottomRight);
        assert_eq!(cursor.advance(), PipCorner::BottomLeft);
        assert_eq!(cursor.advance(), PipCorner::TopLeft);
        assert_eq!(cursor.advance(), PipCorner::TopRight);
    }

    #[test]
    fn session_sync_opens_and_retires_the_descriptor() {
        let mut registry = WindowRegistry::new();
        let mut session = PipOverlaySession::new(PipOverlayState::default());
        let id = session
            .open(&mut registry, PIP_WINDOW_TITLE)
            .expect("opened");
        assert!(registry.contains(id));
        session.close(&mut registry).expect("closed");
        assert!(registry.is_empty());
    }
}
