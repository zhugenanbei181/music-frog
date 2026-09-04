//! Global Command Palette (`Ctrl+K` / search) overlay and keyboard geeks workflow.
//!
//! Charter law (docs/BEVY_UI_FRONTEND.md):
//! - 100% `bsn!` scene composition for the modal scrim and floating search dialog;
//! - Pure state machine core ([`CommandPaletteState`], [`PaletteAction`]) testable headlessly;
//! - Direct dispatch into [`RouteChanged`] for navigation and [`CommandSinkHandle`] for actions.

use bevy::a11y::AccessibilityNode;
use bevy::color::{Alpha, Color};
use bevy::ecs::component::Component;
use bevy::ecs::event::Event;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Res, ResMut};
use bevy::scene::{Scene, bsn, template_value};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderColor, BorderRadius, FlexDirection, JustifyContent, Node,
    PositionType, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Button;
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::switch::ThemeSwitch;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::{LightDark, space};

use crate::app::ThemeMode;
use crate::command::{CommandSinkHandle, UiCommand};
use crate::route::{ActiveRoute, Route, RouteChanged};
use infiltrator_contract::command::ProxyMode;

/// Category classification of a command palette action item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PaletteCategory {
    Navigation,
    ProxyMode,
    Maintenance,
    Appearance,
}

impl PaletteCategory {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Navigation => "页面导航",
            Self::ProxyMode => "代理模式",
            Self::Maintenance => "快捷运维",
            Self::Appearance => "外观偏好",
        }
    }
}

/// A runnable action item in the command palette.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaletteAction {
    pub id: &'static str,
    pub title: &'static str,
    pub category: PaletteCategory,
    pub shortcut_hint: &'static str,
    pub target_route: Option<Route>,
    pub command: Option<UiCommand>,
}

/// Pure state machine for the global command palette.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct CommandPaletteState {
    pub is_open: bool,
    pub query: String,
    pub selected_index: usize,
    pub all_actions: Vec<PaletteAction>,
    pub filtered_indices: Vec<usize>,
}

