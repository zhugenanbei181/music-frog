//! About, Disclaimer, Licenses, and Tools hub views.

use crate::state::AppState;
use crate::types::app::Route;
use crate::types::message::Message;
use crate::view::component_card::card;
use crate::view::component_forms::{style_accent, style_ghost, text_btn};
use crate::view::components::modern_scrollable;
use crate::view::theme;
use crate::view::theme::{FONT_SEMIBOLD, MONO, SP_MD, tokens};
use iced::widget::{Space, column, container, row, text};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_shared::assets::{
    disclaimer_text, harmonyos_font_license_text, third_party_notices_text,
};
use infiltrator_shared::locales::{Lang, Localizer};

/// About page view: product identity, version, and links to licenses and disclaimer.
pub fn about_view(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);

    let header = column![
        text(lang.tr("nav_about").into_owned())
            .size(24)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_primary),
            }),
        text(lang.tr("about_app_subtitle").into_owned())
            .size(12)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary),
            }),
    ]
    .spacing(theme::SP_XS);

    let info_card = card(
        Some(lang.tr("about_app_info").into_owned()),
        column![
            row![
                text(lang.tr("about_version_label").into_owned())
                    .size(13)
                    .font(FONT_SEMIBOLD),
                text(format!("v{}", env!("CARGO_PKG_VERSION")))
                    .size(13)
                    .font(MONO),
            ]
            .spacing(theme::SP_SM),
            text(lang.tr("app_description").into_owned())
                .size(12)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary),
                }),
            Space::new().height(theme::SP_SM),
            row![
                text_btn(
                    lang.tr("nav_disclaimer").into_owned(),
                    style_ghost,
                    Some(Message::Navigate(Route::Disclaimer))
                ),
                text_btn(
                    lang.tr("nav_licenses").into_owned(),
                    style_ghost,
                    Some(Message::Navigate(Route::Licenses))
                ),
                text_btn(
                    lang.tr("nav_tools").into_owned(),
                    style_accent,
                    Some(Message::Navigate(Route::Tools))
                ),
            ]
            .spacing(theme::SP_MD),
        ]
        .spacing(theme::SP_SM),
    );

    let content = column![header, Space::new().height(theme::SP_MD), info_card,]
        .spacing(SP_MD)
        .max_width(800);

    modern_scrollable(container(content).padding(theme::SP_LG)).into()
}

/// Product disclaimer view: compliance and safety warning.
pub fn disclaimer_view(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);

    let header = row![
        text(lang.tr("nav_disclaimer").into_owned())
            .size(24)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_primary),
            }),
        Space::new().width(Length::Fill),
        text_btn(
            lang.tr("nav_about").into_owned(),
            style_ghost,
            Some(Message::Navigate(Route::About))
        ),
    ]
    .align_y(Alignment::Center);

    let disclaimer_body = disclaimer_text(&state.shell.lang);
    let card_content = column![
        text(disclaimer_body)
            .size(13)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_primary),
            }),
        Space::new().height(theme::SP_MD),
        text_btn(
            lang.tr("nav_about").into_owned(),
            style_accent,
            Some(Message::Navigate(Route::About))
        ),
    ]
    .spacing(theme::SP_SM);

    let content = column![
        header,
        Space::new().height(theme::SP_MD),
        card(None, card_content),
    ]
    .spacing(SP_MD)
    .max_width(800);

    modern_scrollable(container(content).padding(theme::SP_LG)).into()
}

