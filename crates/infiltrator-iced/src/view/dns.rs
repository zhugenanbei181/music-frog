use crate::locales::{Lang, Localizer};
use crate::types::{AdvancedEditMode, DnsTab, EditorLazyState, RebuildFlowState};
use crate::view::components::{card, modern_scrollable};
use crate::view::icons;
use crate::{AppState, Message};
use iced::widget::{
    Space, button, checkbox, column, container, pick_list, row, text, text_editor, text_input,
};
use iced::{Alignment, Color, Element, Font, Length};

fn tab_button<'a>(
    label: &'a str,
    active: bool,
    on_press: Message,
) -> iced::widget::Button<'a, Message> {
    button(text(label).size(12))
        .padding([6, 12])
        .style(if active {
            button::primary
        } else {
            button::secondary
        })
        .on_press(on_press)
}

fn mode_button<'a>(
    label: &'a str,
    active: bool,
    on_press: Message,
) -> iced::widget::Button<'a, Message> {
    button(text(label).size(11))
        .padding([5, 10])
        .style(if active {
            button::primary
        } else {
            button::secondary
        })
        .on_press(on_press)
}

fn save_button<'a>(
    saving: bool,
    dirty: bool,
    on_press: Message,
    label: &'a str,
) -> Element<'a, Message> {
    if saving {
        button(text("Saving...").size(12))
            .padding([6, 12])
            .style(button::secondary)
            .into()
    } else if dirty {
        button(row![text(icons::SAVE).size(12), text(label).size(12)].spacing(8))
            .on_press(on_press)
            .padding([6, 12])
            .style(button::primary)
            .into()
    } else {
        button(text("Saved").size(12))
            .padding([6, 12])
            .style(button::secondary)
            .into()
    }
}

fn rebuild_status_text(
    state: &RebuildFlowState,
    label: &str,
    dirty: bool,
    loading: bool,
) -> (String, Color) {
    if loading {
        return ("加载中".to_string(), Color::from_rgb(0.6, 0.6, 0.6));
    }
    if dirty {
        return ("已修改".to_string(), Color::from_rgb(0.95, 0.75, 0.25));
    }
    match state {
        RebuildFlowState::Saving { label: current } if current == label => {
            ("保存中".to_string(), Color::from_rgb(0.25, 0.65, 0.95))
        }
        RebuildFlowState::Rebuilding { label: current } if current == label => {
            ("重建中".to_string(), Color::from_rgb(0.95, 0.7, 0.25))
        }
        RebuildFlowState::Done { label: current } if current == label => {
            ("完成".to_string(), Color::from_rgb(0.25, 0.8, 0.45))
        }
        RebuildFlowState::Failed { label: current, .. } if current == label => {
            ("失败".to_string(), Color::from_rgb(0.95, 0.3, 0.3))
        }
        _ => ("已保存".to_string(), Color::from_rgb(0.35, 0.8, 0.55)),
    }
}

fn form_mode_header<'a>(
    title: &'a str,
    status: (String, Color),
    refresh: Message,
    save: Message,
    saving: bool,
    dirty: bool,
) -> iced::widget::Column<'a, Message> {
    column![
        row![
            text(title).size(16),
            Space::new().width(Length::Fill),
            text(status.0).size(11).style(move |_| text::Style {
                color: Some(status.1)
            }),
            Space::new().width(10),
            button(row![text(icons::REFRESH).size(12), text("Refresh").size(12)].spacing(8))
                .on_press(refresh)
                .padding([6, 12])
                .style(button::secondary),
            Space::new().width(8),
            save_button(saving, dirty, save, "Save")
        ]
        .align_y(Alignment::Center)
    ]
}

fn mode_tabs(tab: DnsTab, current: AdvancedEditMode) -> iced::widget::Row<'static, Message> {
    row![
        mode_button(
            "Form",
            current == AdvancedEditMode::Form,
            Message::SetAdvancedMode(tab, AdvancedEditMode::Form)
        ),
        mode_button(
            "Raw JSON",
            current == AdvancedEditMode::Json,
            Message::SetAdvancedMode(tab, AdvancedEditMode::Json)
        ),
    ]
    .spacing(8)
}

