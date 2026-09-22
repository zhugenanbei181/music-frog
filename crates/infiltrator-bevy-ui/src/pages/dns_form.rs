//! DNS workbench form scenes: the six system-level switches, the domain
//! mapping mode and the Fake-IP filter mode segmented pills.
//!
//! These builders are consumed by [`crate::pages::dns::dns_page`] and restamp
//! from the shared [`crate::pages::dns::DnsProjection`].

use bevy::ecs::hierarchy::Children;
use bevy::scene::{Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::BorderColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, PositionType,
    UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Button;
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::dns::{
    DnsCoreSwitches, DnsEnhancedMode, DnsFakeIpFilterMode, DnsSwitchField,
};

use crate::pages::dns::{
    DnsBorder, DnsConfigCard, DnsEnhancedModeControl, DnsEnhancedModePill, DnsFilterModeControl,
    DnsFilterModePill, DnsLine, DnsLineKind, DnsProjection, DnsSwitchButton, DnsSwitchKnob,
    DnsSwitchTrack, enhanced_mode_pill_label, filter_mode_label,
};

fn dns_switch_row_scene(
    field: DnsSwitchField,
    label: &'static str,
    switches: DnsCoreSwitches,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let name = field.key();
    let enabled = switches.value(field);
    let text_str = format!("{name} ({label})");
    let status_str = if enabled { "已开启" } else { "已关闭" };
    let status_color = if enabled {
        palette.success
    } else {
        palette.ink_dim
    };
    let switch_bg = if enabled {
        palette.accent
    } else {
        palette.surface_elevated
    };
    let knob_left = if enabled { Val::Px(18.0) } else { Val::Px(2.0) };
    let knob_color = if enabled {
        palette.on_accent
    } else {
        palette.ink_dim
    };
    let edge_color = if enabled {
        palette.accent
    } else {
        palette.border
    };

    bsn! {
        Node {
            width: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            padding: UiRect::axes(Val::Px(space::S8), Val::Px(space::S6)),
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
        }
        BackgroundColor({ palette.surface_elevated })
        Children [
            (
                Node {
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S8),
                }
                Children [
                    ( Text(text_str) TextRole(Role::Body) ),
                ]
            ),
            (
                Node {
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S8),
                }
                Children [
                    (
                        Text({ status_str.to_owned() })
                        DnsLine(DnsLineKind::SwitchStatus(field))
                        TextRole(Role::Caption)
                        TextColor({ status_color })
                    ),
                    (
                        Node {
                            width: px(38.0),
                            height: px(22.0),
                            border: UiRect::all(Val::Px(palette.hairline_px)),
                            border_radius: BorderRadius::all(Val::Px(11.0)),
                            position_type: PositionType::Relative,
                            align_items: AlignItems::Center,
                        }
                        BackgroundColor({ switch_bg })
                        BorderColor {
                            top: edge_color,
                            right: edge_color,
                            bottom: edge_color,
                            left: edge_color,
                        }
                        DnsSwitchButton(field, enabled)
                        DnsSwitchTrack(field)
                        DnsBorder(field)
                        Button
                        Children [
                            (
                                Node {
                                    position_type: PositionType::Absolute,
                                    left: { knob_left },
                                    width: px(16.0),
                                    height: px(16.0),
                                    border_radius: BorderRadius::all(Val::Px(8.0)),
                                }
                                BackgroundColor({ knob_color })
                                DnsSwitchKnob(field)
                            ),
                        ]
                    ),
                ]
            ),
        ]
    }
}

fn enhanced_mode_pill(
    mode: DnsEnhancedMode,
    selected: bool,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    let bg = if selected {
        palette.accent
    } else {
        palette.surface_elevated
    };
    let ink = if selected {
        palette.on_accent
    } else {
        palette.ink_dim
    };
    let label = enhanced_mode_pill_label(mode);
    Box::new(bsn! {
        Node {
            flex_grow: 1.0,
            height: px(palette.control_height_px * 0.8),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            padding: UiRect::horizontal(Val::Px(space::S8)),
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px * 0.75)),
        }
        BackgroundColor({ bg })
        Button
        DnsEnhancedModePill(mode)
        Children [
            (
                Text({ label.to_owned() })
                TextRole(Role::Caption)
                TextColor({ ink })
                DnsLine(DnsLineKind::EnhancedModeLabel(mode))
            ),
        ]
    })
}