impl Default for CommandPaletteState {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandPaletteState {
    pub fn new() -> Self {
        let all_actions = vec![
            // Navigation (11 routes)
            PaletteAction {
                id: "nav.overview",
                title: "核心概览 (Overview)",
                category: PaletteCategory::Navigation,
                shortcut_hint: "G O",
                target_route: Some(Route::Overview),
                command: None,
            },
            PaletteAction {
                id: "nav.proxies",
                title: "代理策略 (Proxies)",
                category: PaletteCategory::Navigation,
                shortcut_hint: "G P",
                target_route: Some(Route::Proxies),
                command: None,
            },
            PaletteAction {
                id: "nav.profiles",
                title: "配置订阅 (Profiles)",
                category: PaletteCategory::Navigation,
                shortcut_hint: "G S",
                target_route: Some(Route::Profiles),
                command: None,
            },
            PaletteAction {
                id: "nav.rules",
                title: "分流规则 (Rules)",
                category: PaletteCategory::Navigation,
                shortcut_hint: "G R",
                target_route: Some(Route::Rules),
                command: None,
            },
            PaletteAction {
                id: "nav.connections",
                title: "连接审计 (Connections)",
                category: PaletteCategory::Navigation,
                shortcut_hint: "G C",
                target_route: Some(Route::Connections),
                command: None,
            },
            PaletteAction {
                id: "nav.logs",
                title: "运行日志 (Logs)",
                category: PaletteCategory::Navigation,
                shortcut_hint: "G L",
                target_route: Some(Route::Logs),
                command: None,
            },
            PaletteAction {
                id: "nav.dns",
                title: "域名解析 (DNS)",
                category: PaletteCategory::Navigation,
                shortcut_hint: "G D",
                target_route: Some(Route::Dns),
                command: None,
            },
            PaletteAction {
                id: "nav.doctor",
                title: "自愈诊断 (Doctor)",
                category: PaletteCategory::Navigation,
                shortcut_hint: "G H",
                target_route: Some(Route::Doctor),
                command: None,
            },
            PaletteAction {
                id: "nav.app_routing",
                title: "应用分流 (App Routing)",
                category: PaletteCategory::Navigation,
                shortcut_hint: "G A",
                target_route: Some(Route::AppRouting),
                command: None,
            },
            PaletteAction {
                id: "nav.sync",
                title: "数据同步 (Sync)",
                category: PaletteCategory::Navigation,
                shortcut_hint: "G Y",
                target_route: Some(Route::Sync),
                command: None,
            },
            PaletteAction {
                id: "nav.settings",
                title: "系统设置 (Settings)",
                category: PaletteCategory::Navigation,
                shortcut_hint: "G T",
                target_route: Some(Route::Settings),
                command: None,
            },
            // Modes
            PaletteAction {
                id: "mode.rule",
                title: "切换模式：规则分流 (Rule Mode)",
                category: PaletteCategory::ProxyMode,
                shortcut_hint: "M R",
                target_route: None,
                command: Some(UiCommand::SetProxyMode(ProxyMode::Rule)),
            },
            PaletteAction {
                id: "mode.global",
                title: "切换模式：全局代理 (Global Mode)",
                category: PaletteCategory::ProxyMode,
                shortcut_hint: "M G",
                target_route: None,
                command: Some(UiCommand::SetProxyMode(ProxyMode::Global)),
            },
            PaletteAction {
                id: "mode.direct",
                title: "切换模式：直接连接 (Direct Mode)",
                category: PaletteCategory::ProxyMode,
                shortcut_hint: "M D",
                target_route: None,
                command: Some(UiCommand::SetProxyMode(ProxyMode::Direct)),
            },
            // Maintenance
            PaletteAction {
                id: "maint.clear_logs",
                title: "清空运行日志缓存",
                category: PaletteCategory::Maintenance,
                shortcut_hint: "C L",
                target_route: None,
                command: Some(UiCommand::ClearLogs),
            },
            PaletteAction {
                id: "maint.clear_dns",
                title: "刷新 DNS 缓存与 Fake-IP",
                category: PaletteCategory::Maintenance,
                shortcut_hint: "C D",
                target_route: None,
                command: Some(UiCommand::ClearDnsCache),
            },
            PaletteAction {
                id: "maint.test_latency",
                title: "全面测试全部代理策略组延时",
                category: PaletteCategory::Maintenance,
                shortcut_hint: "T A",
                target_route: None,
                command: Some(UiCommand::TestAllProxyGroups),
            },
            PaletteAction {
                id: "maint.run_doctor",
                title: "运行系统全景自愈体检",
                category: PaletteCategory::Maintenance,
                shortcut_hint: "D R",
                target_route: None,
                command: Some(UiCommand::RunDoctorDiagnostics),
            },
            PaletteAction {
                id: "tool.mini_hud",
                title: "切换迷你网速悬浮窗 (Toggle Mini HUD)",
                category: PaletteCategory::Maintenance,
                shortcut_hint: "Ctrl+M",
                target_route: None,
                command: None,
            },
            PaletteAction {
                id: "maint.close_connections",
                title: "断开全部实时活动连接",
                category: PaletteCategory::Maintenance,
                shortcut_hint: "C A",
                target_route: None,
                command: Some(UiCommand::CloseAllConnections),
            },
            PaletteAction {
                id: "maint.sync_now",
                title: "立即触发 WebDAV 配置同步",
                category: PaletteCategory::Maintenance,
                shortcut_hint: "S N",
                target_route: None,
                command: Some(UiCommand::SyncNow),
            },
            // Appearance
            PaletteAction {
                id: "theme.toggle",
                title: "切换界面外观明暗主题",
                category: PaletteCategory::Appearance,
                shortcut_hint: "T T",
                target_route: None,
                command: None,
            },
        ];

        let filtered_indices = (0..all_actions.len()).collect();
        Self {
            is_open: false,
            query: String::new(),
            selected_index: 0,
            all_actions,
            filtered_indices,
        }
    }

    pub fn open(&mut self) {
        self.is_open = true;
        self.query.clear();
        self.refilter();
        self.selected_index = 0;
    }

    pub fn close(&mut self) {
        self.is_open = false;
        self.query.clear();
        self.selected_index = 0;
    }

    pub fn toggle(&mut self) {
        if self.is_open {
            self.close();
        } else {
            self.open();
        }
    }

    pub fn set_query(&mut self, query: &str) {
        self.query = query.to_lowercase();
        self.refilter();
        self.selected_index = 0;
    }

    fn refilter(&mut self) {
        if self.query.is_empty() {
            self.filtered_indices = (0..self.all_actions.len()).collect();
        } else {
            self.filtered_indices = self
                .all_actions
                .iter()
                .enumerate()
                .filter(|(_, a)| {
                    let title = a.title.to_lowercase();
                    let category = a.category.label().to_lowercase();
                    let hint = a.shortcut_hint.to_lowercase();
                    title.contains(&self.query)
                        || category.contains(&self.query)
                        || hint.contains(&self.query)
                })
                .map(|(idx, _)| idx)
                .collect();
        }
    }

