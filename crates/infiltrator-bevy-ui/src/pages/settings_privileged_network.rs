//! Bevy Settings projection for privileged network regression transactions.

use super::LastSettingsProjection;
use super::settings_core::SettingsProjection;
use crate::command::{CommandSinkHandle, UiCommand};
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::system::{Query, Res};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, UiRect, Val,
    percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_application::host_network_projection::privileged;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::interaction_block::InteractionBlocked;
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PrivilegedNetworkRunButton;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PrivilegedNetworkStatusLine;

pub(super) fn scene(projection: &SettingsProjection, palette: &UiPalette) -> Box<dyn Scene> {
    let view = privileged(&projection.privileged_network, UiLocale::default().code());
    Box::new(surface_scene(
        vec![Box::new(bsn! {
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
                            LocalizedText::plain("settings_privileged_network_title") TextRole(Role::Body)
                            --
                            Text({format!("{} · {}",view.status,view.details)}) PrivilegedNetworkStatusLine TextRole(Role::Mono)
                        ]
                        --
                        Node {
                            min_height: px(palette.control_height_px),
                            padding: UiRect::horizontal(Val::Px(space::S12)),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                        }
                        BackgroundColor({ palette.accent })
                        PrivilegedNetworkRunButton
                        Button
                        Children [
                            LocalizedText::plain("privileged_network_run") TextRole(Role::BodyStrong)
                        ]
                    ]
        })],
        palette,
    ))
}

pub(super) fn on_action_activated(
    activate: On<Activate>,
    buttons: Query<&ButtonDisabled, With<PrivilegedNetworkRunButton>>,
    blocked: Query<(), With<InteractionBlocked>>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    if !blocked.contains(activate.entity)
        && buttons
            .get(activate.entity)
            .is_ok_and(|disabled| !disabled.0)
        && let Some(handle) = handle
    {
        handle.submit(UiCommand::RunPrivilegedNetworkRegression);
    }
}
pub(super) fn replay(
    last: Res<LastSettingsProjection>,
    locale: Res<UiLocale>,
    mut lines: Query<&mut Text, With<PrivilegedNetworkStatusLine>>,
    mut buttons: Query<&mut ButtonDisabled, With<PrivilegedNetworkRunButton>>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(projection) = last.0.as_ref() else {
        return;
    };
    let view = privileged(&projection.privileged_network, locale.code());
    let next = format!("{} · {}", view.status, view.details);
    for mut text in &mut lines {
        if text.0 != next {
            text.0 = next.clone();
        }
    }
    for mut disabled in &mut buttons {
        let enabled = handle.is_some() && view.start_enabled;
        if disabled.0 == enabled {
            disabled.0 = !enabled;
        }
    }
}
