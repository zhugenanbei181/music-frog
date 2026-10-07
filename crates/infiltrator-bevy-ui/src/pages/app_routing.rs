//! The App Routing page (应用分流): split tunneling per-application rules,
//! process binary matching, system app inclusion filter, and direct/proxy/block policies.
//!
//! **Update seam**: mutable nodes carry typed markers ([`AppRoutingLine`],
//! [`AppRuleText`], [`AppProcessText`], [`AppNameText`]). [`AppRoutingPagePlugin`] registers
//! [`apply_app_routing_projection`] and action observers once at product
//! assembly. When [`AppRoutingProjectionUpdated`] fires, texts,
//! rules, and items restamp in place without tree rebuilds.

use crate::command::{CommandSinkHandle, UiCommand};
use crate::localized_widgets::localized_checkbox_scene;
use crate::pages::app_routing_copy;
use crate::pages::app_routing_uwp::{apply_projection, on_action_activated, uwp_exemption_scene};
use crate::pages::app_routing_uwp_copy;
use crate::route::{PageRoot, Route};
use bevy::a11y::AccessibilityNode;
use bevy::app::{App, Plugin, Update};
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::event::Event;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::scene::{Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, Overflow,
    UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_application::routing_projection::{process_copy, routing_summary, rule_key};
use infiltrator_bevy_widgets::button::ControlVisual;
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::uwp::{UwpLoopbackSnapshot, UwpPackageSnapshot};
use infiltrator_domain::app_routing::{AppRoutingMode, AppRoutingRule};
use infiltrator_shared::locales::{Lang, Localizer};

/// Root marker on the App Routing page scene.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct AppRoutingPageRoot;

/// Marker for text lines updated by the projection observer.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AppRoutingLine(pub AppRoutingLineKind);

/// Different text lines on the app routing page.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AppRoutingLineKind {
    /// Overview summary: active apps count and mode.
    #[default]
    Summary,
}

/// Marker for an app name text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AppNameText(pub usize);

/// Marker for an app process text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AppProcessText(pub usize);

/// Marker for an app rule text and color.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AppRuleText(pub usize);

/// Marker for "Add App Routing Rule" button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AddAppRouteButton;

/// Marker for switching an individual application's rule.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct SwitchAppRuleButton {
    pub app_id: String,
    pub app_idx: usize,
    pub current_rule: AppRoutingRule,
}

/// Color for app routing rule.
pub fn app_rule_color(rule: AppRoutingRule, palette: &UiPalette) -> Color {
    match rule {
        AppRoutingRule::Proxy => palette.accent,
        AppRoutingRule::Direct => palette.success,
        AppRoutingRule::Block => palette.danger,
    }
}

/// An application entry for split tunneling.
#[derive(Clone, Debug, PartialEq)]
pub struct AppItem {
    pub id: String,
    pub name: String,
    pub process_name: String,
    pub rule: AppRoutingRule,
    pub is_system: bool,
}

/// Snapshot of the App Routing domain.
#[derive(Clone, Debug, PartialEq)]
pub struct AppRoutingProjection {
    pub mode: AppRoutingMode,
    pub include_system: bool,
    pub apps: Vec<AppItem>,
    pub uwp_loopback: UwpLoopbackSnapshot,
}