fn dns_form_panel(state: &AppState) -> Element<'_, Message> {
    let dirty = state.dns_form_dirty || state.dns_json_dirty;
    let status = rebuild_status_text(
        &state.rebuild_flow,
        "DNS",
        dirty,
        !state.advanced_configs_loaded_once,
    );

    let mut content = column![
        form_mode_header(
            "DNS",
            status,
            Message::RefreshDnsOnly,
            Message::SaveDns,
            state.is_saving_dns,
            dirty
        ),
        Space::new().height(12),
        checkbox(state.dns_form.enable)
            .label("enable".to_string())
            .on_toggle(Message::UpdateDnsFormEnable)
            .size(14),
        checkbox(state.dns_form.ipv6)
            .label("ipv6".to_string())
            .on_toggle(Message::UpdateDnsFormIpv6)
            .size(14),
        checkbox(state.dns_form.cache)
            .label("cache".to_string())
            .on_toggle(Message::UpdateDnsFormCache)
            .size(14),
        checkbox(state.dns_form.use_hosts)
            .label("use_hosts".to_string())
            .on_toggle(Message::UpdateDnsFormUseHosts)
            .size(14),
        checkbox(state.dns_form.use_system_hosts)
            .label("use_system_hosts".to_string())
            .on_toggle(Message::UpdateDnsFormUseSystemHosts)
            .size(14),
        checkbox(state.dns_form.respect_rules)
            .label("respect_rules".to_string())
            .on_toggle(Message::UpdateDnsFormRespectRules)
            .size(14),
        Space::new().height(6),
        row![
            text("enhanced_mode").size(12).width(Length::Fixed(150.0)),
            pick_list(
                &["fake-ip", "redir-host"][..],
                if state.dns_form.enhanced_mode == "fake-ip"
                    || state.dns_form.enhanced_mode == "redir-host"
                {
                    Some(state.dns_form.enhanced_mode.as_str())
                } else {
                    None
                },
                |v| Message::UpdateDnsFormEnhancedMode(v.to_string())
            )
            .width(Length::Fixed(180.0))
        ]
        .align_y(Alignment::Center),
        Space::new().height(6),
        text("nameserver (comma/newline separated)").size(11),
        text_input(
            "https://dns.google/dns-query, 1.1.1.1",
            &state.dns_form.nameserver
        )
        .on_input(Message::UpdateDnsFormNameserver)
        .padding(8)
        .size(12),
        text("fallback (comma/newline separated)").size(11),
        text_input("https://1.0.0.1/dns-query", &state.dns_form.fallback)
            .on_input(Message::UpdateDnsFormFallback)
            .padding(8)
            .size(12),
        text("fake_ip_range").size(11),
        text_input("198.18.0.1/16", &state.dns_form.fake_ip_range)
            .on_input(Message::UpdateDnsFormFakeIpRange)
            .padding(8)
            .size(12),
        text("fake_ip_filter (comma/newline separated)").size(11),
        text_input(
            "*.lan, localhost.ptlogin2.qq.com",
            &state.dns_form.fake_ip_filter
        )
        .on_input(Message::UpdateDnsFormFakeIpFilter)
        .padding(8)
        .size(12),
        text("proxy_server_nameserver (comma/newline separated)").size(11),
        text_input(
            "tls://223.5.5.5:853",
            &state.dns_form.proxy_server_nameserver
        )
        .on_input(Message::UpdateDnsFormProxyServerNameserver)
        .padding(8)
        .size(12),
        text("direct_nameserver (comma/newline separated)").size(11),
        text_input("system", &state.dns_form.direct_nameserver)
            .on_input(Message::UpdateDnsFormDirectNameserver)
            .padding(8)
            .size(12),
    ]
    .spacing(8);

    if let Some(error) = &state.advanced_validation.dns {
        content = content.push(
            container(text(error).size(11).style(|_| text::Style {
                color: Some(Color::from_rgb(0.95, 0.35, 0.35)),
            }))
            .padding([6, 10]),
        );
    }

    card(content)
}

