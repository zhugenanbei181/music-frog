//! 3-Way Merge sync conflict component (字段级三向冲突差异合并).

use crate::pages::sync::{LastSyncProjection, SyncConflictInfo};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::query::{Has, QueryData, With};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::scene::CommandsSceneExt;
use bevy::scene::{Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, UiRect, Val,
    percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Button;
use infiltrator_application::sync_projection;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

/// Marker on the 3-Way Merge card root.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SyncMergeRoot;

/// Marker for accepting local configuration button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AcceptLocalButton;

/// Marker for accepting cloud configuration button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AcceptCloudButton;

/// Marker for merging both configurations button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MergeBothButton;
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct SyncMergeChoice;

/// Construct declarative scene for 3-Way Merge & Conflict Resolver card.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct SyncMergeFieldsSlot;
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct SyncMergeRow(pub usize);
#[derive(Resource, Default)]
pub struct SyncMergeRowsCache {
    slot: Option<Entity>,
    keys: Vec<String>,
}

fn field_scenes(conflict: Option<&SyncConflictInfo>, locale: &UiLocale) -> Vec<Box<dyn Scene>> {
    let fields = conflict
        .map(|conflict| conflict.conflicting_keys.as_slice())
        .unwrap_or_default();
    if fields.is_empty() {
        return vec![Box::new(bsn! {
            LocalizedText::plain("sync_observation_fields_unobserved") TextRole(Role::Body)
        })];
    }
    fields.iter().enumerate().map(|(index, field)| {
        let copy = sync_projection::conflict_field(&field.key, &field.local_value, &field.remote_value, locale.code());
        Box::new(bsn! {
            Node { width: percent(100), min_width: px(0.0), padding: UiRect::vertical(px(space::S4)) }
            Text(copy) SyncMergeRow(index) TextRole(Role::Body)
        }) as Box<dyn Scene>
    }).collect()
}

pub fn sync_three_way_merge_scene(
    conflict: Option<&SyncConflictInfo>,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let conflict_rows = field_scenes(conflict, &UiLocale::default());
    let unavailable = conflict.is_none_or(|conflict| conflict.conflicting_keys.is_empty());

    surface_scene(
        vec![
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::SpaceBetween,
                                padding: UiRect::bottom(Val::Px(space::S8)),
                            }
                            SyncMergeRoot
                            Children [
                                Node {
                                    align_items: AlignItems::Center,
                                    column_gap: Val::Px(space::S8),
                                }
                                Children [
                                    @{ icon_tile_scene(IconId::Network, 24.0, palette) }
                                    --
                                    LocalizedText::plain("sync_conflict_resolver_title") TextRole(Role::BodyStrong)
                                ]
                            ]
            }),
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(space::S4),
                                padding: UiRect::all(Val::Px(space::S8)),
                                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                            }
                            BackgroundColor({ palette.window_clear }) SyncMergeFieldsSlot
                            Children [
                                { conflict_rows }
                            ]
            }),
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(space::S8),
                                padding: UiRect::vertical(Val::Px(space::S4)),
                            }
                            Children [
                                Node {
                                    min_height: px(palette.control_height_px * 0.85),
                                    padding: UiRect::horizontal(Val::Px(space::S12)),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::Center,
                                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                                }
                                BackgroundColor({ palette.surface_elevated })
                                Button
                                AcceptLocalButton SyncMergeChoice ButtonDisabled(unavailable)
                                Children [
                                    LocalizedText::plain("sync_prefer_local_action") TextRole(Role::Body)
                                ]
                                --
                                Node {
                                    min_height: px(palette.control_height_px * 0.85),
                                    padding: UiRect::horizontal(Val::Px(space::S12)),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::Center,
                                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                                }
                                BackgroundColor({ palette.surface_elevated })
                                Button
                                AcceptCloudButton SyncMergeChoice ButtonDisabled(unavailable)
                                Children [
                                    LocalizedText::plain("sync_prefer_remote_action") TextRole(Role::Body)
                                ]
                                --
                                Node {
                                    min_height: px(palette.control_height_px * 0.85),
                                    padding: UiRect::horizontal(Val::Px(space::S12)),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::Center,
                                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                                }
                                BackgroundColor({ palette.accent })
                                Button
                                MergeBothButton SyncMergeChoice ButtonDisabled(unavailable)
                                Children [
                                    LocalizedText::plain("sync_merge_both_action") TextRole(Role::BodyStrong)
                                ]
                            ]
            }),
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                padding: UiRect::top(Val::Px(space::S4)),
                            }
                            Children [
                                LocalizedText::plain("sync_merge_hint") TextRole(Role::Caption)
                            ]
            }),
        ],
        palette,
    )
}

/// Only the read-only field subtree changes shape; native action controls remain mounted.
pub fn replay_fields(
    last: Res<LastSyncProjection>,
    locale: Res<UiLocale>,
    mut cache: ResMut<SyncMergeRowsCache>,
    slots: Query<(Entity, Option<&Children>), With<SyncMergeFieldsSlot>>,
    mut rows: Query<(&SyncMergeRow, &mut Text)>,
    mut commands: Commands,
) {
    let Some(projection) = last.0.as_ref() else {
        return;
    };
    let conflict = projection.conflict.as_ref();
    let fields = conflict
        .map(|conflict| conflict.conflicting_keys.as_slice())
        .unwrap_or_default();
    let keys = fields
        .iter()
        .map(|field| field.key.clone())
        .collect::<Vec<_>>();
    let Ok((slot, children)) = slots.single() else {
        return;
    };
    if cache.slot != Some(slot) || cache.keys != keys {
        if let Some(children) = children {
            for child in children {
                commands.entity(*child).despawn();
            }
        }
        let scenes = field_scenes(conflict, &locale);
        commands.spawn_scene(bsn! {
            Node { width: percent(100), min_width: px(0.0), flex_direction: FlexDirection::Column,
                row_gap: px(space::S4) } ChildOf(slot) Children [{ scenes }]
        });
        cache.slot = Some(slot);
        cache.keys = keys;
        return;
    }
    for (row, mut text) in &mut rows {
        if let Some(field) = fields.get(row.0) {
            let value = sync_projection::conflict_field(
                &field.key,
                &field.local_value,
                &field.remote_value,
                locale.code(),
            );
            if text.0 != value {
                text.0 = value;
            }
        }
    }
}

#[derive(QueryData)]
#[query_data(mutable)]
pub struct MergeChoiceView {
    disabled: &'static mut ButtonDisabled,
    fill: &'static mut BackgroundColor,
    children: &'static Children,
    merge: Has<MergeBothButton>,
}

/// Missing field observations disable both SDK input and its visible affordance.
pub fn replay_availability(
    last: Res<LastSyncProjection>,
    palette: Res<UiPalette>,
    mut choices: Query<MergeChoiceView, With<SyncMergeChoice>>,
    mut labels: Query<&mut TextColor>,
) {
    let unavailable = last
        .0
        .as_ref()
        .and_then(|projection| projection.conflict.as_ref())
        .is_none_or(|conflict| conflict.conflicting_keys.is_empty());
    for mut choice in &mut choices {
        choice.disabled.0 = unavailable;
        choice.fill.0 = if !unavailable && choice.merge {
            palette.accent
        } else {
            palette.surface_elevated
        };
        let ink = if unavailable {
            palette.ink_dim
        } else if choice.merge {
            palette.on_accent
        } else {
            palette.ink
        };
        for child in choice.children {
            if let Ok(mut color) = labels.get_mut(*child) {
                color.0 = ink;
            }
        }
    }
}
