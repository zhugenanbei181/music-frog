//! Projection-driven Windows UWP loopback controls for App Routing.

#[path = "app_routing_uwp_query_access.rs"]
pub mod query_access;
use self::query_access::{
    ApplyProjectionNamesFilter, ApplyProjectionStatesFilter, ApplyProjectionStatusLinesFilter,
};

use super::app_routing::{AppRoutingProjection, AppRoutingProjectionUpdated};
use crate::command::{CommandSinkHandle, UiCommand};
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::system::{Query, Res};
use bevy::scene::{Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, UiRect, Val,
    percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_application::routing_projection::{uwp_empty, uwp_state_key, uwp_summary};
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::uwp::UwpPackageSnapshot;
use infiltrator_shared::locales::{Lang, Localizer};

/// Marker for the UWP exemption card root.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UwpExemptionRoot;

/// Marker for the live UWP status line.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UwpStatusLine;

#[derive(Component, Clone, Copy, Debug, Default)]
pub struct UwpEmptyLine;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UwpAction {
    #[default]
    Scan,
    ExemptAll,
    ClearAll,
}

/// Marker carrying the action represented by a UWP control.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UwpActionButton(pub UwpAction);

/// Marker carrying the package identity for a per-app toggle.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct ToggleUwpButton {
    pub sid: String,
    pub exempt: bool,
}

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UwpPackageName(pub usize);

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UwpPackageState(pub usize);

/// Scene constructor for the UWP Loopback Exemption card.
pub fn uwp_exemption_scene(
    projection: &AppRoutingProjection,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let packages = &projection.uwp_loopback.packages;
    let app_rows: Vec<Box<dyn Scene>> = packages
        .iter()
        .enumerate()
        .map(|(index, package)| {
            Box::new(package_row_scene(index, package, palette)) as Box<dyn Scene>
        })
        .collect();
    let app_rows = if app_rows.is_empty() {
        vec![Box::new(bsn! {
                    Node {
                        width: percent(100),
                        padding: UiRect::vertical(Val::Px(space::S4)),
                    }
                    Children [
                        Text({ uwp_empty(&projection.uwp_loopback.availability, "en-US") }) UwpEmptyLine TextRole(Role::Caption)
                    ]
        }) as Box<dyn Scene>]
    } else {
        app_rows
    };
    let status = uwp_summary(
        &projection.uwp_loopback.availability,
        projection
            .uwp_loopback
            .packages
            .iter()
            .map(|package| package.loopback_exempt),
        "en-US",
    );

    surface_scene(
        vec![
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::SpaceBetween,
                                padding: UiRect::bottom(Val::Px(space::S8)),
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(space::S8),
                            }
                            UwpExemptionRoot
                            Children [
                                Node {
                                    width: percent(100),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::SpaceBetween,
                                }
                                Children [
                                    Node {
                                        align_items: AlignItems::Center,
                                        column_gap: Val::Px(space::S8),
                                    }
                                    Children [
                                        @{ icon_tile_scene(IconId::Settings, 24.0, palette) }
                                        --
                                        LocalizedText::plain("settings_uwp_loopback_title") TextRole(Role::BodyStrong)
                                    ]
                                    --
                                    Node {
                                        align_items: AlignItems::Center,
                                        column_gap: Val::Px(space::S4),
                                    }
                                    Children [
                                        @{ action_button("uwp_btn_scan", UwpAction::Scan, palette) }
                                        --
                                        @{ action_button("uwp_btn_exempt_all", UwpAction::ExemptAll, palette) }
                                        --
                                        @{ action_button("uwp_btn_clear_all", UwpAction::ClearAll, palette) }
                                    ]
                                ]
                                --
                                Text(status) UwpStatusLine TextRole(Role::Caption)
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
                            BackgroundColor({ palette.window_clear })
                            Children [
                                { app_rows }
                            ]
            }),
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                padding: UiRect::top(Val::Px(space::S4)),
                            }
                            Children [
                                LocalizedText::plain("settings_uwp_loopback_hint") TextRole(Role::Caption)
                            ]
            }),
        ],
        palette,
    )
}