fn fake_ip_form_panel(state: &AppState) -> Element<'_, Message> {
    let dirty = state.fake_ip_form_dirty || state.fake_ip_json_dirty;
    let status = rebuild_status_text(
        &state.rebuild_flow,
        "Fake-IP",
        dirty,
        !state.advanced_configs_loaded_once,
    );

    let mut content = column![
        row![
            text("Fake-IP").size(16),
            Space::new().width(Length::Fill),
            text(status.0).size(11).style(move |_| text::Style {
                color: Some(status.1)
            }),
            Space::new().width(10),
            button(text("Flush fake-ip cache").size(12))
                .on_press(Message::FlushFakeIpCache)
                .padding([6, 12])
                .style(button::secondary),
            Space::new().width(8),
            button(row![text(icons::REFRESH).size(12), text("Refresh").size(12)].spacing(8))
                .on_press(Message::RefreshFakeIpOnly)
                .padding([6, 12])
                .style(button::secondary),
            Space::new().width(8),
            save_button(
                state.is_saving_fake_ip,
                dirty,
                Message::SaveFakeIpConfig,
                "Save"
            )
        ]
        .align_y(Alignment::Center),
        Space::new().height(12),
        text("fake_ip_range").size(11),
        text_input("198.18.0.1/16", &state.fake_ip_form.fake_ip_range)
            .on_input(Message::UpdateFakeIpFormRange)
            .padding(8)
            .size(12),
        text("fake_ip_filter (comma/newline separated)").size(11),
        text_input(
            "*.lan, localhost.ptlogin2.qq.com",
            &state.fake_ip_form.fake_ip_filter
        )
        .on_input(Message::UpdateFakeIpFormFilter)
        .padding(8)
        .size(12),
        checkbox(state.fake_ip_form.store_fake_ip)
            .label("store_fake_ip".to_string())
            .on_toggle(Message::UpdateFakeIpFormStore)
            .size(14),
    ]
    .spacing(8);

    if let Some(error) = &state.advanced_validation.fake_ip {
        content = content.push(
            container(text(error).size(11).style(|_| text::Style {
                color: Some(Color::from_rgb(0.95, 0.35, 0.35)),
            }))
            .padding([6, 10]),
        );
    }

    card(content)
}

fn tun_form_panel(state: &AppState) -> Element<'_, Message> {
    let dirty = state.tun_form_dirty || state.tun_json_dirty;
    let status = rebuild_status_text(
        &state.rebuild_flow,
        "TUN",
        dirty,
        !state.advanced_configs_loaded_once,
    );

    let mut content = column![
        form_mode_header(
            "TUN",
            status,
            Message::RefreshTunOnly,
            Message::SaveTunConfig,
            state.is_saving_tun,
            dirty
        ),
        Space::new().height(12),
        checkbox(state.tun_form.enable)
            .label("enable".to_string())
            .on_toggle(Message::UpdateTunFormEnable)
            .size(14),
        row![
            text("stack").size(12).width(Length::Fixed(150.0)),
            pick_list(
                &["gvisor", "system"][..],
                if state.tun_form.stack == "gvisor" || state.tun_form.stack == "system" {
                    Some(state.tun_form.stack.as_str())
                } else {
                    None
                },
                |v| Message::UpdateTunFormStack(v.to_string())
            )
            .width(Length::Fixed(180.0))
        ]
        .align_y(Alignment::Center),
        text("mtu").size(11),
        text_input("1500", &state.tun_form.mtu)
            .on_input(Message::UpdateTunFormMtu)
            .padding(8)
            .size(12),
        text("dns_hijack (comma/newline separated)").size(11),
        text_input("any:53", &state.tun_form.dns_hijack)
            .on_input(Message::UpdateTunFormDnsHijack)
            .padding(8)
            .size(12),
        checkbox(state.tun_form.auto_route)
            .label("auto_route".to_string())
            .on_toggle(Message::UpdateTunFormAutoRoute)
            .size(14),
        checkbox(state.tun_form.auto_detect_interface)
            .label("auto_detect_interface".to_string())
            .on_toggle(Message::UpdateTunFormAutoDetectInterface)
            .size(14),
        checkbox(state.tun_form.strict_route)
            .label("strict_route".to_string())
            .on_toggle(Message::UpdateTunFormStrictRoute)
            .size(14),
    ]
    .spacing(8);

    if let Some(error) = &state.advanced_validation.tun {
        content = content.push(
            container(text(error).size(11).style(|_| text::Style {
                color: Some(Color::from_rgb(0.95, 0.35, 0.35)),
            }))
            .padding([6, 10]),
        );
    }

    card(content)
}

