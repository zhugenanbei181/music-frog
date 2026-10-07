//! Proxies page (代理组与节点) — Clash-Party-style proxy-group cards with
//! expandable node grids, regional country flag emojis, alive-only filtering,
//! favorite pinning, and node detail inspection.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::{
    BadgeKind, badge, card_surface, chip, empty_state, icon_button, modern_scrollable,
    section_header,
};
use crate::view::proxy_history_card::proxy_history_card;
use crate::view::proxy_latency::latency_badge;
use crate::view::proxy_search::node_name;
use crate::view::speedtest_modal::speedtest_card;
use crate::view::svg_icons::Icon;
use crate::view::theme::tokens;
use crate::view::{proxies_controls, svg_icons, theme};
use iced::advanced::widget::Id;
use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Border, Color, Element, Length, Shadow, Theme, Vector, border};
use infiltrator_shared::country_flags::node_flag_emoji;
use infiltrator_shared::locales::{Lang, Localizer};

pub fn view(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);

    // ------------------------------------------------------------------
    // Header: section title with trailing ghost actions
    // ------------------------------------------------------------------
    let test_all_btn: Element<'_, Message> = if state.runtime.runtime_testing_all_delays {
        container(
            text(lang.tr("runtime_delay_testing_all"))
                .size(12)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary),
                }),
        )
        .padding([7, 12])
        .style(pill_surface)
        .into()
    } else {
        button(
            row![
                svg_icons::icon_themed(Icon::Zap, 13.0, |t: &Theme| tokens(t).text_secondary),
                text(lang.tr("runtime_delay_test_all"))
                    .size(12)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_secondary),
                    }),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        )
        .padding([7, 12])
        .style(ghost_pill)
        .on_press(Message::TestAllProxyDelays)
        .into()
    };

    let view_switch_icon = if state.runtime.proxy_compact_view {
        Icon::LayoutGrid
    } else {
        Icon::ListChecks
    };

    let add_node_btn = button(
        row![
            svg_icons::icon_themed(Icon::Plus, 13.0, |t: &Theme| tokens(t).text_secondary),
            text(lang.tr("proxies_add_node_btn"))
                .size(12)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary),
                }),
        ]
        .spacing(4)
        .align_y(Alignment::Center),
    )
    .padding([7, 12])
    .style(ghost_pill)
    .on_press(Message::OpenAddCustomNodeModal(true));

    let header = section_header(
        lang.tr("proxies_title").as_ref(),
        Some(
            row![
                add_node_btn,
                test_all_btn,
                icon_button(Icon::RefreshCw, 15.0, Message::LoadProxies),
                Space::new().width(theme::SP_XS),
                icon_button(view_switch_icon, 15.0, Message::ToggleProxyCompactView),
            ]
            .spacing(theme::SP_SM)
            .align_y(Alignment::Center)
            .into(),
        ),
    );

    // ------------------------------------------------------------------
    // Controls: search box, alive-only toggle, sort pills, delay settings
    // ------------------------------------------------------------------
    let controls = proxies_controls::controls(state, &lang);

    if state.runtime.runtime.is_none() && state.commands.is_none() && !state.shell.demo {
        return column![
            header,
            Space::new().height(theme::SP_MD),
            controls,
            Space::new().height(theme::SP_LG),
            container(empty_state(
                Icon::Globe,
                lang.tr("proxy_not_running").as_ref(),
                "",
            ))
            .width(Length::Fill)
            .padding(theme::SP_LG)
            .style(card_surface),
        ]
        .into();
    }

    // ------------------------------------------------------------------
    // Group cards: icon tile, name + subtitle, count badge, group delay
    // ------------------------------------------------------------------
    let mut groups_col = column![].spacing(theme::SP_MD);

    if state.runtime.filtered_groups.is_empty() {
        groups_col = groups_col.push(
            container(empty_state(
                Icon::Globe,
                lang.tr("proxy_groups_empty").as_ref(),
                "",
            ))
            .width(Length::Fill)
            .padding(theme::SP_LG)
            .style(card_surface),
        );
    }

    for (group_name, members) in &state.runtime.filtered_groups {
        let Some(group_info) = state
            .runtime
            .proxy_groups
            .iter()
            .find(|group| &group.name == group_name)
        else {
            continue;
        };

        let is_expanded = group_info.expanded;

        let group_type = group_info.group_type.as_str();

        let icon_tile = container(svg_icons::icon_themed(
            group_icon(group_type),
            18.0,
            |t: &Theme| tokens(t).accent,
        ))
        .width(38)
        .height(38)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(|t: &Theme| {
            let tk = tokens(t);
            container::Style {
                background: Some(tk.accent_soft.into()),
                border: Border {
                    radius: border::Radius::from(theme::R_CONTROL),
                    width: theme::HAIRLINE,
                    color: Color {
                        a: 0.20,
                        ..tk.accent
                    },
                },
                ..Default::default()
            }
        });

        let subtitle = if group_info.current.trim().is_empty() {
            group_type.to_string()
        } else {
            format!("{group_type} · {}", group_info.current)
        };

        let total_count = group_info.proxies.len();

        let test_group_btn: Element<'_, Message> = if state.runtime.runtime_testing_all_delays {
            container(svg_icons::icon_themed(Icon::Target, 15.0, |t: &Theme| {
                tokens(t).text_tertiary
            }))
            .padding(6)
            .into()
        } else {
            icon_button(
                Icon::Target,
                15.0,
                Message::TestGroupDelay(group_name.clone()),
            )
        };

        let chevron_icon = if is_expanded {
            Icon::ChevronDown
        } else {
            Icon::ChevronRight
        };

        let group_header = row![
            icon_tile,
            column![
                text(group_name)
                    .size(15)
                    .font(theme::FONT_SEMIBOLD)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_primary),
                    }),
                text(subtitle).size(12).style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary),
                }),
            ]
            .spacing(2),
            Space::new().width(Length::Fill),
            badge(total_count.to_string(), BadgeKind::Neutral),
            icon_button(
                Icon::ArrowUp,
                12.0,
                Message::MoveProxyGroupUp(group_name.clone()),
            ),
            icon_button(
                Icon::ArrowDown,
                12.0,
                Message::MoveProxyGroupDown(group_name.clone()),
            ),
            test_group_btn,
            icon_button(
                chevron_icon,
                15.0,
                Message::ToggleProxyGroupExpanded(group_name.clone()),
            ),
        ]
        .spacing(theme::SP_MD)
        .align_y(Alignment::Center);

        let mut card_body = column![group_header].spacing(theme::SP_MD);

        if is_expanded {
            card_body = card_body.push(if state.runtime.proxy_compact_view {
                node_compact_list(state, group_name, members)
            } else {
                node_grid(state, group_name, members)
            });
        }

        groups_col = groups_col.push(
            container(card_body)
                .width(Length::Fill)
                .padding(theme::SP_LG)
                .style(card_surface),
        );
    }

    // Keep the primary node controls reachable at every viewport. Diagnostics
    // share the same scroll surface instead of consuming its available height.
    let body = groups_col
        .push(Space::new().height(theme::SP_MD))
        .push(speedtest_card(state, &lang))
        .push(Space::new().height(theme::SP_MD))
        .push(proxy_history_card(state, &lang));
    column![
        header,
        Space::new().height(theme::SP_MD),
        controls,
        Space::new().height(theme::SP_LG),
        modern_scrollable(body).height(Length::Fill),
    ]
    .into()
}

