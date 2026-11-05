//! Independent source-bound cleanup review; cancel never changes the draft.
use super::card::{modal_backdrop, modal_card};
use crate::state::AppState;
use crate::types::message::Message;
use crate::view::scrollbar::auto_hide_scrollable;
use crate::view::theme;
use crate::view_root::interaction_regions::InteractionRegion;
use iced::advanced::text::Renderer;
use iced::widget::{button, column, container, row};
use iced::{Element, Font, Length, Theme};
use infiltrator_presentation::rule_statistics_inspector_projection::project_inspector;
use infiltrator_shared::locales::{Lang, Localizer};

pub fn statistics_confirmation<'a, R: Renderer<Font = Font> + 'a>(
    state: &'a AppState,
) -> Element<'a, Message, Theme, R> {
    let model = &state.editor.rule_hit_audit;
    let lang = Lang(&state.shell.lang);
    let projection = project_inspector(model, &state.editor.rule_list, &Lang(&state.shell.lang));
    let mut targets = column![];
    let Some(confirmation) = model.confirmation() else {
        return theme::role_body_text(String::new()).into();
    };
    for target in &confirmation.targets {
        targets = targets.push(theme::role_body_text(target.raw.clone()));
    }
    let content = column![
        theme::role_body_text(lang.tr("rules_stats_cleanup_title").into_owned()),
        theme::role_body_text(lang.tr("rules_stats_cleanup_description").into_owned()),
        theme::role_body_text(projection.confirmation_summary),
        auto_hide_scrollable(targets.spacing(6)).height(Length::Fill),
        theme::role_body_text(projection.confirmation_status),
        row![
            container(
                button(theme::role_body_text(lang.tr("modal_cancel").into_owned()))
                    .on_press(Message::CancelConfirmation)
            )
            .id(InteractionRegion::ConfirmationCancel.id()),
            container(
                button(theme::role_body_text(
                    lang.tr("rules_stats_cleanup_confirm").into_owned()
                ))
                .on_press_maybe(
                    projection
                        .can_confirm_cleanup
                        .then_some(Message::ConfirmAction)
                )
            )
            .id(InteractionRegion::ConfirmationAccept.id()),
        ]
        .spacing(12)
        .wrap(),
    ]
    .spacing(12)
    .height(340);
    modal_backdrop(modal_card(
        container(content)
            .id(InteractionRegion::StatisticsReview.id())
            .into(),
        560.0,
    ))
}
