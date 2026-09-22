//! DUAL-05-03…05-12: the typed parameter blocks of the custom-node modal.
//!
//! One section per family, every field reading and writing the shared
//! [`ProtocolDraft`] from `infiltrator-contract::protocol_params`. The modal
//! calls [`params_section`]; nothing here keeps its own copy of a protocol
//! fact, and fields the pinned mihomo v1.19.18 ignores are rendered next to
//! the honest notes the shared report computed.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::{form_input_style, toggle_switch};
use crate::view::theme::{self, FONT_SEMIBOLD, MONO, tokens};
use iced::widget::{Space, column, container, row, text, text_input};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_contract::protocol_fidelity::{ProtocolDraft, ProtocolFamily};
use infiltrator_contract::protocol_params::EchParams;
use infiltrator_contract::protocol_params_ext::TransportParams;
use infiltrator_shared::locales::{Lang, Localizer};

/// Apply one edit to a clone of the shared draft and publish it back.
pub(super) fn edit(draft: &ProtocolDraft, apply: impl FnOnce(&mut ProtocolDraft)) -> Message {
    let mut next = draft.clone();
    apply(&mut next);
    Message::UpdateCustomNodeDraft(Box::new(next))
}

pub(super) fn label_text<'a>(label: String) -> Element<'a, Message> {
    text(label)
        .size(11)
        .font(FONT_SEMIBOLD)
        .style(|t: &Theme| text::Style {
            color: Some(tokens(t).text_secondary),
        })
        .into()
}

pub(super) fn field<'a>(
    label: String,
    placeholder: &str,
    value: &str,
    on_input: impl Fn(String) -> Message + 'a,
) -> Element<'a, Message> {
    column![
        label_text(label),
        Space::new().height(2.0),
        text_input(placeholder, value)
            .on_input(on_input)
            .padding([6, 10])
            .size(12)
            .font(MONO)
            .style(form_input_style),
    ]
    .width(Length::Fill)
    .into()
}

fn toggle_row<'a>(
    label: String,
    value: bool,
    on_toggle: impl Fn(bool) -> Message + 'a,
) -> Element<'a, Message> {
    column![
        label_text(label),
        Space::new().height(4.0),
        toggle_switch(value, on_toggle),
    ]
    .width(Length::FillPortion(1))
    .into()
}

pub(super) fn params_row<'a>(items: Vec<Element<'a, Message>>) -> Element<'a, Message> {
    let mut row_items: Vec<Element<'a, Message>> = Vec::new();
    for item in items {
        if !row_items.is_empty() {
            row_items.push(Space::new().width(theme::SP_SM).into());
        }
        row_items.push(item);
    }
    row(row_items).align_y(Alignment::End).into()
}

/// DUAL-05-12: ALPN order + ECH, gated by the families that carry them.
fn tls_params<'a>(draft: &'a ProtocolDraft, lang: &Lang<'_>) -> Vec<Element<'a, Message>> {
    let mut items = Vec::new();
    let alpn = draft.alpn.join(",");
    items.push(field(
        lang.tr("custom_node_alpn").to_string(),
        "h2,http/1.1",
        &alpn,
        move |value| {
            let entries = value
                .split(',')
                .map(|entry| entry.trim().to_string())
                .filter(|entry| !entry.is_empty())
                .collect();
            edit(draft, move |next| next.alpn = entries)
        },
    ));
    if EchParams::supported_by(draft.family()) {
        let config = draft.params.ech.config.as_str();
        let ech_draft = draft;
        items.push(field(
            lang.tr("custom_node_ech_config").to_string(),
            "base64 ECHConfigList",
            config,
            move |value| {
                edit(ech_draft, move |next| {
                    next.params.ech.config = value;
                })
            },
        ));
    }
    vec![params_row(items)]
}

