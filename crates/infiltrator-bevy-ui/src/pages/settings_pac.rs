//! Bevy Settings controls for the shared PAC generator and local service.

use super::SettingsProjectionUpdated;
use super::settings_core::SettingsProjection;
use crate::command::{CommandSinkHandle, UiCommand};
use crate::localized_widgets::localized_checkbox_scene;
use crate::pages::page_models::SettingsPageModelState;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::observer::On;
use bevy::ecs::query::{Has, With};
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::feathers::theme::ThemeBackgroundColor;
use bevy::scene::{Scene, bsn};
use bevy::ui::Checked;
use bevy::ui::prelude::{
    AlignItems, BorderRadius, FlexDirection, JustifyContent, Node, UiRect, percent,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button, Checkbox, ValueChange};
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_input::state::TextFieldInput;
use infiltrator_bevy_widgets::text_input::{TextField, text_field_with_placeholder_scene};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::pac::PacServiceState;
use infiltrator_presentation::host_network_projection::pac;
use infiltrator_presentation::settings_page_model::SettingsPageModel;
use infiltrator_shared::locales::Lang;

/// One-off layout value (not a ladder rung): the width of the PAC bypass list
/// field next to its label.
const PAC_BYPASS_FIELD_WIDTH_PX: f32 = 360.0;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PacToggle;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PacBypassField;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PacApplyButton;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PacStatusLine;

pub(super) fn scene(projection: &SettingsProjection, palette: &UiPalette) -> Box<dyn Scene> {
    let bypass = projection.pac.bypass_domains.join(", ");
    let status = pac(&projection.pac, &Lang(UiLocale::default().code()));
    Box::new(surface_scene(
        vec![Box::new(bsn! {
                    Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Column,
                        row_gap: rem(space::S6),
                    }
                    Children [
                        LocalizedText::plain("settings_pac_title") TextRole(Role::BodyStrong)
                        --
                        Node {
                            width: percent(100),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::SpaceBetween,
                            padding: UiRect::all(rem(space::S8)),
                            border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                        }
                        ThemeBackgroundColor({ Token::SurfaceElevated })
                        PacToggle
                        Children [
                            @{ localized_checkbox_scene(LocalizedText::plain("pac_enable_service"), matches!(projection.pac.state, PacServiceState::Running { .. }), palette) }
                            --
                            Text(status) PacStatusLine TextRole(Role::Mono)
                        ]
                        --
                        Node {
                            width: percent(100),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::SpaceBetween,
                            padding: UiRect::all(rem(space::S8)),
                        }
                        Children [
                            LocalizedText::plain("settings_pac_bypass_label") TextRole(Role::Body)
                            --
                            Node { width: rem(PAC_BYPASS_FIELD_WIDTH_PX) }
                            PacBypassField
                            Children [
                                @{ text_field_with_placeholder_scene(bypass, "localhost, *.lan, example.com".to_owned(), palette) }
                            ]
                        ]
                        --
                        Node {
                            width: percent(100),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::FlexEnd,
                        }
                        Children [
                            Node {
                                min_height: rem(palette.control_height_px),
                                padding: UiRect::horizontal(rem(space::S12)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                            }
                            ThemeBackgroundColor({ Token::Accent })
                            PacApplyButton
                            Button
                            Children [
                                LocalizedText::plain("settings_pac_apply_action") TextRole(Role::BodyStrong)
                            ]
                        ]
                    ]
        })],
        palette,
    ))
}

pub(super) fn on_changed(
    change: On<ValueChange<bool>>,
    parents: Query<&ChildOf>,
    toggles: Query<(), With<PacToggle>>,
    fields: Query<&Children, With<PacBypassField>>,
    text_fields: Query<&TextField>,
    handle: Option<Res<CommandSinkHandle>>,
    mut model: Option<ResMut<SettingsPageModelState>>,
) {
    let Some(handle) = handle else {
        return;
    };
    let Some(model) = model.as_deref_mut() else {
        return;
    };
    let Ok(parent) = parents.get(change.source) else {
        return;
    };
    if toggles.get(parent.0).is_ok() {
        submit(&handle, &mut model.0, change.value, &fields, &text_fields);
    }
}

