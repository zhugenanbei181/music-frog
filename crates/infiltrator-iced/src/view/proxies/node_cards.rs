use super::*;
use iced::widget;
use infiltrator_presentation::proxies_page_model::proxy_protocol_chip;
use lurker_core_contract::outbound::preferences::OutboundLayout;

fn favorite_message(state: &AppState, member_name: &str) -> Message {
    member_view(state, member_name)
        .and_then(|node| node.id.clone())
        .map(Message::ToggleFavoriteOutbound)
        .unwrap_or(Message::Noop)
}

fn inspect_message(state: &AppState, member_name: &str) -> Option<Message> {
    member_view(state, member_name)
        .and_then(|node| node.id.clone())
        .map(|id| Message::InspectOutbound(Some(id)))
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
    node_type: Option<String>,
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
            node_type: node.and_then(|node| node.protocol_display_name.clone()),
            udp: node.is_some_and(OutboundMemberView::supports_udp),
            // The typed contract currently reports UDP but has no XUDP field.
            is_xudp: false,
            delay: node.and_then(OutboundMemberView::latency_ms),
            flag: node_flag_emoji(
                node.map(OutboundMemberView::search_name)
                    .unwrap_or(member_name),
            ),
            is_favorite: node.is_some_and(|node| node.favorite),
        }
    }
}

/// 2-column grid of node cards for one expanded group. Column count follows
/// the shared tier operator (1 / 2 / 3 / 4), or a single dense column while the
/// user's compact-view preference is on.
pub(super) fn node_grid<'a>(
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
            .is_some_and(|group| group.current() == Some(member))
    };

    let columns = state
        .shell
        .viewport
        .tier
        .proxy_grid_columns(state.runtime.proxies_model.layout() == OutboundLayout::CompactList)
        .max(1);

    let mut grid = widget::column![].spacing(theme::SP_SM);
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
pub(super) fn node_compact_list<'a>(
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
            .is_some_and(|group| group.current() == Some(member))
    };

    let mut list = widget::column![].spacing(theme::SP_XS);
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

    let mut chips = row![].spacing(theme::SP_XS);
    if let Some(node_type) = &meta.node_type {
        chips = chips.push(chip(proxy_protocol_chip(node_type)));
    }
    if meta.udp {
        chips = chips.push(chip("udp"));
    }
    if meta.is_xudp {
        chips = chips.push(chip("xudp"));
    }

    let star_btn = icon_button(Icon::Pin, 13.0, favorite_message(state, member_name));
    let info_btn: Element<'a, Message> = inspect_message(state, member_name).map_or_else(
        || Space::new().width(0).into(),
        |message| icon_button(Icon::Activity, 13.0, message),
    );
    let active_pill = if is_active {
        active_indicator()
    } else {
        Space::new().width(0).into()
    };

    let body = row![
        text(meta.flag).size(16),
        Space::new().width(theme::SP_XS),
        widget::column![
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

    if !is_active && let Some(message) = state.outbound_selection_message(group_name, member_name) {
        card_btn = card_btn.on_press(message);
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

    let mut chips = row![].spacing(theme::SP_XS);
    if let Some(node_type) = &meta.node_type {
        chips = chips.push(chip(proxy_protocol_chip(node_type)));
    }
    if meta.udp {
        chips = chips.push(chip("udp"));
    }
    if meta.is_xudp {
        chips = chips.push(chip("xudp"));
    }

    let star_btn = icon_button(Icon::Pin, 12.0, favorite_message(state, member_name));
    let info_btn: Element<'a, Message> = inspect_message(state, member_name).map_or_else(
        || Space::new().width(0).into(),
        |message| icon_button(Icon::Activity, 12.0, message),
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

    if !is_active && let Some(message) = state.outbound_selection_message(group_name, member_name) {
        row_btn = row_btn.on_press(message);
    }

    row_btn.into()
}
