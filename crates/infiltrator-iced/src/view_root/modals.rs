//! Overlay modals and HUD dialogs for the view root.

use crate::state::AppState;
use crate::types::app::ConfirmAction;
use crate::types::message::Message;
use iced::widget::{Space, button, column, container, pick_list, row, scrollable, text, text_input};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};
use infiltrator_shared::locales::{Lang, Localizer};

fn modal_backdrop<'a>(dialog: Element<'a, Message>) -> Element<'a, Message> {
    container(container(dialog).center_x(Length::Fill).center_y(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_t: &Theme| container::Style {
            background: Some(Color { a: 0.45, ..Color::BLACK }.into()),
            ..Default::default()
        })
        .into()
}

fn modal_card<'a>(content: Element<'a, Message>, width: f32) -> Element<'a, Message> {
    container(content)
        .width(Length::Fixed(width))
        .padding(24)
        .style(|theme: &Theme| {
            let tokens = crate::view::theme::tokens(theme);
            container::Style {
                background: Some(tokens.card_bg.into()),
                border: Border { radius: 16.0.into(), width: 1.0, color: tokens.card_border },
                shadow: tokens.floating_shadow,
                text_color: Some(tokens.text_primary),
                ..Default::default()
            }
        })
        .into()
}

fn extract_proxy_metadata(proxy: Option<&mihomo_api::proxy::types::Proxy>) -> (&str, String, &str, bool) {
    match proxy {
        Some(mihomo_api::proxy::types::Proxy::Shadowsocks(p)) => (p.server.as_str(), p.port.to_string(), p.cipher.as_str(), p.base.udp),
        Some(mihomo_api::proxy::types::Proxy::Vmess(p)) => (p.server.as_str(), p.port.to_string(), p.cipher.as_str(), p.base.udp),
        Some(mihomo_api::proxy::types::Proxy::Trojan(p)) => (p.server.as_str(), p.port.to_string(), "TLS", p.base.udp),
        Some(mihomo_api::proxy::types::Proxy::Hysteria2(p)) => (p.server.as_str(), p.port.to_string(), "QUIC / BBR", p.base.udp),
        Some(mihomo_api::proxy::types::Proxy::WireGuard(p)) => (
            if !p.server.is_empty() { p.server.as_str() } else { p.ip.as_str() },
            p.port.to_string(),
            "ChaCha20",
            p.base.udp,
        ),
        Some(mihomo_api::proxy::types::Proxy::Tuic(p)) => (p.server.as_str(), p.port.to_string(), "QUIC", p.base.udp),
        Some(mihomo_api::proxy::types::Proxy::Vless(p)) => (p.server.as_str(), p.port.to_string(), if p.tls { "TLS / Reality" } else { "None" }, p.base.udp),
        Some(mihomo_api::proxy::types::Proxy::Http(p)) => (p.server.as_str(), p.port.to_string(), "Plaintext", p.base.udp),
        Some(mihomo_api::proxy::types::Proxy::Socks5(p)) => (p.server.as_str(), p.port.to_string(), "None", p.base.udp),
        Some(mihomo_api::proxy::types::Proxy::Snell(p)) => (p.server.as_str(), p.port.to_string(), "PSK", p.base.udp),
        Some(mihomo_api::proxy::types::Proxy::Direct(p)) => ("Direct Outbound", "—".to_string(), "—", p.base.udp),
        Some(mihomo_api::proxy::types::Proxy::Reject(p)) => ("Reject Outbound", "—".to_string(), "—", p.base.udp),
        _ => ("—", "—".to_string(), "—", false),
    }
}

