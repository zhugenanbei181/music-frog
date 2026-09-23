use crate::state::AppState;
use crate::types::dns::{AdvancedEditMode, DnsTab};
use crate::types::editor::EditorLazyState;
use crate::types::message::Message;
use crate::types::runtime::RebuildFlowState;
use crate::view::component_forms::{
    banner_alert, editor_frame_surface, form_field_label, form_input_style, form_pick_style,
    form_toggle_row, row_card_surface, style_accent, style_ghost, text_btn,
};
use crate::view::components::{
    BadgeKind, badge, card, empty_state, icon_button, modern_scrollable, section_header,
    segmented_control,
};
use crate::view::dns_form_panel::{
    dns_cache_flush_status, dns_form_field_widget, dynamic_token_section, form_issue_banner,
};
use crate::view::dns_hosts_panel::{
    fake_ip_pool_panel, hosts_panel, latency_policy_line, self_heal_panel,
};
use crate::view::svg_icons::Icon;
use crate::view::theme::{self, FONT_SEMIBOLD, MONO, SP_LG, tokens};
use iced::widget::{Space, column, container, pick_list, row, text, text_editor, text_input};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_contract::dns::DnsServerTag;
use infiltrator_contract::dns_form::DnsFormField;
use infiltrator_shared::locales::{Lang, Localizer};

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

fn rebuild_status_badge(
    lang: &Lang<'_>,
    state: &RebuildFlowState,
    label: &str,
    dirty: bool,
    loading: bool,
) -> Element<'static, Message> {
    let (txt, kind) = if loading {
        (
            lang.tr("dns_status_loading").to_string(),
            BadgeKind::Neutral,
        )
    } else if dirty {
        (
            lang.tr("dns_status_modified").to_string(),
            BadgeKind::Warning,
        )
    } else {
        match state {
            RebuildFlowState::Saving { label: c } if c == label => {
                (lang.tr("dns_status_saving").to_string(), BadgeKind::Accent)
            }
            RebuildFlowState::Rebuilding { label: c } if c == label => (
                lang.tr("dns_status_rebuilding").to_string(),
                BadgeKind::Warning,
            ),
            RebuildFlowState::Done { label: c } if c == label => {
                (lang.tr("dns_status_done").to_string(), BadgeKind::Success)
            }
            RebuildFlowState::Failed { label: c, .. } if c == label => {
                (lang.tr("dns_status_failed").to_string(), BadgeKind::Danger)
            }
            _ => (lang.tr("dns_status_saved").to_string(), BadgeKind::Success),
        }
    };
    badge(txt, kind)
}

fn validation_error_banner(error: &str, lang: &Lang<'_>) -> Element<'static, Message> {
    let title = lang.tr("dns_validation_error");
    banner_alert(BadgeKind::Danger, title, error.to_string(), None)
}

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
        move |idx| {
            Message::SetAdvancedMode(
                tab,
                if idx == 1 {
                    AdvancedEditMode::Json
                } else {
                    AdvancedEditMode::Form
                },
            )
        },
    )
}

/// Protocol chip label for a nameserver address (shared contract decode).
pub fn dns_protocol_chip(server: &str) -> &'static str {
    infiltrator_contract::dns::DnsUpstreamProtocol::from_address(server).chip_label()
}

pub(crate) fn parse_item_list(raw: &str) -> Vec<String> {
    infiltrator_contract::dns::parse_server_list(raw)
}

pub(crate) fn remove_item_from_list(raw: &str, index: usize) -> String {
    infiltrator_contract::dns::remove_server_at(raw, index)
}

pub(crate) fn append_item_to_list(raw: &str, item: &str) -> String {
    infiltrator_contract::dns::append_server(raw, item)
}

pub(crate) fn server_tag_label(tag: DnsServerTag, lang: &Lang<'_>) -> String {
    match tag {
        DnsServerTag::Domestic => lang.tr("dns_tag_domestic"),
        DnsServerTag::Fallback => lang.tr("dns_tag_fallback"),
        DnsServerTag::Encrypted => lang.tr("dns_tag_encrypted"),
        DnsServerTag::Plain => lang.tr("dns_tag_plain"),
    }
    .to_string()
}