/// DUAL-05-03: TUIC congestion / relay parameters.
fn tuic_params<'a>(draft: &'a ProtocolDraft, lang: &Lang<'_>) -> Vec<Element<'a, Message>> {
    let heartbeat = draft.params.tuic.heartbeat_interval.to_string();
    let request_timeout = draft.params.tuic.request_timeout.to_string();
    let recv = draft.params.tuic.recv_window.to_string();
    vec![
        params_row(vec![
            field(
                lang.tr("custom_node_tuic_cc").to_string(),
                "bbr / cubic / new-reno",
                draft.params.tuic.congestion_controller.as_str(),
                move |value| {
                    edit(draft, move |next| {
                        next.params.tuic.congestion_controller = value;
                    })
                },
            ),
            field(
                lang.tr("custom_node_tuic_udp_relay").to_string(),
                "native / quic",
                draft.params.tuic.udp_relay_mode.as_str(),
                move |value| {
                    edit(draft, move |next| {
                        next.params.tuic.udp_relay_mode = value;
                    })
                },
            ),
            toggle_row(
                lang.tr("custom_node_tuic_reduce_rtt").to_string(),
                draft.params.tuic.reduce_rtt,
                move |enabled| edit(draft, move |next| next.params.tuic.reduce_rtt = enabled),
            ),
        ]),
        params_row(vec![
            field(
                lang.tr("custom_node_tuic_heartbeat").to_string(),
                "10000",
                &heartbeat,
                move |value| {
                    let parsed = value.trim().parse::<u64>().unwrap_or(0);
                    edit(draft, move |next| {
                        next.params.tuic.heartbeat_interval = parsed;
                    })
                },
            ),
            field(
                lang.tr("custom_node_tuic_request_timeout").to_string(),
                "8000",
                &request_timeout,
                move |value| {
                    let parsed = value.trim().parse::<u64>().unwrap_or(0);
                    edit(draft, move |next| {
                        next.params.tuic.request_timeout = parsed;
                    })
                },
            ),
            field(
                lang.tr("custom_node_recv_window").to_string(),
                "0",
                &recv,
                move |value| {
                    let parsed = value.trim().parse::<u64>().unwrap_or(0);
                    edit(draft, move |next| {
                        next.params.tuic.recv_window = parsed;
                        next.params.tuic.recv_window_conn =
                            next.params.tuic.recv_window_conn.min(parsed);
                    })
                },
            ),
        ]),
    ]
}

/// DUAL-05-03: Hysteria2 port hopping / obfs / bandwidth.
fn hysteria2_params<'a>(draft: &'a ProtocolDraft, lang: &Lang<'_>) -> Vec<Element<'a, Message>> {
    let hop = draft.params.hysteria2.hop_interval.to_string();
    let cwnd = draft.params.hysteria2.cwnd.to_string();
    let udp_mtu = draft.params.hysteria2.udp_mtu.to_string();
    vec![
        params_row(vec![
            field(
                lang.tr("custom_node_hy2_ports").to_string(),
                "20000-30000,8443",
                draft.params.hysteria2.ports.as_str(),
                move |value| edit(draft, move |next| next.params.hysteria2.ports = value),
            ),
            field(
                lang.tr("custom_node_hy2_hop").to_string(),
                "30",
                &hop,
                move |value| {
                    let parsed = value.trim().parse::<u64>().unwrap_or(0);
                    edit(draft, move |next| {
                        next.params.hysteria2.hop_interval = parsed
                    })
                },
            ),
            field(
                lang.tr("custom_node_hy2_cwnd").to_string(),
                "0",
                &cwnd,
                move |value| {
                    let parsed = value.trim().parse::<u64>().unwrap_or(0);
                    edit(draft, move |next| next.params.hysteria2.cwnd = parsed)
                },
            ),
            field(
                lang.tr("custom_node_hy2_udp_mtu").to_string(),
                "1197",
                &udp_mtu,
                move |value| {
                    let parsed = value.trim().parse::<u64>().unwrap_or(0);
                    edit(draft, move |next| next.params.hysteria2.udp_mtu = parsed)
                },
            ),
        ]),
        params_row(vec![
            field(
                lang.tr("custom_node_hy2_obfs").to_string(),
                "salamander",
                draft.params.hysteria2.obfs.as_str(),
                move |value| edit(draft, move |next| next.params.hysteria2.obfs = value),
            ),
            field(
                lang.tr("custom_node_hy2_obfs_password").to_string(),
                "obfs password",
                draft.params.hysteria2.obfs_password.as_str(),
                move |value| {
                    edit(draft, move |next| {
                        next.params.hysteria2.obfs_password = value
                    })
                },
            ),
            field(
                lang.tr("custom_node_hy2_up").to_string(),
                "100 Mbps",
                draft.params.hysteria2.up.as_str(),
                move |value| edit(draft, move |next| next.params.hysteria2.up = value),
            ),
            field(
                lang.tr("custom_node_hy2_down").to_string(),
                "200 Mbps",
                draft.params.hysteria2.down.as_str(),
                move |value| edit(draft, move |next| next.params.hysteria2.down = value),
            ),
        ]),
    ]
}