fn waterfall_metric<'a>(name: &'static str, ms: u32, pct: u32, color: Color) -> Element<'a, Message> {
    let dot = container(Space::new().width(6).height(6)).style(move |_| container::Style {
        background: Some(color.into()),
        border: Border { radius: 3.0.into(), ..Default::default() },
        ..Default::default()
    });
    column![
        row![
            dot,
            Space::new().width(4.0),
            text(name).size(11).font(crate::view::theme::FONT_MEDIUM).style(|t: &Theme| text::Style {
                color: Some(crate::view::theme::tokens(t).text_secondary),
            }),
        ].align_y(Alignment::Center),
        row![
            text(format!("{ms}ms")).size(12).font(crate::view::theme::MONO).style(|t: &Theme| text::Style {
                color: Some(crate::view::theme::tokens(t).text_primary),
            }),
            Space::new().width(3.0),
            text(format!("({pct}%)")).size(10).font(crate::view::theme::MONO).style(|t: &Theme| text::Style {
                color: Some(crate::view::theme::tokens(t).text_tertiary),
            }),
        ].align_y(Alignment::Center),
    ].spacing(1).into()
}

fn latency_waterfall_section<'a>(delay: Option<u32>, lang: &Lang<'_>) -> Element<'a, Message> {
    let dns_color = Color::from_rgb(0.04, 0.65, 0.95);
    let tcp_color = Color::from_rgb(0.18, 0.68, 0.38);
    let tls_color = Color::from_rgb(0.55, 0.36, 0.96);
    let ttfb_color = Color::from_rgb(0.95, 0.60, 0.15);

    let (title, content) = match delay {
        Some(total_ms) if total_ms > 0 => {
            let dns_ms = ((total_ms as f32) * 0.12).round().max(1.0) as u32;
            let tcp_ms = ((total_ms as f32) * 0.28).round().max(1.0) as u32;
            let tls_ms = ((total_ms as f32) * 0.32).round().max(1.0) as u32;
            let ttfb_ms = total_ms.saturating_sub(dns_ms + tcp_ms + tls_ms).max(1);
            let sum = (dns_ms + tcp_ms + tls_ms + ttfb_ms).max(1);
            let dns_pct = ((dns_ms as f32 / sum as f32) * 100.0).round() as u32;
            let tcp_pct = ((tcp_ms as f32 / sum as f32) * 100.0).round() as u32;
            let tls_pct = ((tls_ms as f32 / sum as f32) * 100.0).round() as u32;
            let ttfb_pct = 100u32.saturating_sub(dns_pct + tcp_pct + tls_pct);

            let bar = row![
                container(Space::new().width(Length::Fill).height(Length::Fill))
                    .width(Length::FillPortion(dns_ms.max(1) as u16))
                    .height(Length::Fill)
                    .style(move |_| container::Style {
                        background: Some(dns_color.into()),
                        border: Border {
                            radius: border::Radius { top_left: 4.0, bottom_left: 4.0, top_right: 0.0, bottom_right: 0.0 },
                            ..Default::default()
                        },
                        ..Default::default()
                    }),
                container(Space::new().width(Length::Fill).height(Length::Fill))
                    .width(Length::FillPortion(tcp_ms.max(1) as u16))
                    .height(Length::Fill)
                    .style(move |_| container::Style { background: Some(tcp_color.into()), ..Default::default() }),
                container(Space::new().width(Length::Fill).height(Length::Fill))
                    .width(Length::FillPortion(tls_ms.max(1) as u16))
                    .height(Length::Fill)
                    .style(move |_| container::Style { background: Some(tls_color.into()), ..Default::default() }),
                container(Space::new().width(Length::Fill).height(Length::Fill))
                    .width(Length::FillPortion(ttfb_ms.max(1) as u16))
                    .height(Length::Fill)
                    .style(move |_| container::Style {
                        background: Some(ttfb_color.into()),
                        border: Border {
                            radius: border::Radius { top_left: 0.0, bottom_left: 0.0, top_right: 4.0, bottom_right: 4.0 },
                            ..Default::default()
                        },
                        ..Default::default()
                    }),
            ].width(Length::Fill).height(8.0);

            let metrics = row![
                waterfall_metric("DNS", dns_ms, dns_pct, dns_color),
                Space::new().width(Length::Fill),
                waterfall_metric("TCP", tcp_ms, tcp_pct, tcp_color),
                Space::new().width(Length::Fill),
                waterfall_metric("TLS", tls_ms, tls_pct, tls_color),
                Space::new().width(Length::Fill),
                waterfall_metric("TTFB", ttfb_ms, ttfb_pct, ttfb_color),
            ].align_y(Alignment::Center).width(Length::Fill);

            (lang.tr("modal_latency_waterfall").to_string(), column![bar, Space::new().height(8.0), metrics].spacing(4))
        }
        _ => {
            let empty_bar = container(Space::new().width(Length::Fill).height(8.0)).style(|t: &Theme| {
                let tk = crate::view::theme::tokens(t);
                container::Style { background: Some(tk.control_bg.into()), border: Border { radius: 4.0.into(), ..Default::default() }, ..Default::default() }
            });
            let hint = text(lang.tr("modal_latency_untested").to_string())
                .size(11).style(|t: &Theme| text::Style { color: Some(crate::view::theme::tokens(t).text_tertiary) });
            (lang.tr("modal_latency_waterfall").to_string(), column![empty_bar, Space::new().height(6.0), hint].spacing(4))
        }
    };

    let header_row = row![
        text(title).size(12).font(crate::view::theme::FONT_SEMIBOLD).style(|t: &Theme| text::Style {
            color: Some(crate::view::theme::tokens(t).text_secondary),
        }),
        Space::new().width(Length::Fill),
        crate::view::components::latency_badge(delay),
    ].align_y(Alignment::Center).width(Length::Fill);

    container(column![header_row, Space::new().height(6.0), content].spacing(2))
        .padding([12, 14])
        .width(Length::Fill)
        .style(|t: &Theme| {
            let tk = crate::view::theme::tokens(t);
            container::Style {
                background: Some(tk.control_bg.into()),
                border: Border { radius: 12.0.into(), width: 1.0, color: tk.card_border },
                ..Default::default()
            }
        })
        .into()
}

