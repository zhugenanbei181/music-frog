use crate::locales::{Lang, Localizer};
use crate::view::components::{card, modern_scrollable, section_header, status_dot, toggle_switch};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, R_CONTROL, SP_LG, tokens};
use crate::{AppState, Message};
use iced::widget::{Space, button, column, container, row, text, text_input};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};

// ---------------------------------------------------------------------------
// Token-driven control styles (ui-wave2-r)
// ---------------------------------------------------------------------------

fn style_accent(t: &Theme, status: button::Status) -> button::Style {
    let tk = tokens(t);
    let (bg, fg) = match status {
        button::Status::Disabled => (tk.accent_soft, tk.accent),
        button::Status::Hovered | button::Status::Pressed => (
            Color {
                a: 0.85,
                ..tk.accent
            },
            tk.on_accent,
        ),
        _ => (tk.accent, tk.on_accent),
    };
    button::Style {
        background: Some(bg.into()),
        border: Border {
            radius: border::Radius::from(R_CONTROL),
            ..Default::default()
        },
        text_color: fg,
        ..Default::default()
    }
}

fn style_ghost(t: &Theme, status: button::Status) -> button::Style {
    let tk = tokens(t);
    button::Style {
        background: match status {
            button::Status::Hovered | button::Status::Pressed => Some(tk.control_bg.into()),
            _ => None,
        },
        border: Border {
            radius: border::Radius::from(R_CONTROL),
            width: 1.0,
            color: tk.card_border,
        },
        text_color: match status {
            button::Status::Disabled => tk.text_tertiary,
            button::Status::Hovered | button::Status::Pressed => tk.text_primary,
            _ => tk.text_secondary,
        },
        ..Default::default()
    }
}

/// Text push button: `on_press == None` renders the disabled state.
fn text_btn<'a>(
    label: String,
    style: fn(&Theme, button::Status) -> button::Style,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    button(text(label).size(12).font(FONT_MEDIUM))
        .padding([8, 16])
        .style(style)
        .on_press_maybe(on_press)
        .into()
}

fn input_style(t: &Theme, status: text_input::Status) -> text_input::Style {
    let tk = tokens(t);
    let (border_color, border_width) = match status {
        text_input::Status::Focused { .. } => (tk.accent, 1.5),
        _ => (tk.card_border, 1.0),
    };
    text_input::Style {
        background: tk.control_bg.into(),
        border: Border {
            radius: border::Radius::from(R_CONTROL),
            width: border_width,
            color: border_color,
        },
        icon: tk.text_tertiary,
        placeholder: tk.text_tertiary,
        value: tk.text_primary,
        selection: Color {
            a: 0.25,
            ..tk.accent
        },
    }
}

fn field_label(value: String) -> text::Text<'static> {
    text(value)
        .size(11)
        .font(theme::FONT_MEDIUM)
        .style(|t: &Theme| text::Style {
            color: Some(tokens(t).text_secondary),
        })
}

fn toggle_row<'a>(
    label: String,
    value: bool,
    on_change: impl Fn(bool) -> Message + 'a,
) -> Element<'a, Message> {
    row![
        text(label).size(13).style(|t: &Theme| text::Style {
            color: Some(tokens(t).text_primary),
        }),
        Space::new().width(Length::Fill),
        toggle_switch(value, on_change),
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill)
    .into()
}