fn dns_json_panel(state: &AppState) -> Element<'_, Message> {
    if state.dns_editor_state == EditorLazyState::Unloaded {
        return card(
            column![
                text("DNS Raw JSON").size(16),
                text("Editor will load on demand").size(12),
                button(text("Load Editor").size(12))
                    .padding([6, 12])
                    .style(button::secondary)
                    .on_press(Message::EnsureDnsEditorLoaded)
            ]
            .spacing(10),
        );
    }
    let dirty = state.dns_json_dirty || state.dns_form_dirty;
    let status = rebuild_status_text(
        &state.rebuild_flow,
        "DNS",
        dirty,
        !state.advanced_configs_loaded_once,
    );
    let mut content = column![
        row![
            text("DNS Raw JSON").size(16),
            Space::new().width(Length::Fill),
            text(status.0).size(11).style(move |_| text::Style {
                color: Some(status.1)
            }),
            Space::new().width(10),
            button(row![text(icons::REFRESH).size(12), text("Refresh").size(12)].spacing(8))
                .on_press(Message::RefreshDnsOnly)
                .padding([6, 12])
                .style(button::secondary),
            Space::new().width(8),
            save_button(state.is_saving_dns, dirty, Message::SaveDns, "Save")
        ]
        .align_y(Alignment::Center),
        Space::new().height(10),
        text_editor(&state.dns_json_content)
            .on_action(Message::DnsConfigEditorAction)
            .padding(10)
            .height(Length::Fixed(520.0))
    ]
    .spacing(8);
    if let Some(error) = &state.advanced_validation.dns {
        content = content.push(text(error).size(11).style(|_| text::Style {
            color: Some(Color::from_rgb(0.95, 0.35, 0.35)),
        }));
    }
    card(content)
}

fn fake_ip_json_panel(state: &AppState) -> Element<'_, Message> {
    if state.fake_ip_editor_state == EditorLazyState::Unloaded {
        return card(
            column![
                text("Fake-IP Raw JSON").size(16),
                text("Editor will load on demand").size(12),
                button(text("Load Editor").size(12))
                    .padding([6, 12])
                    .style(button::secondary)
                    .on_press(Message::EnsureFakeIpEditorLoaded)
            ]
            .spacing(10),
        );
    }
    let dirty = state.fake_ip_json_dirty || state.fake_ip_form_dirty;
    let status = rebuild_status_text(
        &state.rebuild_flow,
        "Fake-IP",
        dirty,
        !state.advanced_configs_loaded_once,
    );
    let mut content = column![
        row![
            text("Fake-IP Raw JSON").size(16),
            Space::new().width(Length::Fill),
            text(status.0).size(11).style(move |_| text::Style {
                color: Some(status.1)
            }),
            Space::new().width(10),
            button(text("Flush fake-ip cache").size(12))
                .on_press(Message::FlushFakeIpCache)
                .padding([6, 12])
                .style(button::secondary),
            Space::new().width(8),
            button(row![text(icons::REFRESH).size(12), text("Refresh").size(12)].spacing(8))
                .on_press(Message::RefreshFakeIpOnly)
                .padding([6, 12])
                .style(button::secondary),
            Space::new().width(8),
            save_button(
                state.is_saving_fake_ip,
                dirty,
                Message::SaveFakeIpConfig,
                "Save"
            )
        ]
        .align_y(Alignment::Center),
        Space::new().height(10),
        text_editor(&state.fake_ip_json_content)
            .on_action(Message::FakeIpConfigEditorAction)
            .padding(10)
            .height(Length::Fixed(520.0))
    ]
    .spacing(8);
    if let Some(error) = &state.advanced_validation.fake_ip {
        content = content.push(text(error).size(11).style(|_| text::Style {
            color: Some(Color::from_rgb(0.95, 0.35, 0.35)),
        }));
    }
    card(content)
}