/// DUAL-05-04: WireGuard / AmneziaWG parameters.
fn wireguard_params<'a>(draft: &'a ProtocolDraft, lang: &Lang<'_>) -> Vec<Element<'a, Message>> {
    let mtu = draft.params.wireguard.mtu.to_string();
    let keepalive = draft.params.wireguard.persistent_keepalive.to_string();
    let workers = draft.params.wireguard.workers.to_string();
    let jc = draft.params.wireguard.amnezia.jc.map(|v| v.to_string());
    let jmin = draft.params.wireguard.amnezia.jmin.map(|v| v.to_string());
    let jmax = draft.params.wireguard.amnezia.jmax.map(|v| v.to_string());
    let allowed_ips = draft.params.wireguard.allowed_ips.join(",");
    vec![
        params_row(vec![
            field(
                lang.tr("custom_node_wg_private_key").to_string(),
                "base64 32 bytes",
                draft.params.wireguard.private_key.as_str(),
                move |value| edit(draft, move |next| next.params.wireguard.private_key = value),
            ),
            field(
                lang.tr("custom_node_wg_public_key").to_string(),
                "base64 32 bytes",
                draft.params.wireguard.public_key.as_str(),
                move |value| edit(draft, move |next| next.params.wireguard.public_key = value),
            ),
            field(
                lang.tr("custom_node_wg_preshared_key").to_string(),
                "base64 32 bytes",
                draft.params.wireguard.pre_shared_key.as_str(),
                move |value| {
                    edit(draft, move |next| {
                        next.params.wireguard.pre_shared_key = value
                    })
                },
            ),
        ]),
        params_row(vec![
            field(
                lang.tr("custom_node_wg_reserved").to_string(),
                "1,2,3 or base64",
                draft.params.wireguard.reserved.as_str(),
                move |value| {
                    let base64 = value
                        .trim()
                        .chars()
                        .any(|c| c == '=' || c == '/' || c == '+');
                    edit(draft, move |next| {
                        next.params.wireguard.reserved = value;
                        next.params.wireguard.reserved_is_base64 = base64;
                    })
                },
            ),
            field(
                lang.tr("custom_node_wg_ip").to_string(),
                "172.16.0.2/32",
                draft.params.wireguard.ip.as_str(),
                move |value| edit(draft, move |next| next.params.wireguard.ip = value),
            ),
            field(
                lang.tr("custom_node_wg_mtu").to_string(),
                "1420",
                &mtu,
                move |value| {
                    let parsed = value.trim().parse::<u32>().unwrap_or(0);
                    edit(draft, move |next| next.params.wireguard.mtu = parsed)
                },
            ),
        ]),
        params_row(vec![
            field(
                lang.tr("custom_node_wg_keepalive").to_string(),
                "25",
                &keepalive,
                move |value| {
                    let parsed = value.trim().parse::<u32>().unwrap_or(0);
                    edit(draft, move |next| {
                        next.params.wireguard.persistent_keepalive = parsed
                    })
                },
            ),
            field(
                lang.tr("custom_node_wg_workers").to_string(),
                "0",
                &workers,
                move |value| {
                    let parsed = value.trim().parse::<u32>().unwrap_or(0);
                    edit(draft, move |next| next.params.wireguard.workers = parsed)
                },
            ),
            field(
                lang.tr("custom_node_wg_allowed_ips").to_string(),
                "0.0.0.0/0",
                &allowed_ips,
                move |value| {
                    let entries = value
                        .split(',')
                        .map(|entry| entry.trim().to_string())
                        .filter(|entry| !entry.is_empty())
                        .collect();
                    edit(draft, move |next| {
                        next.params.wireguard.allowed_ips = entries
                    })
                },
            ),
        ]),
        params_row(vec![
            field(
                lang.tr("custom_node_awg_jc").to_string(),
                "4",
                jc.as_deref().unwrap_or(""),
                move |value| {
                    let parsed = value.trim().parse::<u8>().ok();
                    edit(draft, move |next| next.params.wireguard.amnezia.jc = parsed)
                },
            ),
            field(
                lang.tr("custom_node_awg_jmin").to_string(),
                "40",
                jmin.as_deref().unwrap_or(""),
                move |value| {
                    let parsed = value.trim().parse::<u16>().ok();
                    edit(draft, move |next| {
                        next.params.wireguard.amnezia.jmin = parsed
                    })
                },
            ),
            field(
                lang.tr("custom_node_awg_jmax").to_string(),
                "70",
                jmax.as_deref().unwrap_or(""),
                move |value| {
                    let parsed = value.trim().parse::<u16>().ok();
                    edit(draft, move |next| {
                        next.params.wireguard.amnezia.jmax = parsed
                    })
                },
            ),
        ]),
    ]
}

