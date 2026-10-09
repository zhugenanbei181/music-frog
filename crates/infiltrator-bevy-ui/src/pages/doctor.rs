//! The Doctor page (自愈诊断): system diagnostics, TUN health check,
//! port conflict detector, DNS poisoning leak check, and one-click repair.
//!
//! **Update seam**: mutable nodes carry typed markers ([`DoctorLine`],
//! [`CheckStateText`], [`CheckDetailText`]). [`DoctorPagePlugin`] registers
//! [`apply_doctor_projection`] and action observers once at product
//! assembly. When [`DoctorProjectionUpdated`] fires, texts, state colors,
//! and check items restamp in place without tree rebuilds.

use crate::localized_widgets::localized_button_scene;
use crate::pages::doctor_actions::{BootstrapDoctorButton, DoctorFeedbackText, RetryDoctorButton};
use crate::pages::doctor_rows::{DoctorRows, check_row_scene, refresh_rows};
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
use bevy::ui::FlexWrap;
use bevy::ui::prelude::{
    AlignItems, FlexDirection, JustifyContent, Node, Overflow, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Button, ScrollArea};
use infiltrator_application::doctor_projection::{summary, watchdog_status_text};
use infiltrator_bevy_widgets::button::{ButtonDisabled, ButtonVariant};
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::doctor::{DoctorCheckKind, DoctorStatus};
use infiltrator_contract::snapshot::CoreWatchdogSnapshot;
use infiltrator_shared::i18n_interpolator::localize;
use infiltrator_shared::locales::{Lang, Localizer};

#[path = "doctor_self_heal.rs"]
pub mod self_heal;

/// Root marker on the Doctor page scene.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct DoctorPageRoot;

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
    /// Shared core crash-watchdog status.
    Watchdog,
}

/// Marker for a check item's status text and color.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct CheckStateText(pub String);

/// Marker for a check item's detail description text.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct CheckDetailText(pub String);

/// Marker for "Run Doctor Diagnostics" button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[require(Button, ButtonDisabled)]
pub struct RunDoctorDiagnosticsButton;

/// Marker for "Repair All Doctor Issues" button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[require(Button, ButtonDisabled)]
pub struct RepairAllDoctorButton;

/// Marker for repairing a specific check issue.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
#[require(Button, ButtonDisabled)]
pub struct RepairDoctorRowButton {
    pub check_id: String,
}

/// Color for check state tag.
pub fn check_state_color(state: DoctorStatus, palette: &UiPalette) -> Color {
    match state {
        DoctorStatus::Pass => palette.success,
        DoctorStatus::Warn => palette.warning,
        DoctorStatus::Fail => palette.danger,
        DoctorStatus::Skip => palette.ink_dim,
    }
}

/// An individual diagnostic check item.
#[derive(Clone, Debug, PartialEq)]
pub struct DoctorCheckItem {
    pub kind: Option<DoctorCheckKind>,
    pub detail_copy_key: Option<String>,
    pub id: String,
    pub name: String,
    pub category: String,
    pub state: DoctorStatus,
    pub detail: String,
    pub hint: Option<String>,
    pub fix_available: bool,
}

/// Snapshot of the Doctor domain.
#[derive(Clone, Debug, PartialEq)]
pub struct DoctorProjection {
    pub overall_healthy: bool,
    pub report_finished_at: Option<u64>,
    pub last_run: String,
    pub checks: Vec<DoctorCheckItem>,
    pub watchdog: CoreWatchdogSnapshot,
}

