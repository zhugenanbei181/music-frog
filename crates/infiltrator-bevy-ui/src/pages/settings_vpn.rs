//! Bevy Settings projection for Android VpnService lifecycle.

use super::LastSettingsProjection;
use super::settings_core::SettingsProjection;
use crate::command::{CommandSinkHandle, UiCommand};
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::query::{Has, Or, QueryData, QueryFilter};
use bevy::ecs::system::{Query, Res};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, UiRect, Val,
    percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_application::host_network_projection::vpn;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::interaction_block::InteractionBlocked;
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_shared::locales::{Lang, Localizer};

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VpnStartButton;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VpnStopButton;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VpnStatusLine;
#[derive(Component, Clone, Copy, Default)]
pub struct VpnStartLabel;
#[derive(Component, Clone, Copy, Default)]
pub struct VpnStopLabel;

pub(super) fn scene(projection: &SettingsProjection, palette: &UiPalette) -> Box<dyn Scene> {
    let locale = UiLocale::default();
    let view = vpn(&projection.vpn, locale.code());
    Box::new(surface_scene(
        vec![Box::new(bsn! {
                    Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(space::S6),
                    }
                    Children [
                        LocalizedText::plain("settings_android_vpn_title") TextRole(Role::BodyStrong)
                        --
                        Node {
                            width: percent(100),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::SpaceBetween,
                            padding: UiRect::all(Val::Px(space::S8)),
                            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                        }
                        BackgroundColor({ palette.surface_elevated })
                        Children [
                            Text({format!("{} · {}",view.status,view.details)}) VpnStatusLine TextRole(Role::Mono)
                            --
                            Node {
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(space::S6),
                            }
                            Children [
                                Node {
                                    min_height: px(palette.control_height_px),
                                    padding: UiRect::horizontal(Val::Px(space::S12)),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::Center,
                                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                                }
                                BackgroundColor({ palette.accent })
                                VpnStartButton
                                Button
                                Children [
                                    Text({Lang(locale.code()).tr(view.start_key).into_owned()}) VpnStartLabel TextRole(Role::BodyStrong)
                                ]
                                --
                                Node {
                                    min_height: px(palette.control_height_px),
                                    padding: UiRect::horizontal(Val::Px(space::S12)),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::Center,
                                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                                }
                                BackgroundColor({ palette.surface_elevated })
                                VpnStopButton
                                Button
                                Children [
                                    Text({Lang(locale.code()).tr(view.stop_key).into_owned()}) VpnStopLabel TextRole(Role::Body)
                                ]
                            ]
                        ]
                    ]
        })],
        palette,
    ))
}

pub(super) fn on_action_activated(
    activate: On<Activate>,
    start_buttons: Query<&ButtonDisabled, With<VpnStartButton>>,
    stop_buttons: Query<&ButtonDisabled, With<VpnStopButton>>,
    blocked: Query<(), With<InteractionBlocked>>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    if blocked.contains(activate.entity) {
        return;
    }
    if start_buttons
        .get(activate.entity)
        .is_ok_and(|disabled| !disabled.0)
    {
        handle.submit(UiCommand::StartVpn);
    } else if stop_buttons
        .get(activate.entity)
        .is_ok_and(|disabled| !disabled.0)
    {
        handle.submit(UiCommand::StopVpn);
    }
}
#[derive(QueryData)]
#[query_data(mutable)]
pub struct VpnText {
    text: &'static mut Text,
    status: Has<VpnStatusLine>,
    start: Has<VpnStartLabel>,
    stop: Has<VpnStopLabel>,
}
#[derive(QueryFilter)]
pub struct VpnTextFilter {
    lines: Or<(With<VpnStatusLine>, With<VpnStartLabel>, With<VpnStopLabel>)>,
}
#[derive(QueryData)]
#[query_data(mutable)]
pub struct VpnButton {
    disabled: &'static mut ButtonDisabled,
    start: Has<VpnStartButton>,
    stop: Has<VpnStopButton>,
}
#[derive(QueryFilter)]
pub struct VpnButtonFilter {
    buttons: Or<(With<VpnStartButton>, With<VpnStopButton>)>,
}
pub(super) fn replay(
    last: Res<LastSettingsProjection>,
    locale: Res<UiLocale>,
    mut lines: Query<VpnText, VpnTextFilter>,
    mut buttons: Query<VpnButton, VpnButtonFilter>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(projection) = last.0.as_ref() else {
        return;
    };
    let view = vpn(&projection.vpn, locale.code());
    for parts in &mut lines {
        let VpnTextItem {
            mut text,
            status,
            start,
            stop,
        } = parts;
        let next = if status {
            format!("{} · {}", view.status, view.details)
        } else if start {
            Lang(locale.code()).tr(view.start_key).into_owned()
        } else if stop {
            Lang(locale.code()).tr(view.stop_key).into_owned()
        } else {
            continue;
        };
        if text.0 != next {
            text.0 = next;
        }
    }
    for parts in &mut buttons {
        let VpnButtonItem {
            mut disabled,
            start,
            stop,
        } = parts;
        let enabled =
            handle.is_some() && ((start && view.start_enabled) || (stop && view.stop_enabled));
        if disabled.0 == enabled {
            disabled.0 = !enabled;
        }
    }
}
