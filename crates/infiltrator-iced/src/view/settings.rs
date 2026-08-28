use crate::locales::{Lang, Localizer};
use crate::view::components::{card, modern_scrollable};
use crate::view::icons;
use crate::{AppState, Message};
use iced::widget::{Space, button, checkbox, column, container, pick_list, row, text, text_input};
use iced::{Alignment, Border, Color, Element, Font, Length, Theme, border};

pub fn view(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.lang);
    let bold_font = Font {
        weight: iced::font::Weight::Bold,
        ..Default::default()
    };

    let header = text(lang.tr("nav_settings")).size(24).font(bold_font);

    // 0. UAC Prompt (if not admin)
    let uac_banner = if !state.is_admin {
        Some(
            container(column![
                text(lang.tr("admin_status")).font(bold_font).size(16),
                Space::new().height(8),
                text(lang.tr("settings_uac_desc")).size(13),
                Space::new().height(15),
                button(text(lang.tr("settings_uac_request")).size(12))
                    .on_press(Message::RequestAdminPrivilege)
                    .padding([8, 16])
                    .style(button::primary),
            ])
            .padding(20)
            .width(Length::Fill)
            .style(|_theme: &Theme| container::Style {
                background: Some(Color::from_rgba(0.8, 0.4, 0.0, 0.1).into()),
                border: Border {
                    radius: border::Radius::from(12.0),
                    width: 1.0,
                    color: Color::from_rgba(0.8, 0.4, 0.0, 0.5),
                },
                ..Default::default()
            }),
        )
    } else {
        None
    };

    // 1. System Integration Card
    let system_section = card(column![
        text(lang.tr("settings_system_integration")).font(bold_font),
        Space::new().height(15),
        checkbox(state.autostart_enabled)
            .label(lang.tr("autostart").into_owned())
            .on_toggle(Message::SetAutostart)
            .size(18),
        Space::new().height(10),
        checkbox(state.system_proxy_enabled)
            .label(lang.tr("system_proxy").into_owned())
            .on_toggle(Message::SetSystemProxy)
            .size(18),
        Space::new().height(15),
        row![
            text(lang.tr("theme")).size(14).width(Length::Fill),
            button(
                text(if state.theme == Theme::Dark {
                    "Dark Mode"
                } else {
                    "Light Mode"
                })
                .size(12)
            )
            .on_press(Message::ToggleTheme)
            .padding([6, 12])
            .style(button::secondary)
        ]
        .align_y(Alignment::Center),
        Space::new().height(15),
        button(text(lang.tr("settings_factory_reset")).size(12))
            .on_press(Message::FactoryReset)
            .padding([8, 16])
            .style(button::danger),
    ]);

    // 2. TUN Mode Section
    let tun_section = card(column![
        text(lang.tr("tun_mode")).font(bold_font),
        Space::new().height(15),
        row![
            text(lang.tr("tun_stack")).size(14),
            Space::new().width(10),
            pick_list(
                &["gvisor", "mixed", "system"][..],
                Some(state.tun_stack.as_str()),
                |s| { Message::SetTunStack(s.to_string()) }
            )
            .width(Length::Fixed(150.0)),
        ]
        .align_y(Alignment::Center),
        Space::new().height(10),
        checkbox(state.tun_auto_route)
            .label(lang.tr("tun_auto_route").into_owned())
            .on_toggle(Message::SetTunAutoRoute)
            .size(18),
        Space::new().height(10),
        checkbox(state.tun_strict_route)
            .label(lang.tr("tun_strict_route").into_owned())
            .on_toggle(Message::SetTunStrictRoute)
            .size(18),
    ]);

    // 3. Sniffer Section
    let sniffer_section = card(column![
        text(lang.tr("settings_sniffer")).font(bold_font),
        Space::new().height(8),
        text(lang.tr("settings_sniffer_desc"))
            .size(12)
            .style(|_| text::Style {
                color: Some(Color::from_rgb(0.5, 0.5, 0.5))
            }),
        Space::new().height(15),
        checkbox(state.sniffer_enabled)
            .label(lang.tr("settings_sniffer").into_owned())
            .on_toggle(Message::SetSnifferEnabled)
            .size(18),
    ]);

    // 4. Editor Path
    let editor_section = card(column![
        text("External Editor").font(bold_font),
        Space::new().height(8),
        text("Set a preferred editor executable path (optional).")
            .size(12)
            .style(|_| text::Style {
                color: Some(Color::from_rgb(0.5, 0.5, 0.5))
            }),
        Space::new().height(12),
        text_input(
            "e.g. C:\\Program Files\\Sublime Text\\subl.exe",
            &state.editor_path_setting
        )
        .on_input(Message::UpdateEditorPathSetting)
        .padding(10)
        .size(14),
        Space::new().height(12),
        row![
            if state.is_saving_app_settings {
                Element::from(
                    button(text("Saving...").size(12))
                        .padding([6, 12])
                        .style(button::secondary),
                )
            } else {
                Element::from(
                    button(text("Save Editor Path").size(12))
                        .on_press(Message::SaveAppSettings)
                        .padding([6, 12])
                        .style(button::secondary),
                )
            },
            Space::new().width(10),
            button(text("Reset").size(12))
                .on_press(Message::UpdateEditorPathSetting(String::new()))
                .padding([6, 12])
                .style(button::secondary),
        ]
        .align_y(Alignment::Center),
    ]);

    // 5. Kernel Management
    let mut kernel_list = column![
        row![
            text(lang.tr("settings_kernel_mgmt"))
                .font(bold_font)
                .width(Length::Fill),
            if state.is_checking_update {
                Element::from(text(lang.tr("settings_checking")).size(12))
            } else {
                button(
                    row![
                        text(icons::REFRESH).size(12),
                        text(lang.tr("settings_check_update")).size(12)
                    ]
                    .spacing(8),
                )
                .on_press(Message::CheckCoreUpdate)
                .padding([6, 12])
                .style(button::secondary)
                .into()
            }
        ]
        .align_y(Alignment::Center),
        Space::new().height(15),
    ]
    .spacing(10);

    if let Some(latest) = &state.latest_core_version {
        kernel_list = kernel_list.push(
            container(
                row![
                    text(format!("{} {}", lang.tr("settings_available"), latest))
                        .size(13)
                        .width(Length::Fill),
                    button(
                        row![
                            text(icons::UPDATE).size(12),
                            text(lang.tr("settings_download")).size(12)
                        ]
                        .spacing(8)
                    )
                    .on_press(Message::DownloadCore(latest.clone()))
                    .padding([6, 12])
                    .style(button::primary)
                ]
                .align_y(Alignment::Center),
            )
            .padding(10)
            .style(|_| container::Style {
                background: Some(Color::from_rgba(0.2, 0.5, 0.2, 0.1).into()),
                border: border::Border {
                    radius: border::Radius::from(8.0),
                    ..Default::default()
                },
                ..Default::default()
            }),
        );
    }

    if state.installed_kernels.is_empty() {
        kernel_list =
            kernel_list.push(Element::from(text(lang.tr("settings_no_kernels")).size(12)));
    } else {
        for kernel in &state.installed_kernels {
            kernel_list = kernel_list.push(
                container(
                    row![
                        column![
                            text(&kernel.version).size(14).font(bold_font),
                            text(if kernel.is_default {
                                lang.tr("active_tag")
                            } else {
                                "".into()
                            })
                            .size(10)
                            .style(|_| text::Style {
                                color: Some(Color::from_rgb(0.3, 0.6, 1.0))
                            }),
                        ]
                        .width(Length::Fill),
                        if !kernel.is_default {
                            Element::from(row![
                                button(text(lang.tr("settings_set_default")).size(10))
                                    .on_press(Message::SetDefaultKernel(kernel.version.clone()))
                                    .padding([4, 8])
                                    .style(button::secondary),
                                Space::new().width(10),
                                button(text(icons::DELETE).size(12))
                                    .on_press(Message::DeleteKernel(kernel.version.clone()))
                                    .padding([4, 8])
                                    .style(button::danger),
                            ])
                        } else {
                            Element::from(text(lang.tr("settings_installed")).size(11).style(
                                |_| text::Style {
                                    color: Some(Color::from_rgb(0.4, 0.4, 0.4)),
                                },
                            ))
                        }
                    ]
                    .align_y(Alignment::Center),
                )
                .padding(12)
                .style(|_| container::Style {
                    background: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.03).into()),
                    border: border::Border {
                        radius: border::Radius::from(8.0),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
            );
        }
    }

    let mut content = column![header, Space::new().height(20)].spacing(10);

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
        .push(card(kernel_list))
        .push(Space::new().height(40));

    modern_scrollable(content).height(Length::Fill).into()
}
