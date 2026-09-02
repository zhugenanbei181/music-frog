//! The App Routing page (应用分流): split tunneling per-application rules,
//! process binary matching, system app inclusion filter, and direct/proxy/block policies.
//!
//! **Update seam**: mutable nodes carry typed markers ([`AppRoutingLine`],
//! [`AppRuleMarker`]). The page self-registers
//! [`apply_app_routing_projection`] once per world via [`AppRoutingPageRoot`].

use bevy::a11y::AccessibilityNode;
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::event::Event;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::lifecycle::HookContext;
use bevy::ecs::observer::On;
use bevy::ecs::query::{With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::ecs::world::DeferredWorld;
use bevy::scene::{Scene, bsn, template_value};
use bevy::text::TextColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, Overflow,
    UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Button;
use infiltrator_bevy_widgets::button::ControlVisual;
use infiltrator_bevy_widgets::checkbox::checkbox_scene;
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

use crate::route::{PageRoot, Route};

/// Root marker on the App Routing page scene.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
#[component(on_insert = bind_app_routing_page)]
pub struct AppRoutingPageRoot;

/// Once-per-world guard preventing duplicate observer registration.
#[derive(Resource)]
struct AppRoutingPageBound;

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

/// Marker for an app rule text and color.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AppRuleText(pub usize);

/// Mode of split tunneling.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AppRoutingMode {
    #[default]
    ProxyAll,
    BypassList,
    ProxyList,
}

impl AppRoutingMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::ProxyAll => "全局分流 (全部应用代理)",
            Self::BypassList => "白名单分流 (指定应用直连)",
            Self::ProxyList => "黑名单分流 (仅指定应用代理)",
        }
    }
}

/// Rule assigned to an individual application.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AppRouteRule {
    #[default]
    Proxy,
    Direct,
    Block,
}

impl AppRouteRule {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Proxy => "代理 (Proxy)",
            Self::Direct => "直连 (Direct)",
            Self::Block => "拦截 (Block)",
        }
    }
}

/// Color for app routing rule.
pub fn app_rule_color(rule: AppRouteRule, palette: &UiPalette) -> Color {
    match rule {
        AppRouteRule::Proxy => palette.accent,
        AppRouteRule::Direct => palette.success,
        AppRouteRule::Block => palette.danger,
    }
}

/// An application entry for split tunneling.
#[derive(Clone, Debug, PartialEq)]
pub struct AppItem {
    pub id: String,
    pub name: String,
    pub process_name: String,
    pub rule: AppRouteRule,
    pub is_system: bool,
}

/// Snapshot of the App Routing domain.
#[derive(Clone, Debug, PartialEq)]
pub struct AppRoutingProjection {
    pub mode: AppRoutingMode,
    pub include_system: bool,
    pub apps: Vec<AppItem>,
}

impl AppRoutingProjection {
    /// Believable demo fixture for the App Routing page.
    pub fn demo() -> Self {
        Self {
            mode: AppRoutingMode::BypassList,
            include_system: false,
            apps: vec![
                AppItem {
                    id: "app-1".to_owned(),
                    name: "Google Chrome 浏览器".to_owned(),
                    process_name: "chrome / google-chrome".to_owned(),
                    rule: AppRouteRule::Proxy,
                    is_system: false,
                },
                AppItem {
                    id: "app-2".to_owned(),
                    name: "Steam 游戏平台".to_owned(),
                    process_name: "steam / steamwebhelper".to_owned(),
                    rule: AppRouteRule::Direct,
                    is_system: false,
                },
                AppItem {
                    id: "app-3".to_owned(),
                    name: "Spotify 音乐".to_owned(),
                    process_name: "spotify".to_owned(),
                    rule: AppRouteRule::Proxy,
                    is_system: false,
                },
                AppItem {
                    id: "app-4".to_owned(),
                    name: "Discord 通讯".to_owned(),
                    process_name: "Discord".to_owned(),
                    rule: AppRouteRule::Proxy,
                    is_system: false,
                },
                AppItem {
                    id: "app-5".to_owned(),
                    name: "WeChat 微信".to_owned(),
                    process_name: "wechat".to_owned(),
                    rule: AppRouteRule::Direct,
                    is_system: false,
                },
                AppItem {
                    id: "app-6".to_owned(),
                    name: "systemd-networkd".to_owned(),
                    process_name: "systemd-networkd".to_owned(),
                    rule: AppRouteRule::Direct,
                    is_system: true,
                },
            ],
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
    let summary = format!(
        "应用分流 · {} (已配置 {} 个应用)",
        projection.mode.label(),
        projection.apps.len()
    );

    let app_scenes: Vec<Box<dyn Scene>> = projection
        .apps
        .iter()
        .enumerate()
        .map(|(idx, item)| Box::new(app_row_scene(idx, item, palette)) as Box<dyn Scene>)
        .collect();

    bsn! {
        Node {
            width: percent(100),
            height: percent(100),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(space::S16),
            overflow: Overflow::scroll_y(),
        }
        PageRoot(Route::AppRouting)
        AppRoutingPageRoot
        Children [
            ( { header_card_scene(summary, projection.include_system, palette) } ),
            ( { apps_container_scene(app_scenes, palette) } ),
        ]
    }
}

fn header_card_scene(
    summary: String,
    include_system: bool,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let mut header_a11y = accesskit::Node::new(accesskit::Role::Header);
    header_a11y.set_label("应用分流概览");

    surface_scene(
        vec![
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    column_gap: Val::Px(space::S16),
                }
                template_value(AccessibilityNode(header_a11y))
                Children [
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S12),
                        }
                        Children [
                            ( { icon_tile_scene(IconId::Globe, 36.0, palette) } ),
                            ( Text(summary) AppRoutingLine(AppRoutingLineKind::Summary) TextRole(Role::Heading) ),
                        ]
                    ),
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S8),
                        }
                        Children [
                            (
                                Node {
                                    min_height: px(palette.control_height_px),
                                    padding: UiRect::horizontal(Val::Px(space::S12)),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::Center,
                                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                                }
                                BackgroundColor({ palette.accent })
                                Button
                                Children [
                                    ( Text({ "添加应用分流".to_owned() }) TextRole(Role::BodyStrong) ),
                                ]
                            ),
                        ]
                    ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    padding: UiRect::top(Val::Px(space::S8)),
                }
                Children [
                    ( { checkbox_scene("显示系统后台进程 (Include System Processes)".to_owned(), include_system, palette) } ),
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
                    ( Text({ "进程与应用分流策略列表 (Application Rules)".to_owned() }) TextRole(Role::BodyStrong) ),
                    ( Text({ "按进程匹配并重定向流量".to_owned() }) TextRole(Role::Caption) ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S8),
                }
                Children [
                    { app_scenes },
                ]
            }),
        ],
        palette,
    )
}