fn dns_form_panel<'a>(state: &'a AppState, lang: &Lang<'a>) -> Element<'a, Message> {
    let dirty = state.editor.dns_form_dirty || state.editor.dns_json_dirty;
    let status = rebuild_status_badge(
        lang,
        &state.runtime.rebuild_flow,
        "DNS",
        dirty,
        !state.editor.advanced_configs_loaded_once,
    );
    let title = lang.tr("dns_config_sec");

    let mut content = column![
        section_header(
            title.as_ref(),
            Some(
                row![
                    status,
                    Space::new().width(theme::SP_SM),
                    header_actions(
                        Message::RefreshDnsOnly,
                        Message::SaveDns,
                        state.editor.is_saving_dns,
                        dirty
                    )
                ]
                .align_y(Alignment::Center)
                .into()
            )
        ),
        Space::new().height(theme::SP_MD),
    ]
    .spacing(theme::SP_SM);

    // DUAL-14-14: the panel is assembled from the shared field set, so a
    // field cannot exist on one surface and be missing on the other.
    for field in DnsFormField::ALL {
        content = content.push(dns_form_field_widget(field, &state.editor.dns_form, lang));
    }

    let issues = state.editor.dns_form.validate();
    if !issues.is_empty() {
        content = content.push(form_issue_banner(&issues, lang));
    }
    if let Some(error) = &state.editor.advanced_validation.dns {
        content = content.push(validation_error_banner(error, lang));
    }
    card(None, content)
}

fn fake_ip_form_panel<'a>(state: &'a AppState, lang: &Lang<'a>) -> Element<'a, Message> {
    let dirty = state.editor.fake_ip_form_dirty || state.editor.fake_ip_json_dirty;
    let status = rebuild_status_badge(
        lang,
        &state.runtime.rebuild_flow,
        "Fake-IP",
        dirty,
        !state.editor.advanced_configs_loaded_once,
    );
    let title = lang.tr("dns_fakeip_sec");
    let flush_label = lang.tr("dns_flush_fakeip").to_string();

    let mut content = column![
        section_header(
            title.as_ref(),
            Some(
                row![
                    status,
                    Space::new().width(theme::SP_SM),
                    text_btn(
                        flush_label.clone(),
                        style_ghost,
                        Some(Message::FlushFakeIpCache)
                    ),
                    Space::new().width(theme::SP_SM),
                    header_actions(
                        Message::RefreshFakeIpOnly,
                        Message::SaveFakeIpConfig,
                        state.editor.is_saving_fake_ip,
                        dirty
                    )
                ]
                .align_y(Alignment::Center)
                .into()
            )
        ),
        Space::new().height(theme::SP_MD),
        form_field_label("fake_ip_range".to_string()),
        text_input("198.18.0.1/16", &state.editor.fake_ip_form.fake_ip_range)
            .on_input(Message::UpdateFakeIpFormRange)
            .padding([8, 12])
            .size(12)
            .font(MONO)
            .style(form_input_style),
        Space::new().height(theme::SP_XS),
        dynamic_token_section(
            "fake_ip_filter",
            &state.editor.fake_ip_form.fake_ip_filter,
            "*.lan, localhost.ptlogin2.qq.com",
            &[
                "*.lan",
                "localhost.ptlogin2.qq.com",
                "*.local",
                "+.msftconnecttest.com"
            ],
            true,
            lang,
            Message::UpdateFakeIpFormFilter
        ),
        Space::new().height(theme::SP_SM),
        form_toggle_row(
            lang.tr("dns_store_fake_ip").to_string(),
            state.editor.fake_ip_form.store_fake_ip,
            Message::UpdateFakeIpFormStore
        ),
        Space::new().height(theme::SP_SM),
        container(
            row![
                text(lang.tr("dns_flush_fakeip_desc").to_string())
                    .size(12)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_secondary)
                    }),
                Space::new().width(Length::Fill),
                text_btn(flush_label, style_ghost, Some(Message::FlushFakeIpCache)),
            ]
            .align_y(Alignment::Center)
        )
        .padding([8, 12])
        .style(row_card_surface),
        Space::new().height(theme::SP_XS),
        dns_cache_flush_status(state, lang),
    ]
    .spacing(theme::SP_SM);

    if let Some(error) = &state.editor.advanced_validation.fake_ip {
        content = content.push(validation_error_banner(error, lang));
    }
    card(None, content)
}