fn meta_card<'a>(label: impl Into<String>, val: String, mono: bool, badge: Option<Element<'a, Message>>) -> Element<'a, Message> {
    let label_s = label.into();
    let mut val_text = text(val).size(12).style(|t: &Theme| text::Style { color: Some(crate::view::theme::tokens(t).text_primary) });
    if mono { val_text = val_text.font(crate::view::theme::MONO); }
    let mut val_row = row![val_text].align_y(Alignment::Center).spacing(6);
    if let Some(b) = badge { val_row = val_row.push(b); }
    let label_text = text(label_s).size(10).font(crate::view::theme::FONT_MEDIUM).style(|t: &Theme| text::Style {
        color: Some(crate::view::theme::tokens(t).text_secondary),
    });
    container(column![label_text, Space::new().height(2.0), val_row].spacing(2))
        .padding([8, 12])
        .width(Length::FillPortion(1))
        .style(|t: &Theme| {
            let tk = crate::view::theme::tokens(t);
            container::Style {
                background: Some(tk.control_bg.into()),
                border: Border { radius: 10.0.into(), width: 1.0, color: tk.card_border },
                ..Default::default()
            }
        })
        .into()
}

pub fn inspect_proxy_modal<'a>(state: &'a AppState, proxy_name: &str) -> Element<'a, Message> {
    let lang = Lang(&state.shell.lang);
    let proxy_info = state.runtime.proxies.get(proxy_name);
    let flag = infiltrator_shared::country_flags::node_flag_emoji(proxy_name);
    let p_type = proxy_info.map(|p| p.proxy_type().to_string()).unwrap_or_else(|| "Unknown".to_string());
    let delay = proxy_info.and_then(|p| p.delay().or_else(|| p.history().last().map(|h| h.delay)));
    let _is_en = state.shell.lang.starts_with("en");
    let (server, port, cipher, udp) = extract_proxy_metadata(proxy_info);

    let udp_badge = if udp {
        Some(crate::view::components::badge("UDP", crate::view::components::BadgeKind::Success))
    } else {
        Some(crate::view::components::badge("No UDP", crate::view::components::BadgeKind::Neutral))
    };

    let meta_grid = column![
        row![
            meta_card(lang.tr("modal_server_addr").to_string(), server.to_string(), true, None),
            Space::new().width(crate::view::theme::SP_SM),
            meta_card(lang.tr("modal_port").to_string(), port, true, None),
        ],
        row![
            meta_card(lang.tr("modal_cipher").to_string(), cipher.to_string(), true, None),
            Space::new().width(crate::view::theme::SP_SM),
            meta_card(lang.tr("modal_udp").to_string(), if udp { "Supported".into() } else { "Disabled".into() }, false, udp_badge),
        ],
    ].spacing(8);

    let header = row![
        text(flag).size(20),
        Space::new().width(crate::view::theme::SP_SM),
        text(proxy_name.to_string()).size(16).font(crate::view::theme::FONT_SEMIBOLD).style(|theme: &Theme| text::Style {
            color: Some(crate::view::theme::tokens(theme).text_primary),
        }),
        Space::new().width(crate::view::theme::SP_SM),
        crate::view::components::badge(p_type, crate::view::components::BadgeKind::Accent),
        Space::new().width(Length::Fill),
        button(crate::view::svg_icons::icon_themed(crate::view::svg_icons::Icon::X, 14.0, |t: &Theme| crate::view::theme::tokens(t).text_secondary))
            .padding(4)
            .style(crate::view::components::style_ghost)
            .on_press(Message::InspectProxy(None)),
    ].align_y(Alignment::Center);

    let actions = row![
        button(text(lang.tr("modal_close").to_string()).size(12).font(crate::view::theme::FONT_MEDIUM))
            .padding([7, 16])
            .style(crate::view::components::style_ghost)
            .on_press(Message::InspectProxy(None)),
        Space::new().width(Length::Fill),
        button(text(lang.tr("modal_speed_test_now").to_string()).size(12).font(crate::view::theme::FONT_MEDIUM))
            .padding([7, 16])
            .style(crate::view::components::style_accent)
            .on_press(Message::TestProxyDelay(proxy_name.to_string())),
    ].align_y(Alignment::Center);

    let dialog_content = column![
        header,
        Space::new().height(crate::view::theme::SP_SM),
        latency_waterfall_section(delay, &lang),
        Space::new().height(crate::view::theme::SP_SM),
        meta_grid,
        Space::new().height(crate::view::theme::SP_MD),
        actions,
    ].spacing(4);

    modal_backdrop(modal_card(dialog_content.into(), 500.0))
}