impl DoctorProjection {
    /// Believable demo fixture for the Doctor page.
    pub fn demo() -> Self {
        Self {
            overall_healthy: true,
            report_finished_at: Some(1788344070),
            last_run: "2026-09-02 10:14:30".to_owned(),
            checks: vec![
                DoctorCheckItem {
                    id: "chk-1".to_owned(),
                    kind: Some(DoctorCheckKind::TunHealth),
                    detail_copy_key: Some(DoctorCheckKind::TunHealth.copy_keys().2.into()),
                    name: Lang(UiLocale::default().code())
                        .tr(DoctorCheckKind::TunHealth.copy_keys().0)
                        .into_owned(),
                    category: Lang(UiLocale::default().code())
                        .tr(DoctorCheckKind::TunHealth.copy_keys().1)
                        .into_owned(),
                    state: DoctorStatus::Pass,
                    detail: Lang(UiLocale::default().code())
                        .tr(DoctorCheckKind::TunHealth.copy_keys().2)
                        .into_owned(),
                    hint: None,
                    fix_available: false,
                },
                DoctorCheckItem {
                    id: "chk-2".to_owned(),
                    kind: Some(DoctorCheckKind::SystemProxy),
                    detail_copy_key: Some(DoctorCheckKind::SystemProxy.copy_keys().2.into()),
                    name: Lang(UiLocale::default().code())
                        .tr(DoctorCheckKind::SystemProxy.copy_keys().0)
                        .into_owned(),
                    category: Lang(UiLocale::default().code())
                        .tr(DoctorCheckKind::SystemProxy.copy_keys().1)
                        .into_owned(),
                    state: DoctorStatus::Pass,
                    detail: Lang(UiLocale::default().code())
                        .tr(DoctorCheckKind::SystemProxy.copy_keys().2)
                        .into_owned(),
                    hint: None,
                    fix_available: false,
                },
                DoctorCheckItem {
                    id: "chk-3".to_owned(),
                    kind: Some(DoctorCheckKind::Ports),
                    detail_copy_key: Some(DoctorCheckKind::Ports.copy_keys().2.into()),
                    name: Lang(UiLocale::default().code())
                        .tr(DoctorCheckKind::Ports.copy_keys().0)
                        .into_owned(),
                    category: Lang(UiLocale::default().code())
                        .tr(DoctorCheckKind::Ports.copy_keys().1)
                        .into_owned(),
                    state: DoctorStatus::Pass,
                    detail: Lang(UiLocale::default().code())
                        .tr(DoctorCheckKind::Ports.copy_keys().2)
                        .into_owned(),
                    hint: None,
                    fix_available: false,
                },
                DoctorCheckItem {
                    id: "chk-4".to_owned(),
                    kind: Some(DoctorCheckKind::DnsPrivacy),
                    detail_copy_key: Some(DoctorCheckKind::DnsPrivacy.copy_keys().2.into()),
                    name: Lang(UiLocale::default().code())
                        .tr(DoctorCheckKind::DnsPrivacy.copy_keys().0)
                        .into_owned(),
                    category: Lang(UiLocale::default().code())
                        .tr(DoctorCheckKind::DnsPrivacy.copy_keys().1)
                        .into_owned(),
                    state: DoctorStatus::Pass,
                    detail: Lang(UiLocale::default().code())
                        .tr(DoctorCheckKind::DnsPrivacy.copy_keys().2)
                        .into_owned(),
                    hint: None,
                    fix_available: false,
                },
                DoctorCheckItem {
                    id: "chk-5".to_owned(),
                    kind: Some(DoctorCheckKind::Privileges),
                    detail_copy_key: Some(DoctorCheckKind::Privileges.copy_keys().2.into()),
                    name: Lang(UiLocale::default().code())
                        .tr(DoctorCheckKind::Privileges.copy_keys().0)
                        .into_owned(),
                    category: Lang(UiLocale::default().code())
                        .tr(DoctorCheckKind::Privileges.copy_keys().1)
                        .into_owned(),
                    state: DoctorStatus::Pass,
                    detail: Lang(UiLocale::default().code())
                        .tr(DoctorCheckKind::Privileges.copy_keys().2)
                        .into_owned(),
                    hint: None,
                    fix_available: false,
                },
                DoctorCheckItem {
                    id: "chk-6".to_owned(),
                    kind: Some(DoctorCheckKind::Configuration),
                    detail_copy_key: Some(DoctorCheckKind::Configuration.copy_keys().2.into()),
                    name: Lang(UiLocale::default().code())
                        .tr(DoctorCheckKind::Configuration.copy_keys().0)
                        .into_owned(),
                    category: Lang(UiLocale::default().code())
                        .tr(DoctorCheckKind::Configuration.copy_keys().1)
                        .into_owned(),
                    state: DoctorStatus::Pass,
                    detail: Lang(UiLocale::default().code())
                        .tr(DoctorCheckKind::Configuration.copy_keys().2)
                        .into_owned(),
                    hint: None,
                    fix_available: false,
                },
            ],
            watchdog: CoreWatchdogSnapshot::default(),
        }
    }

    pub fn passed_count(&self) -> usize {
        self.checks
            .iter()
            .filter(|c| c.state == DoctorStatus::Pass)
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
    let code = UiLocale::default();
    let summary = summary(
        projection.checks.iter().map(|check| check.state),
        projection.report_finished_at,
        code.code(),
    );
    let last_run_str = localize(
        code.code(),
        "doctor_last_run_value",
        &[("time", projection.last_run.clone())],
    );
    let watchdog_str = watchdog_status_text(&projection.watchdog, code.code());

    let check_scenes: Vec<Box<dyn Scene>> = projection
        .checks
        .iter()
        .map(|item| Box::new(check_row_scene(item, palette, code.code())) as Box<dyn Scene>)
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
            ScrollArea
            Children [
                @{ header_card_scene(summary, last_run_str, watchdog_str, palette) }
                --
                @{ self_heal::self_heal_card_scene(palette) }
                --
                @{ checks_container_scene(check_scenes, palette) }
            ]
    }
}