fn tun_json_panel(state: &AppState) -> Element<'_, Message> {
    if state.tun_editor_state == EditorLazyState::Unloaded {
        return card(
            column![
                text("TUN Raw JSON").size(16),
                text("Editor will load on demand").size(12),
                button(text("Load Editor").size(12))
                    .padding([6, 12])
                    .style(button::secondary)
                    .on_press(Message::EnsureTunEditorLoaded)
            ]
            .spacing(10),
        );
    }
    let dirty = state.tun_json_dirty || state.tun_form_dirty;
    let status = rebuild_status_text(
        &state.rebuild_flow,
        "TUN",
        dirty,
        !state.advanced_configs_loaded_once,
    );
    let mut content = column![
        row![
            text("TUN Raw JSON").size(16),
            Space::new().width(Length::Fill),
            text(status.0).size(11).style(move |_| text::Style {
                color: Some(status.1)
            }),
            Space::new().width(10),
            button(row![text(icons::REFRESH).size(12), text("Refresh").size(12)].spacing(8))
                .on_press(Message::RefreshTunOnly)
                .padding([6, 12])
                .style(button::secondary),
            Space::new().width(8),
            save_button(state.is_saving_tun, dirty, Message::SaveTunConfig, "Save")
        ]
        .align_y(Alignment::Center),
        Space::new().height(10),
        text_editor(&state.tun_json_content)
            .on_action(Message::TunConfigEditorAction)
            .padding(10)
            .height(Length::Fixed(520.0))
    ]
    .spacing(8);
    if let Some(error) = &state.advanced_validation.tun {
        content = content.push(text(error).size(11).style(|_| text::Style {
            color: Some(Color::from_rgb(0.95, 0.35, 0.35)),
        }));
    }
    card(content)
}

pub fn view(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.lang);
    let bold_font = Font {
        weight: iced::font::Weight::Bold,
        ..Default::default()
    };

    let header =
        row![text(lang.tr("dns_title")).size(24).font(bold_font)].align_y(Alignment::Center);

    let tabs = row![
        tab_button(
            "DNS",
            state.dns_tab == DnsTab::Dns,
            Message::SetDnsTab(DnsTab::Dns)
        ),
        tab_button(
            "Fake-IP",
            state.dns_tab == DnsTab::FakeIp,
            Message::SetDnsTab(DnsTab::FakeIp)
        ),
        tab_button(
            "TUN",
            state.dns_tab == DnsTab::Tun,
            Message::SetDnsTab(DnsTab::Tun)
        ),
    ]
    .spacing(8);

    if !state.dns_heavy_ready {
        return modern_scrollable(
            column![
                header,
                Space::new().height(12),
                tabs,
                Space::new().height(16),
                card(
                    column![
                        text("Preparing advanced panels...").font(bold_font),
                        text("Heavy editors are mounted lazily after first paint.").size(12)
                    ]
                    .spacing(8),
                )
            ]
            .spacing(10),
        )
        .height(Length::Fill)
        .into();
    }

    let section = match state.dns_tab {
        DnsTab::Dns => {
            let mode_tabs = mode_tabs(DnsTab::Dns, state.dns_mode);
            let body = if state.dns_mode == AdvancedEditMode::Form {
                dns_form_panel(state)
            } else {
                dns_json_panel(state)
            };
            column![mode_tabs, Space::new().height(10), body].spacing(0)
        }
        DnsTab::FakeIp => {
            let mode_tabs = mode_tabs(DnsTab::FakeIp, state.fake_ip_mode);
            let body = if state.fake_ip_mode == AdvancedEditMode::Form {
                fake_ip_form_panel(state)
            } else {
                fake_ip_json_panel(state)
            };
            column![mode_tabs, Space::new().height(10), body].spacing(0)
        }
        DnsTab::Tun => {
            let mode_tabs = mode_tabs(DnsTab::Tun, state.tun_mode);
            let body = if state.tun_mode == AdvancedEditMode::Form {
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
            Space::new().height(12),
            tabs,
            Space::new().height(12),
            section
        ]
        .spacing(10),
    )
    .height(Length::Fill)
    .into()
}