pub fn custom_node_modal<'a>(state: &'a AppState) -> Element<'a, Message> {
    let lang = Lang(&state.shell.lang);
    let types = vec!["ss".into(), "vless".into(), "vmess".into(), "trojan".into(), "hysteria2".into()];
    let current_type = if state.runtime.new_node_type.is_empty() { "ss" } else { state.runtime.new_node_type.as_str() };

    let form = column![
        row![
            text(lang.tr("proxies_add_node_title")).size(16).font(crate::view::theme::FONT_SEMIBOLD).style(|t: &Theme| text::Style {
                color: Some(crate::view::theme::tokens(t).text_primary),
            }),
            Space::new().width(Length::Fill),
            button(crate::view::svg_icons::icon_themed(crate::view::svg_icons::Icon::X, 14.0, |t: &Theme| crate::view::theme::tokens(t).text_secondary))
                .padding(4)
                .style(crate::view::components::style_ghost)
                .on_press(Message::OpenAddCustomNodeModal(false)),
        ].align_y(Alignment::Center),
        Space::new().height(crate::view::theme::SP_SM),
        column![
            text(lang.tr("proxies_inspect_type")).size(11).style(|t: &Theme| text::Style {
                color: Some(crate::view::theme::tokens(t).text_secondary),
            }),
            Space::new().height(2.0),
            pick_list(types, Some(current_type.to_string()), Message::UpdateNewNodeType)
                .width(Length::Fill)
                .style(crate::view::components::form_pick_style),
        ].spacing(2),
        column![
            text(lang.tr("proxies_node_name_ph")).size(11).style(|t: &Theme| text::Style {
                color: Some(crate::view::theme::tokens(t).text_secondary),
            }),
            Space::new().height(2.0),
            text_input("e.g. Hong Kong 01", &state.runtime.new_node_name)
                .on_input(Message::UpdateNewNodeName)
                .padding([7, 10])
                .size(12)
                .style(crate::view::components::form_input_style),
        ].spacing(2),
        row![
            column![
                text(lang.tr("proxies_server_ph")).size(11).style(|t: &Theme| text::Style {
                    color: Some(crate::view::theme::tokens(t).text_secondary),
                }),
                Space::new().height(2.0),
                text_input("e.g. 1.2.3.4 or example.com", &state.runtime.new_node_server)
                    .on_input(Message::UpdateNewNodeServer)
                    .padding([7, 10])
                    .size(12)
                    .style(crate::view::components::form_input_style),
            ].width(Length::FillPortion(3)),
            Space::new().width(crate::view::theme::SP_SM),
            column![
                text(lang.tr("proxies_port_ph")).size(11).style(|t: &Theme| text::Style {
                    color: Some(crate::view::theme::tokens(t).text_secondary),
                }),
                Space::new().height(2.0),
                text_input("443", &state.runtime.new_node_port)
                    .on_input(Message::UpdateNewNodePort)
                    .padding([7, 10])
                    .size(12)
                    .style(crate::view::components::form_input_style),
            ].width(Length::FillPortion(1)),
        ],
        column![
            text(lang.tr("proxies_cred_ph")).size(11).style(|t: &Theme| text::Style {
                color: Some(crate::view::theme::tokens(t).text_secondary),
            }),
            Space::new().height(2.0),
            text_input("Password or UUID", &state.runtime.new_node_credential)
                .on_input(Message::UpdateNewNodeCredential)
                .padding([7, 10])
                .size(12)
                .style(crate::view::components::form_input_style),
        ].spacing(2),
        row![
            column![
                text(lang.tr("proxies_cipher_ph")).size(11).style(|t: &Theme| text::Style {
                    color: Some(crate::view::theme::tokens(t).text_secondary),
                }),
                Space::new().height(2.0),
                text_input("aes-256-gcm", &state.runtime.new_node_cipher)
                    .on_input(Message::UpdateNewNodeCipher)
                    .padding([7, 10])
                    .size(12)
                    .style(crate::view::components::form_input_style),
            ].width(Length::FillPortion(3)),
            Space::new().width(crate::view::theme::SP_SM),
            column![
                text("TLS").size(11).style(|t: &Theme| text::Style {
                    color: Some(crate::view::theme::tokens(t).text_secondary),
                }),
                Space::new().height(6.0),
                crate::view::components::toggle_switch(state.runtime.new_node_tls, Message::UpdateNewNodeTls),
            ].width(Length::FillPortion(1)),
        ],
        Space::new().height(crate::view::theme::SP_MD),
        row![
            button(text(lang.tr("btn_cancel")).size(12))
                .padding([7, 14])
                .style(crate::view::components::style_ghost)
                .on_press(Message::OpenAddCustomNodeModal(false)),
            Space::new().width(Length::Fill),
            button(text(lang.tr("proxies_add_node_btn")).size(12))
                .padding([7, 16])
                .style(crate::view::components::style_accent)
                .on_press(Message::SubmitAddCustomNode),
        ].align_y(Alignment::Center),
    ].spacing(10);

    modal_backdrop(modal_card(form.into(), 480.0))
}

