//! The Doctor page (自愈诊断): system diagnostics, TUN health check,
//! port conflict detector, DNS poisoning leak check, and one-click repair.
//!
//! **Update seam**: mutable nodes carry typed markers ([`DoctorLine`],
//! [`CheckStateText`], [`CheckDetailText`]). The page self-registers
//! [`apply_doctor_projection`] and action observers once per world via
//! [`DoctorPageRoot`]. When [`DoctorProjectionUpdated`] fires, texts, state colors,
//! and check items restamp in place without tree rebuilds.

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
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

use crate::command::{CommandSinkHandle, UiCommand};
use crate::route::{PageRoot, Route};

/// Root marker on the Doctor page scene.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
#[component(on_insert = bind_doctor_page)]
pub struct DoctorPageRoot;

/// Once-per-world guard preventing duplicate observer registration.
#[derive(Resource)]
struct DoctorPageBound;

/// Marker for text lines updated by the projection observer.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DoctorLine(pub DoctorLineKind);

/// Different text lines on the doctor page.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DoctorLineKind {
    /// Overview summary: health status and pass count.
    #[default]
    Summary,
    /// Last diagnostic run time.
    LastRun,
}

/// Marker for a check item's status text and color.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CheckStateText(pub usize);

/// Marker for a check item's detail description text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CheckDetailText(pub usize);

/// Marker for "Run Doctor Diagnostics" button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RunDoctorDiagnosticsButton;

/// Marker for "Repair All Doctor Issues" button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RepairAllDoctorButton;

/// Marker for repairing a specific check issue.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct RepairDoctorRowButton {
    pub check_id: String,
    pub check_idx: usize,
}

/// State of an individual diagnostic check.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DoctorCheckState {
    #[default]
    Pass,
    Warning,
    Fail,
}

impl DoctorCheckState {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Pass => "正常 (PASS)",
            Self::Warning => "警告 (WARN)",
            Self::Fail => "异常 (FAIL)",
        }
    }
}

/// Color for check state tag.
pub fn check_state_color(state: DoctorCheckState, palette: &UiPalette) -> Color {
    match state {
        DoctorCheckState::Pass => palette.success,
        DoctorCheckState::Warning => palette.warning,
        DoctorCheckState::Fail => palette.danger,
    }
}

/// An individual diagnostic check item.
#[derive(Clone, Debug, PartialEq)]
pub struct DoctorCheckItem {
    pub id: String,
    pub name: String,
    pub category: String,
    pub state: DoctorCheckState,
    pub detail: String,
    pub fix_available: bool,
}

/// Snapshot of the Doctor domain.
#[derive(Clone, Debug, PartialEq)]
pub struct DoctorProjection {
    pub overall_healthy: bool,
    pub last_run: String,
    pub checks: Vec<DoctorCheckItem>,
}

impl DoctorProjection {
    /// Believable demo fixture for the Doctor page.
    pub fn demo() -> Self {
        Self {
            overall_healthy: true,
            last_run: "2026-09-02 10:14:30".to_owned(),
            checks: vec![
                DoctorCheckItem {
                    id: "chk-1".to_owned(),
                    name: "TUN 虚拟网卡与路由表健康度".to_owned(),
                    category: "网络栈".to_owned(),
                    state: DoctorCheckState::Pass,
                    detail: "utun9 设备正常就绪，默认路由接管生效中".to_owned(),
                    fix_available: false,
                },
                DoctorCheckItem {
                    id: "chk-2".to_owned(),
                    name: "系统代理注册与回退探测".to_owned(),
                    category: "系统集成".to_owned(),
                    state: DoctorCheckState::Pass,
                    detail: "HTTP/SOCKS5 代理注册 127.0.0.1:7890 正常".to_owned(),
                    fix_available: false,
                },
                DoctorCheckItem {
                    id: "chk-3".to_owned(),
                    name: "核心端口独占性 (7890 / 9090)".to_owned(),
                    category: "端口绑定".to_owned(),
                    state: DoctorCheckState::Pass,
                    detail: "端口无外部进程抢占冲突".to_owned(),
                    fix_available: false,
                },
                DoctorCheckItem {
                    id: "chk-4".to_owned(),
                    name: "DNS 污染与泄露防护测试".to_owned(),
                    category: "DNS 安全".to_owned(),
                    state: DoctorCheckState::Pass,
                    detail: "Fake-IP 198.18.0.0/16 隔离良好，无真实 IP 泄露".to_owned(),
                    fix_available: false,
                },
                DoctorCheckItem {
                    id: "chk-5".to_owned(),
                    name: "进程管理员特权与 CAP_NET_ADMIN".to_owned(),
                    category: "权限环境".to_owned(),
                    state: DoctorCheckState::Pass,
                    detail: "Linux Capabilities 网络配置权限满足".to_owned(),
                    fix_available: false,
                },
                DoctorCheckItem {
                    id: "chk-6".to_owned(),
                    name: "配置文件语法与规则集合规性".to_owned(),
                    category: "配置校验".to_owned(),
                    state: DoctorCheckState::Pass,
                    detail: "3 份 MRS 规则集校验通过，0 语法警告".to_owned(),
                    fix_available: false,
                },
            ],
        }
    }

