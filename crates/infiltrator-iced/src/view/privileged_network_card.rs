//! Iced projection for the privileged network regression transaction.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::component_card::card;
use crate::view::component_forms::style_accent;
use crate::view::components::{BadgeKind, badge};
use crate::view::theme;
use crate::view::theme::tokens;
use iced::widget::{Space, button, column, row, text};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_presentation::host_network_projection::privileged;
use infiltrator_shared::locales::{Lang, Localizer};

pub fn privileged_network_card<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let view = privileged(&state.runtime.privileged_network, &Lang(lang.0));
    let run = button(theme::role_body_text(lang.tr(view.start_key).into_owned()))
        .padding([4, 12])
        .style(style_accent)
        .on_press_maybe(
            view.start_enabled
                .then_some(Message::RunPrivilegedNetworkRegression),
        );
    card(
        Some(lang.tr("privileged_network_title").to_string()),
        column![
            theme::role_body_text(lang.tr("privileged_network_desc").to_string()).style(
                |t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary),
                }
            ),
            Space::new().height(theme::SP_XS),
            row![
                badge(view.status, BadgeKind::Accent),
                Space::new().width(Length::Fill),
                theme::role_body_text(view.details),
            ]
            .align_y(Alignment::Center),
            Space::new().height(theme::SP_XS),
            run,
        ]
        .spacing(theme::SP_SM),
    )
}

#[cfg(test)]
#[path = "../../tests/gui/privileged_network_card_tests.rs"]
mod privileged_network_card_tests;
