use infiltrator_shared::locales::{Lang, Localizer};
use crate::types::dns::{AdvancedEditMode, DnsTab};
use crate::types::editor::EditorLazyState;
use crate::types::runtime::RebuildFlowState;
use crate::view::components::{
    BadgeKind, card, empty_state, icon_button, modern_scrollable, section_header, segmented_control,
};
use crate::view::svg_icons::Icon;
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, MONO, R_CONTROL, SP_LG, tokens};
use crate::state::AppState;
use crate::types::message::Message;
use iced::widget::{
    Space, button, column, container, pick_list, row, text, text_editor, text_input,
};
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

/// Framed surface for embedded text editors (mono code area).
fn editor_frame(t: &Theme) -> container::Style {
    let tk = tokens(t);
    container::Style {
        background: Some(tk.control_bg.into()),
        border: Border {
            radius: border::Radius::from(R_CONTROL),
            width: 1.0,
            color: tk.card_border,
        },
        ..Default::default()
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
        crate::view::components::toggle_switch(value, on_change),
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill)
    .into()
}

/// Save / Saving… / Saved action used across DNS panels.
fn save_button(
    dirty: bool,
    saving: bool,
    on_press: Message,
    label: &str,
) -> Element<'static, Message> {
    if saving {
        text_btn("Saving...".to_string(), style_ghost, None)
    } else if dirty {
        text_btn(label.to_string(), style_accent, Some(on_press))
    } else {
        text_btn("Saved".to_string(), style_ghost, None)
    }
}

/// Rebuild/save flow status rendered as a tinted pill.
fn rebuild_status_badge(
    state: &RebuildFlowState,
    label: &str,
    dirty: bool,
    loading: bool,
) -> Element<'static, Message> {
    let (text, kind): (&str, BadgeKind) = if loading {
        ("加载中", BadgeKind::Neutral)
    } else if dirty {
        ("已修改", BadgeKind::Warning)
    } else {
        match state {
            RebuildFlowState::Saving { label: current } if current == label => {
                ("保存中", BadgeKind::Accent)
            }
            RebuildFlowState::Rebuilding { label: current } if current == label => {
                ("重建中", BadgeKind::Warning)
            }
            RebuildFlowState::Done { label: current } if current == label => {
                ("完成", BadgeKind::Success)
            }
            RebuildFlowState::Failed { label: current, .. } if current == label => {
                ("失败", BadgeKind::Danger)
            }
            _ => ("已保存", BadgeKind::Success),
        }
    };
    crate::view::components::badge(text, kind)
}

fn validation_error(value: String) -> Element<'static, Message> {
    container(text(value).size(11).style(|t: &Theme| text::Style {
        color: Some(tokens(t).danger),
    }))
    .padding([6, 10])
    .style(|t: &Theme| {
        let tk = tokens(t);
        container::Style {
            background: Some(
                Color {
                    a: 0.14,
                    ..tk.danger
                }
                .into(),
            ),
            border: Border {
                radius: border::Radius::from(theme::R_CHIP),
                ..Default::default()
            },
            ..Default::default()
        }
    })
    .into()
}

fn field_label(value: String) -> text::Text<'static> {
    text(value).size(11).style(|t: &Theme| text::Style {
        color: Some(tokens(t).text_secondary),
    })
}

/// Refresh (ghost icon) + Save (accent) pair shown in card headers.
fn header_actions<'a>(
    refresh: Message,
    save: Message,
    saving: bool,
    dirty: bool,
) -> iced::widget::Row<'a, Message> {
    row![
        icon_button(Icon::RefreshCw, 14.0, refresh),
        Space::new().width(theme::SP_SM),
        save_button(dirty, saving, save, "Save"),
    ]
    .align_y(Alignment::Center)
}

fn lazy_editor_placeholder<'a>(title: String, on_press: Message) -> Element<'a, Message> {
    card(
        None,
        column![
            empty_state(Icon::Code2, title.as_str(), "Editor will load on demand"),
            Space::new().height(theme::SP_SM),
            text_btn("Load Editor".to_string(), style_accent, Some(on_press)),
        ]
        .align_x(Alignment::Center),
    )
}

