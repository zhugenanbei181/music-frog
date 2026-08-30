//! Proxies page (代理组与节点) — Clash-Party-style proxy-group cards with
//! expandable node grids.
//!
//! ui-wave2-p: presentation-only restyle over the Wave 1 design tokens and
//! shared components. Filtering, sorting, delay testing and node switching
//! still flow through the exact same `Message`s as before.

use crate::locales::{Lang, Localizer};
use crate::view::components::{
    BadgeKind, badge, card_surface, chip, empty_state, icon_button, latency_badge,
    modern_scrollable, section_header,
};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, tokens};
use crate::{AppState, Message};
use iced::widget::{Space, button, column, container, row, text, text_input};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};

/// Node cards per row inside an expanded group (the reference layout uses a
/// 2-column grid; chunked with fillers like the previous 3-column layout).
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
    // Header: section title with trailing ghost actions (test all /
    // refresh) — same `Message`s as the previous control row.
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

    let header = section_header(
        lang.tr("proxies_title").as_ref(),
        Some(
            row![
                test_all_btn,
                icon_button(Icon::RefreshCw, 15.0, Message::LoadProxies),
            ]
            .spacing(theme::SP_SM)
            .align_y(Alignment::Center)
            .into(),
        ),
    );

    // ------------------------------------------------------------------
    // Controls: search box (Message::FilterProxies), sort segmented pills
    // (Message::UpdateProxyDelaySort), delay-test URL / timeout inputs.
    // ------------------------------------------------------------------
    let search_box = row![
        svg_icons::icon_themed(Icon::Search, 15.0, |t: &Theme| tokens(t).text_tertiary),
        text_input(
            lang.tr("proxies_search_placeholder").as_ref(),
            &state.runtime.proxy_filter
        )
        .on_input(Message::FilterProxies)
        .size(13)
        .width(Length::Fixed(190.0)),
        if state.runtime.proxy_filter.is_empty() {
            Space::new().width(0).into()
        } else {
            icon_button(Icon::X, 12.0, Message::FilterProxies(String::new()))
        },
    ]
    .spacing(theme::SP_SM)
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
                if active {
                    button::Style {
                        background: Some(tokens(t).card_bg.into()),
                        border: Border {
                            radius: border::Radius::from(theme::R_CONTROL),
                            ..Default::default()
                        },
                        shadow: tokens(t).card_shadow,
                        text_color: tokens(t).text_primary,
                        ..Default::default()
                    }
                } else {
                    button::Style {
                        text_color: tokens(t).text_secondary,
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
                ..Default::default()
            },
            ..Default::default()
        });

    let controls = row![
        search_box,
        text(lang.tr("runtime_delay_sort"))
            .size(11)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_tertiary),
            }),
        sort_segment,
        Space::new().width(Length::Fill),
        delay_test_group(state, &lang),
    ]
    .spacing(theme::SP_SM)
    .align_y(Alignment::Center)
    .width(Length::Fill);

    if state.runtime.runtime.is_none() && !state.shell.demo {
        // demo-mode: a demo session has no live runtime but ships fixture
        // groups, so it falls through to the full group rendering below.
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
    // test (Message::TestGroupDelay), expand chevron. Expanded groups show
    // a 2-column grid of node cards (Message::SelectProxy to switch).
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

        // Pristine state (`None`) mirrors the reference layout: the first
        // group starts expanded. After the user toggles anything, the
        // explicit id list decides.
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
        .style(|t: &Theme| container::Style {
            background: Some(tokens(t).accent_soft.into()),
            border: Border {
                radius: border::Radius::from(theme::R_CONTROL),
                ..Default::default()
            },
            ..Default::default()
        });

        // "Selector · currently-selected-node" style subtitle, from the
        // group's existing state fields.
        let subtitle = match group_info.now() {
            Some(now) if !now.trim().is_empty() => format!("{group_type} · {now}"),
            _ => group_type.to_string(),
        };

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
            badge(members.len().to_string(), BadgeKind::Neutral),
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
            card_body = card_body.push(node_grid(state, group_name, members));
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
        Space::new().height(theme::SP_LG),
        modern_scrollable(groups_col).height(Length::Fill),
    ]
    .into()
}