pub fn view(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);

    let header = text(lang.tr("sync_title").to_string())
        .size(24)
        .font(FONT_SEMIBOLD)
        .style(|t: &Theme| text::Style {
            color: Some(tokens(t).text_primary),
        });

    let save_settings_btn: Element<'_, Message> = if state.profile.is_saving_app_settings {
        text_btn("Saving...".to_string(), style_ghost, None)
    } else {
        text_btn(
            "Save Settings".to_string(),
            style_accent,
            Some(Message::SaveAppSettings),
        )
    };

    // Live sync status card: dot reflects WebDAV activity/enablement.
    let status_section = card(
        None,
        row![
            status_dot(state.profile.is_syncing || state.profile.webdav_enabled),
            Space::new().width(theme::SP_MD),
            column![
                text(if state.profile.is_syncing {
                    "Syncing...".to_string()
                } else if state.profile.webdav_enabled {
                    "WebDAV auto sync enabled".to_string()
                } else {
                    "WebDAV auto sync disabled".to_string()
                })
                .size(13)
                .font(FONT_SEMIBOLD)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_primary),
                }),
                text(if state.profile.webdav_sync_on_startup {
                    "Syncs on startup".to_string()
                } else {
                    "Manual sync only".to_string()
                })
                .size(11)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary),
                }),
            ]
            .spacing(2),
            Space::new().width(Length::Fill),
            text(format!("{} min", state.profile.webdav_sync_interval_mins))
                .size(12)
                .font(theme::MONO)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary),
                }),
        ]
        .align_y(Alignment::Center),
    );

    let sync_form = card(
        None,
        column![
            toggle_row(
                "Enable WebDAV auto sync".to_string(),
                state.profile.webdav_enabled,
                Message::UpdateWebDavEnabled,
            ),
            Space::new().height(theme::SP_MD),
            column![
                field_label(lang.tr("sync_url").to_string()),
                Space::new().height(theme::SP_XS),
                text_input("https://dav.example.com", &state.profile.webdav_url)
                    .on_input(Message::UpdateWebDavUrl)
                    .padding([8, 12])
                    .size(13)
                    .style(input_style),
            ]
            .spacing(theme::SP_XS),
            Space::new().height(theme::SP_MD),
            row![
                column![
                    field_label(lang.tr("sync_user").to_string()),
                    Space::new().height(theme::SP_XS),
                    text_input(lang.tr("sync_user").as_ref(), &state.profile.webdav_user)
                        .on_input(Message::UpdateWebDavUser)
                        .padding([8, 12])
                        .size(13)
                        .style(input_style),
                ]
                .width(Length::FillPortion(1))
                .spacing(theme::SP_XS),
                Space::new().width(theme::SP_LG),
                column![
                    field_label(lang.tr("sync_pass").to_string()),
                    Space::new().height(theme::SP_XS),
                    text_input(lang.tr("sync_pass").as_ref(), &state.profile.webdav_pass)
                        .on_input(Message::UpdateWebDavPass)
                        .padding([8, 12])
                        .size(13)
                        .secure(true)
                        .style(input_style),
                ]
                .width(Length::FillPortion(1))
                .spacing(theme::SP_XS),
            ],
            Space::new().height(theme::SP_MD),
            row![
                column![
                    field_label("Sync Interval (mins)".to_string()),
                    Space::new().height(theme::SP_XS),
                    text_input("60", &state.profile.webdav_sync_interval_mins)
                        .on_input(Message::UpdateWebDavSyncInterval)
                        .padding([8, 12])
                        .size(13)
                        .font(theme::MONO)
                        .style(input_style),
                ]
                .width(Length::FillPortion(1))
                .spacing(theme::SP_XS),
                Space::new().width(theme::SP_LG),
                column![
                    field_label("Startup Behavior".to_string()),
                    Space::new().height(theme::SP_XS),
                    toggle_row(
                        "Sync on startup".to_string(),
                        state.profile.webdav_sync_on_startup,
                        Message::UpdateWebDavSyncOnStartup,
                    ),
                ]
                .width(Length::FillPortion(1))
                .spacing(theme::SP_XS),
            ],
            Space::new().height(theme::SP_LG),
            save_settings_btn,
            Space::new().height(theme::SP_LG),
            row![
                button(
                    container(
                        text(lang.tr("sync_upload").to_string())
                            .size(12)
                            .font(FONT_MEDIUM)
                    )
                    .width(Length::Fill)
                    .align_x(Alignment::Center)
                    .padding(10)
                )
                .width(Length::FillPortion(1))
                .style(style_ghost)
                .on_press(Message::SyncUpload),
                Space::new().width(theme::SP_LG),
                button(
                    container(
                        text(lang.tr("sync_download").to_string())
                            .size(12)
                            .font(FONT_MEDIUM)
                    )
                    .width(Length::Fill)
                    .align_x(Alignment::Center)
                    .padding(10)
                )
                .width(Length::FillPortion(1))
                .style(style_accent)
                .on_press(Message::SyncDownload),
            ],
        ]
        .spacing(theme::SP_SM),
    );

    let content = column![
        header,
        Space::new().height(theme::SP_LG),
        section_header("WebDAV", None),
        status_section,
        Space::new().height(SP_LG),
        sync_form,
    ]
    .spacing(theme::SP_MD);

    modern_scrollable(content)
        .height(Length::Fill)
        .width(Length::Fill)
        .into()
}
