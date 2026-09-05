//! Proxies page (代理组与节点) — Clash-Party-style proxy-group cards with
//! expandable node grids, regional country flag emojis, alive-only filtering,
//! favorite pinning, and node detail inspection.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::{
    BadgeKind, badge, card_surface, chip, empty_state, icon_button, latency_badge,
    modern_scrollable, section_header, toggle_switch,
};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, tokens};
use iced::widget::{Space, button, column, container, row, text, text_input};
use iced::{Alignment, Border, Color, Element, Length, Shadow, Theme, Vector, border};
use infiltrator_shared::country_flags::node_flag_emoji;
use infiltrator_shared::locales::{Lang, Localizer};

/// Node cards per row inside an expanded group (2-column grid or compact 1-column list).
const NODE_GRID_COLUMNS: usize = 2;

/// Sort options already understood by `Message::UpdateProxyDelaySort`.
const SORT_KEYS: [&str; 4] = ["delay_asc", "delay_desc", "name_asc", "name_desc"];
const SORT_LABEL_KEYS: [&str; 4] = [
    "runtime_delay_sort_delay_asc",
    "runtime_delay_sort_delay_desc",
    "runtime_delay_sort_name_asc",
    "runtime_delay_sort_name_desc",
];

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
    let search_box = row![
        svg_icons::icon_themed(Icon::Search, 14.0, |t: &Theme| tokens(t).text_tertiary),
        text_input(
            lang.tr("proxies_search_placeholder").as_ref(),
            &state.runtime.proxy_filter,
        )
        .on_input(Message::FilterProxies)
        .size(12)
        .padding([5, 9])
        .width(Length::Fixed(180.0))
        .style(delay_input_style),
        if state.runtime.proxy_filter.is_empty() {
            Element::from(Space::new().width(0))
        } else {
            icon_button(Icon::X, 12.0, Message::FilterProxies(String::new()))
        },
    ]
    .spacing(theme::SP_XS)
    .align_y(Alignment::Center);

    let alive_toggle = row![
        text(lang.tr("proxies_filter_alive"))
            .size(11)
            .font(theme::FONT_MEDIUM)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary),
            }),
        Space::new().width(theme::SP_XS),
        toggle_switch(state.runtime.filter_alive_only, Message::ToggleFilterAlive),
    ]
    .spacing(theme::SP_XS)
    .align_y(Alignment::Center);

    let mut sort_segment = row![].spacing(2);
    for (index, key) in SORT_KEYS.iter().enumerate() {
        let active = state.runtime.proxy_delay_sort == *key;
        let label = text(lang.tr(SORT_LABEL_KEYS[index]))
            .size(11)
            .font(if active {
                theme::FONT_SEMIBOLD
            } else {
                theme::FONT_MEDIUM
            })
            .style(move |t: &Theme| text::Style {
                color: Some(if active {
                    tokens(t).text_primary
                } else {
                    tokens(t).text_secondary
                }),
            });

        let mut segment = button(container(label).padding([5, 10])).padding(0).style(
            move |t: &Theme, _status| {
                let tk = tokens(t);
                if active {
                    button::Style {
                        background: Some(tk.card_bg.into()),
                        border: Border {
                            radius: border::Radius::from(theme::R_CONTROL),
                            ..Default::default()
                        },
                        shadow: tk.card_shadow,
                        text_color: tk.text_primary,
                        ..Default::default()
                    }
                } else {
                    button::Style {
                        background: None,
                        border: Border {
                            radius: border::Radius::from(theme::R_CONTROL),
                            ..Default::default()
                        },
                        text_color: tk.text_secondary,
                        ..Default::default()
                    }
                }
            },
        );

        if !active {
            segment = segment.on_press(Message::UpdateProxyDelaySort((*key).to_string()));
        }

        sort_segment = sort_segment.push(segment);
    }

    let sort_segment = container(sort_segment)
        .padding(2)
        .style(|t: &Theme| container::Style {
            background: Some(tokens(t).control_bg.into()),
            border: Border {
                radius: border::Radius::from(theme::R_CONTROL),
                width: 1.0,
                color: tokens(t).card_border,
            },
            ..Default::default()
        });

    let controls = row![
        search_box,
        Space::new().width(theme::SP_SM),
        alive_toggle,
        Space::new().width(theme::SP_SM),
        sort_segment,
        Space::new().width(Length::Fill),
        delay_test_group(state, &lang),
    ]
    .spacing(theme::SP_SM)
    .align_y(Alignment::Center)
    .width(Length::Fill);

    if state.runtime.runtime.is_none() && !state.shell.demo {
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

    for (index, (group_name, members)) in state.runtime.filtered_groups.iter().enumerate() {
        let Some(group_info) = state.runtime.proxies.get(group_name) else {
            continue;
        };

        let is_expanded = match state.runtime.proxy_groups_expanded.as_ref() {
            Some(ids) => ids.iter().any(|id| id == group_name),
            None => index == 0,
        };

        let group_type = group_info.proxy_type();

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
                    width: 1.0,
                    color: Color {
                        a: 0.20,
                        ..tk.accent
                    },
                },
                ..Default::default()
            }
        });

        let subtitle = match group_info.now() {
            Some(now) if !now.trim().is_empty() => format!("{group_type} · {now}"),
            _ => group_type.to_string(),
        };

        let total_count = group_info.all().map_or(members.len(), |all| all.len());

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

    column![
        header,
        Space::new().height(theme::SP_MD),
        controls,
        Space::new().height(theme::SP_MD),
        crate::view::speedtest_modal::speedtest_card(state, &lang),
        Space::new().height(theme::SP_MD),
        crate::view::latency_radar_card::latency_radar_card(state, &lang),
        Space::new().height(theme::SP_LG),
        modern_scrollable(groups_col).height(Length::Fill),
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
                border: Border { radius: border::Radius::from(theme::R_CONTROL), width: 1.5, color: tk.accent },
                shadow: Shadow {
                    color: Color { a: if theme::is_amoled(t) { 0.35 } else { glow_alpha }, ..tk.accent },
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
                border: Border { radius: border::Radius::from(theme::R_CONTROL), width: 1.0, color: tk.card_border },
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
        let node = state.runtime.proxies.get(member_name);
        Self {
            node_type: node.map(|p| p.proxy_type().to_string()).unwrap_or_default(),
            udp: node
                .map(infiltrator_domain::proxy::Proxy::udp)
                .unwrap_or(false),
            is_xudp: member_name.to_ascii_lowercase().contains("xudp"),
            delay: node.and_then(|p| p.history().last().map(|h| h.delay)).filter(|d| *d > 0),
            flag: node_flag_emoji(member_name),
            is_favorite: state.runtime.favorite_proxies.contains(member_name),
        }
    }
}

