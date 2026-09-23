//! Slide-out Deep Link Telemetry drawer for inspecting single connection details.

use crate::state::AppState;
use crate::types::message::Message;
use crate::utils::format_bytes;
use crate::view::components::{chip, modern_scrollable, style_accent, style_danger, style_ghost};
use crate::view::svg_icons::{Icon, icon_themed};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, MONO, tokens};
use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};
use infiltrator_domain::connection_view;
use infiltrator_domain::runtime::Connection;
use infiltrator_shared::locales::{Lang, Localizer};

pub fn connection_drawer_modal<'a>(state: &'a AppState, conn_id: &'a str) -> Element<'a, Message> {
    let lang = Lang(&state.shell.lang);

    let conn: Option<&Connection> = state
        .diag
        .connections
        .as_ref()
        .and_then(|snap| snap.connections.iter().find(|c| c.id == conn_id));

    let Some(conn) = conn else {
        return container(Space::new().width(0).height(0)).into();
    };

    let meta = &conn.metadata;
    let target_host = if !meta.host.is_empty() {
        format!("{}:{}", meta.host, meta.destination_port)
    } else {
        format!("{}:{}", meta.destination_ip, meta.destination_port)
    };

    // Header with host and protocol
    let header = row![
        icon_themed(Icon::Activity, 20.0, |t: &Theme| tokens(t).accent),
        Space::new().width(theme::SP_SM),
        column![
            text(lang.tr("conn_drawer_title"))
                .size(15)
                .font(FONT_SEMIBOLD),
            text(target_host.clone())
                .size(12)
                .font(MONO)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary),
                }),
        ]
        .spacing(2)
        .width(Length::Fill),
        chip(meta.network.to_uppercase()),
        Space::new().width(theme::SP_SM),
        button(icon_themed(Icon::X, 16.0, |t: &Theme| tokens(t).text_tertiary))
            .style(style_ghost)
            .padding(6)
            .on_press(Message::InspectConnection(None)),
    ]
    .align_y(Alignment::Center);

    // Section 1: Lifecycle latency. The mihomo `/connections` payload does not
    // carry DNS/TCP/TLS/TTFB timings, so DUAL-13-04 is reported as typed
    // unsupported instead of the fabricated 18/42/68/92 ms bars this drawer
    // previously drew.
    let latency_section = column![
        row![
            icon_themed(Icon::Zap, 14.0, |t: &Theme| tokens(t).warning),
            Space::new().width(theme::SP_XS),
            text(lang.tr("conn_drawer_section_lifecycle"))
                .size(13)
                .font(FONT_SEMIBOLD),
        ]
        .align_y(Alignment::Center),
        Space::new().height(theme::SP_XS),
        text(lang.tr("conn_drawer_timing_unsupported"))
            .size(11)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_tertiary),
            }),
    ]
    .spacing(6);

    // Section 2: Real-time Throughput & Traffic. The instantaneous rates are
    // the shared DUAL-13-10/12 derivation (successive total diff over a
    // measured interval); before a second observation they read as an honest
    // "no data yet" rather than a fabricated speed.
    let rate = state.diag.connection_rate_book.get(&conn.id);
    let rate_row = if rate.peak_bps() > 0.0 {
        row![
            stat_card(
                lang.tr("conn_drawer_upload_speed"),
                format!("{}/s", format_bytes(rate.upload_bps.max(0.0) as u64)),
                Icon::ArrowUp,
                |t| tokens(t).accent
            ),
            Space::new().width(theme::SP_SM),
            stat_card(
                lang.tr("conn_drawer_download_speed"),
                format!("{}/s", format_bytes(rate.download_bps.max(0.0) as u64)),
                Icon::ArrowDown,
                |t| tokens(t).success
            ),
        ]
    } else {
        row![
            text(lang.tr("conn_drawer_rate_pending"))
                .size(11)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_tertiary),
                })
        ]
    };

    let throughput_section = column![
        row![
            icon_themed(Icon::Network, 14.0, |t: &Theme| tokens(t).accent),
            Space::new().width(theme::SP_XS),
            text(lang.tr("conn_drawer_section_throughput"))
                .size(13)
                .font(FONT_SEMIBOLD),
        ]
        .align_y(Alignment::Center),
        Space::new().height(theme::SP_XS),
        row![
            stat_card(
                lang.tr("conn_drawer_total_upload"),
                format_bytes(conn.upload),
                Icon::ArrowUp,
                |t| tokens(t).accent
            ),
            Space::new().width(theme::SP_SM),
            stat_card(
                lang.tr("conn_drawer_total_download"),
                format_bytes(conn.download),
                Icon::ArrowDown,
                |t| tokens(t).success
            ),
        ],
        Space::new().height(theme::SP_XS),
        rate_row,
    ]
    .spacing(6);

    // Section 3: Routing & Outbound Proxy Chain
    // DUAL-13-06: every hop of the parsed chain is rendered as its own stage
    // through the shared route-chain model.
    let chain = connection_view::route_chain(conn);

    let routing_section = column![
        row![
            icon_themed(Icon::Target, 14.0, |t: &Theme| tokens(t).accent),
            Space::new().width(theme::SP_XS),
            text(lang.tr("conn_drawer_section_routing"))
                .size(13)
                .font(FONT_SEMIBOLD),
        ]
        .align_y(Alignment::Center),
        Space::new().height(theme::SP_XS),
        meta_field_row(
            lang.tr("conn_drawer_matched_rule"),
            if conn.rule.is_empty() {
                "MATCH".to_string()
            } else {
                conn.rule.clone()
            }
        ),
        meta_field_row(
            lang.tr("conn_drawer_rule_payload"),
            if conn.rule_payload.is_empty() {
                "—".to_string()
            } else {
                conn.rule_payload.clone()
            }
        ),
        route_chain_row(lang.tr("conn_drawer_proxy_chain"), &chain),
    ]
    .spacing(6);

    // Section 4: Process & Network Stack
    let proc_name = if !meta.process_path.is_empty() {
        meta.process_path
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(&meta.process_path)
            .to_string()
    } else {
        "—".to_string()
    };

    let local_endpoint = format!("{}:{}", meta.source_ip, meta.source_port);
    let remote_endpoint = format!("{}:{}", meta.destination_ip, meta.destination_port);

    let process_section = column![
        row![
            icon_themed(Icon::Server, 14.0, |t: &Theme| tokens(t).accent),
            Space::new().width(theme::SP_XS),
            text(lang.tr("conn_drawer_section_process"))
                .size(13)
                .font(FONT_SEMIBOLD),
        ]
        .align_y(Alignment::Center),
        Space::new().height(theme::SP_XS),
        meta_field_row(lang.tr("conn_drawer_process_name"), proc_name),
        meta_field_row(lang.tr("conn_drawer_local_addr"), local_endpoint),
        meta_field_row(lang.tr("conn_drawer_remote_addr"), remote_endpoint),
        meta_field_row(lang.tr("conn_drawer_network"), meta.network.to_uppercase()),
        // DUAL-13-05: the kernel's own GEOIP/IP-ASN rule-evaluation results for
        // the target IP. The client reads no MMDB here; when the kernel did not
        // evaluate a matching rule, the row says so instead of guessing.
        meta_field_row(
            lang.tr("conn_drawer_kernel_asn"),
            kernel_asn_label(&meta.destination_ip_asn, &lang),
        ),
        meta_field_row(
            lang.tr("conn_drawer_kernel_geo"),
            kernel_geo_label(meta.destination_geo_ip.as_deref(), &lang),
        ),
    ]
    .spacing(6);

    // Actions
    let close_conn_id = conn.id.clone();
    // DUAL-13-09: the reverse rule draft is the shared domain spec (bare host,
    // no port); an un-draftable destination disables the button instead of
    // emitting a bogus empty pattern.
    let rule_spec =
        connection_view::quick_rule_spec(conn, connection_view::DEFAULT_QUICK_RULE_TARGET);
    let actions_bar = row![
        button(
            row![
                icon_themed(Icon::Trash2, 14.0, |t: &Theme| tokens(t).danger),
                Space::new().width(theme::SP_XS),
                text(lang.tr("conn_drawer_close_conn_btn"))
                    .size(12)
                    .font(FONT_MEDIUM),
            ]
            .align_y(Alignment::Center)
        )
        .style(style_danger)
        .padding([8, 14])
        .on_press(Message::CloseConnection(close_conn_id)),
        Space::new().width(theme::SP_SM),
        button(
            row![
                icon_themed(Icon::Plus, 14.0, |t: &Theme| tokens(t).on_accent),
                Space::new().width(theme::SP_XS),
                text(lang.tr("quick_rule_btn")).size(12).font(FONT_MEDIUM),
            ]
            .align_y(Alignment::Center)
        )
        .style(style_accent)
        .padding([8, 14])
        .on_press_maybe(rule_spec.is_draftable().then(|| {
            Message::AddQuickRuleFromConnection {
                pattern: rule_spec.pattern.clone(),
                target: rule_spec.target.clone(),
            }
        })),
        Space::new().width(theme::SP_SM),
        button(
            row![
                icon_themed(Icon::Copy, 14.0, |t: &Theme| tokens(t).text_secondary),
                Space::new().width(theme::SP_XS),
                text(lang.tr("conn_drawer_copy_host_btn"))
                    .size(12)
                    .font(FONT_MEDIUM),
            ]
            .align_y(Alignment::Center)
        )
        .style(style_ghost)
        .padding([8, 14])
        .on_press(Message::ShowToast(
            format!("Copied: {target_host}"),
            crate::types::app::ToastStatus::Success,
        )),
        Space::new().width(Length::Fill),
        button(
            text(lang.tr("conn_drawer_close"))
                .size(12)
                .font(FONT_MEDIUM)
        )
        .style(style_ghost)
        .padding([8, 16])
        .on_press(Message::InspectConnection(None)),
    ]
    .align_y(Alignment::Center);

    let content = column![
        header,
        Space::new().height(theme::SP_SM),
        latency_section,
        Space::new().height(theme::SP_SM),
        throughput_section,
        Space::new().height(theme::SP_SM),
        routing_section,
        Space::new().height(theme::SP_SM),
        process_section,
        Space::new().height(theme::SP_MD),
        actions_bar,
    ]
    .spacing(10);

    let drawer_panel = container(modern_scrollable(content).height(Length::Fill))
        .width(Length::Fixed(
            state.shell.viewport.detail_panel_width_px(480.0),
        ))
        .height(Length::Fill)
        .padding(24)
        .style(|t: &Theme| {
            let tk = tokens(t);
            container::Style {
                background: Some(tk.card_bg.into()),
                border: Border {
                    radius: border::Radius {
                        top_left: 16.0,
                        top_right: 0.0,
                        bottom_right: 0.0,
                        bottom_left: 16.0,
                    },
                    width: theme::HAIRLINE,
                    color: tk.card_border,
                },
                shadow: tk.floating_shadow,
                text_color: Some(tk.text_primary),
                ..Default::default()
            }
        });

    container(row![
        container(Space::new().width(Length::Fill).height(Length::Fill))
            .width(Length::Fill)
            .height(Length::Fill),
        drawer_panel,
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|t: &Theme| container::Style {
        background: Some(tokens(t).scrim.into()),
        ..Default::default()
    })
    .into()
}

