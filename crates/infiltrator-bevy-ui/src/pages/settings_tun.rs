//! Projection restamping for TUN checkboxes.

use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::{Has, With};
use bevy::ecs::system::{Commands, Query};
use bevy::ui::Checked;
use bevy::ui_widgets::Checkbox;

use super::settings_core::{TunEnableToggle, TunRouteToggle, TunRouteToggleKind};
use super::SettingsProjectionUpdated;

pub(super) fn apply_tun_toggle_projection(
    update: bevy::ecs::observer::On<SettingsProjectionUpdated>,
    mut route_toggles: Query<(&TunRouteToggle, &Children)>,
    mut enable_toggles: Query<&Children, With<TunEnableToggle>>,
    checkboxes: Query<(Entity, Has<Checked>), With<Checkbox>>,
    mut commands: Commands,
) {
    let projection = &update.0;
    for (toggle, children) in &mut route_toggles {
        let wanted = match toggle.0 {
            TunRouteToggleKind::AutoRoute => projection.tun_auto_route,
            TunRouteToggleKind::StrictRoute => projection.tun_strict_route,
        };
        restamp_checkbox(&mut commands, &checkboxes, children, wanted);
    }
    for children in &mut enable_toggles {
        restamp_checkbox(&mut commands, &checkboxes, children, projection.tun_enabled);
    }
}

fn restamp_checkbox(
    commands: &mut Commands,
    checkboxes: &Query<(Entity, Has<Checked>), With<Checkbox>>,
    children: &Children,
    wanted: bool,
) {
    for child in children.iter() {
        if let Ok((entity, checked)) = checkboxes.get(*child)
            && checked != wanted
        {
            if wanted {
                commands.entity(entity).insert(Checked);
            } else {
                commands.entity(entity).remove::<Checked>();
            }
        }
    }
}