/// Icon glyph for a proxy-group type tile.
pub fn group_icon(proxy_type: &str) -> Icon {
    match proxy_type {
        "URLTest" | "url-test" | "UrlTest" => Icon::Zap,
        "Fallback" | "fallback" => Icon::Shield,
        "LoadBalance" | "load-balance" | "Load-Balance" => Icon::ListChecks,
        _ => Icon::Globe,
    }
}

/// Canonical display name for proxy protocols (Shadowsocks, Vless, VMess, Trojan, Hysteria2).
pub fn format_protocol_chip(raw_type: &str) -> String {
    match raw_type.to_ascii_lowercase().as_str() {
        "shadowsocks" | "ss" => "Shadowsocks".to_string(),
        "vless" => "Vless".to_string(),
        "vmess" => "VMess".to_string(),
        "trojan" => "Trojan".to_string(),
        "hysteria2" | "hy2" => "Hysteria2".to_string(),
        "wireguard" => "WireGuard".to_string(),
        "tuic" => "Tuic".to_string(),
        "http" => "HTTP".to_string(),
        "socks5" => "SOCKS5".to_string(),
        "snell" => "Snell".to_string(),
        "direct" => "Direct".to_string(),
        "reject" => "Reject".to_string(),
        _ if !raw_type.is_empty() => raw_type.to_string(),
        _ => "Proxy".to_string(),
    }
}

/// Small checkmark indicator pill for the active selected proxy node in `badge_accent`.
fn active_indicator<'a>() -> Element<'a, Message> {
    badge("✓", BadgeKind::Accent)
}