fn stat_card<'a, Message: 'a>(
    label: impl Into<String>,
    value: impl Into<String>,
    icon: Icon,
    color: fn(&Theme) -> Color,
) -> Element<'a, Message> {
    let label_s = label.into();
    let val_s = value.into();
    container(
        row![
            icon_themed(icon, 16.0, color),
            Space::new().width(theme::SP_SM),
            column![
                text(label_s).size(10).style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_tertiary),
                }),
                text(val_s)
                    .size(14)
                    .font(FONT_SEMIBOLD)
                    .style(move |t: &Theme| text::Style {
                        color: Some(color(t)),
                    }),
            ]
            .spacing(2),
        ]
        .align_y(Alignment::Center),
    )
    .padding([8, 14])
    .width(Length::Fill)
    .style(|t: &Theme| {
        let tk = tokens(t);
        container::Style {
            background: Some(tk.control_bg.into()),
            border: Border {
                radius: 8.0.into(),
                width: theme::HAIRLINE,
                color: tk.card_border,
            },
            ..Default::default()
        }
    })
    .into()
}

fn route_chain_row<'a>(
    label: impl Into<String>,
    chain: &connection_view::RouteChain,
) -> Element<'a, Message> {
    let label_s = label.into();
    let tertiary = |t: &Theme| text::Style {
        color: Some(tokens(t).text_tertiary),
    };
    let mut items: Vec<Element<'a, Message>> = vec![
        text(label_s)
            .size(11)
            .width(Length::Fixed(120.0))
            .style(tertiary)
            .into(),
    ];
    if chain.is_empty() {
        items.push(
            text("DIRECT")
                .size(11)
                .font(MONO)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_primary),
                })
                .into(),
        );
    } else {
        for (hop_idx, hop) in chain.hops().iter().enumerate() {
            if hop_idx > 0 {
                items.push(text(" ➔ ").size(11).font(MONO).style(tertiary).into());
            }
            items.push(chip(hop.clone()));
        }
    }
    row(items).align_y(Alignment::Center).into()
}