fn mode_tabs(tab: DnsTab, current: AdvancedEditMode) -> Element<'static, Message> {
    segmented_control(
        &["Form".to_string(), "Raw JSON".to_string()],
        if current == AdvancedEditMode::Json {
            1
        } else {
            0
        },
        move |index| {
            Message::SetAdvancedMode(
                tab,
                if index == 1 {
                    AdvancedEditMode::Json
                } else {
                    AdvancedEditMode::Form
                },
            )
        },
    )
}

fn dns_form_panel(state: &AppState) -> Element<'_, Message> {
    let dirty = state.editor.dns_form_dirty || state.editor.dns_json_dirty;
    let status = rebuild_status_badge(
        &state.runtime.rebuild_flow,
        "DNS",
        dirty,
        !state.editor.advanced_configs_loaded_once,
    );

    let mut content = column![
        section_header(
            "DNS",
            Some(
                row![
                    status,
                    Space::new().width(theme::SP_SM),
                    header_actions(
                        Message::RefreshDnsOnly,
                        Message::SaveDns,
                        state.editor.is_saving_dns,
                        dirty,
                    ),
                ]
                .align_y(Alignment::Center)
                .into(),
            ),
        ),
        Space::new().height(theme::SP_MD),
        toggle_row(
            "enable".to_string(),
            state.editor.dns_form.enable,
            Message::UpdateDnsFormEnable
        ),
        toggle_row(
            "ipv6".to_string(),
            state.editor.dns_form.ipv6,
            Message::UpdateDnsFormIpv6
        ),
        toggle_row(
            "cache".to_string(),
            state.editor.dns_form.cache,
            Message::UpdateDnsFormCache
        ),
        toggle_row(
            "use_hosts".to_string(),
            state.editor.dns_form.use_hosts,
            Message::UpdateDnsFormUseHosts
        ),
        toggle_row(
            "use_system_hosts".to_string(),
            state.editor.dns_form.use_system_hosts,
            Message::UpdateDnsFormUseSystemHosts,
        ),
        toggle_row(
            "respect_rules".to_string(),
            state.editor.dns_form.respect_rules,
            Message::UpdateDnsFormRespectRules,
        ),
        Space::new().height(theme::SP_SM),
        row![
            text("enhanced_mode")
                .size(13)
                .width(Length::Fixed(150.0))
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_primary),
                }),
            pick_list(
                &["fake-ip", "redir-host"][..],
                if state.editor.dns_form.enhanced_mode == "fake-ip"
                    || state.editor.dns_form.enhanced_mode == "redir-host"
                {
                    Some(state.editor.dns_form.enhanced_mode.as_str())
                } else {
                    None
                },
                |v| Message::UpdateDnsFormEnhancedMode(v.to_string())
            )
            .width(Length::Fixed(180.0))
            .style(pick_style),
        ]
        .align_y(Alignment::Center),
        Space::new().height(theme::SP_SM),
        field_label("nameserver (comma/newline separated)".to_string()),
        text_input(
            "https://dns.google/dns-query, 1.1.1.1",
            &state.editor.dns_form.nameserver
        )
        .on_input(Message::UpdateDnsFormNameserver)
        .padding([8, 12])
        .size(12)
        .style(input_style),
        field_label("fallback (comma/newline separated)".to_string()),
        text_input("https://1.0.0.1/dns-query", &state.editor.dns_form.fallback)
            .on_input(Message::UpdateDnsFormFallback)
            .padding([8, 12])
            .size(12)
            .style(input_style),
        field_label("fake_ip_range".to_string()),
        text_input("198.18.0.1/16", &state.editor.dns_form.fake_ip_range)
            .on_input(Message::UpdateDnsFormFakeIpRange)
            .padding([8, 12])
            .size(12)
            .font(MONO)
            .style(input_style),
        field_label("fake_ip_filter (comma/newline separated)".to_string()),
        text_input(
            "*.lan, localhost.ptlogin2.qq.com",
            &state.editor.dns_form.fake_ip_filter
        )
        .on_input(Message::UpdateDnsFormFakeIpFilter)
        .padding([8, 12])
        .size(12)
        .style(input_style),
        field_label("proxy_server_nameserver (comma/newline separated)".to_string()),
        text_input(
            "tls://223.5.5.5:853",
            &state.editor.dns_form.proxy_server_nameserver
        )
        .on_input(Message::UpdateDnsFormProxyServerNameserver)
        .padding([8, 12])
        .size(12)
        .style(input_style),
        field_label("direct_nameserver (comma/newline separated)".to_string()),
        text_input("system", &state.editor.dns_form.direct_nameserver)
            .on_input(Message::UpdateDnsFormDirectNameserver)
            .padding([8, 12])
            .size(12)
            .style(input_style),
    ]
    .spacing(theme::SP_SM);

    if let Some(error) = &state.editor.advanced_validation.dns {
        content = content.push(validation_error(error.clone()));
    }

    card(None, content)
}

