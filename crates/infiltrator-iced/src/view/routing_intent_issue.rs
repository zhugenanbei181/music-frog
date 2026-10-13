//! Inline routing-intent failure surface remains reachable at every viewport and route.
use crate::state::AppState;
use crate::types::app::Route;
use crate::types::message::Message;
use crate::view::component_card::card;
use crate::view::component_forms::style_ghost;
use crate::view::theme;
use crate::view_root::interaction_regions::InteractionRegion;
use iced::advanced::text::Renderer;
use iced::widget::{button, column, container, row, text};
use iced::{Element, Font, Length, Theme};
use infiltrator_presentation::routing_intent_projection::routing_intent_failure_copy;
use infiltrator_shared::locales::{Lang, Localizer};

pub fn routing_intent_issue(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);
    container(card(
        None,
        column![
            text(routing_intent_failure_copy(
                &state.runtime.routing_intent_actions,
                lang.0,
            ))
            .size(12)
            .style(|theme: &Theme| text::Style {
                color: Some(theme::tokens(theme).text_primary)
            }),
            controls(state),
        ]
        .spacing(theme::SP_XS)
        .width(Length::Fill),
    ))
    .id(InteractionRegion::RoutingIntentFailure.id())
    .into()
}

pub fn controls<'a, R: Renderer<Font = Font> + 'a>(
    state: &'a AppState,
) -> Element<'a, Message, Theme, R> {
    let lang = Lang(&state.shell.lang);
    let retry = button(
        text(lang.tr("routing_intent_retry").into_owned())
            .size(12)
            .font(theme::FONT_MEDIUM),
    )
    .padding([7, 14])
    .style(style_ghost)
    .on_press_maybe(
        state
            .runtime
            .routing_intent_actions
            .retry_target()
            .map(|_| Message::RetryRoutingIntent),
    );
    let dismiss = button(
        text(lang.tr("routing_intent_dismiss").into_owned())
            .size(12)
            .font(theme::FONT_MEDIUM),
    )
    .padding([7, 14])
    .style(style_ghost)
    .on_press_maybe(
        state
            .runtime
            .routing_intent_actions
            .pending_change()
            .is_none()
            .then_some(Message::DismissRoutingIntentFailure),
    );
    let buttons = row![
        container(retry).id(InteractionRegion::RoutingIntentRetry.id()),
        container(dismiss).id(InteractionRegion::RoutingIntentDismiss.id())
    ]
    .spacing(theme::SP_SM);
    let mut actions = column![buttons].spacing(theme::SP_XS);
    if state
        .runtime
        .routing_intent_actions
        .needs_controller_settings()
    {
        actions = actions.push(
            container(
                button(text(lang.tr("routing_intent_settings").into_owned()).size(12))
                    .padding([7, 14])
                    .style(style_ghost)
                    .on_press(Message::Navigate(Route::Settings)),
            )
            .id(InteractionRegion::RoutingIntentSettings.id()),
        );
    }
    actions.into()
}