/// Common button style for active vs resting node cards with accent glow.
fn node_button_style(
    is_active: bool,
    glow_alpha: f32,
    blur_radius: f32,
) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |t: &Theme, status| {
        let tk = tokens(t);
        if is_active {
            button::Style {
                background: Some(tk.accent_soft.into()),
                border: Border {
                    radius: border::Radius::from(theme::R_CONTROL),
                    width: 1.5,
                    color: tk.accent,
                },
                shadow: Shadow {
                    color: Color {
                        a: if theme::is_amoled(t) {
                            0.35
                        } else {
                            glow_alpha
                        },
                        ..tk.accent
                    },
                    offset: Vector::new(0.0, 2.0),
                    blur_radius,
                },
                ..Default::default()
            }
        } else {
            button::Style {
                background: match status {
                    button::Status::Hovered | button::Status::Pressed => Some(tk.control_bg.into()),
                    _ => Some(tk.card_bg.into()),
                },
                border: Border {
                    radius: border::Radius::from(theme::R_CONTROL),
                    width: theme::HAIRLINE,
                    color: tk.card_border,
                },
                shadow: tk.card_shadow,
                ..Default::default()
            }
        }
    }
}

/// Node metadata extracted for card and row rendering.
struct NodeMetadata {
    node_type: String,
    udp: bool,
    is_xudp: bool,
    delay: Option<u32>,
    flag: &'static str,
    is_favorite: bool,
}

impl NodeMetadata {
    fn extract(state: &AppState, member_name: &str) -> Self {
        let node = state
            .runtime
            .proxy_groups
            .iter()
            .flat_map(|group| &group.proxies)
            .find(|node| node.name == member_name);
        Self {
            node_type: node.map(|node| node.node_type.clone()).unwrap_or_default(),
            udp: node.is_some_and(|node| node.features.iter().any(|feature| feature == "UDP")),
            is_xudp: node.is_some_and(|node| {
                node.features
                    .iter()
                    .any(|feature| feature.eq_ignore_ascii_case("xudp"))
            }),
            delay: node.and_then(|node| node.delay_ms),
            flag: node_flag_emoji(member_name),
            is_favorite: node.is_some_and(|node| node.favorite),
        }
    }
}

/// 2-column grid of node cards for one expanded group. Column count follows
/// the shared tier operator (1 / 2 / 3 / 4), or a single dense column while the
/// user's compact-view preference is on.
fn node_grid<'a>(
    state: &'a AppState,
    group_name: &str,
    members: &'a [String],
) -> Element<'a, Message> {
    let is_active = |member: &str| {
        state
            .runtime
            .proxy_groups
            .iter()
            .find(|group| group.name == group_name)
            .is_some_and(|group| group.current == member)
    };

    let columns = state
        .shell
        .viewport
        .tier
        .proxy_grid_columns(state.runtime.proxy_compact_view)
        .max(1);

    let mut grid = column![].spacing(theme::SP_SM);
    let mut cells = row![].spacing(theme::SP_SM);
    let mut laid_out = 0usize;

    for member in members {
        cells = cells.push(node_card(
            state,
            group_name,
            member,
            is_active(member.as_str()),
        ));
        laid_out += 1;
        if laid_out.is_multiple_of(columns) {
            grid = grid.push(cells);
            cells = row![].spacing(theme::SP_SM);
        }
    }

    if !laid_out.is_multiple_of(columns) {
        for _ in 0..(columns - laid_out % columns) {
            cells = cells.push(Space::new().width(Length::FillPortion(1)));
        }
        grid = grid.push(cells);
    }

    grid.into()
}

/// Compact single-column list of nodes for high-density viewing.
fn node_compact_list<'a>(
    state: &'a AppState,
    group_name: &str,
    members: &'a [String],
) -> Element<'a, Message> {
    let is_active = |member: &str| {
        state
            .runtime
            .proxy_groups
            .iter()
            .find(|group| group.name == group_name)
            .is_some_and(|group| group.current == member)
    };

    let mut list = column![].spacing(theme::SP_XS);
    for member in members {
        list = list.push(node_compact_row(
            state,
            group_name,
            member,
            is_active(member.as_str()),
        ));
    }
    list.into()
}

