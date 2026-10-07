//! Stable diagnostic identities preserve native controls through updates and locale replay.
use crate::localized_widgets::localized_button_scene;
use crate::pages::doctor::{
    CheckDetailText, CheckStateText, DoctorCheckItem, DoctorLine, DoctorLineKind,
    DoctorProjectionUpdated, LastDoctorProjection, RepairDoctorRowButton, check_state_color,
};
use bevy::ecs::change_detection::DetectChanges;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::observer::On;
use bevy::ecs::query::{QueryData, With};
use bevy::ecs::system::{Commands, Query, Res};
use bevy::scene::{CommandsSceneExt, Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::widget::Text;
use bevy::ui::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, Node, UiRect, percent, px,
};
use infiltrator_application::doctor_projection::{
    check_detail, check_name, status_key, summary, watchdog_status_text,
};
use infiltrator_bevy_widgets::button::{ButtonDisabled, ButtonVariant};
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_shared::i18n_interpolator::localize;
use infiltrator_shared::locales::{Lang, Localizer};
use std::collections::HashMap;

#[derive(Component, Clone, Default)]
pub struct DoctorRows;
#[derive(Component, Clone, Default)]
pub struct DoctorRow(pub String);
#[derive(Component, Clone, Default)]
pub struct DoctorName(pub String);
#[derive(Component, Clone, Default)]
pub struct DoctorHint(pub String);