fn action_button(
    label: &'static str,
    action: UwpAction,
    palette: &UiPalette,
) -> impl Scene + use<> {
    bsn! {
            Node {
                min_height: px(palette.control_height_px),
                padding: UiRect::horizontal(Val::Px(space::S8)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
            }
            BackgroundColor({ palette.accent })
            UwpActionButton(action)
            Button
            Children [
                LocalizedText::plain(label) TextRole(Role::Caption)
            ]
    }
}

fn package_row_scene(
    index: usize,
    package: &UwpPackageSnapshot,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let state = Lang("en-US")
        .tr(uwp_state_key(package.loopback_exempt))
        .into_owned();
    let color = if package.loopback_exempt {
        palette.success
    } else {
        palette.ink_dim
    };
    bsn! {
            Node {
                width: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::vertical(Val::Px(space::S4)),
            }
            ToggleUwpButton {
                sid: { package.sid.clone() },
                exempt: { package.loopback_exempt },
            }
            Button
            Children [
                Text({ package.display_name.clone() }) UwpPackageName(index) TextRole(Role::Caption)
                --
                Node {
                    padding: UiRect::axes(Val::Px(space::S6), Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                }
                BackgroundColor({ color })
                Children [
                    Text({ state.to_owned() }) UwpPackageState(index) TextColor({ palette.on_accent }) TextRole(Role::Caption)
                ]
            ]
    }
}

pub(super) fn on_action_activated(
    activate: On<Activate>,
    action_buttons: Query<&UwpActionButton>,
    toggle_buttons: Query<&ToggleUwpButton>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    if let Ok(action) = action_buttons.get(activate.entity) {
        match action.0 {
            UwpAction::Scan => handle.submit(UiCommand::ScanUwpApps),
            UwpAction::ExemptAll => {
                handle.submit(UiCommand::SetAllUwpExemptions { exempt: true });
            }
            UwpAction::ClearAll => {
                handle.submit(UiCommand::SetAllUwpExemptions { exempt: false });
            }
        }
    } else if let Ok(button) = toggle_buttons.get(activate.entity) {
        handle.submit(UiCommand::SetUwpAppExemption {
            sid: button.sid.clone(),
            exempt: !button.exempt,
        });
    }
}

pub(super) fn apply_projection(
    update: On<AppRoutingProjectionUpdated>,
    locale: Res<UiLocale>,
    mut status_lines: Query<&mut Text, ApplyProjectionStatusLinesFilter>,
    mut names: Query<(&mut Text, &UwpPackageName), ApplyProjectionNamesFilter>,
    mut states: Query<(&mut Text, &UwpPackageState), ApplyProjectionStatesFilter>,
    mut buttons: Query<&mut ToggleUwpButton>,
) {
    let status = uwp_summary(
        &update.0.uwp_loopback.availability,
        update
            .0
            .uwp_loopback
            .packages
            .iter()
            .map(|package| package.loopback_exempt),
        locale.code(),
    );
    for mut line in &mut status_lines {
        line.0 = status.clone();
    }
    for (mut text, marker) in &mut names {
        text.0 = update
            .0
            .uwp_loopback
            .packages
            .get(marker.0)
            .map(|package| package.display_name.clone())
            .unwrap_or_else(|| "(package no longer present)".to_owned());
    }
    for (mut text, marker) in &mut states {
        text.0 = update
            .0
            .uwp_loopback
            .packages
            .get(marker.0)
            .map(|package| {
                Lang(locale.code())
                    .tr(uwp_state_key(package.loopback_exempt))
                    .into_owned()
            })
            .unwrap_or_else(|| "Unavailable".to_owned());
    }
    for mut button in &mut buttons {
        if let Some(package) = update
            .0
            .uwp_loopback
            .packages
            .iter()
            .find(|package| package.sid == button.sid)
        {
            button.exempt = package.loopback_exempt;
        }
    }
}
