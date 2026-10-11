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
use bevy::feathers::theme::ThemeBackgroundColor;
use bevy::scene::{Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::widget::Text;
use bevy::ui::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, FlexWrap, Node, UiRect, percent,
};
use bevy::ui_widgets::{Activate, Button};
use infiltrator_application::language_choice::{
    LanguageChoiceFinish, LanguageChoicePhase, LanguageChoiceState, LanguagePersistence,
    LanguageReadPhase,
};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
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
enum LanguageChoiceAdmission {
    Choose(LanguagePreference),
    Retry,
}
pub fn language_card(palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
        Node { width: percent(100), flex_shrink: 0.0, padding: UiRect::all(rem(16.0)), flex_direction: FlexDirection::Column, row_gap: rem(12.0), border_radius: BorderRadius::all(rem(palette.card_radius_px)) }
        LanguageChoiceCard ThemeBackgroundColor({ Token::Surface })
        Children [
            LocalizedText::plain("settings_language_title") TextRole(Role::BodyStrong)
            --
            @{ choices_scene(palette) }
        ]
    }
}
pub fn choices_scene(palette: &UiPalette) -> impl Scene + use<> {
    let choices: Vec<Box<dyn Scene>> = LanguagePreference::ALL.into_iter().map(|preference| Box::new(bsn! {
        Node { min_height: rem(palette.control_height_px), padding: UiRect::all(rem(8.0)), align_items: AlignItems::Center }
        Button LanguageChoiceButton(preference) ButtonDisabled(true) ThemeBackgroundColor({ Token::SurfaceElevated })
        LocalizedLabel::plain(preference.label_key())
        Children [ LocalizedText::plain(preference.label_key()) TextRole(Role::Body) ]
    }) as Box<dyn Scene>).collect();
    bsn! {
        Node { width: percent(100), flex_direction: FlexDirection::Column, row_gap: rem(8.0) }
        LanguageChoices
        Children [
            Node { width: percent(100), flex_wrap: FlexWrap::Wrap, column_gap: rem(8.0), row_gap: rem(8.0) }
            Children [ { choices } ]
            --
            Node { width: percent(100), flex_wrap: FlexWrap::Wrap, align_items: AlignItems::Center, column_gap: rem(8.0) }
            Children [
                Text(String::new()) LanguageChoiceStatus TextRole(Role::Caption)
                --
                Node { min_height: rem(palette.control_height_px), padding: UiRect::all(rem(8.0)) }
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
    let admission = match choices.get(event.entity) {
        Ok(button) => LanguageChoiceAdmission::Choose(button.0),
        Err(_) if retries.contains(event.entity) => LanguageChoiceAdmission::Retry,
        Err(_) => return,
    };
    let pending = match match admission {
        LanguageChoiceAdmission::Choose(preference) => state.model.begin(preference),
        LanguageChoiceAdmission::Retry => state.model.begin_retry(),
    } {
        Ok(pending) => pending,
        Err(failure) => {
            state.model.reject(failure);
            return;
        }
    };
    let preference = pending.preference;
    if let Some(request) =
        sink.and_then(|sink| sink.submit_tracked(UiCommand::SetLanguage { preference }))
    {
        state.request = Some((request, pending.token));
    } else {
        let _ = state.model.finish(
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
    let pending_matches = match &state.model.phase {
        LanguageChoicePhase::Pending(pending) => {
            pending.token == token && pending.preference == preference
        }
        LanguageChoicePhase::Ready
        | LanguageChoicePhase::Rejected(_)
        | LanguageChoicePhase::Failed { .. }
        | LanguageChoicePhase::AwaitingObservation { .. } => false,
    };
    if request != event.request_id || !pending_matches {
        return;
    }
    match state.model.finish(token, event.unit_result()) {
        LanguageChoiceFinish::Stale => {}
        LanguageChoiceFinish::Failed(_) => state.request = None,
        LanguageChoiceFinish::Saved(preference) => {
            state.request = None;
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
    let preference = match &state.model.read {
        LanguageReadPhase::Unobserved => None,
        LanguageReadPhase::Observed { preference } => *preference,
        LanguageReadPhase::Failed { last_known, .. } => *last_known,
    };
    if let Some(preference) = preference {
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
    let applied = match &model.read {
        LanguageReadPhase::Unobserved => None,
        LanguageReadPhase::Observed { preference } => *preference,
        LanguageReadPhase::Failed { last_known, .. } => *last_known,
    };
    let persistence_available = model.persistence == LanguagePersistence::Available;
    let operation_pending = match &model.phase {
        LanguageChoicePhase::Pending(_) | LanguageChoicePhase::AwaitingObservation { .. } => true,
        LanguageChoicePhase::Ready
        | LanguageChoicePhase::Rejected(_)
        | LanguageChoicePhase::Failed { .. } => false,
    };
    for mut parts in &mut choices {
        let active = applied == Some(parts.choice.0);
        let unavailable = !persistence_available || operation_pending || active;
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
        let disabled = !model.can_retry();
        if retry.0 != disabled {
            retry.0 = disabled;
        }
    }
    let message = match (&model.phase, &model.read, model.persistence) {
        (LanguageChoicePhase::Rejected(failure), _, _)
        | (LanguageChoicePhase::Failed { failure, .. }, _, _) => failure.message.clone(),
        (LanguageChoicePhase::Pending(_), _, _)
        | (LanguageChoicePhase::AwaitingObservation { .. }, _, _) => {
            LocalizedText::plain("language_saving").render(&locale)
        }
        (LanguageChoicePhase::Ready, LanguageReadPhase::Failed { failure, .. }, _) => {
            failure.message.clone()
        }
        (LanguageChoicePhase::Ready, _, LanguagePersistence::Unavailable) => {
            LocalizedText::plain("language_save_unavailable").render(&locale)
        }
        (LanguageChoicePhase::Ready, _, LanguagePersistence::Available) => String::new(),
    };
    for mut text in &mut status {
        if text.0 != message {
            text.0 = message.clone();
        }
    }
}
