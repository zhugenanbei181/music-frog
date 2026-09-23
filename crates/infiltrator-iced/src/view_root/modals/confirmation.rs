//! Confirmation modal and its per-action copy.

use super::card::{modal_backdrop, modal_card};
use crate::state::AppState;
use crate::types::app::ConfirmAction;
use crate::types::message::Message;
use iced::widget::{Space, button, column, row, text};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_shared::locales::{Lang, Localizer};

pub fn confirmation_modal<'a>(
    state: &'a AppState,
    action: &'a ConfirmAction,
) -> Element<'a, Message> {
    let lang = Lang(&state.shell.lang);
    let (title, detail, confirm_label) = confirmation_copy(action, &lang);
    let cancel_label = lang.tr("modal_cancel");

    let content = column![
        row![
            text(title)
                .size(16)
                .font(crate::view::theme::FONT_SEMIBOLD)
                .style(|theme: &Theme| text::Style {
                    color: Some(crate::view::theme::tokens(theme).text_primary),
                }),
            Space::new().width(Length::Fill),
            button(crate::view::svg_icons::icon_themed(
                crate::view::svg_icons::Icon::X,
                14.0,
                |t: &Theme| crate::view::theme::tokens(t).text_secondary
            ))
            .padding(4)
            .style(crate::view::components::style_ghost)
            .on_press(Message::CancelConfirmation),
        ]
        .align_y(Alignment::Center),
        Space::new().height(crate::view::theme::SP_XS),
        text(detail).size(13).style(|theme: &Theme| text::Style {
            color: Some(crate::view::theme::tokens(theme).text_secondary),
        }),
        Space::new().height(crate::view::theme::SP_SM),
        row![
            button(
                text(cancel_label)
                    .size(12)
                    .font(crate::view::theme::FONT_MEDIUM)
            )
            .padding([7, 14])
            .style(crate::view::components::style_ghost)
            .on_press(Message::CancelConfirmation),
            Space::new().width(Length::Fill),
            button(
                text(confirm_label)
                    .size(12)
                    .font(crate::view::theme::FONT_MEDIUM)
            )
            .padding([7, 16])
            .style(crate::view::components::style_danger)
            .on_press(Message::ConfirmAction),
        ]
        .align_y(Alignment::Center),
    ]
    .spacing(12);

    modal_backdrop(modal_card(content.into(), 420.0))
}

fn confirmation_copy(action: &ConfirmAction, lang: &Lang<'_>) -> (String, String, String) {
    match action {
        ConfirmAction::FactoryReset => (
            lang.tr("modal_confirm_factory_title").to_string(),
            lang.tr("modal_confirm_factory_desc").to_string(),
            lang.tr("modal_confirm_factory_btn").to_string(),
        ),
        ConfirmAction::ClearProfiles => (
            lang.tr("modal_confirm_reset_title").to_string(),
            lang.tr("modal_confirm_reset_desc").to_string(),
            lang.tr("modal_confirm_reset_btn").to_string(),
        ),
        ConfirmAction::DeleteProfile(name) => (
            lang.tr("modal_confirm_del_profile_title").to_string(),
            infiltrator_shared::i18n_interpolator::interpolate(
                &lang.tr("modal_confirm_del_profile_desc"),
                &[("name", name)],
            ),
            lang.tr("modal_delete").to_string(),
        ),
        ConfirmAction::DeleteKernel(version) => (
            lang.tr("modal_confirm_del_kernel_title").to_string(),
            infiltrator_shared::i18n_interpolator::interpolate(
                &lang.tr("modal_confirm_del_kernel_desc"),
                &[("version", version)],
            ),
            lang.tr("modal_delete").to_string(),
        ),
        ConfirmAction::CloseAllConnections => (
            lang.tr("modal_confirm_disconnect_all_title").to_string(),
            lang.tr("modal_confirm_disconnect_all_desc").to_string(),
            lang.tr("modal_confirm_disconnect_all_btn").to_string(),
        ),
    }
}
