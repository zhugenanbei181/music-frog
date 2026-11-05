//! Independent review surface; cancel cannot write reviewed artifacts.
use super::card::{modal_backdrop, modal_card};
use crate::state::AppState;
use crate::types::message::Message;
use crate::types::script::ScriptAction;
use crate::view::component_forms::{style_accent, style_ghost};
use crate::view::theme;
use crate::view_root::interaction_regions::InteractionRegion;
use iced::widget::{Scrollable, Space, button, column, container, row};
use iced::{Element, Length};
use infiltrator_presentation::script_export_projection::project_script_export;
use infiltrator_shared::locales::{Lang, Localizer};

pub(crate) fn modal(state: &AppState) -> Element<'_, Message> {
    let model = &state.editor.script_sandbox;
    let lang = Lang(&state.shell.lang);
    let view = project_script_export(model, &Lang(&state.shell.lang));
    let body = column![
        theme::role_body_text(lang.tr("script_export_title")),
        theme::role_body_text(view.status).width(Length::Fill),
        theme::role_body_text(view.details).width(Length::Fill),
        theme::role_body_text(view.content).width(Length::Fill),
        theme::role_body_text(view.path).width(Length::Fill)
    ]
    .spacing(12)
    .width(Length::Fill);
    let saved = model
        .export()
        .is_some_and(|snapshot| snapshot.outcome.is_saved());
    let mut actions = row![
        button(theme::role_body_text(lang.tr(if saved {
            "logs_export_close"
        } else {
            "logs_export_cancel"
        })))
        .style(style_ghost)
        .on_press_maybe(
            view.close
                .then_some(Message::Script(ScriptAction::CancelExport))
        ),
        Space::new().width(Length::Fill)
    ]
    .spacing(8);
    if view.confirm {
        actions = actions.push(
            button(theme::role_body_text(lang.tr("logs_export_confirm")))
                .style(style_accent)
                .on_press_maybe(
                    view.close
                        .then_some(Message::Script(ScriptAction::ConfirmExport)),
                ),
        );
    }
    if view.retry {
        actions = actions.push(
            button(theme::role_body_text(lang.tr("logs_export_retry")))
                .style(style_accent)
                .on_press_maybe(view.close.then_some(Message::Script(ScriptAction::Retry))),
        );
    }
    let content = column![Scrollable::new(body).height(Length::Fill), actions].spacing(16);
    container(modal_backdrop(modal_card(
        content.into(),
        (state.shell.viewport.width_px - 48.0).clamp(96.0, 560.0),
    )))
    .id(InteractionRegion::ScriptExportReview.id())
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}
