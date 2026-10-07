//! Independent review surface; cancel cannot write reviewed artifacts.
use super::card::{modal_backdrop, modal_card};
use crate::state::AppState;
use crate::types::message::Message;
use crate::types::script::ScriptAction;
use crate::view::component_forms::{style_accent, style_ghost};
use iced::widget::{Scrollable, Space, button, column, row, text};
use iced::{Element, Length};
use infiltrator_application::script_export_projection::project_script_export;
use infiltrator_shared::locales::{Lang, Localizer};

pub(crate) fn modal(state: &AppState) -> Element<'_, Message> {
    let model = &state.editor.script_sandbox;
    let lang = Lang(&state.shell.lang);
    let view = project_script_export(model, &state.shell.lang);
    let body = column![
        text(lang.tr("script_export_title")).size(17),
        text(view.status).size(12).width(Length::Fill),
        text(view.details).size(12).width(Length::Fill),
        text(view.content).size(12).width(Length::Fill),
        text(view.path).size(12).width(Length::Fill)
    ]
    .spacing(12)
    .width(Length::Fill);
    let saved = model
        .export
        .as_ref()
        .is_some_and(|snapshot| snapshot.outcome.is_saved());
    let mut actions = row![
        button(text(lang.tr(if saved {
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
            button(text(lang.tr("logs_export_confirm")))
                .style(style_accent)
                .on_press_maybe(
                    view.close
                        .then_some(Message::Script(ScriptAction::ConfirmExport)),
                ),
        );
    }
    if view.retry {
        actions = actions.push(
            button(text(lang.tr("logs_export_retry")))
                .style(style_accent)
                .on_press_maybe(view.close.then_some(Message::Script(ScriptAction::Retry))),
        );
    }
    let content = column![Scrollable::new(body).height(Length::Fill), actions].spacing(16);
    modal_backdrop(modal_card(
        content.into(),
        (state.shell.viewport.width_px - 48.0).clamp(96.0, 560.0),
    ))
}