    pub fn passed_count(&self) -> usize {
        self.checks
            .iter()
            .filter(|c| c.state == DoctorCheckState::Pass)
            .count()
    }
}

/// The typed event dispatched when doctor data updates.
#[derive(Event, Clone, Debug, PartialEq)]
pub struct DoctorProjectionUpdated(pub DoctorProjection);

/// Last projection resource for theme replay.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct LastDoctorProjection(pub Option<DoctorProjection>);

// ---- Scene constructors ---------------------------------------------------

pub fn doctor_page(projection: &DoctorProjection, palette: &UiPalette) -> impl Scene + use<> {
    let summary = format!(
        "自愈诊断 · 健康评估 ({} / {} 项检查通过)",
        projection.passed_count(),
        projection.checks.len()
    );
    let last_run_str = format!("最近诊断: {}", projection.last_run);

    let check_scenes: Vec<Box<dyn Scene>> = projection
        .checks
        .iter()
        .enumerate()
        .map(|(idx, item)| Box::new(check_row_scene(idx, item, palette)) as Box<dyn Scene>)
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
        PageRoot(Route::Doctor)
        DoctorPageRoot
        Children [
            ( { header_card_scene(summary, last_run_str, palette) } ),
            ( { checks_container_scene(check_scenes, palette) } ),
        ]
    }
}

fn header_card_scene(summary: String, last_run: String, palette: &UiPalette) -> impl Scene + use<> {
    let mut header_a11y = accesskit::Node::new(accesskit::Role::Header);
    header_a11y.set_label("自愈诊断概览");

    surface_scene(
        vec![Box::new(bsn! {
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
                        ( { icon_tile_scene(IconId::Activity, 36.0, palette) } ),
                        (
                            Node {
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(space::S4),
                            }
                            Children [
                                ( Text(summary) DoctorLine(DoctorLineKind::Summary) TextRole(Role::Heading) ),
                                ( Text(last_run) DoctorLine(DoctorLineKind::LastRun) TextRole(Role::Caption) ),
                            ]
                        ),
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
                            RunDoctorDiagnosticsButton
                            Button
                            Children [
                                ( Text({ "立即诊断".to_owned() }) TextRole(Role::BodyStrong) ),
                            ]
                        ),
                        (
                            Node {
                                min_height: px(palette.control_height_px),
                                padding: UiRect::horizontal(Val::Px(space::S12)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                            }
                            BackgroundColor({ palette.surface_elevated })
                            RepairAllDoctorButton
                            Button
                            Children [
                                ( Text({ "一键修复".to_owned() }) TextRole(Role::Body) ),
                            ]
                        ),
                    ]
                ),
            ]
        })],
        palette,
    )
}

fn checks_container_scene(
    check_scenes: Vec<Box<dyn Scene>>,
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
                    ( Text({ "诊断检查清单 (Diagnostic Suite)".to_owned() }) TextRole(Role::BodyStrong) ),
                    ( Text({ "涵盖网络栈、系统代理、端口、DNS 与权限".to_owned() }) TextRole(Role::Caption) ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S8),
                }
                Children [
                    { check_scenes },
                ]
            }),
        ],
        palette,
    )
}