    pub fn select_next(&mut self) {
        if !self.filtered_indices.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.filtered_indices.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.filtered_indices.is_empty() {
            if self.selected_index == 0 {
                self.selected_index = self.filtered_indices.len() - 1;
            } else {
                self.selected_index -= 1;
            }
        }
    }

    pub fn current_selected_action(&self) -> Option<&PaletteAction> {
        self.filtered_indices
            .get(self.selected_index)
            .and_then(|&idx| self.all_actions.get(idx))
    }
}

/// Event triggering opening of the command palette.
#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenCommandPalette;

/// Event triggering closing of the command palette.
#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct CloseCommandPalette;

/// Event triggering toggle of the command palette.
#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToggleCommandPalette;

/// Event executing the selected command palette action.
#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExecuteSelectedPaletteAction;

/// Marker component on the command palette root entity.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CommandPaletteOverlayRoot;

/// Marker on an individual action row button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CommandPaletteRow(pub usize);

/// Accessibility node constructor for Command Palette dialog.
pub fn command_palette_semantic_node() -> AccessibilityNode {
    let mut node = accesskit::Node::new(accesskit::Role::Dialog);
    node.set_label("命令面板 (Command Palette)");
    AccessibilityNode(node)
}

/// Declarative scene for an individual action row.
pub fn command_palette_item_scene(
    palette: &UiPalette,
    action: &PaletteAction,
    is_selected: bool,
    display_index: usize,
) -> impl Scene + use<> {
    let bg = if is_selected {
        palette.accent.with_alpha(0.18)
    } else {
        Color::NONE
    };
    let edge = if is_selected {
        palette.accent
    } else {
        Color::NONE
    };
    let title_text = action.title.to_owned();
    let category_text = action.category.label().to_owned();
    let hint_text = action.shortcut_hint.to_owned();

    bsn! {
        Node {
            width: percent(100),
            height: px(40.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            padding: UiRect::axes(Val::Px(space::S12), Val::Px(space::S4)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(6.0)),
        }
        BackgroundColor(bg)
        BorderColor { top: edge, right: edge, bottom: edge, left: edge }
        Button
        CommandPaletteRow(display_index)
        Children [
            (
                Node {
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S8),
                }
                Children [
                    (
                        Text({ category_text })
                        TextRole(Role::Caption)
                    ),
                    (
                        Text({ title_text })
                        TextRole(Role::Body)
                    ),
                ]
            ),
            (
                Node {
                    padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                }
                BackgroundColor({ palette.surface_elevated })
                Children [
                    (
                        Text({ hint_text })
                        TextRole(Role::Caption)
                    ),
                ]
            ),
        ]
    }
}

/// Declarative BSN modal scene for the Command Palette.
pub fn command_palette_modal_scene(
    palette: &UiPalette,
    state: &CommandPaletteState,
) -> impl Scene + use<> {
    let semantic = command_palette_semantic_node();
    let query_display = if state.query.is_empty() {
        "输入关键词检索 11 个页面或快捷运维指令...".to_owned()
    } else {
        state.query.clone()
    };
    let edge = palette.border;

    let items_boxed: Vec<Box<dyn Scene>> = state
        .filtered_indices
        .iter()
        .take(8)
        .enumerate()
        .filter_map(|(disp_idx, &act_idx)| {
            let action = state.all_actions.get(act_idx)?;
            let is_sel = disp_idx == state.selected_index;
            Some(Box::new(command_palette_item_scene(
                palette, action, is_sel, disp_idx,
            )) as Box<dyn Scene>)
        })
        .collect();

    bsn! {
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::FlexStart,
            padding: UiRect::top(Val::Px(80.0)),
        }
        BackgroundColor({ palette.scrim() })
        CommandPaletteOverlayRoot
        template_value(semantic)
        Children [
            (
                Node {
                    width: px(560.0),
                    max_width: percent(92),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(space::S16)),
                    row_gap: Val::Px(space::S12),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(12.0)),
                }
                BackgroundColor({ palette.surface })
                BorderColor { top: edge, right: edge, bottom: edge, left: edge }
                Children [
                    // Search bar row
                    (
                        Node {
                            width: percent(100),
                            height: px(44.0),
                            align_items: AlignItems::Center,
                            padding: UiRect::axes(Val::Px(space::S12), Val::Px(space::S8)),
                            border: UiRect::all(Val::Px(1.0)),
                            border_radius: BorderRadius::all(Val::Px(8.0)),
                            column_gap: Val::Px(space::S8),
                        }
                        BackgroundColor({ palette.surface_elevated })
                        BorderColor { top: edge, right: edge, bottom: edge, left: edge }
                        Children [
                            ( { icon_tile_scene(IconId::Activity, 24.0, palette) } ),
                            (
                                Text({ query_display })
                                TextRole(Role::Body)
                            ),
                        ]
                    ),
                    // Items list
                    (
                        Node {
                            width: percent(100),
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(space::S4),
                        }
                        Children [
                            { items_boxed },
                        ]
                    ),
                    // Footer hint
                    (
                        Node {
                            width: percent(100),
                            justify_content: JustifyContent::SpaceBetween,
                            padding: UiRect::top(Val::Px(space::S4)),
                        }
                        Children [
                            (
                                Text({ "↑↓ 导航 · Enter 执行 · Esc 关闭".to_owned() })
                                TextRole(Role::Caption)
                            ),
                            (
                                Text({ format!("{}/{} 项", state.filtered_indices.len(), state.all_actions.len()) })
                                TextRole(Role::Caption)
                            ),
                        ]
                    ),
                ]
            ),
        ]
    }
}