impl AppRoutingProjection {
    /// Believable demo fixture for the App Routing page.
    pub fn demo() -> Self {
        Self {
            mode: AppRoutingMode::ProxySelected,
            include_system: false,
            apps: vec![
                AppItem {
                    id: "app-1".to_owned(),
                    name: "Google Chrome".to_owned(),
                    process_name: "chrome / google-chrome".to_owned(),
                    rule: AppRoutingRule::Proxy,
                    is_system: false,
                },
                AppItem {
                    id: "app-2".to_owned(),
                    name: "Steam".to_owned(),
                    process_name: "steam / steamwebhelper".to_owned(),
                    rule: AppRoutingRule::Direct,
                    is_system: false,
                },
                AppItem {
                    id: "app-3".to_owned(),
                    name: "Spotify".to_owned(),
                    process_name: "spotify".to_owned(),
                    rule: AppRoutingRule::Proxy,
                    is_system: false,
                },
                AppItem {
                    id: "app-4".to_owned(),
                    name: "Discord".to_owned(),
                    process_name: "Discord".to_owned(),
                    rule: AppRoutingRule::Proxy,
                    is_system: false,
                },
                AppItem {
                    id: "app-5".to_owned(),
                    name: "WeChat".to_owned(),
                    process_name: "wechat".to_owned(),
                    rule: AppRoutingRule::Direct,
                    is_system: false,
                },
                AppItem {
                    id: "app-6".to_owned(),
                    name: "systemd-networkd".to_owned(),
                    process_name: "systemd-networkd".to_owned(),
                    rule: AppRoutingRule::Direct,
                    is_system: true,
                },
            ],
            uwp_loopback: UwpLoopbackSnapshot::supported(
                1,
                vec![
                    UwpPackageSnapshot {
                        sid: "S-1-15-2-1".to_owned(),
                        display_name: "Microsoft Store".to_owned(),
                        package_family_name: "Microsoft.WindowsStore".to_owned(),
                        loopback_exempt: true,
                    },
                    UwpPackageSnapshot {
                        sid: "S-1-15-2-2".to_owned(),
                        display_name: "Xbox App".to_owned(),
                        package_family_name: "Microsoft.XboxApp".to_owned(),
                        loopback_exempt: false,
                    },
                    UwpPackageSnapshot {
                        sid: "S-1-15-2-3".to_owned(),
                        display_name: "Windows Terminal".to_owned(),
                        package_family_name: "Microsoft.WindowsTerminal".to_owned(),
                        loopback_exempt: true,
                    },
                ],
            ),
        }
    }
}

/// The typed event dispatched when app routing data updates.
#[derive(Event, Clone, Debug, PartialEq)]
pub struct AppRoutingProjectionUpdated(pub AppRoutingProjection);

/// Last projection resource for theme replay.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct LastAppRoutingProjection(pub Option<AppRoutingProjection>);

// ---- Scene constructors ---------------------------------------------------

pub fn app_routing_page(
    projection: &AppRoutingProjection,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let summary = routing_summary(projection.mode, projection.apps.len(), "en-US");

    let app_scenes: Vec<Box<dyn Scene>> = projection
        .apps
        .iter()
        .enumerate()
        .map(|(idx, item)| Box::new(app_row_scene(idx, item, palette)) as Box<dyn Scene>)
        .collect();

    bsn! {
            Node {
                width: percent(100),
                min_width: px(0.0),
                max_width: percent(100),
                height: percent(100),
                min_height: px(0.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(space::S16),
                overflow: Overflow::scroll_y(),
            }
            PageRoot(Route::AppRouting)
            AppRoutingPageRoot
            Children [
                @{ header_card_scene(summary, projection.include_system, palette) }
                --
                @{ uwp_exemption_scene(projection, palette) }
                --
                @{ apps_container_scene(app_scenes, palette) }
            ]
    }
}

fn header_card_scene(
    summary: String,
    include_system: bool,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let mut header_a11y = accesskit::Node::new(accesskit::Role::Header);
    header_a11y.set_label("App routing");

    surface_scene(
        vec![
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::SpaceBetween,
                                column_gap: Val::Px(space::S16),
                            }
                            AccessibilityNode(header_a11y) LocalizedLabel::plain("app_routing_title")
                            Children [
                                Node {
                                    align_items: AlignItems::Center,
                                    column_gap: Val::Px(space::S12),
                                }
                                Children [
                                    @{ icon_tile_scene(IconId::Globe, 36.0, palette) }
                                    --
                                    Text(summary) AppRoutingLine(AppRoutingLineKind::Summary) TextRole(Role::Heading)
                                ]
                                --
                                Node {
                                    align_items: AlignItems::Center,
                                    column_gap: Val::Px(space::S8),
                                }
                                Children [
                                    Node {
                                        min_height: px(palette.control_height_px),
                                        padding: UiRect::horizontal(Val::Px(space::S12)),
                                        align_items: AlignItems::Center,
                                        justify_content: JustifyContent::Center,
                                        border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                                    }
                                    BackgroundColor({ palette.accent })
                                    AddAppRouteButton
                                    Button
                                    Children [
                                        LocalizedText::plain("app_routing_add_action") TextRole(Role::BodyStrong)
                                    ]
                                ]
                            ]
            }),
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                align_items: AlignItems::Center,
                                padding: UiRect::top(Val::Px(space::S8)),
                            }
                            Children [
                                @{ localized_checkbox_scene(LocalizedText::plain("app_routing_include_system"), include_system, palette) }
                            ]
            }),
        ],
        palette,
    )
}

