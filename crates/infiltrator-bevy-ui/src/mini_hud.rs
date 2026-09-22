//! Mini Speed HUD floating desktop scene for Bevy UI (260x90 桌面悬浮小窗).
//!
//! The scene renders the shared `infiltrator_contract::mini_hud` read model:
//! duplex rates, the real exit-node pill, the shared toggle state letters and
//! the proxy-mode chip. Runtime wiring (mount/unmount, toggle event, pin
//! persistence) lives in [`crate::mini_hud_shell`].

use bevy::color::{Alpha, Color};
use bevy::ecs::component::Component;
use bevy::ecs::event::Event;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::resource::Resource;
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderColor, BorderRadius, FlexDirection, JustifyContent, Node,
    PositionType, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Button;
use infiltrator_bevy_widgets::button::{ButtonDisabled, pill_caption_scene};
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::mini_hud::MiniHudReadModel;
use infiltrator_contract::system_toggle::SystemToggle;

/// Toggle state for Mini HUD mode.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MiniHudMode(pub bool);

/// The Bevy mirror of the shared read model, rebuilt each frame from the live
/// projections (traffic, active exit, shared system toggles, placement).
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct MiniHudModel(pub MiniHudReadModel);

/// Toggle the Mini HUD visibility (the `Ctrl+Alt+M` shortcut and the palette
/// row both trigger this).
#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToggleMiniHud;

/// Pin/unpin the Mini HUD (always-on-top), persisting through the shared
/// settings command.
#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SetMiniHudPinned(pub bool);

/// Marker for the Mini HUD scene root.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MiniHudRoot;

/// Marker for the expand button restoring the main window.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MiniHudExpandButton;

/// Marker for the pin button (always on top).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MiniHudPinButton;

/// Marker on the mode chip text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MiniHudModeLabel;

/// Marker on the exit-node pill text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MiniHudNodeLabel;

/// Marker on the shared toggle-state line.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MiniHudToggleLabel;

/// Marker on the HUD's system-proxy quick switch (same shared command path as
/// the sidebar switch).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MiniHudSystemProxyToggle;

/// Marker on the HUD's TUN quick switch.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MiniHudTunToggle;

/// Format a byte/second rate with shared product units (B/KB/MB/GB).
pub fn format_rate(bytes_per_second: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes_per_second >= GB {
        format!("{:.2} GB/s", bytes_per_second as f64 / GB as f64)
    } else if bytes_per_second >= MB {
        format!("{:.2} MB/s", bytes_per_second as f64 / MB as f64)
    } else if bytes_per_second >= KB {
        format!("{:.2} KB/s", bytes_per_second as f64 / KB as f64)
    } else {
        format!("{bytes_per_second} B/s")
    }
}