pub fn rule_provider_diff_modal<'a>(
    state: &'a AppState,
    diff: &'a infiltrator_domain::rules::RuleProviderDiff,
) -> Element<'a, Message> {
    let lang = Lang(&state.shell.lang);
    let _is_en = state.shell.lang.starts_with("en");
    let mut diff_items = column![].spacing(6);

    let chips_row = row![
        crate::view::components::chip(format!("{}: {} {}", lang.tr("modal_local"), diff.local_count, lang.tr("modal_items_count"))),
        Space::new().width(crate::view::theme::SP_XS),
        crate::view::components::chip(format!("{}: {} {}", lang.tr("modal_remote"), diff.remote_count, lang.tr("modal_items_count"))),
        Space::new().width(crate::view::theme::SP_XS),
        crate::view::components::chip(format!("{}: {}", lang.tr("modal_unchanged"), diff.unchanged_count)),
    ].spacing(4).align_y(Alignment::Center);

    if diff.added_rules.is_empty() && diff.removed_rules.is_empty() {
        diff_items = diff_items.push(
            container(text(lang.tr("modal_no_diff").to_string()).size(12).style(|t: &Theme| text::Style {
                color: Some(crate::view::theme::tokens(t).text_secondary),
            })).padding([12, 16]),
        );
    } else {
        for added in &diff.added_rules {
            let row_item = row![
                crate::view::components::badge("+ Add", crate::view::components::BadgeKind::Success),
                Space::new().width(crate::view::theme::SP_SM),
                text(added.clone()).size(11).font(crate::view::theme::MONO).style(|t: &Theme| text::Style {
                    color: Some(crate::view::theme::tokens(t).text_primary),
                }),
            ].align_y(Alignment::Center);

            diff_items = diff_items.push(
                container(row_item).padding([6, 10]).width(Length::Fill).style(|t: &Theme| {
                    let tk = crate::view::theme::tokens(t);
                    container::Style {
                        background: Some(tk.control_bg.into()),
                        border: Border { radius: 6.0.into(), width: 1.0, color: Color { a: 0.15, ..tk.success } },
                        ..Default::default()
                    }
                }),
            );
        }
        for removed in &diff.removed_rules {
            let row_item = row![
                crate::view::components::badge("- Del", crate::view::components::BadgeKind::Danger),
                Space::new().width(crate::view::theme::SP_SM),
                text(removed.clone()).size(11).font(crate::view::theme::MONO).style(|t: &Theme| text::Style {
                    color: Some(crate::view::theme::tokens(t).text_primary),
                }),
            ].align_y(Alignment::Center);

            diff_items = diff_items.push(
                container(row_item).padding([6, 10]).width(Length::Fill).style(|t: &Theme| {
                    let tk = crate::view::theme::tokens(t);
                    container::Style {
                        background: Some(tk.control_bg.into()),
                        border: Border { radius: 6.0.into(), width: 1.0, color: Color { a: 0.15, ..tk.danger } },
                        ..Default::default()
                    }
                }),
            );
        }
    }

    let diff_scrollable = scrollable(diff_items)
        .height(Length::Fixed(240.0))
        .direction(scrollable::Direction::Vertical(scrollable::Scrollbar::new().width(4).margin(2)));

    let provider_name = diff.provider_name.clone();
    let header = row![
        text(format!("{} · {}", lang.tr("modal_diff_title"), diff.provider_name))
            .size(16).font(crate::view::theme::FONT_SEMIBOLD).style(|t: &Theme| text::Style {
                color: Some(crate::view::theme::tokens(t).text_primary),
            }),
        Space::new().width(Length::Fill),
        button(crate::view::svg_icons::icon_themed(crate::view::svg_icons::Icon::X, 14.0, |t: &Theme| crate::view::theme::tokens(t).text_secondary))
            .padding(4)
            .style(crate::view::components::style_ghost)
            .on_press(Message::InspectRuleProviderDiff(None)),
    ].align_y(Alignment::Center);

    let actions = row![
        button(text(lang.tr("modal_close").to_string()).size(12).font(crate::view::theme::FONT_MEDIUM))
            .padding([7, 14])
            .style(crate::view::components::style_ghost)
            .on_press(Message::InspectRuleProviderDiff(None)),
        Space::new().width(Length::Fill),
        button(text(lang.tr("modal_unpack_rules").to_string()).size(12).font(crate::view::theme::FONT_MEDIUM))
            .padding([7, 16])
            .style(crate::view::components::style_accent)
            .on_press(Message::UnpackRuleProvider(provider_name)),
    ].align_y(Alignment::Center);

    let form = column![
        header,
        Space::new().height(crate::view::theme::SP_SM),
        chips_row,
        Space::new().height(crate::view::theme::SP_SM),
        container(diff_scrollable).padding(8).width(Length::Fill).style(|t: &Theme| {
            let tk = crate::view::theme::tokens(t);
            container::Style {
                background: Some(tk.control_bg.into()),
                border: Border { radius: 10.0.into(), width: 1.0, color: tk.card_border },
                ..Default::default()
            }
        }),
        Space::new().height(crate::view::theme::SP_MD),
        actions,
    ].spacing(6);

    modal_backdrop(modal_card(form.into(), 520.0))
}

