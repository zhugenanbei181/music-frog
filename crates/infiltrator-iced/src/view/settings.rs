use crate::locales::{Lang, Localizer};
use crate::view::components::{
    BadgeKind, card, icon_button, modern_scrollable, section_header, status_dot, toggle_switch,
};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, MONO, R_CONTROL, SP_MD, tokens};
use crate::{AppState, Message};
use iced::widget::{Space, button, column, container, pick_list, row, text, text_input};
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

fn style_danger(t: &Theme, status: button::Status) -> button::Style {
    let tk = tokens(t);
    let (bg, fg) = match status {
        button::Status::Disabled => (tk.accent_soft, tk.text_tertiary),
        button::Status::Hovered | button::Status::Pressed => (
            Color {
                a: 0.24,
                ..tk.danger
            },
            tk.on_accent,
        ),
        _ => (
            Color {
                a: 0.14,
                ..tk.danger
            },
            tk.danger,
        ),
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

/// Text push button: `on_press == None` renders the disabled state.
fn text_btn<'a>(
    label: String,
    style: fn(&Theme, button::Status) -> button::Style,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    button(text(label).size(12).font(FONT_MEDIUM))
        .padding([7, 14])
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

fn pick_style(t: &Theme, _status: pick_list::Status) -> pick_list::Style {
    let tk = tokens(t);
    pick_list::Style {
        text_color: tk.text_primary,
        placeholder_color: tk.text_tertiary,
        handle_color: tk.text_secondary,
        background: tk.control_bg.into(),
        border: Border {
            radius: border::Radius::from(R_CONTROL),
            width: 1.0,
            color: tk.card_border,
        },
    }
}

/// iOS-style toggle row: label on the left, switch on the right.
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

fn secondary_text(value: String) -> Element<'static, Message> {
    text(value)
        .size(12)
        .style(|t: &Theme| text::Style {
            color: Some(tokens(t).text_secondary),
        })
        .into()
}

pub fn view(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);

    let header = text(lang.tr("nav_settings").to_string())
        .size(24)
        .font(FONT_SEMIBOLD)
        .style(|t: &Theme| text::Style {
            color: Some(tokens(t).text_primary),
        });

    // 0. UAC Prompt (if not admin)
    let uac_banner = if !state.shell.is_admin {
        let banner_body = card(
            None,
            column![
                row![
                    svg_icons::icon_themed(Icon::Shield, 16.0, |t: &Theme| tokens(t).warning),
                    Space::new().width(theme::SP_SM),
                    text(lang.tr("admin_status").to_string())
                        .size(15)
                        .font(FONT_SEMIBOLD)
                        .style(|t: &Theme| text::Style {
                            color: Some(tokens(t).text_primary),
                        }),
                ]
                .align_y(Alignment::Center),
                Space::new().height(theme::SP_SM),
                secondary_text(lang.tr("settings_uac_desc").to_string()),
                Space::new().height(theme::SP_MD),
                text_btn(
                    lang.tr("settings_uac_request").to_string(),
                    style_accent,
                    Some(Message::RequestAdminPrivilege),
                ),
            ]
            .spacing(theme::SP_SM),
        );
        Some(container(banner_body).style(|t: &Theme| {
            let tk = tokens(t);
            container::Style {
                background: Some(
                    Color {
                        a: 0.10,
                        ..tk.warning
                    }
                    .into(),
                ),
                border: Border {
                    radius: border::Radius::from(theme::R_CARD),
                    width: 1.0,
                    color: Color {
                        a: 0.35,
                        ..tk.warning
                    },
                },
                ..Default::default()
            }
        }))
    } else {
        None
    };

    // 1. System Integration Card
    let system_section = card(
        Some(lang.tr("settings_system_integration").to_string()),
        column![
            toggle_row(
                lang.tr("autostart").to_string(),
                state.runtime.autostart_enabled,
                Message::SetAutostart,
            ),
            toggle_row(
                lang.tr("system_proxy").to_string(),
                state.runtime.system_proxy_enabled,
                Message::SetSystemProxy,
            ),
            Space::new().height(theme::SP_SM),
            row![
                text(lang.tr("theme").to_string())
                    .size(13)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_primary),
                    }),
                Space::new().width(Length::Fill),
                text_btn(
                    if state.shell.theme == Theme::Dark {
                        "Dark Mode".to_string()
                    } else {
                        "Light Mode".to_string()
                    },
                    style_ghost,
                    Some(Message::ToggleTheme),
                ),
            ]
            .align_y(Alignment::Center),
            Space::new().height(theme::SP_MD),
            text_btn(
                lang.tr("settings_factory_reset").to_string(),
                style_danger,
                Some(Message::FactoryReset),
            ),
        ]
        .spacing(theme::SP_SM),
    );

    // 2. TUN Mode Section
    let tun_section = card(
        Some(lang.tr("tun_mode").to_string()),
        column![
            row![
                text(lang.tr("tun_stack").to_string())
                    .size(13)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_primary),
                    }),
                Space::new().width(theme::SP_MD),
                pick_list(
                    &["gvisor", "mixed", "system"][..],
                    Some(state.editor.tun_stack.as_str()),
                    |s| { Message::SetTunStack(s.to_string()) }
                )
                .width(Length::Fixed(150.0))
                .style(pick_style),
                Space::new().width(Length::Fill),
            ]
            .align_y(Alignment::Center),
            toggle_row(
                lang.tr("tun_auto_route").to_string(),
                state.editor.tun_auto_route,
                Message::SetTunAutoRoute,
            ),
            toggle_row(
                lang.tr("tun_strict_route").to_string(),
                state.editor.tun_strict_route,
                Message::SetTunStrictRoute,
            ),
        ]
        .spacing(theme::SP_SM),
    );

    // 3. Sniffer Section
    let sniffer_section = card(
        Some(lang.tr("settings_sniffer").to_string()),
        column![
            secondary_text(lang.tr("settings_sniffer_desc").to_string()),
            toggle_row(
                lang.tr("settings_sniffer").to_string(),
                state.editor.sniffer_enabled,
                Message::SetSnifferEnabled,
            ),
        ]
        .spacing(theme::SP_SM),
    );

    // 4. Editor Path
    let editor_section = card(
        Some("External Editor".to_string()),
        column![
            secondary_text("Set a preferred editor executable path (optional).".to_string()),
            text_input(
                "e.g. C:\\Program Files\\Sublime Text\\subl.exe",
                &state.editor.editor_path_setting
            )
            .on_input(Message::UpdateEditorPathSetting)
            .padding([8, 12])
            .size(13)
            .style(input_style),
            row![
                if state.profile.is_saving_app_settings {
                    text_btn("Saving...".to_string(), style_ghost, None)
                } else {
                    text_btn(
                        "Save Editor Path".to_string(),
                        style_ghost,
                        Some(Message::SaveAppSettings),
                    )
                },
                Space::new().width(theme::SP_SM),
                text_btn(
                    "Reset".to_string(),
                    style_ghost,
                    Some(Message::UpdateEditorPathSetting(String::new())),
                ),
            ]
            .align_y(Alignment::Center),
        ]
        .spacing(theme::SP_SM),
    );

    // 5. Web Admin (loopback management UI served to the browser)
    let admin_running = state.shell.admin_server.is_running();
    let admin_section = card(
        Some(lang.tr("settings_admin_web").to_string()),
        column![
            secondary_text(lang.tr("settings_admin_desc").to_string()),
            toggle_row(
                lang.tr("settings_admin_enable").to_string(),
                state.shell.admin_enabled,
                Message::SetAdminEnabled,
            ),
            row![
                text(lang.tr("settings_admin_port").to_string())
                    .size(13)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_primary),
                    }),
                Space::new().width(theme::SP_MD),
                text_input("25210", &state.shell.admin_port_input)
                    .on_input(Message::UpdateAdminPort)
                    .width(Length::Fixed(120.0))
                    .padding([8, 12])
                    .size(13)
                    .font(MONO)
                    .style(input_style),
                Space::new().width(theme::SP_MD),
                text_btn(
                    lang.tr("settings_admin_apply").to_string(),
                    style_ghost,
                    Some(Message::ApplyAdminSettings),
                ),
                Space::new().width(Length::Fill),
            ]
            .align_y(Alignment::Center),
            row![
                status_dot(admin_running),
                Space::new().width(theme::SP_SM),
                text(if admin_running {
                    lang.tr("settings_admin_running").into_owned()
                } else {
                    lang.tr("settings_admin_stopped").into_owned()
                })
                .size(12)
                .style(move |t: &Theme| text::Style {
                    color: Some(if admin_running {
                        tokens(t).success
                    } else {
                        tokens(t).text_secondary
                    }),
                }),
                Space::new().width(theme::SP_MD),
                if admin_running {
                    row![
                        text(state.shell.admin_server.url().unwrap_or_default())
                            .size(12)
                            .font(MONO)
                            .style(|t: &Theme| text::Style {
                                color: Some(tokens(t).text_secondary),
                            }),
                        Space::new().width(theme::SP_MD),
                        text_btn(
                            lang.tr("settings_admin_open").to_string(),
                            style_ghost,
                            Some(Message::OpenWebAdmin),
                        ),
                    ]
                    .align_y(Alignment::Center)
                    .into()
                } else {
                    Element::from(Space::new().width(Length::Shrink))
                },
            ]
            .align_y(Alignment::Center),
        ]
        .spacing(theme::SP_SM),
    );

    // 6. Kernel Management
    let mut kernel_rows = column![].spacing(theme::SP_SM);

    if let Some(latest) = &state.runtime.latest_core_version {
        kernel_rows = kernel_rows.push(
            container(
                row![
                    text(format!("{} {}", lang.tr("settings_available"), latest))
                        .size(13)
                        .width(Length::Fill),
                    text_btn(
                        lang.tr("settings_download").to_string(),
                        style_accent,
                        Some(Message::DownloadCore(latest.clone())),
                    ),
                ]
                .align_y(Alignment::Center),
            )
            .padding(theme::SP_MD)
            .width(Length::Fill)
            .style(|t: &Theme| {
                let tk = tokens(t);
                container::Style {
                    background: Some(
                        Color {
                            a: 0.10,
                            ..tk.success
                        }
                        .into(),
                    ),
                    border: border::Border {
                        radius: border::Radius::from(R_CONTROL),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            }),
        );
    }

    if state.runtime.installed_kernels.is_empty() {
        kernel_rows = kernel_rows.push(secondary_text(lang.tr("settings_no_kernels").to_string()));
    } else {
        for kernel in &state.runtime.installed_kernels {
            kernel_rows = kernel_rows.push(
                container(
                    row![
                        column![
                            text(&kernel.version).size(13).font(FONT_SEMIBOLD).style(
                                |t: &Theme| text::Style {
                                    color: Some(tokens(t).text_primary),
                                }
                            ),
                            if kernel.is_default {
                                crate::view::components::badge(
                                    lang.tr("active_tag").trim().to_string(),
                                    BadgeKind::Success,
                                )
                            } else {
                                Space::new().width(0).height(0).into()
                            },
                        ]
                        .spacing(2)
                        .width(Length::Fill),
                        if !kernel.is_default {
                            Element::from(
                                row![
                                    text_btn(
                                        lang.tr("settings_set_default").to_string(),
                                        style_ghost,
                                        Some(Message::SetDefaultKernel(kernel.version.clone())),
                                    ),
                                    Space::new().width(theme::SP_SM),
                                    icon_button(
                                        Icon::Trash2,
                                        14.0,
                                        Message::DeleteKernel(kernel.version.clone()),
                                    ),
                                ]
                                .align_y(Alignment::Center),
                            )
                        } else {
                            Element::from(secondary_text(lang.tr("settings_installed").to_string()))
                        },
                    ]
                    .align_y(Alignment::Center),
                )
                .padding([theme::SP_SM, SP_MD])
                .width(Length::Fill)
                .style(|t: &Theme| {
                    let tk = tokens(t);
                    container::Style {
                        background: Some(tk.card_bg.into()),
                        border: Border {
                            radius: border::Radius::from(R_CONTROL),
                            width: 1.0,
                            color: tk.card_border,
                        },
                        ..Default::default()
                    }
                }),
            );
        }
    }

    let mut content = column![header, Space::new().height(theme::SP_LG)].spacing(10);

    if let Some(banner) = uac_banner {
        content = content.push(banner);
        content = content.push(Space::new().height(10));
    }

    content = content
        .push(system_section)
        .push(Space::new().height(10))
        .push(tun_section)
        .push(Space::new().height(10))
        .push(sniffer_section)
        .push(Space::new().height(10))
        .push(editor_section)
        .push(Space::new().height(10))
        .push(admin_section)
        .push(Space::new().height(10))
        .push(card(
            None,
            column![
                section_header(
                    lang.tr("settings_kernel_mgmt").as_ref(),
                    Some(if state.runtime.is_checking_update {
                        text(lang.tr("settings_checking").to_string())
                            .size(12)
                            .style(|t: &Theme| text::Style {
                                color: Some(tokens(t).text_tertiary),
                            })
                            .into()
                    } else {
                        icon_button(Icon::RefreshCw, 14.0, Message::CheckCoreUpdate)
                    }),
                ),
                Space::new().height(theme::SP_MD),
                kernel_rows,
            ],
        ))
        .push(Space::new().height(40));

    modern_scrollable(content).height(Length::Fill).into()
}