/// Icon glyph for a proxy-group type tile.
fn group_icon(proxy_type: &str) -> Icon {
    match proxy_type {
        "URLTest" => Icon::Zap,
        "Fallback" => Icon::Shield,
        "LoadBalance" => Icon::ListChecks,
        _ => Icon::Globe,
    }
}

/// 2-column grid of node cards for one expanded group, using the same
/// row-chunking + filler mechanism as the previous members layout.
fn node_grid<'a>(
    state: &'a AppState,
    group_name: &str,
    members: &'a [String],
) -> Element<'a, Message> {
    let is_active = |member: &str| {
        state
            .runtime
            .proxies
            .get(group_name)
            .and_then(|group| group.now())
            .is_some_and(|now| now == member)
    };

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

/// One node card: name + protocol chips, JetBrains-Mono latency colored by
/// tier. Selected node gets the accent border + soft tint; clicking any
/// other node emits the existing switch Message (`Message::SelectProxy`).
fn node_card<'a>(
    state: &'a AppState,
    group_name: &str,
    member_name: &'a str,
    is_active: bool,
) -> Element<'a, Message> {
    let node = state.runtime.proxies.get(member_name);
    let node_type = node
        .map(|p: &mihomo_api::proxy::Proxy| p.proxy_type().to_string())
        .unwrap_or_default();
    let node_udp = node.map(mihomo_api::proxy::Proxy::udp).unwrap_or(false);
    let delay = node.and_then(|p: &mihomo_api::proxy::Proxy| p.history().last().map(|h| h.delay));

    let mut chips = row![chip(node_type)].spacing(theme::SP_XS);
    if node_udp {
        chips = chips.push(chip("udp"));
    }

    let body = row![
        column![
            text(member_name)
                .size(13)
                .font(theme::FONT_SEMIBOLD)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_primary),
                }),
            chips,
        ]
        .spacing(theme::SP_XS)
        .width(Length::Fill),
        latency_badge(delay),
    ]
    .spacing(theme::SP_SM)
    .align_y(Alignment::Center)
    .width(Length::Fill);

    let mut card_btn = button(container(body).width(Length::Fill).padding([10, 12]))
        .width(Length::FillPortion(1))
        .style(move |t: &Theme, status| {
            let tk = tokens(t);
            if is_active {
                button::Style {
                    background: Some(tk.accent_soft.into()),
                    border: Border {
                        radius: border::Radius::from(theme::R_CONTROL),
                        width: 1.5,
                        color: tk.accent,
                    },
                    ..Default::default()
                }
            } else {
                button::Style {
                    background: match status {
                        button::Status::Hovered | button::Status::Pressed => {
                            Some(tk.control_bg.into())
                        }
                        _ => Some(tk.card_bg.into()),
                    },
                    border: Border {
                        radius: border::Radius::from(theme::R_CONTROL),
                        width: 1.0,
                        color: tk.card_border,
                    },
                    ..Default::default()
                }
            }
        });

    // The currently-selected node is not clickable (same as before); every
    // other node switches the group through the existing Message.
    if !is_active {
        card_btn = card_btn.on_press(Message::SelectProxy(
            group_name.to_string(),
            member_name.to_string(),
        ));
    }

    card_btn.into()
}

/// Compact bordered control group for the delay-test endpoint: small
/// secondary labels in front of tight token-styled inputs. Emits the exact
/// same `UpdateDelayTestUrl` / `UpdateDelayTimeoutMs` messages as before.
fn delay_test_group<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let label = |key: &'static str| {
        text(lang.tr(key)).size(11).style(|t: &Theme| text::Style {
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
                .width(Length::Fixed(230.0))
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
        ]
        .spacing(theme::SP_SM)
        .align_y(Alignment::Center),
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
        border: Border {
            radius: border::Radius::from(theme::R_CONTROL),
            width: 1.0,
            color: tk.card_border,
        },
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
        border: Border {
            radius: border::Radius::from(theme::R_CONTROL),
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