fn meta_field_row<'a, Message: 'a>(
    label: impl Into<String>,
    val: impl Into<String>,
) -> Element<'a, Message> {
    let label_s = label.into();
    let val_s = val.into();
    row![
        text(label_s)
            .size(11)
            .width(Length::Fixed(120.0))
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_tertiary),
            }),
        text(val_s)
            .size(11)
            .font(MONO)
            .width(Length::Fill)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_primary),
            }),
    ]
    .align_y(Alignment::Center)
    .into()
}

/// DUAL-13-05: render the kernel's raw `destinationIPASN` value through the
/// shared fact reduction. No ASN is inferred client-side.
fn kernel_asn_label(raw: &str, lang: &Lang<'_>) -> String {
    match connection_view::destination_asn_fact(raw) {
        connection_view::DestinationAsnFact::NotEvaluated => {
            lang.tr("conn_drawer_kernel_not_evaluated").to_string()
        }
        connection_view::DestinationAsnFact::NoResult => {
            lang.tr("conn_drawer_kernel_no_result").to_string()
        }
        connection_view::DestinationAsnFact::Reported(value) => value,
    }
}

/// DUAL-13-05: render the kernel's raw `destinationGeoIP` value through the
/// shared fact reduction.
fn kernel_geo_label(codes: Option<&[String]>, lang: &Lang<'_>) -> String {
    match connection_view::destination_geo_fact(codes) {
        connection_view::DestinationGeoFact::NotEvaluated => {
            lang.tr("conn_drawer_kernel_not_evaluated").to_string()
        }
        connection_view::DestinationGeoFact::NoResult => {
            lang.tr("conn_drawer_kernel_no_result").to_string()
        }
        connection_view::DestinationGeoFact::Codes(codes) => codes.join(", "),
    }
}

#[cfg(test)]
#[path = "../../tests/gui/view_connection_drawer_tests.rs"]
mod tests;
