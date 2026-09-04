//! GeoIP / GeoSite Database Manager & Updater component.

use crate::state::AppState;
use crate::types::message::Message;
use crate::utils::format_bytes;
use crate::view::components::{badge, card, style_accent, style_ghost, BadgeKind};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, MONO, tokens};
use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_shared::locales::{Lang, Localizer};

pub fn geodata_card<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let geo = &state.editor.geodata_status;

    let check_btn = button(
        row![
            svg_icons::icon_themed(Icon::Search, 12.0, |t: &Theme| tokens(t).text_secondary),
            Space::new().width(theme::SP_XS),
            text(lang.tr("geodata_btn_check").to_string())
                .size(11)
                .font(FONT_MEDIUM),
        ]
        .align_y(Alignment::Center),
    )
    .padding([4, 10])
    .style(style_ghost)
    .on_press_maybe((!geo.is_updating).then_some(Message::CheckGeoDataUpdates));

    let update_btn = button(
        row![
            svg_icons::icon_themed(Icon::RefreshCw, 12.0, |t: &Theme| tokens(t).on_accent),
            Space::new().width(theme::SP_XS),
            text(if geo.is_updating {
                lang.tr("geodata_updating").to_string()
            } else {
                lang.tr("geodata_btn_update").to_string()
            })
            .size(11)
            .font(FONT_MEDIUM),
        ]
        .align_y(Alignment::Center),
    )
    .padding([4, 12])
    .style(style_accent)
    .on_press_maybe((!geo.is_updating).then_some(Message::TriggerGeoDataUpdate));

    let geoip_ver = if geo.geoip_version.is_empty() {
        "v2026.09.01"
    } else {
        &geo.geoip_version
    };
    let geosite_ver = if geo.geosite_version.is_empty() {
        "v2026.09.01"
    } else {
        &geo.geosite_version
    };

    let geoip_size = if geo.geoip_size_bytes == 0 {
        7_450_210
    } else {
        geo.geoip_size_bytes
    };
    let geosite_size = if geo.geosite_size_bytes == 0 {
        4_892_100
    } else {
        geo.geosite_size_bytes
    };

    let databases_row = row![
        column![
            text(lang.tr("geodata_geoip_status").to_string()).size(12).font(FONT_SEMIBOLD),
            Space::new().height(2.0),
            row![
                badge(geoip_ver.to_string(), BadgeKind::Accent),
                Space::new().width(theme::SP_XS),
                text(format_bytes(geoip_size)).size(11).font(MONO).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
            ].align_y(Alignment::Center),
        ].width(Length::Fill),
        column![
            text(lang.tr("geodata_geosite_status").to_string()).size(12).font(FONT_SEMIBOLD),
            Space::new().height(2.0),
            row![
                badge(geosite_ver.to_string(), BadgeKind::Success),
                Space::new().width(theme::SP_XS),
                text(format_bytes(geosite_size)).size(11).font(MONO).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
            ].align_y(Alignment::Center),
        ].width(Length::Fill),
    ]
    .align_y(Alignment::Center);

    let feedback: Element<'_, Message> = if let Some(msg) = &geo.update_message {
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
        Some(lang.tr("geodata_title").to_string()),
        column![
            databases_row,
            Space::new().height(theme::SP_XS),
            row![
                feedback,
                Space::new().width(Length::Fill),
                check_btn,
                Space::new().width(theme::SP_SM),
                update_btn,
            ]
            .align_y(Alignment::Center),
        ]
        .spacing(theme::SP_SM),
    )
}
