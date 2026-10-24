//! Frameless window chrome for the Bevy shell (DUAL-15-13).
//!
//! Bevy 0.20 exposes a real OS window path: [`bevy::window::Window`]'s
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

#[cfg(not(target_os = "android"))]
use crate::a11y::semantic_node;
use bevy::app::{App, AppExit, Plugin, Startup};
#[cfg(not(target_os = "android"))]
use bevy::color::Color;
use bevy::ecs::component::Component;
#[cfg(not(target_os = "android"))]
use bevy::ecs::hierarchy::Children;
use bevy::ecs::message::MessageWriter;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res, ResMut};
#[cfg(not(target_os = "android"))]
use bevy::feathers::theme::ThemeBackgroundColor;
#[cfg(not(target_os = "android"))]
use bevy::feathers::theme::ThemeTextColor;
use bevy::picking::events::{PointerClick, PointerPress};
use bevy::scene::{Scene, bsn};
#[cfg(target_os = "android")]
use bevy::ui::prelude::Display;
use bevy::ui::prelude::Node;
#[cfg(not(target_os = "android"))]
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderColor, BorderRadius, FlexDirection, JustifyContent, UiRect,
    Val, percent, px,
};
#[cfg(not(target_os = "android"))]
use bevy::ui::widget::Text;
use bevy::ui_widgets::Activate;
#[cfg(not(target_os = "android"))]
use bevy::ui_widgets::Button;
use bevy::window::{PrimaryWindow, Window};
#[cfg(not(target_os = "android"))]
use infiltrator_bevy_widgets::button::{ButtonVariant, ButtonVariantStyle, ControlVisual};
use infiltrator_bevy_widgets::feathers_theme::Token;
#[cfg(not(target_os = "android"))]
use infiltrator_bevy_widgets::icon::{IconId, icon_scene_val};
use infiltrator_bevy_widgets::palette::UiPalette;
#[cfg(not(target_os = "android"))]
use infiltrator_bevy_widgets::palette::rem;
#[cfg(not(target_os = "android"))]
use infiltrator_bevy_widgets::responsive::TouchHitbox;
#[cfg(not(target_os = "android"))]
use infiltrator_bevy_widgets::text::{Role, TextRole};
#[cfg(not(target_os = "android"))]
use infiltrator_bevy_widgets::theme::space;
#[cfg(not(target_os = "android"))]
use infiltrator_contract::a11y::ShellA11yNode;
#[cfg(not(target_os = "android"))]
use infiltrator_contract::window_chrome::CHROME_DRAG_STRIP_HEIGHT_PX;
use infiltrator_contract::window_chrome::{NativeShadow, WindowChrome, WindowChromeSupport};

/// The window control's edge in baseline pixels.
#[cfg(not(target_os = "android"))]
const CHROME_CONTROL_PX: f32 = 28.0;

/// The window key's plate box, in baseline pixels.
///
/// The plate is smaller than the key itself: the key is a target, the plate
/// is the mark inside it.
#[cfg(not(target_os = "android"))]
const CHROME_CONTROL_ICON_PX: f32 = 14.0;

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
    #[cfg(target_os = "android")]
    {
        WindowChromeSupport::Unsupported {
            reason: "mobile-surface-has-no-window-chrome",
        }
    }
    #[cfg(not(target_os = "android"))]
    {
        WindowChromeSupport::Hosted {
            drag: true,
            minimize: true,
            maximize: true,
            native_shadow: NativeShadow::Absent,
        }
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
            .init_resource::<ChromeMaximizeLatch>();
        #[cfg(not(target_os = "android"))]
        {
            if support().is_hosted() {
                app.add_systems(Startup, apply_window_chrome)
                    .add_observer(on_drag_bar_pressed)
                    .add_observer(on_drag_bar_clicked)
                    .add_observer(on_minimize_activated)
                    .add_observer(on_maximize_activated)
                    .add_observer(on_close_activated);
            }
        }
    }
}

/// Apply the shared chrome style to the shell window, but only when the
/// capability report says this host really runs it. Only the primary window is
/// touched: the Mini HUD is an in-window overlay, not a second OS window.
#[cfg(not(target_os = "android"))]
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
#[cfg(not(target_os = "android"))]
fn on_drag_bar_pressed(
    press: On<PointerPress>,
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
#[cfg(not(target_os = "android"))]
fn on_drag_bar_clicked(
    click: On<PointerClick>,
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

#[cfg(not(target_os = "android"))]
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

#[cfg(not(target_os = "android"))]
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

#[cfg(not(target_os = "android"))]
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
    #[cfg(target_os = "android")]
    {
        let _ = palette;
        bsn! {
            Node { display: Display::None }
        }
    }
    #[cfg(not(target_os = "android"))]
    {
        let edge = palette.border;
        bsn! {
            Node {
                width: percent(100),
                height: px(CHROME_DRAG_STRIP_HEIGHT_PX as f32),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                padding: UiRect::horizontal(rem(space::S12)),
                column_gap: rem(space::S8),
                border: UiRect::bottom(Val::Px(palette.hairline_px)),
            }
            ThemeBackgroundColor({ Token::Sidebar })
            BorderColor {
                top: Color::NONE,
                right: Color::NONE,
                bottom: edge,
                left: Color::NONE,
            }
            ChromeDragBar
            Children [
                Text({ "MusicFrog Infiltrator".to_owned() })
                TextRole(Role::Caption)
                ThemeTextColor({ Token::InkDim })
                --
                Node { flex_grow: 1.0 }
                --
                @{ chrome_controls_scene(palette) }
            ]
        }
    }
}

/// The three window controls as one group.
///
/// The group is a single surface with a hairline edge, and the controls
/// inside it are ghost buttons: one object, three targets. Each control
/// carries its own plate — the embedded faces carry no decorative Unicode,
/// so a "✕" glyph would be tofu.
#[cfg(not(target_os = "android"))]
fn chrome_controls_scene(palette: &UiPalette) -> impl Scene + use<> {
    let minimize_node = semantic_node(ShellA11yNode::ChromeMinimize);
    let maximize_node = semantic_node(ShellA11yNode::ChromeMaximize);
    let close_node = semantic_node(ShellA11yNode::ChromeClose);
    let edge = palette.border;
    bsn! {
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: rem(space::S2),
                padding: UiRect::all(rem(space::S2)),
                border: UiRect::all(Val::Px(palette.hairline_px)),
                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
            }
            BackgroundColor({ palette.surface_elevated })
            BorderColor { top: edge, right: edge, bottom: edge, left: edge }
            Children [
                @{ chrome_control_button_scene(IconId::Minus, palette) }
                ChromeMinimizeButton
                minimize_node
                --
                @{ chrome_control_button_scene(IconId::Square, palette) }
                ChromeMaximizeButton
                maximize_node
                --
                @{ chrome_control_button_scene(IconId::Close, palette) }
                ChromeCloseButton
                close_node
            ]
    }
}

/// One window control: a ghost button that only paints on hover.
#[cfg(not(target_os = "android"))]
fn chrome_control_button_scene(icon: IconId, palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
            Node {
                width: rem(CHROME_CONTROL_PX),
                height: rem(CHROME_CONTROL_PX),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
            }
            BackgroundColor({ Color::NONE })
            Button
            ButtonVariantStyle(ButtonVariant::Ghost)
            ControlVisual(false)
            TouchHitbox::default()
            Children [
                @{ icon_scene_val(icon, rem(CHROME_CONTROL_ICON_PX), palette.ink_dim) }
            ]
    }
}
