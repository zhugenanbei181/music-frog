//! Iced projection for the Android VpnService lifecycle.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::component_card::card;
use crate::view::component_forms::{style_accent, style_ghost};
use crate::view::components::{BadgeKind, badge};
use crate::view::theme;
use crate::view::theme::{MONO, tokens};
use iced::widget::{Space, button, column, row, text};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_application::host_network_projection::vpn;
use infiltrator_shared::locales::{Lang, Localizer};

pub fn vpn_card<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let view = vpn(&state.runtime.vpn, lang.0);
    let start = button(text(lang.tr(view.start_key).into_owned()))
        .padding([4, 12])
        .style(style_accent)
        .on_press_maybe(view.start_enabled.then_some(Message::StartVpn));
    let stop = button(text(lang.tr(view.stop_key).into_owned()))
        .padding([4, 12])
        .style(style_ghost)
        .on_press_maybe(view.stop_enabled.then_some(Message::StopVpn));

    card(
        Some(lang.tr("vpn_card_title").to_string()),
        column![
            text(lang.tr("vpn_card_desc").to_string())
                .size(12)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary)
                }),
            Space::new().height(theme::SP_XS),
            row![
                badge(view.status, BadgeKind::Accent),
                Space::new().width(Length::Fill),
                text(view.details).size(11).font(MONO),
            ]
            .align_y(Alignment::Center),
            Space::new().height(theme::SP_XS),
            row![start, Space::new().width(theme::SP_XS), stop].align_y(Alignment::Center),
        ]
        .spacing(theme::SP_SM),
    )
}
