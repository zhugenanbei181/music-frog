//! Native diagnostic rows retain source identity and shared tone through report and locale changes.
use crate::pages::dns::LastDnsProjection;
use crate::pages::dns_leak::tone_color;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::query::{QueryData, QueryFilter, With, Without};
use bevy::ecs::system::{Commands, Query, Res};
use bevy::scene::{CommandsSceneExt, Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::widget::Text;
use bevy::ui::{Display, FlexDirection, Node, percent, px};
use infiltrator_application::dns_leak_projection::{LeakDisplay, LeakRow, project_leak};
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use std::collections::HashMap;

#[derive(Component, Clone, Default)]
pub struct LeakRows;
#[derive(Component, Clone, Default)]
pub struct LeakRowIdentity(pub String, pub String, pub String);
#[derive(Component, Clone, Default)]
pub struct LeakRowLabel(pub LeakRowIdentity, pub bool);
#[derive(Component, Clone, Default)]
pub struct LeakEmpty;
fn identity(row: &LeakRow) -> LeakRowIdentity {
    LeakRowIdentity(
        row.resolver.clone(),
        row.authority.clone(),
        row.question.clone(),
    )
}
fn key(identity: &LeakRowIdentity) -> (&str, &str, &str) {
    (&identity.0, &identity.1, &identity.2)
}
pub fn rows_scene(display: &LeakDisplay, palette: &UiPalette) -> impl Scene + use<> {
    let rows: Vec<Box<dyn Scene>> = display
        .rows
        .iter()
        .map(|row| Box::new(row_scene(row, palette)) as Box<dyn Scene>)
        .collect();
    bsn! {
        Node { width: percent(100), flex_direction: FlexDirection::Column, row_gap: px(space::S8) }
        LeakRows
        Children [
            Text({ display.empty.clone() }) LeakEmpty TextRole(Role::Caption)
            --
            { rows }
        ]
    }
}
fn row_scene(row: &LeakRow, palette: &UiPalette) -> impl Scene + use<> {
    let source = format!("{} → {}", row.resolver, row.authority);
    let id = identity(row);
    bsn! {
        Node { width: percent(100), flex_direction: FlexDirection::Column, row_gap: px(space::S4) }
        LeakRowIdentity({ id.0.clone() }, { id.1.clone() }, { id.2.clone() })
        Children [
            Text(source) LeakRowLabel({ id.clone() }, false) TextRole(Role::Mono)
            --
            Text({ row.outcome.clone() }) LeakRowLabel(id, true) TextRole(Role::Body)
            TextColor({ tone_color(row.tone, palette) })
        ]
    }
}
pub fn reconcile(
    mut commands: Commands,
    last: Res<LastDnsProjection>,
    locale: Res<UiLocale>,
    palette: Res<UiPalette>,
    containers: Query<(Entity, Option<&Children>), With<LeakRows>>,
    rows: Query<&LeakRowIdentity>,
) {
    let Some(projection) = &last.0 else { return };
    let display = project_leak(&projection.leak, locale.code());
    for (container, children) in &containers {
        let existing: HashMap<_, _> = children
            .into_iter()
            .flat_map(|children| children.iter().copied())
            .filter_map(|entity| rows.get(entity).ok().map(|row| (key(row), entity)))
            .collect();
        for row in &display.rows {
            let id = identity(row);
            if !existing.contains_key(&key(&id)) {
                commands
                    .spawn_scene(row_scene(row, &palette))
                    .insert(ChildOf(container));
            }
        }
        for (id, entity) in existing {
            if !display.rows.iter().any(|row| {
                id == (
                    row.resolver.as_str(),
                    row.authority.as_str(),
                    row.question.as_str(),
                )
            }) {
                commands.entity(entity).despawn();
            }
        }
    }
}
#[derive(QueryData)]
#[query_data(mutable)]
pub struct RowText {
    text: &'static mut Text,
    color: &'static mut TextColor,
    label: &'static LeakRowLabel,
}
#[derive(QueryData)]
#[query_data(mutable)]
pub struct EmptyLeakText {
    text: &'static mut Text,
    node: &'static mut Node,
}
#[derive(QueryFilter)]
pub struct EmptyLeakFilter {
    empty: With<LeakEmpty>,
    not_row: Without<LeakRowLabel>,
}
pub fn replay(
    last: Res<LastDnsProjection>,
    locale: Res<UiLocale>,
    palette: Res<UiPalette>,
    mut text: Query<RowText, Without<LeakEmpty>>,
    mut empty: Query<EmptyLeakText, EmptyLeakFilter>,
) {
    let Some(projection) = &last.0 else { return };
    let display = project_leak(&projection.leak, locale.code());
    for mut text in &mut text {
        let Some(row) = display.rows.iter().find(|row| {
            key(&text.label.0)
                == (
                    row.resolver.as_str(),
                    row.authority.as_str(),
                    row.question.as_str(),
                )
        }) else {
            continue;
        };
        let value = if text.label.1 {
            row.outcome.clone()
        } else {
            format!("{} → {}", row.resolver, row.authority)
        };
        if text.text.0 != value {
            text.text.0 = value;
        }
        if text.label.1 {
            text.color.0 = tone_color(row.tone, &palette);
        }
    }
    for mut empty in &mut empty {
        let visible = if display.rows.is_empty() {
            Display::Flex
        } else {
            Display::None
        };
        if empty.node.display != visible {
            empty.node.display = visible;
        }
        if empty.text.0 != display.empty {
            empty.text.0 = display.empty.clone();
        }
    }
}

pub fn sort(
    last: Res<LastDnsProjection>,
    mut containers: Query<&mut Children, With<LeakRows>>,
    rows: Query<&LeakRowIdentity>,
) {
    let Some(projection) = &last.0 else { return };
    for mut children in &mut containers {
        children.sort_by_key(|entity| {
            rows.get(*entity)
                .ok()
                .and_then(|id| {
                    projection.leak.observations.iter().position(|row| {
                        key(id)
                            == (
                                row.resolver.as_str(),
                                row.authority.as_str(),
                                row.question.as_str(),
                            )
                    })
                })
                .unwrap_or(usize::MAX)
        });
    }
}