fn apps_container_scene(
    app_scenes: Vec<Box<dyn Scene>>,
    palette: &UiPalette,
) -> impl Scene + use<> {
    surface_scene(
        vec![
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::SpaceBetween,
                                padding: UiRect::bottom(Val::Px(space::S8)),
                            }
                            Children [
                                LocalizedText::plain("app_routing_rules_title") TextRole(Role::BodyStrong)
                                --
                                LocalizedText::plain("app_routing_rules_hint") TextRole(Role::Caption)
                            ]
            }),
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(space::S8),
                            }
                            Children [
                                { app_scenes }
                            ]
            }),
        ],
        palette,
    )
}

fn app_row_scene(idx: usize, app: &AppItem, palette: &UiPalette) -> impl Scene + use<> {
    let name = app.name.clone();
    let proc_str = process_copy(&app.process_name, "en-US");
    let rule_str = Lang("en-US").tr(rule_key(app.rule)).into_owned();
    let rule_col = app_rule_color(app.rule, palette);

    bsn! {
            Node {
                width: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::all(Val::Px(space::S8)),
                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
            }
            BackgroundColor({ palette.surface_elevated })
            Children [
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S4),
                }
                Children [
                    Text(name) AppNameText(idx) TextRole(Role::BodyStrong)
                    --
                    Text(proc_str) AppProcessText(idx) TextRole(Role::Caption)
                ]
                --
                Node {
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S8),
                }
                Children [
                    Text(rule_str)
                    AppRuleText(idx)
                    TextRole(Role::BodyStrong)
                    TextColor(rule_col)
                    --
                    Node {
                        min_height: px(palette.control_height_px * 0.8),
                        padding: UiRect::horizontal(Val::Px(space::S8)),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                    }
                    BackgroundColor({ palette.surface })
                    ControlVisual(false)
                    SwitchAppRuleButton {
                        app_id: { app.id.clone() },
                        app_idx: { idx },
                        current_rule: { app.rule },
                    }
                    Button
                    Children [
                        LocalizedText::plain("app_routing_switch_policy") TextRole(Role::Caption)
                    ]
                ]
            ]
    }
}

// ---- Plugin assembly and native observers -----------------------------------------------

/// Registers this page once during product assembly; mounting never resets its draft.
#[derive(Default)]
pub struct AppRoutingPagePlugin;

impl Plugin for AppRoutingPagePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LastAppRoutingProjection>();
        app.add_systems(Update, (app_routing_copy::sync, app_routing_uwp_copy::sync));
        app.add_observer(apply_app_routing_projection);
        app.add_observer(on_app_routing_action_activated);
        app.add_observer(on_action_activated);
        app.add_observer(apply_projection);
    }
}

pub(crate) fn on_app_routing_action_activated(
    activate: On<Activate>,
    add_buttons: Query<(), With<AddAppRouteButton>>,
    switch_buttons: Query<&SwitchAppRuleButton>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    if add_buttons.contains(activate.entity) {
        handle.submit(UiCommand::SetAppRule {
            app_id: "new-app".to_owned(),
            rule: "Proxy".to_owned(),
        });
    } else if let Ok(btn) = switch_buttons.get(activate.entity) {
        let next_rule = btn.current_rule.next();
        handle.submit(UiCommand::SetAppRule {
            app_id: btn.app_id.clone(),
            rule: format!("{:?}", next_rule),
        });
    }
}

pub(crate) fn apply_app_routing_projection(
    update: On<AppRoutingProjectionUpdated>,
    mut last: ResMut<LastAppRoutingProjection>,
    mut buttons: Query<&mut SwitchAppRuleButton>,
) {
    for mut button in &mut buttons {
        if let Some(app) = update.0.apps.get(button.app_idx) {
            button.app_id.clone_from(&app.id);
            button.current_rule = app.rule;
        }
    }
    last.0 = Some(update.0.clone());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_app_routing_fixture() {
        let proj = AppRoutingProjection::demo();
        assert_eq!(proj.mode, AppRoutingMode::ProxySelected);
        assert_eq!(proj.apps.len(), 6);
        assert_eq!(proj.apps[0].id, "app-1");
        assert_eq!(proj.apps[0].name, "Google Chrome");
        assert_eq!(proj.apps[0].process_name, "chrome / google-chrome");
        assert_eq!(proj.apps[0].rule, AppRoutingRule::Proxy);
        assert_eq!(proj.apps[1].id, "app-2");
        assert_eq!(proj.apps[1].rule, AppRoutingRule::Direct);
    }
}
