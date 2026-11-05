//! A persistent native confirmation becomes an actual saved-file receipt panel.
use super::card::{modal_backdrop, modal_card};
use crate::state::AppState;
use crate::types::message::Message;
use crate::view::component_forms::{style_accent, style_ghost};
use crate::view::theme;
use crate::view::theme::tokens;
use crate::view_root::interaction_regions::InteractionRegion;
use iced::advanced::text::{Renderer, Wrapping};
use iced::widget::{Scrollable, Space, button, column, container, row};
use iced::{Border, Element, Font, Length, Theme};
use infiltrator_presentation::log_export_projection::project_log_export;
use infiltrator_shared::locales::{Lang, Localizer};

pub(crate) fn content<'a, R: Renderer<Font = Font> + 'a>(
    state: &'a AppState,
) -> Element<'a, Message, Theme, R> {
    let model = &state.diag.log_export;
    let view = project_log_export(model, &Lang(&state.shell.lang));
    let lang = Lang(&state.shell.lang);
    let status = container(theme::role_body_text(view.status).width(Length::Fill))
        .padding(8)
        .width(Length::Fill)
        .style(move |theme: &Theme| container::Style {
            border: Border {
                width: if view.error { 1.0 } else { 0.0 },
                color: tokens(theme).danger,
                ..Default::default()
            },
            ..Default::default()
        })
        .id(InteractionRegion::LogExportStatus.id());
    let body = column![
        status,
        container(theme::role_body_text(view.details).width(Length::Fill))
            .id(InteractionRegion::LogExportDetails.id()),
        container(
            theme::role_body_text(view.path)
                .width(Length::Fill)
                .wrapping(Wrapping::Glyph)
        )
        .id(InteractionRegion::LogExportPath.id())
    ]
    .spacing(12)
    .width(Length::Fill);
    let mut actions = row![
        container(
            button(theme::role_body_text(lang.tr(
                if model.receipt().is_some() {
                    "logs_export_close"
                } else {
                    "logs_export_cancel"
                }
            )))
            .style(style_ghost)
            .on_press_maybe(view.close.then_some(Message::CancelLogExport))
        )
        .id(InteractionRegion::LogExportCancel.id()),
        Space::new().width(Length::Fill)
    ]
    .spacing(8);
    if view.confirm {
        actions = actions.push(
            container(
                button(theme::role_body_text(lang.tr("logs_export_confirm")))
                    .style(style_accent)
                    .on_press(Message::ConfirmLogExport),
            )
            .id(InteractionRegion::LogExportConfirm.id()),
        );
    }
    if view.retry {
        actions = actions.push(
            container(
                button(theme::role_body_text(lang.tr("logs_export_retry")))
                    .style(style_accent)
                    .on_press(Message::RetryLogExport),
            )
            .id(InteractionRegion::LogExportRetry.id()),
        );
    }
    column![
        theme::role_body_text(lang.tr("logs_export_title")),
        Scrollable::new(body).height(Length::Fixed(
            (state.shell.viewport.height_px - 210.0).clamp(90.0, 240.0)
        )),
        actions
    ]
    .spacing(12)
    .width(Length::Fill)
    .into()
}
pub(crate) fn modal(state: &AppState) -> Element<'_, Message> {
    modal_backdrop(
        container(modal_card(
            content(state),
            (state.shell.viewport.width_px - 48.0).clamp(96.0, 560.0),
        ))
        .id(InteractionRegion::LogExportDialog.id())
        .into(),
    )
}

#[cfg(test)]
#[path = "../../../tests/gui/log_export_native_tests.rs"]
pub(crate) mod tests;