/// DUAL-05-05: transport selection plus WS 0-RTT / gRPC / XHTTP fields.
fn transport_params<'a>(draft: &'a ProtocolDraft, lang: &Lang<'_>) -> Vec<Element<'a, Message>> {
    if !TransportParams::supported_by(draft.family()) {
        return Vec::new();
    }
    let early = draft.params.transport.ws.max_early_data.to_string();
    let ws_host = draft.params.transport.ws.host();
    vec![
        params_row(vec![
            field(
                lang.tr("custom_node_transport_network").to_string(),
                "tcp / ws / grpc / h2 / http / xhttp",
                draft.params.transport.network.as_str(),
                move |value| edit(draft, move |next| next.params.transport.network = value),
            ),
            field(
                lang.tr("custom_node_ws_path").to_string(),
                "/ws",
                draft.params.transport.ws.path.as_str(),
                move |value| edit(draft, move |next| next.params.transport.ws.path = value),
            ),
            field(
                lang.tr("custom_node_ws_host").to_string(),
                "cdn.example.com",
                &ws_host,
                move |value| edit(draft, move |next| next.params.transport.ws.set_host(value)),
            ),
            field(
                lang.tr("custom_node_ws_early_data").to_string(),
                "2048",
                &early,
                move |value| {
                    let parsed = value.trim().parse::<u64>().unwrap_or(0);
                    edit(draft, move |next| {
                        next.params.transport.ws.max_early_data = parsed
                    })
                },
            ),
        ]),
        params_row(vec![
            field(
                lang.tr("custom_node_grpc_service").to_string(),
                "service name",
                draft.params.transport.grpc.service_name.as_str(),
                move |value| {
                    edit(draft, move |next| {
                        next.params.transport.grpc.service_name = value
                    })
                },
            ),
            field(
                lang.tr("custom_node_xhttp_mode").to_string(),
                "auto / packet-up / stream-up / stream-one",
                draft.params.transport.xhttp.mode.as_str(),
                move |value| edit(draft, move |next| next.params.transport.xhttp.mode = value),
            ),
            field(
                lang.tr("custom_node_xhttp_path").to_string(),
                "/x",
                draft.params.transport.xhttp.path.as_str(),
                move |value| edit(draft, move |next| next.params.transport.xhttp.path = value),
            ),
        ]),
    ]
}

