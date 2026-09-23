//! Current public-IP probe card with copy action and honest probe source.

use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use crate::view::component_forms::style_ghost;
use crate::view::components::{card_surface, chip, icon_button};
use crate::view::svg_icons::{Icon, icon_themed};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, MONO, tokens};
use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_shared::locales::{Lang, Localizer};

pub fn current_ip_card<'a>(
    state: &'a AppState,
    lang: &Lang<'a>,
    _is_en: bool,
) -> Element<'a, Message> {
    let public_ip_str = state
        .diag
        .public_ip
        .as_deref()
        .unwrap_or(if state.shell.demo {
            "203.0.113.7"
        } else {
            "—"
        });
    let provider_name = state
        .diag
        .public_ip_provider
        .as_deref()
        .unwrap_or("ipapi.is");

    let copy_msg = Message::ShowToast(
        infiltrator_shared::i18n_interpolator::interpolate(
            &lang.tr("overview_copied_ip"),
            &[("ip", public_ip_str)],
        ),
        ToastStatus::Success,
    );

    let copy_btn = button(
        row![
            icon_themed(Icon::Copy, 12.0, |t| tokens(t).text_secondary),
            Space::new().width(theme::SP_XS),
            text(lang.tr("common_copy").to_string())
                .size(11)
                .font(FONT_MEDIUM),
        ]
        .align_y(Alignment::Center),
    )
    .padding([4, 10])
    .style(style_ghost)
    .on_press(copy_msg);

    let card_header = row![
        row![
            icon_themed(Icon::Globe, 16.0, |t: &Theme| tokens(t).accent),
            Space::new().width(theme::SP_SM),
            text(lang.tr("overview_current_ip").to_string())
                .size(14)
                .font(FONT_SEMIBOLD)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_primary)
                }),
        ]
        .align_y(Alignment::Center),
        Space::new().width(Length::Fill),
        chip(provider_name),
        Space::new().width(theme::SP_SM),
        icon_button(Icon::RefreshCw, 14.0, Message::FetchIpInfo),
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill);

    let ip_readout_row = row![
        text(public_ip_str)
            .size(20)
            .font(MONO)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_primary),
            }),
        Space::new().width(Length::Fill),
        copy_btn,
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill);

    let meta_text: Element<'a, Message> = if let Some(err) = state.diag.public_ip_error.as_deref() {
        text(format!("{}: {err}", lang.tr("overview_probe_failed")))
            .size(11)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).danger),
            })
            .into()
    } else if let Some(checked_at) = state.diag.public_ip_checked_at.as_deref() {
        text(format!(
            "{} · {provider_name} · {checked_at}",
            lang.tr("overview_via_current_proxy")
        ))
        .size(11)
        .style(|t: &Theme| text::Style {
            color: Some(tokens(t).text_secondary),
        })
        .into()
    } else {
        text(lang.tr("overview_probe_source_desc").to_string())
            .size(11)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_tertiary),
            })
            .into()
    };

    container(
        column![
            card_header,
            Space::new().height(theme::SP_MD),
            ip_readout_row,
            Space::new().height(theme::SP_XS),
            meta_text,
        ]
        .spacing(theme::SP_XS),
    )
    .width(Length::FillPortion(1))
    .padding(theme::SP_XXL)
    .style(card_surface)
    .into()
}