fn header_card_scene(
    summary: String,
    last_run: String,
    watchdog: String,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let header_a11y = accesskit::Node::new(accesskit::Role::Header);

    surface_scene(
        vec![
            Box::new(bsn! {
                        Node {
                            width: percent(100),
                            align_items: AlignItems::Center,
                            flex_direction: FlexDirection::Column,
                            row_gap: px(space::S12),
                        }
                        AccessibilityNode(header_a11y) LocalizedLabel::plain("doctor_header_label")
                        Children [
                            Node {
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(space::S12),
                            }
                            Children [
                                @{ icon_tile_scene(IconId::Activity, 36.0, palette) }
                                --
                                Node {
                                    flex_direction: FlexDirection::Column,
                                    row_gap: Val::Px(space::S4),
                                }
                                Children [
                                    Text(summary) DoctorLine(DoctorLineKind::Summary) TextRole(Role::Heading)
                                    --
                                    Text(last_run) DoctorLine(DoctorLineKind::LastRun) TextRole(Role::Caption)
                                    --
                                    Text(watchdog) DoctorLine(DoctorLineKind::Watchdog) TextRole(Role::Caption)
                                ]
                            ]
                            --
                            Node {
                                align_items: AlignItems::Center,
                                flex_wrap: FlexWrap::Wrap,
                                row_gap: px(space::S8),
                                column_gap: Val::Px(space::S8),
                            }
                            Children [
                                @{ (localized_button_scene(LocalizedText::plain("doctor_diagnose_action"), ButtonVariant::Primary, palette), bsn! { RunDoctorDiagnosticsButton }) }
                                --
                                @{ (localized_button_scene(LocalizedText::plain("doctor_btn_fix"), ButtonVariant::Default, palette), bsn! { RepairAllDoctorButton }) }
                                --
                                @{ (localized_button_scene(LocalizedText::plain("doctor_btn_bootstrap"), ButtonVariant::Default, palette), bsn! { BootstrapDoctorButton }) }
                                --
                                @{ (localized_button_scene(LocalizedText::plain("doctor_retry_action"), ButtonVariant::Default, palette), bsn! { RetryDoctorButton }) }
                            ]
                        ]
            }),
            Box::new(bsn! { Text::default() DoctorFeedbackText TextRole(Role::Caption) }),
        ],
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
                                LocalizedText::plain("doctor_suite_title") TextRole(Role::BodyStrong)
                                --
                                LocalizedText::plain("doctor_suite_hint") TextRole(Role::Caption)
                            ]
            }),
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(space::S8),
                            }
                            DoctorRows
                            Children [
                                { check_scenes }
                            ]
            }),
        ],
        palette,
    )
}

// ---- Plugin assembly and native observers -----------------------------------------------

/// Registers this page once during product assembly; mounting never resets its draft.
#[derive(Default)]
pub struct DoctorPagePlugin;

impl Plugin for DoctorPagePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<self_heal::DoctorSelfHealBoard>();
        app.add_observer(apply_doctor_projection);
        app.add_observer(refresh_rows);
        app.add_observer(self_heal::on_apply_best_node);
        app.add_systems(Update, self_heal::refresh_self_heal);
    }
}

pub(crate) fn apply_doctor_projection(
    update: On<DoctorProjectionUpdated>,
    mut last: Option<ResMut<LastDoctorProjection>>,
    locale: Option<Res<UiLocale>>,
    mut lines: Query<(&mut Text, &DoctorLine), With<DoctorLine>>,
) {
    let code = locale.as_ref().map_or("zh-CN", |locale| locale.code());
    let projection = &update.0;

    for (mut text, line) in &mut lines {
        match line.0 {
            DoctorLineKind::Summary => {
                text.0 = summary(
                    projection.checks.iter().map(|check| check.state),
                    projection.report_finished_at,
                    code,
                );
            }
            DoctorLineKind::LastRun => {
                text.0 = localize(
                    code,
                    "doctor_last_run_value",
                    &[("time", projection.last_run.clone())],
                );
            }
            DoctorLineKind::Watchdog => {
                text.0 = watchdog_status_text(&projection.watchdog, code);
            }
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
        assert_eq!(proj.checks[0].state, DoctorStatus::Pass);
    }
}