/// Open source licenses and third-party notices view.
pub fn licenses_view(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);

    let header = row![
        text(lang.tr("nav_licenses").into_owned())
            .size(24)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_primary),
            }),
        Space::new().width(Length::Fill),
        text_btn(
            lang.tr("nav_about").into_owned(),
            style_ghost,
            Some(Message::Navigate(Route::About))
        ),
    ]
    .align_y(Alignment::Center);

    let font_card = card(
        Some(lang.tr("settings_font_attribution_title").into_owned()),
        column![
            text(lang.tr("settings_font_attribution_copy").into_owned())
                .size(12)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary),
                }),
            container(
                modern_scrollable(text(harmonyos_font_license_text()).size(10).font(MONO))
                    .height(180)
            )
            .padding(theme::SP_SM)
            .style(|t: &Theme| container::Style {
                background: Some(tokens(t).card_bg.into()),
                ..Default::default()
            }),
        ]
        .spacing(theme::SP_SM),
    );

    let notices_card = card(
        Some(lang.tr("about_third_party_title").into_owned()),
        column![
            container(
                modern_scrollable(text(third_party_notices_text()).size(10).font(MONO)).height(300)
            )
            .padding(theme::SP_SM)
            .style(|t: &Theme| container::Style {
                background: Some(tokens(t).card_bg.into()),
                ..Default::default()
            }),
        ]
        .spacing(theme::SP_SM),
    );

    let content = column![
        header,
        Space::new().height(theme::SP_MD),
        font_card,
        Space::new().height(theme::SP_SM),
        notices_card,
    ]
    .spacing(SP_MD)
    .max_width(800);

    modern_scrollable(container(content).padding(theme::SP_LG)).into()
}

/// Tools hub view: quick navigation to all diagnostic and management utilities.
pub fn tools_view(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);

    let header = column![
        text(lang.tr("nav_tools").into_owned())
            .size(24)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_primary),
            }),
        text(lang.tr("tools_subtitle").into_owned())
            .size(12)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary),
            }),
    ]
    .spacing(theme::SP_XS);

    let btn_open_label = lang.tr("btn_open").into_owned();

    let tools_list = column![
        tool_row(
            lang.tr("nav_doctor").into_owned(),
            lang.tr("tools_doctor_desc").into_owned(),
            btn_open_label.clone(),
            Route::Doctor,
        ),
        tool_row(
            lang.tr("nav_app_routing").into_owned(),
            lang.tr("tools_app_routing_desc").into_owned(),
            btn_open_label.clone(),
            Route::AppRouting,
        ),
        tool_row(
            lang.tr("nav_sync").into_owned(),
            lang.tr("tools_sync_desc").into_owned(),
            btn_open_label.clone(),
            Route::Sync,
        ),
        tool_row(
            lang.tr("tools_editor_title").into_owned(),
            lang.tr("tools_editor_desc").into_owned(),
            btn_open_label.clone(),
            Route::Editor,
        ),
        tool_row(
            lang.tr("nav_dns").into_owned(),
            lang.tr("tools_dns_desc").into_owned(),
            btn_open_label.clone(),
            Route::Dns,
        ),
        tool_row(
            lang.tr("nav_settings").into_owned(),
            lang.tr("tools_settings_desc").into_owned(),
            btn_open_label.clone(),
            Route::Settings,
        ),
        tool_row(
            lang.tr("nav_about").into_owned(),
            lang.tr("tools_about_desc").into_owned(),
            btn_open_label,
            Route::About,
        ),
    ]
    .spacing(theme::SP_SM);

    let content = column![
        header,
        Space::new().height(theme::SP_MD),
        card(None, tools_list),
    ]
    .spacing(SP_MD)
    .max_width(800);

    modern_scrollable(container(content).padding(theme::SP_LG)).into()
}

fn tool_row<'a>(
    title: String,
    desc: String,
    btn_text: String,
    route: Route,
) -> Element<'a, Message> {
    row![
        column![
            text(title).size(14).font(FONT_SEMIBOLD),
            text(desc).size(12).style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary),
            }),
        ]
        .spacing(2)
        .width(Length::Fill),
        text_btn(btn_text, style_accent, Some(Message::Navigate(route)),),
    ]
    .align_y(Alignment::Center)
    .padding([8, 12])
    .into()
}
