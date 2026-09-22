//! Frameless window chrome for the Bevy shell (DUAL-15-13).
//!
//! Bevy 0.19 exposes a real OS window path: [`bevy::window::Window`]'s
//! `start_drag_move` reaches `winit::Window::drag_window`, `set_minimized` /
//! `set_maximized` reach their winit counterparts, and `decorations` is a
//! real (runtime-toggleable) flag. The shell therefore runs frameless with a
//! mounted chrome bar:
//!
//! * the bar is a pickable node: a press starts the OS drag move, a double
//!   click toggles maximize;
//! * the three controls dispatch minimize / maximize / close through Bevy's
//!   own window API;
//! * the shape (height, double-click rule, shadow availability) is the shared
//!   `infiltrator_contract::window_chrome` shape, and the capability report is
//!   what this host honestly implements.

use bevy::app::{App, AppExit, Plugin, Startup};
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::message::MessageWriter;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::picking::events::{Click, Pointer, Press};
use bevy::scene::{Scene, bsn, template_value};
use bevy::text::TextColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, FlexDirection, Node, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Activate;
use bevy::window::{PrimaryWindow, Window};
use infiltrator_bevy_widgets::button::pill_caption_scene;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::a11y::ShellA11yNode;
use infiltrator_contract::window_chrome::{
    CHROME_DRAG_STRIP_HEIGHT_PX, NativeShadow, WindowChrome, WindowChromeSupport,
};

/// Marker on the draggable chrome bar.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChromeDragBar;

/// Marker on the minimize control.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChromeMinimizeButton;

/// Marker on the maximize/restore control.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChromeMaximizeButton;

/// Marker on the close control.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChromeCloseButton;

/// The shared chrome shape this surface runs.
pub const fn chrome_shape() -> WindowChrome {
    WindowChrome::FRAMELESS
}

/// What this host can actually do with the shared chrome shape.
///
/// `native_shadow` stays `Absent`: Bevy exposes no shadow knob, so no custom
/// shadow is requested on the frameless window's behalf.
pub const fn support() -> WindowChromeSupport {
    WindowChromeSupport::Hosted {
        drag: true,
        minimize: true,
        maximize: true,
        native_shadow: NativeShadow::Absent,
    }
}

/// Queryable capability report, inserted by [`WindowChromePlugin`]. The startup
/// system reads it before touching the window, so the surface never applies a
/// chrome style it does not claim to host.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowChromeReport(pub WindowChromeSupport);

impl Default for WindowChromeReport {
    fn default() -> Self {
        Self(support())
    }
}

/// The last maximize request this surface issued.
///
/// Bevy does not expose the OS maximize state (winit reports no maximize
/// event), so the double-click toggle is decided from this latch. A
/// WM-initiated change can make it stale; the surface only ever toggles its
/// own requests and never pretends to know the OS state.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChromeMaximizeLatch(pub bool);

/// Mounts the frameless chrome bar and wires its drag/minimize/maximize/close
/// actions to the real window API.
pub struct WindowChromePlugin;

impl Plugin for WindowChromePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WindowChromeReport>()
            .init_resource::<ChromeMaximizeLatch>()
            .add_systems(Startup, apply_window_chrome)
            .add_observer(on_drag_bar_pressed)
            .add_observer(on_drag_bar_clicked)
            .add_observer(on_minimize_activated)
            .add_observer(on_maximize_activated)
            .add_observer(on_close_activated);
    }
}

/// Apply the shared chrome style to the shell window, but only when the
/// capability report says this host really runs it. Only the primary window is
/// touched: the Mini HUD is an in-window overlay, not a second OS window.
fn apply_window_chrome(
    report: Res<WindowChromeReport>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    if !report.0.is_hosted() {
        return;
    }
    let chrome = chrome_shape();
    for mut window in &mut windows {
        window.decorations = chrome.os_decorations();
    }
}

/// Press on the bar starts the OS drag move (winit's `drag_window`).
fn on_drag_bar_pressed(
    press: On<Pointer<Press>>,
    bars: Query<(), With<ChromeDragBar>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    if !bars.contains(press.entity) {
        return;
    }
    if let Ok(mut window) = windows.single_mut() {
        window.start_drag_move();
    }
}

/// A double click on the bar toggles maximize.
fn on_drag_bar_clicked(
    click: On<Pointer<Click>>,
    bars: Query<(), With<ChromeDragBar>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut latch: ResMut<ChromeMaximizeLatch>,
) {
    if click.count != 2 || !bars.contains(click.entity) {
        return;
    }
    if let Ok(mut window) = windows.single_mut() {
        let next = !latch.0;
        window.set_maximized(next);
        latch.0 = next;
    }
}

fn on_minimize_activated(
    activate: On<Activate>,
    buttons: Query<(), With<ChromeMinimizeButton>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    if !buttons.contains(activate.entity) {
        return;
    }
    if let Ok(mut window) = windows.single_mut() {
        window.set_minimized(true);
    }
}

fn on_maximize_activated(
    activate: On<Activate>,
    buttons: Query<(), With<ChromeMaximizeButton>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut latch: ResMut<ChromeMaximizeLatch>,
) {
    if !buttons.contains(activate.entity) {
        return;
    }
    if let Ok(mut window) = windows.single_mut() {
        let next = !latch.0;
        window.set_maximized(next);
        latch.0 = next;
    }
}

fn on_close_activated(
    activate: On<Activate>,
    buttons: Query<(), With<ChromeCloseButton>>,
    mut exits: MessageWriter<AppExit>,
) {
    if buttons.contains(activate.entity) {
        exits.write(AppExit::Success);
    }
}

/// The chrome bar mounted above the shell: a draggable title strip plus the
/// three window controls.
pub fn chrome_bar_scene(palette: &UiPalette) -> impl Scene + use<> {
    let minimize_node = crate::a11y::semantic_node(ShellA11yNode::ChromeMinimize);
    let maximize_node = crate::a11y::semantic_node(ShellA11yNode::ChromeMaximize);
    let close_node = crate::a11y::semantic_node(ShellA11yNode::ChromeClose);
    bsn! {
        Node {
            width: percent(100),
            height: px(CHROME_DRAG_STRIP_HEIGHT_PX as f32),
            flex_shrink: 0.0,
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            padding: UiRect::horizontal(Val::Px(space::S12)),
            column_gap: Val::Px(space::S8),
        }
        BackgroundColor({ palette.sidebar })
        ChromeDragBar
        Children [
            (
                Text({ "MusicFrog Infiltrator".to_owned() })
                TextRole(Role::Caption)
                TextColor({ palette.ink_dim })
            ),
            ( Node { flex_grow: 1.0 } ),
            (
                { pill_caption_scene("–".to_owned(), false, palette) }
                ChromeMinimizeButton
                template_value(minimize_node)
            ),
            (
                { pill_caption_scene("▢".to_owned(), false, palette) }
                ChromeMaximizeButton
                template_value(maximize_node)
            ),
            (
                { pill_caption_scene("✕".to_owned(), false, palette) }
                ChromeCloseButton
                template_value(close_node)
            ),
        ]
    }
}