fn app_row_scene(idx: usize, app: &AppItem, palette: &UiPalette) -> impl Scene + use<> {
    let name = app.name.clone();
    let proc_str = format!("进程名: {}", app.process_name);
    let rule_str = app.rule.label().to_owned();
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
            (
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S4),
                }
                Children [
                    ( Text(name) TextRole(Role::BodyStrong) ),
                    ( Text(proc_str) TextRole(Role::Caption) ),
                ]
            ),
            (
                Node {
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S8),
                }
                Children [
                    (
                        Text(rule_str)
                        AppRuleText(idx)
                        TextRole(Role::BodyStrong)
                        TextColor(rule_col)
                    ),
                    (
                        Node {
                            min_height: px(palette.control_height_px * 0.8),
                            padding: UiRect::horizontal(Val::Px(space::S8)),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                        }
                        BackgroundColor({ palette.surface })
                        ControlVisual(false)
                        Button
                        Children [
                            ( Text({ "切换策略".to_owned() }) TextRole(Role::Caption) ),
                        ]
                    ),
                ]
            ),
        ]
    }
}

// ---- Observer & Update Hook -----------------------------------------------

fn bind_app_routing_page(mut world: DeferredWorld<'_>, _context: HookContext) {
    if world.get_resource::<AppRoutingPageBound>().is_some() {
        return;
    }
    let mut commands = world.commands();
    commands.insert_resource(AppRoutingPageBound);
    commands.add_observer(apply_app_routing_projection);
}

#[allow(clippy::type_complexity)]
pub(crate) fn apply_app_routing_projection(
    update: On<AppRoutingProjectionUpdated>,
    palette: Res<UiPalette>,
    mut last: Option<ResMut<LastAppRoutingProjection>>,
    mut lines: Query<(&mut Text, &AppRoutingLine), (With<AppRoutingLine>, Without<AppRuleText>)>,
    mut rules: Query<
        (&mut Text, &mut TextColor, &AppRuleText),
        (With<AppRuleText>, Without<AppRoutingLine>),
    >,
) {
    let projection = &update.0;

    for (mut text, line) in &mut lines {
        match line.0 {
            AppRoutingLineKind::Summary => {
                text.0 = format!(
                    "应用分流 · {} (已配置 {} 个应用)",
                    projection.mode.label(),
                    projection.apps.len()
                );
            }
        }
    }

    for (mut text, mut color, marker) in &mut rules {
        if let Some(app) = projection.apps.get(marker.0) {
            text.0 = app.rule.label().to_owned();
            color.0 = app_rule_color(app.rule, &palette);
        }
    }

    if let Some(ref mut last_proj) = last {
        last_proj.0 = Some(projection.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_app_routing_fixture() {
        let proj = AppRoutingProjection::demo();
        assert_eq!(proj.mode, AppRoutingMode::BypassList);
        assert_eq!(proj.apps.len(), 6);
        assert_eq!(proj.apps[0].rule, AppRouteRule::Proxy);
    }
}