fn check_row_scene(idx: usize, check: &DoctorCheckItem, palette: &UiPalette) -> impl Scene + use<> {
    let name = format!("[{}] {}", check.category, check.name);
    let detail = check.detail.clone();
    let state_str = check.state.label().to_owned();
    let state_col = check_state_color(check.state, palette);

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
                    ( Text(detail) CheckDetailText(idx) TextRole(Role::Caption) ),
                ]
            ),
            (
                Text(state_str)
                CheckStateText(idx)
                TextRole(Role::BodyStrong)
                TextColor(state_col)
            ),
        ]
    }
}

// ---- Observer & Update Hook -----------------------------------------------

fn bind_doctor_page(mut world: DeferredWorld<'_>, _context: HookContext) {
    if world.get_resource::<DoctorPageBound>().is_some() {
        return;
    }
    let mut commands = world.commands();
    commands.insert_resource(DoctorPageBound);
    commands.add_observer(apply_doctor_projection);
    commands.add_observer(on_doctor_action_activated);
}

pub(crate) fn on_doctor_action_activated(
    activate: On<Activate>,
    diag_buttons: Query<(), With<RunDoctorDiagnosticsButton>>,
    repair_all_buttons: Query<(), With<RepairAllDoctorButton>>,
    repair_row_buttons: Query<&RepairDoctorRowButton>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    if diag_buttons.contains(activate.entity) {
        handle.submit(UiCommand::RunDoctorDiagnostics);
    } else if repair_all_buttons.contains(activate.entity) {
        handle.submit(UiCommand::RepairAllDoctorIssues);
    } else if let Ok(btn) = repair_row_buttons.get(activate.entity) {
        handle.submit(UiCommand::RepairDoctorIssue {
            check_id: btn.check_id.clone(),
        });
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn apply_doctor_projection(
    update: On<DoctorProjectionUpdated>,
    palette: Res<UiPalette>,
    mut last: Option<ResMut<LastDoctorProjection>>,
    mut lines: Query<
        (&mut Text, &DoctorLine),
        (
            With<DoctorLine>,
            Without<CheckStateText>,
            Without<CheckDetailText>,
        ),
    >,
    mut states: Query<
        (&mut Text, &mut TextColor, &CheckStateText),
        (
            With<CheckStateText>,
            Without<DoctorLine>,
            Without<CheckDetailText>,
        ),
    >,
    mut details: Query<
        (&mut Text, &CheckDetailText),
        (
            With<CheckDetailText>,
            Without<DoctorLine>,
            Without<CheckStateText>,
        ),
    >,
) {
    let projection = &update.0;

    for (mut text, line) in &mut lines {
        match line.0 {
            DoctorLineKind::Summary => {
                text.0 = format!(
                    "自愈诊断 · 健康评估 ({} / {} 项检查通过)",
                    projection.passed_count(),
                    projection.checks.len()
                );
            }
            DoctorLineKind::LastRun => {
                text.0 = format!("最近诊断: {}", projection.last_run);
            }
        }
    }

    for (mut text, mut color, marker) in &mut states {
        if let Some(check) = projection.checks.get(marker.0) {
            text.0 = check.state.label().to_owned();
            color.0 = check_state_color(check.state, &palette);
        }
    }

    for (mut text, marker) in &mut details {
        if let Some(check) = projection.checks.get(marker.0) {
            text.0 = check.detail.clone();
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
    fn demo_doctor_fixture() {
        let proj = DoctorProjection::demo();
        assert!(proj.overall_healthy);
        assert_eq!(proj.passed_count(), 6);
        assert_eq!(proj.checks.len(), 6);
        assert_eq!(proj.checks[0].id, "chk-1");
        assert_eq!(proj.checks[0].name, "TUN 虚拟网卡与路由表健康度");
        assert_eq!(proj.checks[0].category, "网络栈");
        assert_eq!(proj.checks[0].state, DoctorCheckState::Pass);
    }
}