fn fake_ip_form_panel(state: &AppState) -> Element<'_, Message> {
    let dirty = state.editor.fake_ip_form_dirty || state.editor.fake_ip_json_dirty;
    let status = rebuild_status_badge(
        &state.runtime.rebuild_flow,
        "Fake-IP",
        dirty,
        !state.editor.advanced_configs_loaded_once,
    );

    let mut content = column![
        section_header(
            "Fake-IP",
            Some(
                row![
                    status,
                    Space::new().width(theme::SP_SM),
                    text_btn(
                        "Flush fake-ip cache".to_string(),
                        style_ghost,
                        Some(Message::FlushFakeIpCache),
                    ),
                    Space::new().width(theme::SP_SM),
                    header_actions(
                        Message::RefreshFakeIpOnly,
                        Message::SaveFakeIpConfig,
                        state.editor.is_saving_fake_ip,
                        dirty,
                    ),
                ]
                .align_y(Alignment::Center)
                .into(),
            ),
        ),
        Space::new().height(theme::SP_MD),
        field_label("fake_ip_range".to_string()),
        text_input("198.18.0.1/16", &state.editor.fake_ip_form.fake_ip_range)
            .on_input(Message::UpdateFakeIpFormRange)
            .padding([8, 12])
            .size(12)
            .font(MONO)
            .style(input_style),
        field_label("fake_ip_filter (comma/newline separated)".to_string()),
        text_input(
            "*.lan, localhost.ptlogin2.qq.com",
            &state.editor.fake_ip_form.fake_ip_filter
        )
        .on_input(Message::UpdateFakeIpFormFilter)
        .padding([8, 12])
        .size(12)
        .style(input_style),
        toggle_row(
            "store_fake_ip".to_string(),
            state.editor.fake_ip_form.store_fake_ip,
            Message::UpdateFakeIpFormStore,
        ),
    ]
    .spacing(theme::SP_SM);

    if let Some(error) = &state.editor.advanced_validation.fake_ip {
        content = content.push(validation_error(error.clone()));
    }

    card(None, content)
}

fn tun_form_panel(state: &AppState) -> Element<'_, Message> {
    let dirty = state.editor.tun_form_dirty || state.editor.tun_json_dirty;
    let status = rebuild_status_badge(
        &state.runtime.rebuild_flow,
        "TUN",
        dirty,
        !state.editor.advanced_configs_loaded_once,
    );

    let mut content = column![
        section_header(
            "TUN",
            Some(
                row![
                    status,
                    Space::new().width(theme::SP_SM),
                    header_actions(
                        Message::RefreshTunOnly,
                        Message::SaveTunConfig,
                        state.editor.is_saving_tun,
                        dirty,
                    ),
                ]
                .align_y(Alignment::Center)
                .into(),
            ),
        ),
        Space::new().height(theme::SP_MD),
        toggle_row(
            "enable".to_string(),
            state.editor.tun_form.enable,
            Message::UpdateTunFormEnable
        ),
        row![
            text("stack")
                .size(13)
                .width(Length::Fixed(150.0))
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_primary),
                }),
            pick_list(
                &["gvisor", "system"][..],
                if state.editor.tun_form.stack == "gvisor"
                    || state.editor.tun_form.stack == "system"
                {
                    Some(state.editor.tun_form.stack.as_str())
                } else {
                    None
                },
                |v| Message::UpdateTunFormStack(v.to_string())
            )
            .width(Length::Fixed(180.0))
            .style(pick_style),
        ]
        .align_y(Alignment::Center),
        field_label("mtu".to_string()),
        text_input("1500", &state.editor.tun_form.mtu)
            .on_input(Message::UpdateTunFormMtu)
            .padding([8, 12])
            .size(12)
            .font(MONO)
            .style(input_style),
        field_label("dns_hijack (comma/newline separated)".to_string()),
        text_input("any:53", &state.editor.tun_form.dns_hijack)
            .on_input(Message::UpdateTunFormDnsHijack)
            .padding([8, 12])
            .size(12)
            .style(input_style),
        toggle_row(
            "auto_route".to_string(),
            state.editor.tun_form.auto_route,
            Message::UpdateTunFormAutoRoute
        ),
        toggle_row(
            "auto_detect_interface".to_string(),
            state.editor.tun_form.auto_detect_interface,
            Message::UpdateTunFormAutoDetectInterface,
        ),
        toggle_row(
            "strict_route".to_string(),
            state.editor.tun_form.strict_route,
            Message::UpdateTunFormStrictRoute
        ),
    ]
    .spacing(theme::SP_SM);

    if let Some(error) = &state.editor.advanced_validation.tun {
        content = content.push(validation_error(error.clone()));
    }

    card(None, content)
}