pub(super) fn on_apply_activated(
    activate: On<Activate>,
    apply_buttons: Query<(), With<PacApplyButton>>,
    toggles: Query<&Children, With<PacToggle>>,
    checkboxes: Query<&Checked>,
    fields: Query<&Children, With<PacBypassField>>,
    text_fields: Query<&TextField>,
    handle: Option<Res<CommandSinkHandle>>,
    mut model: Option<ResMut<SettingsPageModelState>>,
) {
    let Some(handle) = handle else {
        return;
    };
    let Some(model) = model.as_deref_mut() else {
        return;
    };
    if apply_buttons.get(activate.entity).is_err() {
        return;
    }
    let enabled = toggles
        .iter()
        .flat_map(|children| children.iter())
        .any(|child| checkboxes.get(*child).is_ok());
    submit(&handle, &mut model.0, enabled, &fields, &text_fields);
}

#[derive(SystemParam)]
pub struct PacReplay<'w, 's> {
    locale: Res<'w, UiLocale>,
    toggles: Query<'w, 's, &'static Children, With<PacToggle>>,
    checkboxes: Query<'w, 's, (Entity, Has<Checked>), With<Checkbox>>,
    fields: Query<'w, 's, &'static Children, With<PacBypassField>>,
    text_fields: Query<'w, 's, &'static mut TextField>,
    status_lines: Query<'w, 's, &'static mut Text, With<PacStatusLine>>,
    commands: Commands<'w, 's>,
}
pub(super) fn apply_projection(update: On<SettingsProjectionUpdated>, surface: PacReplay) {
    let PacReplay {
        locale,
        toggles,
        checkboxes,
        fields,
        mut text_fields,
        mut status_lines,
        mut commands,
    } = surface;
    let enabled = matches!(update.0.pac.state, PacServiceState::Running { .. });
    for children in &toggles {
        for child in children.iter() {
            if let Ok((entity, checked)) = checkboxes.get(*child)
                && checked != enabled
            {
                if enabled {
                    commands.entity(entity).insert(Checked);
                } else {
                    commands.entity(entity).remove::<Checked>();
                }
            }
        }
    }
    let bypass = update.0.pac.bypass_domains.join(", ");
    for children in &fields {
        for child in children.iter() {
            if let Ok(mut field) = text_fields.get_mut(*child)
                && field.0.text() != bypass
            {
                field.0.apply(TextFieldInput::SetText(bypass.clone()));
            }
        }
    }
    let status = pac(&update.0.pac, &Lang(locale.code()));
    for mut line in &mut status_lines {
        line.0 = status.clone();
    }
}

fn submit(
    handle: &CommandSinkHandle,
    model: &mut SettingsPageModel,
    enabled: bool,
    fields: &Query<&Children, With<PacBypassField>>,
    text_fields: &Query<&TextField>,
) {
    let bypass_domains = fields
        .iter()
        .flat_map(|children| children.iter())
        .find_map(|child| text_fields.get(*child).ok())
        .map(|field| split_values(field.0.text()))
        .unwrap_or_default();
    // The shared model owns the PAC draft and the typed intent: the surface
    // only hands it the values the user typed.
    model.edit_pac(|draft| {
        draft.enabled = enabled;
        draft.bypass_domains = bypass_domains.clone();
        draft.bypass_lan = true;
        draft.minify = false;
    });
    let Some(CommandIntent::ApplyPac {
        enabled,
        bypass_domains,
        bypass_lan,
        minify,
    }) = model.pac_intent()
    else {
        return;
    };
    model.pac_write_phase_mut().begin();
    handle.submit(UiCommand::ApplyPac {
        enabled,
        bypass_domains,
        bypass_lan,
        minify,
    });
}

fn split_values(value: &str) -> Vec<String> {
    value
        .split([',', ';', '\n'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect()
}