/// 2-column grid of node cards for one expanded group.
fn node_grid<'a>(state: &'a AppState, group_name: &str, members: &'a [String]) -> Element<'a, Message> {
    let is_active = |member: &str| {
        state.runtime.proxies.get(group_name).and_then(|g| g.now()).is_some_and(|now| now == member)
    };

    let mut grid = column![].spacing(theme::SP_SM);
    let mut cells = row![].spacing(theme::SP_SM);
    let mut laid_out = 0usize;

    for member in members {
        cells = cells.push(node_card(state, group_name, member, is_active(member.as_str())));
        laid_out += 1;
        if laid_out.is_multiple_of(NODE_GRID_COLUMNS) {
            grid = grid.push(cells);
            cells = row![].spacing(theme::SP_SM);
        }
    }

    if !laid_out.is_multiple_of(NODE_GRID_COLUMNS) {
        for _ in 0..(NODE_GRID_COLUMNS - laid_out % NODE_GRID_COLUMNS) {
            cells = cells.push(Space::new().width(Length::FillPortion(1)));
        }
        grid = grid.push(cells);
    }

    grid.into()
}

/// Compact single-column list of nodes for high-density viewing.
fn node_compact_list<'a>(state: &'a AppState, group_name: &str, members: &'a [String]) -> Element<'a, Message> {
    let is_active = |member: &str| {
        state.runtime.proxies.get(group_name).and_then(|g| g.now()).is_some_and(|now| now == member)
    };

    let mut list = column![].spacing(theme::SP_XS);
    for member in members {
        list = list.push(node_compact_row(state, group_name, member, is_active(member.as_str())));
    }
    list.into()
}

/// One node card in grid view with flag emoji, protocol/feature chips, latency, and actions.
fn node_card<'a>(state: &'a AppState, group_name: &str, member_name: &'a str, is_active: bool) -> Element<'a, Message> {
    let meta = NodeMetadata::extract(state, member_name);

    let mut chips = row![chip(format_protocol_chip(&meta.node_type))].spacing(theme::SP_XS);
    if meta.udp { chips = chips.push(chip("udp")); }
    if meta.is_xudp { chips = chips.push(chip("xudp")); }

    let star_btn = icon_button(Icon::Pin, 13.0, Message::ToggleFavoriteProxy(member_name.to_string()));
    let info_btn = icon_button(Icon::Activity, 13.0, Message::InspectProxy(Some(member_name.to_string())));
    let active_pill = if is_active { active_indicator() } else { Space::new().width(0).into() };

    let body = row![
        text(meta.flag).size(16),
        Space::new().width(theme::SP_XS),
        column![
            row![
                text(member_name).size(13).font(theme::FONT_SEMIBOLD).style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_primary),
                }),
                active_pill,
                if meta.is_favorite { Element::from(badge("★", BadgeKind::Warning)) } else { Element::from(Space::new().width(0)) },
            ].spacing(theme::SP_XS).align_y(Alignment::Center),
            chips,
        ].spacing(theme::SP_XS).width(Length::Fill),
        latency_badge(meta.delay),
        star_btn,
        info_btn,
    ].spacing(theme::SP_SM).align_y(Alignment::Center).width(Length::Fill);

    let mut card_btn = button(container(body).width(Length::Fill).padding([10, 12]))
        .width(Length::FillPortion(1))
        .style(node_button_style(is_active, 0.20, 6.0));

    if !is_active {
        card_btn = card_btn.on_press(Message::SelectProxy(group_name.to_string(), member_name.to_string()));
    }

    card_btn.into()
}