pub fn check_row_scene(
    check: &DoctorCheckItem,
    palette: &UiPalette,
    code: &str,
) -> impl Scene + use<> {
    let id = check.id.clone();
    bsn! {
        Node {
            width: percent(100),
            min_width: px(0),
            flex_direction: FlexDirection::Column,
            row_gap: px(space::S4),
            padding: UiRect::all(px(space::S8)),
            border_radius: BorderRadius::all(px(palette.control_radius_px)),
        }
        DoctorRow({ id.clone() })
        BackgroundColor({ palette.surface_elevated })
        Children [
            Text({ check_name(check.kind, &check.name, &check.category, code) })
            DoctorName({ id.clone() }) TextRole(Role::BodyStrong)
            --
            Text({ check_detail(check.detail_copy_key.as_deref(), &check.detail, code) })
            CheckDetailText({ id.clone() }) TextRole(Role::Caption)
            --
            Text({ check.hint.clone().unwrap_or_default() })
            DoctorHint({ id.clone() }) TextRole(Role::Caption)
            --
            Node {
                align_items: AlignItems::Center,
                column_gap: px(space::S12),
            }
            Children [
                Text({ Lang(code).tr(status_key(check.state)).into_owned() })
                CheckStateText({ id.clone() }) TextRole(Role::BodyStrong)
                TextColor({ check_state_color(check.state, palette) })
                --
                @{ (
                    localized_button_scene(LocalizedText::plain("doctor_repair_row"), ButtonVariant::Default, palette),
                    bsn! { RepairDoctorRowButton { check_id: id } ButtonDisabled({ !check.fix_available }) },
                ) }
            ]
        ]
    }
}
#[derive(QueryData)]
#[query_data(mutable)]
pub struct RowText {
    text: &'static mut Text,
    color: Option<&'static mut TextColor>,
    name: Option<&'static DoctorName>,
    detail: Option<&'static CheckDetailText>,
    status: Option<&'static CheckStateText>,
    hint: Option<&'static DoctorHint>,
}
fn apply_copy(
    checks: &[DoctorCheckItem],
    code: &str,
    palette: &UiPalette,
    texts: &mut Query<RowText>,
) {
    for mut row in texts.iter_mut() {
        let id = row
            .name
            .map(|v| &v.0)
            .or_else(|| row.detail.map(|v| &v.0))
            .or_else(|| row.status.map(|v| &v.0))
            .or_else(|| row.hint.map(|v| &v.0));
        let Some(check) = id.and_then(|id| checks.iter().find(|check| check.id == *id)) else {
            continue;
        };
        let value = if row.name.is_some() {
            check_name(check.kind, &check.name, &check.category, code)
        } else if row.detail.is_some() {
            check_detail(check.detail_copy_key.as_deref(), &check.detail, code)
        } else if row.hint.is_some() {
            check.hint.clone().unwrap_or_default()
        } else {
            if let Some(color) = row.color.as_mut() {
                color.0 = check_state_color(check.state, palette);
            }
            Lang(code).tr(status_key(check.state)).into_owned()
        };
        if row.text.0 != value {
            row.text.0 = value;
        }
    }
}
pub fn refresh_rows(
    update: On<DoctorProjectionUpdated>,
    locale: Option<Res<UiLocale>>,
    palette: Res<UiPalette>,
    mut texts: Query<RowText>,
) {
    apply_copy(
        &update.0.checks,
        locale.as_ref().map_or("zh-CN", |locale| locale.code()),
        &palette,
        &mut texts,
    );
}
pub fn replay_copy(
    last: Res<LastDoctorProjection>,
    locale: Option<Res<UiLocale>>,
    palette: Res<UiPalette>,
    mut texts: Query<RowText>,
) {
    if let Some(projection) = &last.0 {
        apply_copy(
            &projection.checks,
            locale.as_ref().map_or("zh-CN", |locale| locale.code()),
            &palette,
            &mut texts,
        );
    }
}
pub fn reconcile_rows(
    mut commands: Commands,
    last: Res<LastDoctorProjection>,
    palette: Res<UiPalette>,
    locale: Option<Res<UiLocale>>,
    roots: Query<(Entity, Option<&Children>), With<DoctorRows>>,
    rows: Query<&DoctorRow>,
) {
    if !last.is_changed() {
        return;
    }
    let Some(projection) = &last.0 else { return };
    for (root, children) in &roots {
        let existing: HashMap<_, _> = children
            .into_iter()
            .flat_map(|children| children.iter().copied())
            .filter_map(|entity| rows.get(entity).ok().map(|row| (row.0.as_str(), entity)))
            .collect();
        for check in &projection.checks {
            if !existing.contains_key(check.id.as_str()) {
                commands
                    .spawn_scene(check_row_scene(
                        check,
                        &palette,
                        locale.as_ref().map_or("zh-CN", |locale| locale.code()),
                    ))
                    .insert(ChildOf(root));
            }
        }
        for (id, entity) in existing {
            if !projection.checks.iter().any(|check| check.id == id) {
                commands.entity(entity).despawn();
            }
        }
    }
}
pub fn sort_rows(
    last: Res<LastDoctorProjection>,
    mut roots: Query<&mut Children, With<DoctorRows>>,
    rows: Query<&DoctorRow>,
) {
    let Some(projection) = &last.0 else { return };
    for mut children in &mut roots {
        children.sort_by_key(|entity| {
            rows.get(*entity)
                .ok()
                .and_then(|row| projection.checks.iter().position(|check| check.id == row.0))
                .unwrap_or(usize::MAX)
        });
    }
}

pub fn replay_header(
    last: Res<LastDoctorProjection>,
    locale: Option<Res<UiLocale>>,
    mut lines: Query<(&mut Text, &DoctorLine)>,
) {
    let Some(projection) = &last.0 else { return };
    let code = locale.as_ref().map_or("zh-CN", |locale| locale.code());
    for (mut text, line) in &mut lines {
        let value = match line.0 {
            DoctorLineKind::Summary => summary(
                projection.checks.iter().map(|check| check.state),
                projection.report_finished_at,
                code,
            ),
            DoctorLineKind::LastRun => localize(
                code,
                "doctor_last_run_value",
                &[("time", projection.last_run.clone())],
            ),
            DoctorLineKind::Watchdog => watchdog_status_text(&projection.watchdog, code),
        };
        if text.0 != value {
            text.0 = value;
        }
    }
}