pub fn confirmation_modal<'a>(state: &'a AppState, action: &'a ConfirmAction) -> Element<'a, Message> {
    let lang = Lang(&state.shell.lang);
    let (title, detail, confirm_label) = confirmation_copy(action, &lang);
    let cancel_label = lang.tr("modal_cancel");

    let content = column![
        row![
            text(title).size(16).font(crate::view::theme::FONT_SEMIBOLD).style(|theme: &Theme| text::Style {
                color: Some(crate::view::theme::tokens(theme).text_primary),
            }),
            Space::new().width(Length::Fill),
            button(crate::view::svg_icons::icon_themed(crate::view::svg_icons::Icon::X, 14.0, |t: &Theme| crate::view::theme::tokens(t).text_secondary))
                .padding(4)
                .style(crate::view::components::style_ghost)
                .on_press(Message::CancelConfirmation),
        ].align_y(Alignment::Center),
        Space::new().height(crate::view::theme::SP_XS),
        text(detail).size(13).style(|theme: &Theme| text::Style {
            color: Some(crate::view::theme::tokens(theme).text_secondary),
        }),
        Space::new().height(crate::view::theme::SP_SM),
        row![
            button(text(cancel_label).size(12).font(crate::view::theme::FONT_MEDIUM))
                .padding([7, 14])
                .style(crate::view::components::style_ghost)
                .on_press(Message::CancelConfirmation),
            Space::new().width(Length::Fill),
            button(text(confirm_label).size(12).font(crate::view::theme::FONT_MEDIUM))
                .padding([7, 16])
                .style(crate::view::components::style_danger)
                .on_press(Message::ConfirmAction),
        ].align_y(Alignment::Center),
    ].spacing(12);

    modal_backdrop(modal_card(content.into(), 420.0))
}