/// Compact single-line row representation.
fn node_compact_row<'a>(state: &'a AppState, group_name: &str, member_name: &'a str, is_active: bool) -> Element<'a, Message> {
    let meta = NodeMetadata::extract(state, member_name);

    let mut chips = row![chip(format_protocol_chip(&meta.node_type))].spacing(theme::SP_XS);
    if meta.udp { chips = chips.push(chip("udp")); }
    if meta.is_xudp { chips = chips.push(chip("xudp")); }

    let star_btn = icon_button(Icon::Pin, 12.0, Message::ToggleFavoriteProxy(member_name.to_string()));
    let info_btn = icon_button(Icon::Activity, 12.0, Message::InspectProxy(Some(member_name.to_string())));
    let active_pill = if is_active { active_indicator() } else { Space::new().width(0).into() };

    let body = row![
        text(meta.flag).size(14),
        Space::new().width(theme::SP_XS),
        text(member_name).size(12).font(theme::FONT_SEMIBOLD).style(|t: &Theme| text::Style {
            color: Some(tokens(t).text_primary),
        }),
        active_pill,
        if meta.is_favorite { Element::from(badge("★", BadgeKind::Warning)) } else { Element::from(Space::new().width(0)) },
        Space::new().width(theme::SP_XS),
        chips,
        Space::new().width(Length::Fill),
        latency_badge(meta.delay),
        star_btn,
        info_btn,
    ].spacing(theme::SP_SM).align_y(Alignment::Center).width(Length::Fill);

    let mut row_btn = button(container(body).width(Length::Fill).padding([6, 10]))
        .width(Length::Fill)
        .style(node_button_style(is_active, 0.16, 4.0));

    if !is_active {
        row_btn = row_btn.on_press(Message::SelectProxy(group_name.to_string(), member_name.to_string()));
    }

    row_btn.into()
}

/// Compact bordered control group for the delay-test endpoint.
fn delay_test_group<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let label = |key: &'static str| {
        text(lang.tr(key)).size(11).font(theme::FONT_MEDIUM).style(|t: &Theme| text::Style {
            color: Some(tokens(t).text_secondary),
        })
    };

    container(
        row![
            label("proxies_delay_test_url_label"),
            Space::new().width(theme::SP_XS),
            text_input("", &state.runtime.runtime_delay_test_url)
                .on_input(Message::UpdateDelayTestUrl)
                .size(12)
                .padding([5, 9])
                .width(Length::Fixed(190.0))
                .style(delay_input_style),
            Space::new().width(theme::SP_MD),
            label("proxies_delay_timeout_label"),
            Space::new().width(theme::SP_XS),
            text_input("", &state.runtime.runtime_delay_timeout_ms)
                .on_input(Message::UpdateDelayTimeoutMs)
                .size(12)
                .font(theme::MONO)
                .padding([5, 9])
                .width(Length::Fixed(76.0))
                .style(delay_input_style),
        ].spacing(theme::SP_SM).align_y(Alignment::Center),
    )
    .padding(theme::SP_SM)
    .style(delay_group_surface)
    .into()
}

/// Hairline-bordered control-group surface (tokens, control radius).
fn delay_group_surface(t: &Theme) -> container::Style {
    let tk = tokens(t);
    container::Style {
        background: Some(tk.control_bg.into()),
        border: Border { radius: border::Radius::from(theme::R_CONTROL), width: 1.0, color: tk.card_border },
        ..Default::default()
    }
}

/// Token-driven text-input style matching the runtime page's inputs.
fn delay_input_style(t: &Theme, status: text_input::Status) -> text_input::Style {
    let tk = tokens(t);
    let (border_color, border_width) = match status {
        text_input::Status::Focused { .. } => (tk.accent, 1.5),
        _ => (tk.card_border, 1.0),
    };
    text_input::Style {
        background: tk.card_bg.into(),
        border: Border { radius: border::Radius::from(theme::R_CONTROL), width: border_width, color: border_color },
        icon: tk.text_tertiary,
        placeholder: tk.text_tertiary,
        value: tk.text_primary,
        selection: Color { a: 0.25, ..tk.accent },
    }
}

/// Neutral pill surface for resting-state labels (e.g. "testing all...").
fn pill_surface(t: &Theme) -> container::Style {
    container::Style {
        background: Some(tokens(t).control_bg.into()),
        border: Border { radius: border::Radius::from(theme::R_CHIP), ..Default::default() },
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
        border: Border { radius: border::Radius::from(theme::R_CHIP), ..Default::default() },
        text_color: tk.text_secondary,
        ..Default::default()
    }
}

#[cfg(test)]
#[path = "../../tests/gui/view_proxies_tests.rs"]
mod tests;
