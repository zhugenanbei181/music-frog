//! Mini Speed HUD floating desktop scene for Bevy UI (260x90 桌面悬浮小窗).
//!
//! The scene renders the shared `infiltrator_contract::mini_hud` read model:
//! duplex rates, the real exit-node pill, the shared toggle state letters and
//! the shared routing intent chip. Runtime wiring (mount/unmount, toggle event, pin
//! persistence) lives in [`crate::mini_hud_shell`].

use crate::a11y::{semantic_node, switch_node};
use bevy::color::{Alpha, Color};
use bevy::ecs::component::Component;
use bevy::ecs::event::Event;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::resource::Resource;
use bevy::feathers::theme::{SurfaceLevel, ThemeBackgroundColor, ThemeContext};
use bevy::scene::{Scene, bsn};
use bevy::ui::FixedNode;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderColor, BorderRadius, FlexDirection, JustifyContent, Node,
    Overflow, PositionType, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Button;
use infiltrator_bevy_widgets::button::{ButtonDisabled, pill_caption_scene};
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::{radius, space};
use infiltrator_contract::a11y::ShellA11yNode;
use infiltrator_contract::mini_hud::{MiniHudReadModel, MiniHudWaveformStrip};
use infiltrator_contract::system_toggle::SystemToggle;
use infiltrator_presentation::routing_intent_projection::routing_intent_status_copy;
use infiltrator_presentation::system_toggle_projection::{compact_label, compact_status_line};
use infiltrator_shared::locales::{Lang, Localizer};

/// The mini HUD card's fixed width in baseline pixels.
const MINI_HUD_CARD_WIDTH_PX: f32 = 280.0;

/// The mini HUD card's fixed height in baseline pixels.
const MINI_HUD_CARD_HEIGHT_PX: f32 = 112.0;

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

/// Marker on the routing intent chip text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MiniHudRoutingIntentLabel;

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

/// Marker on the HUD's downstream waveform strip. The bars come from the
/// shared read model resource (`MiniHudModel`) — the scene only owns the slot,
/// exactly like the chart plates own their spec.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MiniHudDownWaveform;

/// Marker on the HUD's upstream waveform strip.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MiniHudUpWaveform;

/// Marker on the PiP always-on-top control (BEVY-029).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MiniHudPipPinButton;

/// Marker on the PiP pointer click-through control.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MiniHudPipClickThroughButton;

/// Marker on the PiP corner-snap control.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MiniHudPipSnapButton;

/// Marker on the PiP placement readout line restamped from the live session.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MiniHudPipReadout;

#[derive(Clone, Copy, Debug, Default)]
pub enum MiniHudTextKind {
    #[default]
    RoutingIntent,
    Node,
    ToggleLine,
    UpRate,
    DownRate,
    Pin,
}
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct MiniHudText(pub MiniHudTextKind);
#[derive(Component, Clone, Copy, Debug)]
pub struct MiniHudQuickToggle(pub SystemToggle);
impl Default for MiniHudQuickToggle {
    fn default() -> Self {
        Self(SystemToggle::SystemProxy)
    }
}
fn hud_label(key: &str) -> String {
    let language = UiLocale::default().code().to_string();
    Lang(&language).tr(key).into_owned()
}
pub fn exit_label(model: &MiniHudReadModel) -> String {
    if model.exit_node.is_empty() {
        hud_label("mini_hud_empty_exit")
    } else {
        model.exit_node.clone()
    }
}
pub fn pin_label(model: &MiniHudReadModel) -> String {
    hud_label(if model.placement.pinned {
        "mini_hud_unpin"
    } else {
        "mini_hud_pin"
    })
}

fn down_waveform_slot_scene() -> impl Scene + use<> {
    let raster_width_px = MiniHudWaveformStrip::WIDTH_PX as f32;
    let raster_height_px = MiniHudWaveformStrip::HEIGHT_PX as f32;
    bsn! {
            Node {
                width: px(raster_width_px),
                height: px(raster_height_px),
                overflow: Overflow::clip(),
                flex_shrink: 0.0,
            }
            MiniHudDownWaveform
    }
}