fn tun_form_panel<'a>(state: &'a AppState, lang: &Lang<'a>) -> Element<'a, Message> {
    let dirty = state.editor.tun_form_dirty || state.editor.tun_json_dirty;
    let status = rebuild_status_badge(
        lang,
        &state.runtime.rebuild_flow,
        "TUN",
        dirty,
        !state.editor.advanced_configs_loaded_once,
    );
    let title = lang.tr("dns_tun_sec");

    let mut content = column![
        section_header(
            title.as_ref(),
            Some(
                row![
                    status,
                    Space::new().width(theme::SP_SM),
                    header_actions(
                        Message::RefreshTunOnly,
                        Message::SaveTunConfig,
                        state.editor.is_saving_tun,
                        dirty
                    )
                ]
                .align_y(Alignment::Center)
                .into()
            )
        ),
        Space::new().height(theme::SP_MD),
        form_toggle_row(
            "enable",
            state.editor.tun_form.enable,
            Message::UpdateTunFormEnable
        ),
        row![
            text("stack")
                .size(13)
                .width(Length::Fixed(150.0))
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_primary)
                }),
            pick_list(
                &["gvisor", "system", "mixed"][..],
                if ["gvisor", "system", "mixed"].contains(&state.editor.tun_form.stack.as_str()) {
                    Some(state.editor.tun_form.stack.as_str())
                } else {
                    None
                },
                |v| Message::UpdateTunFormStack(v.to_string())
            )
            .width(Length::Fixed(180.0))
            .style(form_pick_style),
        ]
        .align_y(Alignment::Center),
        form_field_label("mtu".to_string()),
        text_input("1500", &state.editor.tun_form.mtu)
            .on_input(Message::UpdateTunFormMtu)
            .padding([8, 12])
            .size(12)
            .font(MONO)
            .style(form_input_style),
        form_field_label("dns_hijack (comma/newline separated)".to_string()),
        text_input("any:53", &state.editor.tun_form.dns_hijack)
            .on_input(Message::UpdateTunFormDnsHijack)
            .padding([8, 12])
            .size(12)
            .style(form_input_style),
        form_toggle_row(
            "auto_route",
            state.editor.tun_form.auto_route,
            Message::UpdateTunFormAutoRoute
        ),
        form_toggle_row(
            "auto_detect_interface",
            state.editor.tun_form.auto_detect_interface,
            Message::UpdateTunFormAutoDetectInterface
        ),
        form_toggle_row(
            "strict_route",
            state.editor.tun_form.strict_route,
            Message::UpdateTunFormStrictRoute
        ),
    ]
    .spacing(theme::SP_SM);

    if let Some(error) = &state.editor.advanced_validation.tun {
        content = content.push(validation_error_banner(error, lang));
    }
    card(None, content)
}

#[allow(clippy::too_many_arguments)]
fn advanced_json_panel<'a>(
    title: &str,
    dirty: bool,
    is_saving: bool,
    rebuild_label: &str,
    loaded_once: bool,
    editor_state: EditorLazyState,
    content: &'a text_editor::Content,
    on_action: impl Fn(text_editor::Action) -> Message + 'static,
    on_ensure_loaded: Message,
    on_refresh: Message,
    on_save: Message,
    validation_error: Option<&'a String>,
    extra_action: Option<Element<'a, Message>>,
    flow_state: &RebuildFlowState,
    lang: &Lang<'a>,
) -> Element<'a, Message> {
    if editor_state == EditorLazyState::Unloaded {
        return lazy_editor_placeholder(format!("{} Raw JSON", rebuild_label), on_ensure_loaded);
    }
    let status = rebuild_status_badge(lang, flow_state, rebuild_label, dirty, !loaded_once);
    let mut header_items = row![status, Space::new().width(theme::SP_SM)];
    if let Some(extra) = extra_action {
        header_items = header_items
            .push(extra)
            .push(Space::new().width(theme::SP_SM));
    }
    header_items = header_items.push(header_actions(on_refresh, on_save, is_saving, dirty));

    let mut panel_content = column![
        section_header(title, Some(header_items.align_y(Alignment::Center).into())),
        Space::new().height(theme::SP_SM),
        container(
            text_editor(content)
                .on_action(on_action)
                .font(MONO)
                .padding(10)
                .height(Length::Fixed(520.0))
        )
        .width(Length::Fill)
        .style(editor_frame_surface),
    ]
    .spacing(theme::SP_SM);

    if let Some(error) = validation_error {
        panel_content = panel_content.push(validation_error_banner(error, lang));
    }
    card(None, panel_content)
}

fn dns_json_panel<'a>(state: &'a AppState, lang: &Lang<'a>) -> Element<'a, Message> {
    let dirty = state.editor.dns_json_dirty || state.editor.dns_form_dirty;
    let title = lang.tr("dns_raw_dns_json");
    advanced_json_panel(
        title.as_ref(),
        dirty,
        state.editor.is_saving_dns,
        "DNS",
        state.editor.advanced_configs_loaded_once,
        state.editor.dns_editor_state,
        &state.editor.dns_json_content,
        Message::DnsConfigEditorAction,
        Message::EnsureDnsEditorLoaded,
        Message::RefreshDnsOnly,
        Message::SaveDns,
        state.editor.advanced_validation.dns.as_ref(),
        None,
        &state.runtime.rebuild_flow,
        lang,
    )
}