/// One node card in grid view with flag emoji, protocol/feature chips, latency, and actions.
fn node_card<'a>(
    state: &'a AppState,
    group_name: &str,
    member_name: &'a str,
    is_active: bool,
) -> Element<'a, Message> {
    let meta = NodeMetadata::extract(state, member_name);

    let mut chips = row![chip(format_protocol_chip(&meta.node_type))].spacing(theme::SP_XS);
    if meta.udp {
        chips = chips.push(chip("udp"));
    }
    if meta.is_xudp {
        chips = chips.push(chip("xudp"));
    }

    let star_btn = icon_button(
        Icon::Pin,
        13.0,
        Message::ToggleFavoriteProxy(member_name.to_string()),
    );
    let info_btn = icon_button(
        Icon::Activity,
        13.0,
        Message::InspectProxy(Some(member_name.to_string())),
    );
    let active_pill = if is_active {
        active_indicator()
    } else {
        Space::new().width(0).into()
    };

    let body = row![
        text(meta.flag).size(16),
        Space::new().width(theme::SP_XS),
        column![
            row![
                node_name(state, member_name, 13),
                active_pill,
                if meta.is_favorite {
                    Element::from(badge("★", BadgeKind::Warning))
                } else {
                    Element::from(Space::new().width(0))
                },
            ]
            .spacing(theme::SP_XS)
            .align_y(Alignment::Center),
            chips,
        ]
        .spacing(theme::SP_XS)
        .width(Length::Fill),
        latency_badge(meta.delay, &Lang(&state.shell.lang)),
        star_btn,
        info_btn,
    ]
    .spacing(theme::SP_SM)
    .align_y(Alignment::Center)
    .width(Length::Fill);

    let mut card_btn = button(
        container(body)
            .id(node_region_id(group_name, member_name))
            .width(Length::Fill)
            .padding([10, 12]),
    )
    .width(Length::FillPortion(1))
    .style(node_button_style(is_active, 0.20, 6.0));

    if !is_active {
        card_btn = card_btn.on_press(Message::SelectProxy(
            group_name.to_string(),
            member_name.to_string(),
        ));
    }

    card_btn.into()
}

/// Compact single-line row representation.
fn node_compact_row<'a>(
    state: &'a AppState,
    group_name: &str,
    member_name: &'a str,
    is_active: bool,
) -> Element<'a, Message> {
    let meta = NodeMetadata::extract(state, member_name);

    let mut chips = row![chip(format_protocol_chip(&meta.node_type))].spacing(theme::SP_XS);
    if meta.udp {
        chips = chips.push(chip("udp"));
    }
    if meta.is_xudp {
        chips = chips.push(chip("xudp"));
    }

    let star_btn = icon_button(
        Icon::Pin,
        12.0,
        Message::ToggleFavoriteProxy(member_name.to_string()),
    );
    let info_btn = icon_button(
        Icon::Activity,
        12.0,
        Message::InspectProxy(Some(member_name.to_string())),
    );
    let active_pill = if is_active {
        active_indicator()
    } else {
        Space::new().width(0).into()
    };

    let body = row![
        text(meta.flag).size(14),
        Space::new().width(theme::SP_XS),
        node_name(state, member_name, 12),
        active_pill,
        if meta.is_favorite {
            Element::from(badge("★", BadgeKind::Warning))
        } else {
            Element::from(Space::new().width(0))
        },
        Space::new().width(theme::SP_XS),
        chips,
        Space::new().width(Length::Fill),
        latency_badge(meta.delay, &Lang(&state.shell.lang)),
        star_btn,
        info_btn,
    ]
    .spacing(theme::SP_SM)
    .align_y(Alignment::Center)
    .width(Length::Fill);

    let mut row_btn = button(
        container(body)
            .id(node_region_id(group_name, member_name))
            .width(Length::Fill)
            .padding([6, 10]),
    )
    .width(Length::Fill)
    .style(node_button_style(is_active, 0.16, 4.0));

    if !is_active {
        row_btn = row_btn.on_press(Message::SelectProxy(
            group_name.to_string(),
            member_name.to_string(),
        ));
    }

    row_btn.into()
}

/// Neutral pill surface for resting-state labels (e.g. "testing all...").
fn pill_surface(t: &Theme) -> container::Style {
    container::Style {
        background: Some(tokens(t).control_bg.into()),
        border: Border {
            radius: border::Radius::from(theme::R_CHIP),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// Ghost pill button (transparent until hover) for header actions.
fn ghost_pill(t: &Theme, status: button::Status) -> button::Style {
    let tk = tokens(t);
    button::Style {
        background: match status {
            button::Status::Hovered | button::Status::Pressed => Some(tk.control_bg.into()),
            _ => None,
        },
        border: Border {
            radius: border::Radius::from(theme::R_CHIP),
            ..Default::default()
        },
        text_color: tk.text_secondary,
        ..Default::default()
    }
}

#[cfg(test)]
#[path = "../../tests/gui/view_proxies_tests.rs"]
mod tests;

/// Stable identity of the real node control's existing content container.
pub fn node_region_id(group: &str, node: &str) -> Id {
    Id::from(format!("proxy-node:{}:{group}{node}", group.len()))
}