fn filter_mode_pill(
    mode: DnsFakeIpFilterMode,
    selected: bool,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    let bg = if selected {
        palette.accent
    } else {
        palette.surface_elevated
    };
    let ink = if selected {
        palette.on_accent
    } else {
        palette.ink_dim
    };
    let label = filter_mode_label(mode);
    Box::new(bsn! {
        Node {
            flex_grow: 1.0,
            height: px(palette.control_height_px * 0.8),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            padding: UiRect::horizontal(Val::Px(space::S8)),
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px * 0.75)),
        }
        BackgroundColor({ bg })
        Button
        DnsFilterModePill(mode)
        Children [
            (
                Text({ label.to_owned() })
                TextRole(Role::Caption)
                TextColor({ ink })
                DnsLine(DnsLineKind::FilterModeLabel(mode))
            ),
        ]
    })
}

fn segmented_row_scene(pills: Vec<Box<dyn Scene>>, palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
        Node {
            width: percent(100),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(space::S4),
            padding: UiRect::all(Val::Px(space::S4)),
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
        }
        BackgroundColor({ palette.surface })
        Children [ { pills } ]
    }
}

pub(crate) fn dns_form_card_scene(
    projection: &DnsProjection,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let enhanced_pills: Vec<Box<dyn Scene>> = DnsEnhancedMode::ALL
        .into_iter()
        .map(|mode| enhanced_mode_pill(mode, mode == projection.mode, palette))
        .collect();
    let filter_pills: Vec<Box<dyn Scene>> = DnsFakeIpFilterMode::ALL
        .into_iter()
        .map(|mode| filter_mode_pill(mode, mode == projection.filter_mode, palette))
        .collect();

    surface_scene(
        vec![
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::bottom(Val::Px(space::S8)),
                }
                DnsConfigCard
                Children [
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S8),
                        }
                        Children [
                            ( { icon_tile_scene(IconId::Settings, 24.0, palette) } ),
                            ( Text({ "DNS 核心配置 (DNS Configuration)".to_owned() }) TextRole(Role::BodyStrong) ),
                        ]
                    ),
                    ( Text({ "6 项开关 · 域名映射 · 过滤模式".to_owned() }) TextRole(Role::Caption) ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S6),
                }
                Children [
                    ( { dns_switch_row_scene(DnsSwitchField::Enable, "启用 DNS 服务", projection.switches, palette) } ),
                    ( { dns_switch_row_scene(DnsSwitchField::Ipv6, "IPv6 解析", projection.switches, palette) } ),
                    ( { dns_switch_row_scene(DnsSwitchField::Cache, "DNS 内存缓存", projection.switches, palette) } ),
                    ( { dns_switch_row_scene(DnsSwitchField::UseHosts, "遵循系统 Hosts", projection.switches, palette) } ),
                    ( { dns_switch_row_scene(DnsSwitchField::UseSystemHosts, "系统默认解析器", projection.switches, palette) } ),
                    ( { dns_switch_row_scene(DnsSwitchField::RespectRules, "分流规则优先", projection.switches, palette) } ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S6),
                    padding: UiRect::top(Val::Px(space::S6)),
                }
                DnsEnhancedModeControl
                Children [
                    ( Text({ "域名映射模式 (enhanced_mode)".to_owned() }) TextRole(Role::Caption) ),
                    ( { segmented_row_scene(enhanced_pills, palette) } ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S6),
                    padding: UiRect::top(Val::Px(space::S4)),
                }
                DnsFilterModeControl
                Children [
                    ( Text({ "过滤模式 (fake_ip_filter_mode)".to_owned() }) TextRole(Role::Caption) ),
                    ( { segmented_row_scene(filter_pills, palette) } ),
                ]
            }),
        ],
        palette,
    )
}