/// Mini HUD scene: compact 260x90 dashboard, every value from the shared read
/// model (no baked mode/status literals).
pub fn mini_hud_scene(model: &MiniHudReadModel, palette: &UiPalette) -> impl Scene + use<> {
    let up_rate = format_rate(model.up_bytes_per_sec);
    let down_rate = format_rate(model.down_bytes_per_sec);
    let mode_label = model.mode_zh.clone();
    let node_label = if model.exit_node.is_empty() {
        "未选择出口节点".to_owned()
    } else {
        model.exit_node.clone()
    };
    let toggle_line = model.status_line();
    let pinned = model.placement.pinned;
    let pin_label = if pinned { "取消置顶" } else { "置顶" };
    // The two quick switches use the same shared action rule as the sidebar:
    // a pending/unknown toggle offers no press (no fabricated transition).
    let proxy_actionable = model.next_value(SystemToggle::SystemProxy).is_some();
    let tun_actionable = model.next_value(SystemToggle::Tun).is_some();
    let proxy_label = model.system_proxy.compact_label().to_owned();
    let proxy_selected = model.system_proxy.is_enabled();
    let tun_label = model.tun.compact_label().to_owned();
    let tun_selected = model.tun.is_enabled();
    let edge = palette.border;
    let scrim = Color::NONE;

    bsn! {
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
        }
        BackgroundColor(scrim)
        MiniHudRoot
        Children [
            (
                Node {
                    width: px(280.0),
                    height: px(112.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S4),
                    padding: UiRect::all(Val::Px(space::S12)),
                    border: UiRect::all(Val::Px(palette.hairline_px)),
                    border_radius: BorderRadius::all(Val::Px(palette.card_radius_px)),
                }
                BackgroundColor({ palette.surface })
                BorderColor { top: edge, right: edge, bottom: edge, left: edge }
                Children [
                    // Header: title, real mode chip, pin and expand actions
                    (
                        Node {
                            width: percent(100),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::SpaceBetween,
                            padding: UiRect::bottom(Val::Px(space::S4)),
                        }
                        Children [
                            (
                                Node {
                                    align_items: AlignItems::Center,
                                    column_gap: Val::Px(space::S6),
                                }
                                Children [
                                    ( { icon_tile_scene(IconId::Activity, 16.0, palette) } ),
                                    ( Text({ "Mini HUD".to_owned() }) TextRole(Role::Caption) ),
                                    (
                                        Node {
                                            padding: UiRect::axes(Val::Px(space::S6), Val::Px(2.0)),
                                            border_radius: BorderRadius::all(Val::Px(4.0)),
                                        }
                                        BackgroundColor({ palette.surface_elevated })
                                        Children [
                                            (
                                                Text({ mode_label })
                                                TextRole(Role::Caption)
                                                MiniHudModeLabel
                                            ),
                                        ]
                                    ),
                                ]
                            ),
                            (
                                Node {
                                    align_items: AlignItems::Center,
                                    column_gap: Val::Px(space::S4),
                                }
                                Children [
                                    (
                                        Node {
                                            min_height: px(20.0),
                                            padding: UiRect::horizontal(Val::Px(space::S6)),
                                            align_items: AlignItems::Center,
                                            justify_content: JustifyContent::Center,
                                            border_radius: BorderRadius::all(Val::Px(4.0)),
                                            border: UiRect::all(Val::Px(1.0)),
                                        }
                                        BackgroundColor({ palette.surface_elevated })
                                        BorderColor { top: edge, right: edge, bottom: edge, left: edge }
                                        Button
                                        MiniHudPinButton
                                        Children [
                                            ( Text({ pin_label.to_owned() }) TextRole(Role::Caption) ),
                                        ]
                                    ),
                                    (
                                        Node {
                                            min_height: px(20.0),
                                            padding: UiRect::horizontal(Val::Px(space::S6)),
                                            align_items: AlignItems::Center,
                                            justify_content: JustifyContent::Center,
                                            border_radius: BorderRadius::all(Val::Px(4.0)),
                                        }
                                        BackgroundColor({ palette.accent })
                                        Button
                                        MiniHudExpandButton
                                        Children [
                                            ( Text({ "展开".to_owned() }) TextRole(Role::Caption) ),
                                        ]
                                    ),
                                ]
                            ),
                        ]
                    ),
                    // Dual-Channel Bandwidth Rates
                    (
                        Node {
                            width: percent(100),
                            flex_direction: FlexDirection::Row,
                            justify_content: JustifyContent::SpaceBetween,
                            align_items: AlignItems::Center,
                            padding: UiRect::vertical(Val::Px(space::S4)),
                        }
                        Children [
                            (
                                Node {
                                    align_items: AlignItems::Center,
                                    column_gap: Val::Px(space::S6),
                                }
                                Children [
                                    ( { icon_tile_scene(IconId::ArrowDown, 16.0, palette) } ),
                                    ( Text(down_rate) TextRole(Role::BodyStrong) ),
                                ]
                            ),
                            (
                                Node {
                                    align_items: AlignItems::Center,
                                    column_gap: Val::Px(space::S6),
                                }
                                Children [
                                    ( { icon_tile_scene(IconId::ArrowUp, 16.0, palette) } ),
                                    ( Text(up_rate) TextRole(Role::BodyStrong) ),
                                ]
                            ),
                        ]
                    ),
                    // Footer: real exit node pill + the shared quick switches
                    // and toggle-state line (same command path as the sidebar).
                    (
                        Node {
                            width: percent(100),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::SpaceBetween,
                            padding: UiRect::top(Val::Px(space::S4)),
                            column_gap: Val::Px(space::S6),
                        }
                        Children [
                            (
                                Node {
                                    padding: UiRect::axes(Val::Px(space::S8), Val::Px(2.0)),
                                    border_radius: BorderRadius::all(Val::Px(4.0)),
                                }
                                BackgroundColor({ palette.accent.with_alpha(0.14) })
                                Children [
                                    (
                                        Text(node_label)
                                        TextRole(Role::Caption)
                                        MiniHudNodeLabel
                                    ),
                                ]
                            ),
                            (
                                Node {
                                    align_items: AlignItems::Center,
                                    column_gap: Val::Px(space::S6),
                                }
                                Children [
                                    (
                                        { pill_caption_scene(proxy_label, proxy_selected, palette) }
                                        MiniHudSystemProxyToggle
                                        ButtonDisabled({ !proxy_actionable })
                                    ),
                                    (
                                        { pill_caption_scene(tun_label, tun_selected, palette) }
                                        MiniHudTunToggle
                                        ButtonDisabled({ !tun_actionable })
                                    ),
                                    (
                                        Text(toggle_line)
                                        TextRole(Role::Caption)
                                        MiniHudToggleLabel
                                    ),
                                ]
                            ),
                        ]
                    ),
                ]
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::MinimalPlugins;
    use bevy::app::App;
    use bevy::asset::{AssetApp, AssetPlugin};
    use bevy::image::Image;
    use bevy::scene::{CommandsSceneExt, ScenePlugin};
    use infiltrator_bevy_widgets::palette::UiPalette;
    use infiltrator_bevy_widgets::theme::Theme;
    use infiltrator_contract::system_toggle::SystemToggleSnapshot;

    fn model() -> MiniHudReadModel {
        let snapshot = SystemToggleSnapshot::from_legacy(true, Some(true), 2);
        MiniHudReadModel::default()
            .with_toggles(&snapshot)
            .with_traffic(120 * 1024, 2 * 1024 * 1024)
            .with_exit_node("HK-01")
            .with_mode("规则模式")
    }

    #[test]
    fn test_mini_hud_scene_mounting() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins((AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>();
        let theme = Theme::dark();
        let palette = UiPalette::new(&theme);

        let scene = mini_hud_scene(&model(), &palette);
        app.world_mut().commands().spawn_scene(scene);
        app.update();

        let world = app.world_mut();
        let hud_root_count = world.query::<&MiniHudRoot>().iter(world).count();
        assert_eq!(hud_root_count, 1, "MiniHudRoot must mount exactly once");

        let expand_count = world.query::<&MiniHudExpandButton>().iter(world).count();
        assert_eq!(expand_count, 1, "MiniHudExpandButton must mount");

        let pin_count = world.query::<&MiniHudPinButton>().iter(world).count();
        assert_eq!(pin_count, 1, "MiniHudPinButton must mount");
    }

    #[test]
    fn test_mini_hud_scene_renders_the_read_model() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins((AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>();
        let theme = Theme::dark();
        let palette = UiPalette::new(&theme);

        let scene = mini_hud_scene(&model(), &palette);
        app.world_mut().commands().spawn_scene(scene);
        app.update();

        let world = app.world_mut();
        let mut mode_labels = world.query::<(&Text, &MiniHudModeLabel)>();
        assert_eq!(
            mode_labels
                .iter(world)
                .next()
                .map(|(text, _)| text.0.clone()),
            Some("规则模式".to_owned())
        );
        let mut node_labels = world.query::<(&Text, &MiniHudNodeLabel)>();
        assert_eq!(
            node_labels
                .iter(world)
                .next()
                .map(|(text, _)| text.0.clone()),
            Some("HK-01".to_owned())
        );
        let mut toggle_labels = world.query::<(&Text, &MiniHudToggleLabel)>();
        assert_eq!(
            toggle_labels
                .iter(world)
                .next()
                .map(|(text, _)| text.0.clone()),
            Some("系统代理: 开 · TUN: 开".to_owned())
        );
    }

    #[test]
    fn rates_use_the_product_units() {
        assert_eq!(format_rate(0), "0 B/s");
        assert_eq!(format_rate(512), "512 B/s");
        assert_eq!(format_rate(120 * 1024), "120.00 KB/s");
        assert_eq!(format_rate(2 * 1024 * 1024), "2.00 MB/s");
    }
}
