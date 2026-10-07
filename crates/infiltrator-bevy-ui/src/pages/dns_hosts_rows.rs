//! Stable native rows survive edits and language changes; removed identities never replay actions.
use crate::localized_widgets::localized_button_scene;
use crate::pages::dns_hosts::{
    DnsHostsEditorState, HostAction, HostRowIdentity, HostRowText, HostsRows,
};
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::query::With;
use bevy::ecs::system::{Commands, Query, Res};
use bevy::scene::{CommandsSceneExt, Scene, bsn};
use bevy::ui::widget::Text;
use bevy::ui::{AlignItems, Node, percent, px};
use infiltrator_application::dns_hosts_editor::HostDraftRow;
use infiltrator_bevy_widgets::button::ButtonVariant;
use infiltrator_bevy_widgets::localization::LocalizedText;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use std::collections::HashMap;
fn row_scene(row: &HostDraftRow, palette: &UiPalette) -> impl Scene + use<> {
    let id = row.id;
    bsn! {
        Node { width: percent(100), align_items: AlignItems::Center, column_gap: px(8.0) }
        HostRowIdentity(id)
        Children [
            Node { width: percent(22), min_width: px(0.0) }
            Children [ Text({ row.entry.address.clone() }) HostRowText(id, true) TextRole(Role::Mono) ]
            --
            Node { width: percent(28), min_width: px(0.0) }
            Children [ Text({ row.entry.domain.clone() }) HostRowText(id, false) TextRole(Role::Caption) ]
            --
            @{ (localized_button_scene(LocalizedText::plain("dns_hosts_edit"), ButtonVariant::Default, palette), bsn! { HostAction::Edit(id) }) }
            --
            @{ (localized_button_scene(LocalizedText::plain("dns_hosts_remove"), ButtonVariant::Default, palette), bsn! { HostAction::Remove(id) }) }
        ]
    }
}
pub fn reconcile(
    mut commands: Commands,
    state: Res<DnsHostsEditorState>,
    palette: Res<UiPalette>,
    lists: Query<(Entity, Option<&Children>), With<HostsRows>>,
    rows: Query<&HostRowIdentity>,
) {
    for (list, children) in &lists {
        let existing: HashMap<_, _> = children
            .into_iter()
            .flat_map(|children| children.iter())
            .filter_map(|entity| rows.get(*entity).ok().map(|row| (row.0, *entity)))
            .collect();
        for row in &state.editor.rows {
            if !existing.contains_key(&row.id) {
                commands
                    .spawn_scene(row_scene(row, &palette))
                    .insert(ChildOf(list));
            }
        }
        for (id, entity) in existing {
            if !state.editor.rows.iter().any(|row| row.id == id) {
                commands.entity(entity).despawn();
            }
        }
    }
}
pub fn sort(
    state: Res<DnsHostsEditorState>,
    mut lists: Query<&mut Children, With<HostsRows>>,
    rows: Query<&HostRowIdentity>,
) {
    for mut children in &mut lists {
        children.sort_by_key(|entity| {
            rows.get(*entity)
                .ok()
                .and_then(|id| state.editor.rows.iter().position(|row| row.id == id.0))
                .unwrap_or(usize::MAX)
        });
    }
}
pub fn replay(state: Res<DnsHostsEditorState>, mut labels: Query<(&mut Text, &HostRowText)>) {
    for (mut text, label) in &mut labels {
        if let Some(row) = state.editor.rows.iter().find(|row| row.id == label.0) {
            let value = if label.1 {
                &row.entry.address
            } else {
                &row.entry.domain
            };
            if &text.0 != value {
                text.0.clone_from(value);
            }
        }
    }
}