fn fake_ip_json_panel<'a>(state: &'a AppState, lang: &Lang<'a>) -> Element<'a, Message> {
    let dirty = state.editor.fake_ip_json_dirty || state.editor.fake_ip_form_dirty;
    let title = lang.tr("dns_raw_fakeip_json");
    let flush_label = lang.tr("dns_flush_fakeip").to_string();
    let flush_btn = text_btn(flush_label, style_ghost, Some(Message::FlushFakeIpCache));
    advanced_json_panel(
        title.as_ref(),
        dirty,
        state.editor.is_saving_fake_ip,
        "Fake-IP",
        state.editor.advanced_configs_loaded_once,
        state.editor.fake_ip_editor_state,
        &state.editor.fake_ip_json_content,
        Message::FakeIpConfigEditorAction,
        Message::EnsureFakeIpEditorLoaded,
        Message::RefreshFakeIpOnly,
        Message::SaveFakeIpConfig,
        state.editor.advanced_validation.fake_ip.as_ref(),
        Some(flush_btn),
        &state.runtime.rebuild_flow,
        lang,
    )
}

fn tun_json_panel<'a>(state: &'a AppState, lang: &Lang<'a>) -> Element<'a, Message> {
    let dirty = state.editor.tun_json_dirty || state.editor.tun_form_dirty;
    let title = lang.tr("dns_raw_tun_json");
    advanced_json_panel(
        title.as_ref(),
        dirty,
        state.editor.is_saving_tun,
        "TUN",
        state.editor.advanced_configs_loaded_once,
        state.editor.tun_editor_state,
        &state.editor.tun_json_content,
        Message::TunConfigEditorAction,
        Message::EnsureTunEditorLoaded,
        Message::RefreshTunOnly,
        Message::SaveTunConfig,
        state.editor.advanced_validation.tun.as_ref(),
        None,
        &state.runtime.rebuild_flow,
        lang,
    )
}

fn dns_leak_panel<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    crate::view::dns_leak_panel::leak_panel(state, lang)
}

fn stun_probe_panel<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    crate::view::stun_probe_panel::stun_panel(state, lang)
}

pub fn view(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);
    let header = row![
        text(lang.tr("dns_title").to_string())
            .size(24)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_primary)
            })
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
        |idx| {
            Message::SetDnsTab(match idx {
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
                                color: Some(tokens(t).text_primary)
                            }),
                        text("Heavy editors are mounted lazily after first paint.")
                            .size(12)
                            .style(|t: &Theme| text::Style {
                                color: Some(tokens(t).text_secondary)
                            }),
                    ]
                    .spacing(theme::SP_SM)
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
                dns_form_panel(state, &lang)
            } else {
                dns_json_panel(state, &lang)
            };
            column![
                mode_tabs,
                Space::new().height(10),
                body,
                Space::new().height(10),
                card(
                    None,
                    latency_policy_line(
                        &state.editor.dns_latency,
                        Some(Message::RunDnsLatencyProbe),
                        state.editor.is_probing_dns_latency,
                        &lang,
                    )
                ),
                Space::new().height(10),
                self_heal_panel(&state.editor.dns_self_heal, &lang),
                Space::new().height(10),
                hosts_panel(state, &lang)
            ]
            .spacing(0)
        }
        DnsTab::FakeIp => {
            let mode_tabs = mode_tabs(DnsTab::FakeIp, state.editor.fake_ip_mode);
            let body = if state.editor.fake_ip_mode == AdvancedEditMode::Form {
                fake_ip_form_panel(state, &lang)
            } else {
                fake_ip_json_panel(state, &lang)
            };
            column![
                mode_tabs,
                Space::new().height(10),
                body,
                Space::new().height(10),
                fake_ip_pool_panel(state, &lang)
            ]
            .spacing(0)
        }
        DnsTab::Tun => {
            let mode_tabs = mode_tabs(DnsTab::Tun, state.editor.tun_mode);
            let stack_card = crate::view::tun_stack_card::tun_stack_card(state, &lang);
            let body = if state.editor.tun_mode == AdvancedEditMode::Form {
                tun_form_panel(state, &lang)
            } else {
                tun_json_panel(state, &lang)
            };
            column![
                stack_card,
                Space::new().height(10),
                mode_tabs,
                Space::new().height(10),
                body
            ]
            .spacing(0)
        }
    };

    modern_scrollable(
        column![
            header,
            Space::new().height(theme::SP_MD),
            dns_leak_panel(state, &lang),
            Space::new().height(theme::SP_SM),
            stun_probe_panel(state, &lang),
            Space::new().height(theme::SP_SM),
            tabs,
            Space::new().height(theme::SP_MD),
            section
        ]
        .spacing(10),
    )
    .height(Length::Fill)
    .into()
}

#[cfg(test)]
#[path = "../../tests/gui/view_dns_tests.rs"]
mod tests;