/// DUAL-05-06: the SIP003 plugin chain (name + the options this build types).
fn plugin_params<'a>(draft: &'a ProtocolDraft, lang: &Lang<'_>) -> Vec<Element<'a, Message>> {
    if draft.family() != ProtocolFamily::Shadowsocks {
        return Vec::new();
    }
    let host = draft.params.plugin.opt("host").unwrap_or_default();
    let password = draft.params.plugin.opt("password").unwrap_or_default();
    let mode = draft.params.plugin.opt("mode").unwrap_or_default();
    vec![params_row(vec![
        field(
            lang.tr("custom_node_plugin_name").to_string(),
            "shadow-tls / v2ray-plugin / obfs / kcptun",
            draft.params.plugin.name.as_str(),
            move |value| edit(draft, move |next| next.params.plugin.name = value),
        ),
        field(
            lang.tr("custom_node_plugin_host").to_string(),
            "bing.com",
            &host,
            move |value| {
                edit(draft, move |next| {
                    next.params.plugin.opts.insert(
                        "host".into(),
                        infiltrator_contract::protocol_params_ext::PluginOptValue::Text(value),
                    );
                })
            },
        ),
        field(
            lang.tr("custom_node_plugin_password").to_string(),
            "password",
            &password,
            move |value| {
                edit(draft, move |next| {
                    next.params.plugin.opts.insert(
                        "password".into(),
                        infiltrator_contract::protocol_params_ext::PluginOptValue::Text(value),
                    );
                })
            },
        ),
        field(
            lang.tr("custom_node_plugin_mode").to_string(),
            "tls / http / websocket",
            &mode,
            move |value| {
                edit(draft, move |next| {
                    next.params.plugin.opts.insert(
                        "mode".into(),
                        infiltrator_contract::protocol_params_ext::PluginOptValue::Text(value),
                    );
                })
            },
        ),
    ])]
}

/// DUAL-05-07: native SSH SOCKS parameters.
fn ssh_params<'a>(draft: &'a ProtocolDraft, lang: &Lang<'_>) -> Vec<Element<'a, Message>> {
    if draft.family() != ProtocolFamily::Ssh {
        return Vec::new();
    }
    let algorithms = draft.params.ssh.host_key_algorithms.join(",");
    vec![params_row(vec![
        field(
            lang.tr("custom_node_ssh_username").to_string(),
            "root",
            draft.params.ssh.username.as_str(),
            move |value| edit(draft, move |next| next.params.ssh.username = value),
        ),
        field(
            lang.tr("custom_node_ssh_private_key").to_string(),
            "path or inline PEM",
            draft.params.ssh.private_key.as_str(),
            move |value| edit(draft, move |next| next.params.ssh.private_key = value),
        ),
        field(
            lang.tr("custom_node_ssh_passphrase").to_string(),
            "passphrase",
            draft.params.ssh.passphrase.as_str(),
            move |value| edit(draft, move |next| next.params.ssh.passphrase = value),
        ),
        field(
            lang.tr("custom_node_ssh_algorithms").to_string(),
            "ssh-ed25519",
            &algorithms,
            move |value| {
                let entries = value
                    .split(',')
                    .map(|entry| entry.trim().to_string())
                    .filter(|entry| !entry.is_empty())
                    .collect();
                edit(draft, move |next| {
                    next.params.ssh.host_key_algorithms = entries
                })
            },
        ),
    ])]
}