fn up_waveform_slot_scene() -> impl Scene + use<> {
    let raster_width_px = MiniHudWaveformStrip::WIDTH_PX as f32;
    let raster_height_px = MiniHudWaveformStrip::HEIGHT_PX as f32;
    bsn! {
            Node {
                width: px(raster_width_px),
                height: px(raster_height_px),
                overflow: Overflow::clip(),
                flex_shrink: 0.0,
            }
            MiniHudUpWaveform
    }
}

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
///
/// The root is the mini-HUD layer's root: it declares the floating surface
/// level and mounts `FixedNode`, so the always-on-top overlay is positioned
/// against the camera viewport instead of a host container's box.
pub fn mini_hud_scene(model: &MiniHudReadModel, palette: &UiPalette) -> impl Scene + use<> {
    let up_rate = format_rate(model.up_bytes_per_sec);
    let down_rate = format_rate(model.down_bytes_per_sec);
    let routing_intent_label =
        routing_intent_status_copy(&model.routing_intent, &Lang(UiLocale::default().code()));
    let node_label = exit_label(model);
    let toggle_line = compact_status_line(
        &model.system_proxy,
        &model.tun,
        &Lang(UiLocale::default().code()),
    );
    let pin_label = pin_label(model);
    // The two quick switches use the same shared action rule as the sidebar:
    // a pending/unknown toggle offers no press (no fabricated transition).
    let proxy_actionable = model.next_value(SystemToggle::SystemProxy).is_some();
    let tun_actionable = model.next_value(SystemToggle::Tun).is_some();
    let proxy_label = compact_label(&model.system_proxy, &Lang(UiLocale::default().code()));
    let proxy_selected = model.system_proxy.is_enabled();
    let tun_label = compact_label(&model.tun, &Lang(UiLocale::default().code()));
    let tun_selected = model.tun.is_enabled();
    // DUAL-15-10: the HUD card and its two quick switches carry the shared
    // semantic rows (role + label), the switches with their live state.
    let card_node = semantic_node(ShellA11yNode::MiniHudCard);
    let proxy_node = switch_node(ShellA11yNode::MiniHudSystemProxySwitch, proxy_selected);
    let tun_node = switch_node(ShellA11yNode::MiniHudTunSwitch, tun_selected);
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
            ThemeContext(SurfaceLevel::Floating)
            FixedNode
            Children [
                Node {
                    width: rem(MINI_HUD_CARD_WIDTH_PX),
                    height: rem(MINI_HUD_CARD_HEIGHT_PX),
                    flex_direction: FlexDirection::Column,
                    row_gap: rem(space::S4),
                    padding: UiRect::all(rem(space::S12)),
                    border: UiRect::all(Val::Px(palette.hairline_px)),
                    border_radius: BorderRadius::all(rem(palette.card_radius_px)),
                }
                ThemeBackgroundColor({ Token::Surface })
                BorderColor { top: edge, right: edge, bottom: edge, left: edge }
                card_node
                Children [
                    // Header: title, observed routing intent, pin and expand actions
                    Node {
                        width: percent(100),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                        padding: UiRect::bottom(rem(space::S4)),
                    }
                    Children [
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: rem(space::S6),
                        }
                        Children [
                            @{ icon_tile_scene(IconId::Activity, 16.0, palette) }
                            --
                            Text({ "Mini HUD".to_owned() }) TextRole(Role::Caption)
                            --
                            Node {
                                padding: UiRect::axes(rem(space::S6), rem(space::S2)),
                                border_radius: BorderRadius::all(rem(radius::XS)),
                            }
                            ThemeBackgroundColor({ Token::SurfaceElevated })
                            MiniHudText(MiniHudTextKind::RoutingIntent)
                            Children [
                                Text({ routing_intent_label })
                                TextRole(Role::Caption)
                                MiniHudRoutingIntentLabel
                            ]
                        ]
                        --
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: rem(space::S4),
                        }
                        Children [
                            Node {
                                min_height: rem(space::S20),
                                padding: UiRect::horizontal(rem(space::S6)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border_radius: BorderRadius::all(rem(radius::XS)),
                                border: UiRect::all(Val::Px(palette.hairline_px)),
                            }
                            ThemeBackgroundColor({ Token::SurfaceElevated })
                            BorderColor { top: edge, right: edge, bottom: edge, left: edge }
                            Button
                            MiniHudPinButton MiniHudText(MiniHudTextKind::Pin)
                            Children [
                                Text(pin_label) TextRole(Role::Caption)
                            ]
                            --
                            Node {
                                min_height: rem(space::S20),
                                padding: UiRect::horizontal(rem(space::S6)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border_radius: BorderRadius::all(rem(radius::XS)),
                            }
                            ThemeBackgroundColor({ Token::Accent })
                            Button
                            MiniHudExpandButton
                            Children [
                                LocalizedText::plain("rules_expand") TextRole(Role::Caption)
                            ]
                        ]
                    ]
                    --
                    // Dual-Channel Bandwidth Rates
                    Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        padding: UiRect::vertical(rem(space::S4)),
                    }
                    Children [
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: rem(space::S6),
                        }
                        Children [
                            @{ icon_tile_scene(IconId::ArrowDown, 16.0, palette) }
                            --
                            Node MiniHudText(MiniHudTextKind::DownRate)
                            Children [ Text(down_rate) TextRole(Role::BodyStrong) ]
                            --
                            @{ down_waveform_slot_scene() }
                        ]
                        --
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: rem(space::S6),
                        }
                        Children [
                            @{ icon_tile_scene(IconId::ArrowUp, 16.0, palette) }
                            --
                            Node MiniHudText(MiniHudTextKind::UpRate)
                            Children [ Text(up_rate) TextRole(Role::BodyStrong) ]
                            --
                            @{ up_waveform_slot_scene() }
                        ]
                    ]
                    --
                    // BEVY-029: PiP placement controls bound to the shared
                    // overlay session (always-on-top, click-through, snap).
                    Node {
                        width: percent(100),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                        column_gap: rem(space::S6),
                    }
                    Children [
                        Node MiniHudPipReadout Children [
                            Text(String::new()) TextRole(Role::Caption)
                        ]
                        --
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: rem(space::S4),
                        }
                        Children [
                            @{ (pill_caption_scene("PiP Pin".to_owned(), false, palette), bsn! { MiniHudPipPinButton }) }
                            --
                            @{ (pill_caption_scene("Click".to_owned(), false, palette), bsn! { MiniHudPipClickThroughButton }) }
                            --
                            @{ (pill_caption_scene("Snap".to_owned(), false, palette), bsn! { MiniHudPipSnapButton }) }
                        ]
                    ]
                    --
                    // Footer: real exit node pill + the shared quick switches
                    // and toggle-state line (same command path as the sidebar).
                    Node {
                        width: percent(100),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                        padding: UiRect::top(rem(space::S4)),
                        column_gap: rem(space::S6),
                    }
                    Children [
                        Node {
                            padding: UiRect::axes(rem(space::S8), rem(space::S2)),
                            border_radius: BorderRadius::all(rem(radius::XS)),
                        }
                        BackgroundColor({ palette.accent.with_alpha(0.14) })
                        MiniHudText(MiniHudTextKind::Node)
                        Children [
                            Text(node_label)
                            TextRole(Role::Caption)
                            MiniHudNodeLabel
                        ]
                        --
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: rem(space::S6),
                        }
                        Children [
                            @{ pill_caption_scene(proxy_label, proxy_selected, palette) }
                            MiniHudSystemProxyToggle MiniHudQuickToggle(SystemToggle::SystemProxy)
                            ButtonDisabled({ !proxy_actionable })
                            proxy_node
                            --
                            @{ pill_caption_scene(tun_label, tun_selected, palette) }
                            MiniHudTunToggle MiniHudQuickToggle(SystemToggle::Tun)
                            ButtonDisabled({ !tun_actionable })
                            tun_node
                            --
                            Node MiniHudText(MiniHudTextKind::ToggleLine)
                            Children [ Text(toggle_line) TextRole(Role::Caption) MiniHudToggleLabel ]
                        ]
                    ]
                ]
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
    #[cfg(test)]
    use infiltrator_contract::traffic_waveform::TrafficWaveformSnapshot;
    use lurker_core_contract::routing_intent::RoutingIntentSnapshot;

    fn model() -> MiniHudReadModel {
        let snapshot = SystemToggleSnapshot::from_legacy(true, Some(true), 2);
        MiniHudReadModel::default()
            .with_toggles(&snapshot)
            .with_traffic(120 * 1024, 2 * 1024 * 1024)
            .with_exit_node("HK-01")
            .with_routing_intent(&RoutingIntentSnapshot::demo_fixture())
            .with_waveform(&waveform_snapshot())
    }

    fn waveform_snapshot() -> TrafficWaveformSnapshot {
        use infiltrator_contract::traffic_waveform::{TrafficSample, TrafficWaveformSnapshot};
        TrafficWaveformSnapshot {
            generation: 1,
            revision: 3,
            samples: vec![
                TrafficSample {
                    sampled_at_epoch_ms: None,
                    upload_bps: 1.0,
                    download_bps: 2.0,
                },
                TrafficSample {
                    sampled_at_epoch_ms: None,
                    upload_bps: 4.0,
                    download_bps: 8.0,
                },
            ],
        }
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

        let down_slots = world.query::<&MiniHudDownWaveform>().iter(world).count();
        assert_eq!(down_slots, 1, "the downstream waveform slot must mount");
        let up_slots = world.query::<&MiniHudUpWaveform>().iter(world).count();
        assert_eq!(up_slots, 1, "the upstream waveform slot must mount");
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
        let mut mode_labels = world.query::<(&Text, &MiniHudRoutingIntentLabel)>();
        assert_eq!(
            mode_labels
                .iter(world)
                .next()
                .map(|(text, _)| text.0.clone()),
            Some("遵循分流规则".to_owned())
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
