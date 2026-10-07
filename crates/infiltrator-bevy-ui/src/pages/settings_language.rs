//! Native language choices save through the shared command service and retain live input entities.
use crate::command::{CommandSinkHandle, UiCommand};
use crate::command_events::CommandExecutedEvent;
use crate::surface::LatestSurfaceSnapshot;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{QueryData, With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::scene::{Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::widget::Text;
use bevy::ui::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, FlexWrap, Node, UiRect, percent, px,
};
use bevy::ui_widgets::{Activate, Button};
use infiltrator_application::language_choice::LanguageChoiceState;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_contract::command::RequestId;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::language::LanguagePreference;

#[derive(Component, Clone, Copy, Default)]
#[require(Button)]
pub struct LanguageChoiceButton(pub LanguagePreference);
#[derive(Component, Clone, Copy, Default)]
pub struct LanguageChoices;
#[derive(Component, Clone, Copy, Default)]
pub struct LanguageChoiceCard;
#[derive(Component, Clone, Copy, Default)]
pub struct LanguageChoiceStatus;
#[derive(Component, Clone, Copy, Default)]
#[require(Button)]
pub struct RetryLanguageChoice;
#[derive(Resource, Default)]
pub struct LanguageChoiceResource {
    pub model: LanguageChoiceState,
    pub request: Option<(RequestId, u64)>,
}
pub fn language_card(palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
        Node { width: percent(100), flex_shrink: 0.0, padding: UiRect::all(px(16.0)), flex_direction: FlexDirection::Column, row_gap: px(12.0), border_radius: BorderRadius::all(px(palette.card_radius_px)) }
        LanguageChoiceCard BackgroundColor({ palette.surface })
        Children [
            LocalizedText::plain("settings_language_title") TextRole(Role::BodyStrong)
            --
            @{ choices_scene(palette) }
        ]
    }
}
pub fn choices_scene(palette: &UiPalette) -> impl Scene + use<> {
    let choices: Vec<Box<dyn Scene>> = LanguagePreference::ALL.into_iter().map(|preference| Box::new(bsn! {
        Node { min_height: px(palette.control_height_px), padding: UiRect::all(px(8.0)), align_items: AlignItems::Center }
        Button LanguageChoiceButton(preference) ButtonDisabled(true) BackgroundColor({ palette.surface_elevated })
        LocalizedLabel::plain(preference.label_key())
        Children [ LocalizedText::plain(preference.label_key()) TextRole(Role::Body) ]
    }) as Box<dyn Scene>).collect();
    bsn! {
        Node { width: percent(100), flex_direction: FlexDirection::Column, row_gap: px(8.0) }
        LanguageChoices
        Children [
            Node { width: percent(100), flex_wrap: FlexWrap::Wrap, column_gap: px(8.0), row_gap: px(8.0) }
            Children [ { choices } ]
            --
            Node { width: percent(100), flex_wrap: FlexWrap::Wrap, align_items: AlignItems::Center, column_gap: px(8.0) }
            Children [
                Text(String::new()) LanguageChoiceStatus TextRole(Role::Caption)
                --
                Node { min_height: px(palette.control_height_px), padding: UiRect::all(px(8.0)) }
                Button RetryLanguageChoice ButtonDisabled(true)
                Children [ LocalizedText::plain("language_retry") TextRole(Role::Body) ]
            ]
        ]
    }
}
pub fn activate_language(
    event: On<Activate>,
    choices: Query<&LanguageChoiceButton>,
    retries: Query<(), With<RetryLanguageChoice>>,
    mut state: ResMut<LanguageChoiceResource>,
    sink: Option<Res<CommandSinkHandle>>,
) {
    if state.model.pending.is_some() {
        return;
    }
    let choice = if let Ok(button) = choices.get(event.entity) {
        Some(button.0)
    } else if retries.contains(event.entity) {
        state.model.requested
    } else {
        None
    };
    let Some(preference) = choice else { return };
    let pending = match state.model.begin(preference) {
        Ok(pending) => pending,
        Err(failure) => {
            state.model.failure = Some(failure);
            return;
        }
    };
    if let Some(request) =
        sink.and_then(|sink| sink.submit_tracked(UiCommand::SetLanguage { preference }))
    {
        state.request = Some((request, pending.token));
    } else {
        state.model.finish(
            pending.token,
            Err(Failure::new(
                ErrorCode::NotReady,
                "language settings cannot be saved right now",
                true,
            )),
        );
    }
}
pub fn finish_language(
    event: On<CommandExecutedEvent>,
    mut state: ResMut<LanguageChoiceResource>,
    mut locale: ResMut<UiLocale>,
) {
    let Some((request, token)) = state.request else {
        return;
    };
    let UiCommand::SetLanguage { preference } = event.command else {
        return;
    };
    if request != event.request_id
        || !state
            .model
            .pending
            .is_some_and(|pending| pending.token == token && pending.preference == preference)
    {
        return;
    }
    if let Some(result) = state.model.finish(token, event.unit_result()) {
        state.request = None;
        if let Ok(preference) = result {
            locale.apply_preference(preference.as_setting());
        }
    }
}
pub fn sync_language_model(
    latest: Res<LatestSurfaceSnapshot>,
    mut state: ResMut<LanguageChoiceResource>,
    mut locale: ResMut<UiLocale>,
) {
    state.model.observe(&latest.0.language_settings);
    if let Some(preference) = state.model.applied {
        locale.observe_preference(preference.as_setting());
    }
}
#[derive(QueryData)]
#[query_data(mutable)]
pub struct LanguageButtonParts {
    choice: &'static LanguageChoiceButton,
    children: &'static Children,
    disabled: &'static mut ButtonDisabled,
    background: &'static mut BackgroundColor,
}
pub fn sync_language_controls(
    state: Res<LanguageChoiceResource>,
    locale: Res<UiLocale>,
    palette: Res<UiPalette>,
    mut choices: Query<LanguageButtonParts, Without<RetryLanguageChoice>>,
    mut ink: Query<&mut TextColor>,
    mut retries: Query<
        &mut ButtonDisabled,
        (With<RetryLanguageChoice>, Without<LanguageChoiceButton>),
    >,
    mut status: Query<&mut Text, With<LanguageChoiceStatus>>,
) {
    let model = &state.model;
    for mut parts in &mut choices {
        let active = model.applied == Some(parts.choice.0);
        let unavailable = !model.can_persist || model.pending.is_some() || active;
        if parts.disabled.0 != unavailable {
            parts.disabled.0 = unavailable;
        }
        let fill = if active {
            palette.accent_container
        } else {
            palette.surface_elevated
        };
        if parts.background.0 != fill {
            parts.background.0 = fill;
        }
        for entity in parts.children {
            if let Ok(mut color) = ink.get_mut(*entity) {
                let value = if unavailable && !active {
                    palette.ink_dim
                } else {
                    palette.ink
                };
                if color.0 != value {
                    color.0 = value;
                }
            }
        }
    }
    for mut retry in &mut retries {
        let disabled = model.failure.is_none()
            || model.requested.is_none()
            || model.pending.is_some()
            || !model.can_persist;
        if retry.0 != disabled {
            retry.0 = disabled;
        }
    }
    let message = model
        .failure
        .as_ref()
        .map(|failure| failure.message.clone())
        .unwrap_or_else(|| {
            if model.pending.is_some() {
                LocalizedText::plain("language_saving").render(&locale)
            } else if !model.can_persist {
                LocalizedText::plain("language_save_unavailable").render(&locale)
            } else {
                String::new()
            }
        });
    for mut text in &mut status {
        if text.0 != message {
            text.0 = message.clone();
        }
    }
}