/// Observer to open command palette.
pub fn on_open_command_palette(
    _trigger: On<OpenCommandPalette>,
    mut state: ResMut<CommandPaletteState>,
) {
    state.open();
}

/// Observer to close command palette.
pub fn on_close_command_palette(
    _trigger: On<CloseCommandPalette>,
    mut state: ResMut<CommandPaletteState>,
) {
    state.close();
}

/// Observer to toggle command palette.
pub fn on_toggle_command_palette(
    _trigger: On<ToggleCommandPalette>,
    mut state: ResMut<CommandPaletteState>,
) {
    state.toggle();
}

/// Observer executing the currently selected command palette action.
pub fn on_execute_selected_palette_action(
    _trigger: On<ExecuteSelectedPaletteAction>,
    mut state: ResMut<CommandPaletteState>,
    mut active_route: ResMut<ActiveRoute>,
    sink: Option<Res<CommandSinkHandle>>,
    theme_mode: Option<ResMut<ThemeMode>>,
    mut commands: Commands,
) {
    if let Some(action) = state.current_selected_action().cloned() {
        if let Some(route) = action.target_route {
            active_route.0 = Some(route);
            commands.trigger(RouteChanged(route));
        }
        if let Some(cmd) = action.command
            && let Some(sink) = sink
        {
            sink.submit(cmd);
        }
        if action.id == "theme.toggle" {
            let next_skin = match theme_mode.as_ref().map(|m| m.0).unwrap_or(LightDark::Dark) {
                LightDark::Dark => LightDark::Light,
                LightDark::Light => LightDark::Dark,
            };
            if let Some(mut tm) = theme_mode {
                tm.0 = next_skin;
            }
            commands.trigger(ThemeSwitch(next_skin));
        }
    }
    state.close();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_palette_lifecycle_and_filtering() {
        let mut state = CommandPaletteState::new();
        assert!(!state.is_open);
        assert_eq!(state.filtered_indices.len(), state.all_actions.len());

        state.open();
        assert!(state.is_open);

        // Filter by 'dns' -> should match nav.dns and maint.clear_dns
        state.set_query("dns");
        assert_eq!(state.filtered_indices.len(), 2);
        let first = state.current_selected_action().unwrap();
        assert_eq!(first.id, "nav.dns");

        // Navigate next
        state.select_next();
        let second = state.current_selected_action().unwrap();
        assert_eq!(second.id, "maint.clear_dns");

        // Wraparound
        state.select_next();
        let wrap = state.current_selected_action().unwrap();
        assert_eq!(wrap.id, "nav.dns");

        // Close
        state.close();
        assert!(!state.is_open);
    }

    #[test]
    fn test_command_palette_category_matching() {
        let mut state = CommandPaletteState::new();
        state.open();

        state.set_query("代理模式");
        assert_eq!(state.filtered_indices.len(), 3);

        state.set_query("运维");
        assert_eq!(state.filtered_indices.len(), 7);
    }
}