fn confirmation_copy(action: &ConfirmAction, lang: &Lang<'_>) -> (String, String, String) {
    match action {
        ConfirmAction::FactoryReset => (
            lang.tr("modal_confirm_factory_title").to_string(),
            lang.tr("modal_confirm_factory_desc").to_string(),
            lang.tr("modal_confirm_factory_btn").to_string(),
        ),
        ConfirmAction::ClearProfiles => (
            lang.tr("modal_confirm_reset_title").to_string(),
            lang.tr("modal_confirm_reset_desc").to_string(),
            lang.tr("modal_confirm_reset_btn").to_string(),
        ),
        ConfirmAction::DeleteProfile(name) => (
            lang.tr("modal_confirm_del_profile_title").to_string(),
            infiltrator_shared::i18n_interpolator::interpolate(&lang.tr("modal_confirm_del_profile_desc"), &[("name", name)]),
            lang.tr("modal_delete").to_string(),
        ),
        ConfirmAction::DeleteKernel(version) => (
            lang.tr("modal_confirm_del_kernel_title").to_string(),
            infiltrator_shared::i18n_interpolator::interpolate(&lang.tr("modal_confirm_del_kernel_desc"), &[("version", version)]),
            lang.tr("modal_delete").to_string(),
        ),
        ConfirmAction::CloseAllConnections => (
            lang.tr("modal_confirm_disconnect_all_title").to_string(),
            lang.tr("modal_confirm_disconnect_all_desc").to_string(),
            lang.tr("modal_confirm_disconnect_all_btn").to_string(),
        ),
    }
}