/// DUAL-05-08: AnyTLS session reuse + trojan-go `ss-opts`.
fn anytls_and_trojan_params<'a>(
    draft: &'a ProtocolDraft,
    lang: &Lang<'_>,
) -> Vec<Element<'a, Message>> {
    let mut items = Vec::new();
    match draft.family() {
        ProtocolFamily::Anytls => {
            let idle = draft.params.anytls.idle_session_timeout.to_string();
            let check = draft.params.anytls.idle_session_check_interval.to_string();
            let min_idle = draft.params.anytls.min_idle_session.to_string();
            items.push(params_row(vec![
                field(
                    lang.tr("custom_node_anytls_idle").to_string(),
                    "30000",
                    &idle,
                    move |value| {
                        let parsed = value.trim().parse::<u64>().unwrap_or(0);
                        edit(draft, move |next| {
                            next.params.anytls.idle_session_timeout = parsed
                        })
                    },
                ),
                field(
                    lang.tr("custom_node_anytls_idle_check").to_string(),
                    "30000",
                    &check,
                    move |value| {
                        let parsed = value.trim().parse::<u64>().unwrap_or(0);
                        edit(draft, move |next| {
                            next.params.anytls.idle_session_check_interval = parsed
                        })
                    },
                ),
                field(
                    lang.tr("custom_node_anytls_min_idle").to_string(),
                    "3",
                    &min_idle,
                    move |value| {
                        let parsed = value.trim().parse::<u64>().unwrap_or(0);
                        edit(draft, move |next| {
                            next.params.anytls.min_idle_session = parsed
                        })
                    },
                ),
            ]));
        }
        ProtocolFamily::Trojan => {
            items.push(params_row(vec![
                toggle_row(
                    lang.tr("custom_node_trojan_ss").to_string(),
                    draft.params.trojan_ss.enabled,
                    move |enabled| edit(draft, move |next| next.params.trojan_ss.enabled = enabled),
                ),
                field(
                    lang.tr("custom_node_trojan_ss_method").to_string(),
                    "aes-128-gcm",
                    draft.params.trojan_ss.method.as_str(),
                    move |value| edit(draft, move |next| next.params.trojan_ss.method = value),
                ),
                field(
                    lang.tr("custom_node_trojan_ss_password").to_string(),
                    "ss password",
                    draft.params.trojan_ss.password.as_str(),
                    move |value| edit(draft, move |next| next.params.trojan_ss.password = value),
                ),
            ]));
        }
        _ => {}
    }
    items
}

/// The whole parameter section: family-gated rows plus the honest notes.
pub(super) fn params_section<'a>(state: &'a AppState) -> Element<'a, Message> {
    let lang = Lang(&state.shell.lang);
    let studio = &state.runtime.custom_node_studio;
    let Some(draft) = studio.draft.as_ref() else {
        return Space::new().height(0).into();
    };
    let mut body =
        column![label_text(lang.tr("custom_node_params_title").to_string())].spacing(theme::SP_XS);
    for row_element in tls_params(draft, &lang) {
        body = body.push(row_element);
    }
    match draft.family() {
        ProtocolFamily::Tuic => {
            for row_element in tuic_params(draft, &lang) {
                body = body.push(row_element);
            }
        }
        ProtocolFamily::Hysteria2 => {
            for row_element in hysteria2_params(draft, &lang) {
                body = body.push(row_element);
            }
        }
        ProtocolFamily::WireGuard => {
            for row_element in wireguard_params(draft, &lang) {
                body = body.push(row_element);
            }
        }
        _ => {}
    }
    for row_element in transport_params(draft, &lang) {
        body = body.push(row_element);
    }
    for row_element in plugin_params(draft, &lang) {
        body = body.push(row_element);
    }
    for row_element in ssh_params(draft, &lang) {
        body = body.push(row_element);
    }
    for row_element in anytls_and_trojan_params(draft, &lang) {
        body = body.push(row_element);
    }
    for block in super::custom_node_trust::hop_and_trust_params(draft, studio, &lang) {
        body = body.push(block);
    }

    let notes: Vec<String> = studio
        .report
        .as_ref()
        .map(|report| report.params.notes.clone())
        .unwrap_or_default();
    for note in notes.iter().take(4) {
        body = body.push(
            text(note.clone())
                .size(10)
                .font(MONO)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_tertiary),
                }),
        );
    }

    container(body)
        .padding([8, 12])
        .width(Length::Fill)
        .style(|t: &Theme| {
            let tk = tokens(t);
            container::Style {
                background: Some(tk.control_bg.into()),
                border: iced::Border {
                    radius: iced::border::Radius::from(theme::R_CONTROL),
                    width: 1.0,
                    color: tk.card_border,
                },
                ..Default::default()
            }
        })
        .into()
}