fn dns_json_panel(state: &AppState) -> Element<'_, Message> {
    if state.editor.dns_editor_state == EditorLazyState::Unloaded {
        return lazy_editor_placeholder("DNS Raw JSON".to_string(), Message::EnsureDnsEditorLoaded);
    }
    let dirty = state.editor.dns_json_dirty || state.editor.dns_form_dirty;
    let status = rebuild_status_badge(
        &state.runtime.rebuild_flow,
        "DNS",
        dirty,
        !state.editor.advanced_configs_loaded_once,
    );
    let mut content = column![
        section_header(
            "DNS Raw JSON",
            Some(
                row![
                    status,
                    Space::new().width(theme::SP_SM),
                    header_actions(
                        Message::RefreshDnsOnly,
                        Message::SaveDns,
                        state.editor.is_saving_dns,
                        dirty,
                    ),
                ]
                .align_y(Alignment::Center)
                .into(),
            ),
        ),
        Space::new().height(theme::SP_SM),
        container(
            text_editor(&state.editor.dns_json_content)
                .on_action(Message::DnsConfigEditorAction)
                .font(MONO)
                .padding(10)
                .height(Length::Fixed(520.0))
        )
        .width(Length::Fill)
        .style(editor_frame),
    ]
    .spacing(theme::SP_SM);
    if let Some(error) = &state.editor.advanced_validation.dns {
        content = content.push(validation_error(error.clone()));
    }
    card(None, content)
}

fn fake_ip_json_panel(state: &AppState) -> Element<'_, Message> {
    if state.editor.fake_ip_editor_state == EditorLazyState::Unloaded {
        return lazy_editor_placeholder(
            "Fake-IP Raw JSON".to_string(),
            Message::EnsureFakeIpEditorLoaded,
        );
    }
    let dirty = state.editor.fake_ip_json_dirty || state.editor.fake_ip_form_dirty;
    let status = rebuild_status_badge(
        &state.runtime.rebuild_flow,
        "Fake-IP",
        dirty,
        !state.editor.advanced_configs_loaded_once,
    );
    let mut content = column![
        section_header(
            "Fake-IP Raw JSON",
            Some(
                row![
                    status,
                    Space::new().width(theme::SP_SM),
                    text_btn(
                        "Flush fake-ip cache".to_string(),
                        style_ghost,
                        Some(Message::FlushFakeIpCache),
                    ),
                    Space::new().width(theme::SP_SM),
                    header_actions(
                        Message::RefreshFakeIpOnly,
                        Message::SaveFakeIpConfig,
                        state.editor.is_saving_fake_ip,
                        dirty,
                    ),
                ]
                .align_y(Alignment::Center)
                .into(),
            ),
        ),
        Space::new().height(theme::SP_SM),
        container(
            text_editor(&state.editor.fake_ip_json_content)
                .on_action(Message::FakeIpConfigEditorAction)
                .font(MONO)
                .padding(10)
                .height(Length::Fixed(520.0))
        )
        .width(Length::Fill)
        .style(editor_frame),
    ]
    .spacing(theme::SP_SM);
    if let Some(error) = &state.editor.advanced_validation.fake_ip {
        content = content.push(validation_error(error.clone()));
    }
    card(None, content)
}

