//! Rule-Provider Unpacker and Local Extraction component.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::{badge, card, style_accent, style_ghost, BadgeKind};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, FONT_MEDIUM, MONO, tokens};
use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_shared::locales::{Lang, Localizer};

pub fn provider_unpack_card<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let unp = &state.editor.provider_unpack;

    let unpack_btn = button(
        row![
            svg_icons::icon_themed(Icon::Plus, 12.0, |t: &Theme| tokens(t).on_accent),
            Space::new().width(theme::SP_XS),
            text(lang.tr("provider_btn_unpack").to_string())
                .size(11)
                .font(FONT_MEDIUM),
        ]
        .align_y(Alignment::Center),
    )
    .padding([4, 12])
    .style(style_accent)
    .on_press(Message::UnpackRuleProviderToCustom("Apple-Provider".to_string()));

    let purge_btn = button(
        row![
            svg_icons::icon_themed(Icon::Trash2, 12.0, |t: &Theme| tokens(t).text_secondary),
            Space::new().width(theme::SP_XS),
            text(lang.tr("provider_btn_purge_cache").to_string())
                .size(11)
                .font(FONT_MEDIUM),
        ]
        .align_y(Alignment::Center),
    )
    .padding([4, 10])
    .style(style_ghost)
    .on_press(Message::PurgeRuleProviderCache);

    let feedback: Element<'_, Message> = if let Some(msg) = &unp.status_message {
        container(
            row![
                svg_icons::icon_themed(Icon::ListChecks, 14.0, |t: &Theme| tokens(t).success),
                Space::new().width(theme::SP_XS),
                text(msg.clone()).size(11).style(|t: &Theme| text::Style { color: Some(tokens(t).success) }),
            ]
            .align_y(Alignment::Center),
        )
        .into()
    } else {
        Element::from(Space::new().height(0))
    };

    card(
        Some(lang.tr("provider_unpack_title").to_string()),
        column![
            text(lang.tr("provider_unpack_desc").to_string()).size(12).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
            Space::new().height(theme::SP_XS),
            row![
                text(format!("Total Unpacked: {} rules", unp.unpacked_rules_count)).size(12).font(MONO).width(Length::Fill),
                badge("Providers Active".to_string(), BadgeKind::Neutral),
            ]
            .align_y(Alignment::Center),
            feedback,
            Space::new().height(theme::SP_XS),
            row![
                Space::new().width(Length::Fill),
                purge_btn,
                Space::new().width(theme::SP_SM),
                unpack_btn,
            ]
            .align_y(Alignment::Center),
        ]
        .spacing(theme::SP_SM),
    )
}
