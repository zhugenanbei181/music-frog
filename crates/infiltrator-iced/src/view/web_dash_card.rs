//! External Web Dashboard launcher component.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::{card, style_accent, style_ghost};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, FONT_MEDIUM, tokens};
use iced::widget::{Space, button, column, row, text};
use iced::{Alignment, Element, Theme};
use infiltrator_shared::locales::{Lang, Localizer};

pub fn web_dash_card<'a>(_state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let metacubexd_btn = button(
        row![
            svg_icons::icon_themed(Icon::Globe, 14.0, |t: &Theme| tokens(t).on_accent),
            Space::new().width(theme::SP_XS),
            text(lang.tr("web_dash_btn_metacubexd").to_string())
                .size(12)
                .font(FONT_MEDIUM),
        ]
        .align_y(Alignment::Center),
    )
    .padding([6, 12])
    .style(style_accent)
    .on_press(Message::LaunchWebDashboard("metacubexd"));

    let yacd_btn = button(
        row![
            svg_icons::icon_themed(Icon::Globe, 14.0, |t: &Theme| tokens(t).text_secondary),
            Space::new().width(theme::SP_XS),
            text(lang.tr("web_dash_btn_yacd").to_string())
                .size(12)
                .font(FONT_MEDIUM),
        ]
        .align_y(Alignment::Center),
    )
    .padding([6, 12])
    .style(style_ghost)
    .on_press(Message::LaunchWebDashboard("yacd"));

    let razord_btn = button(
        row![
            svg_icons::icon_themed(Icon::Globe, 14.0, |t: &Theme| tokens(t).text_secondary),
            Space::new().width(theme::SP_XS),
            text(lang.tr("web_dash_btn_razord").to_string())
                .size(12)
                .font(FONT_MEDIUM),
        ]
        .align_y(Alignment::Center),
    )
    .padding([6, 12])
    .style(style_ghost)
    .on_press(Message::LaunchWebDashboard("razord"));

    card(
        Some(lang.tr("web_dash_title").to_string()),
        column![
            text(lang.tr("web_dash_desc").to_string()).size(12).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
            Space::new().height(theme::SP_XS),
            row![
                metacubexd_btn,
                Space::new().width(theme::SP_SM),
                yacd_btn,
                Space::new().width(theme::SP_SM),
                razord_btn,
            ]
            .align_y(Alignment::Center),
        ]
        .spacing(theme::SP_SM),
    )
}