fn tun_json_panel(state: &AppState) -> Element<'_, Message> {
    if state.editor.tun_editor_state == EditorLazyState::Unloaded {
        return lazy_editor_placeholder("TUN Raw JSON".to_string(), Message::EnsureTunEditorLoaded);
    }
    let dirty = state.editor.tun_json_dirty || state.editor.tun_form_dirty;
    let status = rebuild_status_badge(
        &state.runtime.rebuild_flow,
        "TUN",
        dirty,
        !state.editor.advanced_configs_loaded_once,
    );
    let mut content = column![
        section_header(
            "TUN Raw JSON",
            Some(
                row![
                    status,
                    Space::new().width(theme::SP_SM),
                    header_actions(
                        Message::RefreshTunOnly,
                        Message::SaveTunConfig,
                        state.editor.is_saving_tun,
                        dirty,
                    ),
                ]
                .align_y(Alignment::Center)
                .into(),
            ),
        ),
        Space::new().height(theme::SP_SM),
        container(
            text_editor(&state.editor.tun_json_content)
                .on_action(Message::TunConfigEditorAction)
                .font(MONO)
                .padding(10)
                .height(Length::Fixed(520.0))
        )
        .width(Length::Fill)
        .style(editor_frame),
    ]
    .spacing(theme::SP_SM);
    if let Some(error) = &state.editor.advanced_validation.tun {
        content = content.push(validation_error(error.clone()));
    }
    card(None, content)
}

pub fn view(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);

    let header = row![
        text(lang.tr("dns_title").to_string())
            .size(24)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_primary),
            }),
    ]
    .align_y(Alignment::Center);

    let tab_index = match state.editor.dns_tab {
        DnsTab::FakeIp => 1,
        DnsTab::Tun => 2,
        DnsTab::Dns => 0,
    };
    let tabs = segmented_control(
        &["DNS".to_string(), "Fake-IP".to_string(), "TUN".to_string()],
        tab_index,
        |index| {
            Message::SetDnsTab(match index {
                1 => DnsTab::FakeIp,
                2 => DnsTab::Tun,
                _ => DnsTab::Dns,
            })
        },
    );

    if !state.editor.dns_heavy_ready {
        return modern_scrollable(
            column![
                header,
                Space::new().height(theme::SP_MD),
                tabs,
                Space::new().height(SP_LG),
                card(
                    None,
                    column![
                        text("Preparing advanced panels...")
                            .size(14)
                            .font(FONT_SEMIBOLD)
                            .style(|t: &Theme| text::Style {
                                color: Some(tokens(t).text_primary),
                            }),
                        text("Heavy editors are mounted lazily after first paint.")
                            .size(12)
                            .style(|t: &Theme| text::Style {
                                color: Some(tokens(t).text_secondary),
                            }),
                    ]
                    .spacing(theme::SP_SM),
                ),
            ]
            .spacing(10),
        )
        .height(Length::Fill)
        .into();
    }

    let section = match state.editor.dns_tab {
        DnsTab::Dns => {
            let mode_tabs = mode_tabs(DnsTab::Dns, state.editor.dns_mode);
            let body = if state.editor.dns_mode == AdvancedEditMode::Form {
                dns_form_panel(state)
            } else {
                dns_json_panel(state)
            };
            column![mode_tabs, Space::new().height(10), body].spacing(0)
        }
        DnsTab::FakeIp => {
            let mode_tabs = mode_tabs(DnsTab::FakeIp, state.editor.fake_ip_mode);
            let body = if state.editor.fake_ip_mode == AdvancedEditMode::Form {
                fake_ip_form_panel(state)
            } else {
                fake_ip_json_panel(state)
            };
            column![mode_tabs, Space::new().height(10), body].spacing(0)
        }
        DnsTab::Tun => {
            let mode_tabs = mode_tabs(DnsTab::Tun, state.editor.tun_mode);
            let body = if state.editor.tun_mode == AdvancedEditMode::Form {
                tun_form_panel(state)
            } else {
                tun_json_panel(state)
            };
            column![mode_tabs, Space::new().height(10), body].spacing(0)
        }
    };

    modern_scrollable(
        column![
            header,
            Space::new().height(theme::SP_MD),
            tabs,
            Space::new().height(theme::SP_MD),
            section,
        ]
        .spacing(10),
    )
    .height(Length::Fill)
    .into()
}
