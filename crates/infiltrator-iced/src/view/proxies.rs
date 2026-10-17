//! Proxies page (代理组与节点) — Clash-Party-style proxy-group cards with
//! expandable node grids, regional country flag emojis, alive-only filtering,
//! favorite pinning, and node detail inspection.

use crate::state::AppState;
use crate::state::outbound_inventory::{
    OutboundGroupView, OutboundMemberView, OutboundSelectionViewState,
};
use crate::state::proxy_probe::OutboundProbeBatchState;
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
use lurker_core_contract::outbound::details::OutboundCollectionKind;
use lurker_core_contract::outbound::details::OutboundObservation;
use lurker_core_contract::outbound::{
    inventory::{OutboundInventoryState, SelectionAvailability},
    probe::{OutboundProbeBatchOutcome, OutboundProbeBatchScope},
};
use lurker_core_contract::page::PageStatus;

pub mod node_cards;
use node_cards::{node_compact_list, node_grid};

pub fn view(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);

    // ------------------------------------------------------------------
    // Header: section title with trailing ghost actions
    // ------------------------------------------------------------------
    let test_all_btn: Element<'_, Message> = if state.runtime.outbound_probe_batch.is_running() {
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
        .on_press_maybe(state.outbound_probe_batch_message(OutboundProbeBatchScope::All))
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
                icon_button(Icon::RefreshCw, 15.0, Message::RefreshOutboundInventory),
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
                lang.tr("core_not_running").as_ref(),
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
        let (title, detail) = outbound_empty_copy(state, &lang);
        groups_col = groups_col.push(
            container(empty_state(Icon::Globe, &title, &detail))
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

        let group_type = outbound_group_status(group_info, &lang);

        let icon_tile = container(svg_icons::icon_themed(
            outbound_group_icon(group_info),
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

        let subtitle = group_info.current().map_or_else(
            || group_type.clone(),
            |current| format!("{group_type} · {current}"),
        );

        let total_count = group_info.proxies.len();

        let test_group_btn: Element<'_, Message> =
            if state.runtime.outbound_probe_batch.is_running() {
                container(svg_icons::icon_themed(Icon::Target, 15.0, |t: &Theme| {
                    tokens(t).text_tertiary
                }))
                .padding(6)
                .into()
            } else {
                icon_button(
                    Icon::Target,
                    15.0,
                    group_info
                        .id
                        .clone()
                        .and_then(|id| {
                            state.outbound_probe_batch_message(OutboundProbeBatchScope::Collection(
                                id,
                            ))
                        })
                        .unwrap_or(Message::Noop),
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
                group_info
                    .id
                    .clone()
                    .map(Message::MoveProxyGroupUp)
                    .unwrap_or(Message::Noop),
            ),
            icon_button(
                Icon::ArrowDown,
                12.0,
                group_info
                    .id
                    .clone()
                    .map(Message::MoveProxyGroupDown)
                    .unwrap_or(Message::Noop),
            ),
            test_group_btn,
            icon_button(
                chevron_icon,
                15.0,
                group_info
                    .id
                    .clone()
                    .map(Message::ToggleOutboundGroupExpanded)
                    .unwrap_or(Message::Noop),
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
    let mut body = groups_col;
    if let Some(batch_card) = outbound_probe_batch_card(state) {
        body = body
            .push(Space::new().height(theme::SP_MD))
            .push(batch_card);
    }
    let body = body
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

fn outbound_probe_batch_card(state: &AppState) -> Option<Element<'_, Message>> {
    let close = icon_button(Icon::X, 13.0, Message::DismissOutboundProbeBatch);
    let card = match &state.runtime.outbound_probe_batch {
        OutboundProbeBatchState::Idle => return None,
        OutboundProbeBatchState::Running { request, .. } => {
            let title = match request.target_scope() {
                OutboundProbeBatchScope::All => "Probing all observed outbounds".to_owned(),
                OutboundProbeBatchScope::Collection(id) => {
                    format!("Probing collection {}", id.value())
                }
                OutboundProbeBatchScope::One(id) => format!("Probing outbound {}", id.value()),
                OutboundProbeBatchScope::Selected(ids) => {
                    format!("Probing {} selected outbounds", ids.len())
                }
            };
            container(
                row![
                    text(title).size(13),
                    Space::new().width(Length::Fill),
                    close,
                ]
                .align_y(Alignment::Center),
            )
        }
        OutboundProbeBatchState::Ready {
            request, snapshot, ..
        } => {
            let title = match request.target_scope() {
                OutboundProbeBatchScope::All => "Probe results · all observed outbounds".to_owned(),
                OutboundProbeBatchScope::Collection(id) => {
                    format!("Probe results · {}", id.value())
                }
                OutboundProbeBatchScope::One(id) => format!("Probe results · {}", id.value()),
                OutboundProbeBatchScope::Selected(ids) => {
                    format!("Probe results · {} selected outbounds", ids.len())
                }
            };
            let mut rows = column![].spacing(theme::SP_XS);
            for item in snapshot.items() {
                let name = state
                    .runtime
                    .outbound_inventory
                    .data
                    .as_ref()
                    .and_then(|inventory| match inventory.inventory() {
                        OutboundInventoryState::Ready { entries, .. } => entries
                            .iter()
                            .find(|entry| entry.id == item.outbound_id)
                            .map(|entry| {
                                entry
                                    .display_name
                                    .clone()
                                    .unwrap_or_else(|| item.outbound_id.value().to_owned())
                            }),
                        _ => None,
                    })
                    .unwrap_or_else(|| item.outbound_id.value().to_owned());
                let result = match &item.outcome {
                    OutboundProbeBatchOutcome::Measured { delay_ms } => {
                        format!("{name}: {delay_ms} ms")
                    }
                    OutboundProbeBatchOutcome::TimedOut => format!("{name}: timed out"),
                    OutboundProbeBatchOutcome::Unsupported => format!("{name}: unsupported"),
                    OutboundProbeBatchOutcome::Failed { failure } => {
                        format!("{name}: {}", failure.message)
                    }
                };
                rows = rows.push(text(result).size(12));
            }
            container(
                column![
                    row![
                        text(title).size(13),
                        Space::new().width(Length::Fill),
                        close
                    ]
                    .align_y(Alignment::Center),
                    rows,
                ]
                .spacing(theme::SP_SM),
            )
        }
        OutboundProbeBatchState::Failed {
            request, failure, ..
        } => {
            let retry = state.outbound_probe_batch_message(request.target_scope().clone());
            let retry_button = button(text("Retry").size(12))
                .padding([5, 10])
                .on_press_maybe(retry);
            container(
                row![
                    text(failure.message.clone()).size(12),
                    Space::new().width(Length::Fill),
                    retry_button,
                    close,
                ]
                .spacing(theme::SP_SM)
                .align_y(Alignment::Center),
            )
        }
    };
    Some(
        card.width(Length::Fill)
            .padding(theme::SP_MD)
            .style(card_surface)
            .into(),
    )
}

fn outbound_empty_copy(state: &AppState, lang: &Lang<'_>) -> (String, String) {
    let observation = &state.runtime.outbound_inventory;
    match (&observation.status, observation.data.as_ref()) {
        (PageStatus::Loading, _) => (
            lang.tr("outbound_inventory_loading_title").into_owned(),
            lang.tr("outbound_inventory_loading_detail").into_owned(),
        ),
        (PageStatus::Unavailable { failure }, _) => (
            lang.tr("outbound_inventory_unavailable_title").into_owned(),
            failure.message.clone(),
        ),
        (PageStatus::Failed { failure }, _) => (
            lang.tr("outbound_inventory_failed_title").into_owned(),
            failure.message.clone(),
        ),
        (PageStatus::Empty, _) => (
            lang.tr("outbound_inventory_empty_title").into_owned(),
            lang.tr("outbound_inventory_empty_detail").into_owned(),
        ),
        (PageStatus::Ready, Some(snapshot)) => match snapshot.inventory() {
            OutboundInventoryState::Unknown => (
                lang.tr("outbound_inventory_unknown_title").into_owned(),
                lang.tr("outbound_inventory_unknown_detail").into_owned(),
            ),
            OutboundInventoryState::Empty => (
                lang.tr("outbound_inventory_empty_title").into_owned(),
                lang.tr("outbound_inventory_empty_detail").into_owned(),
            ),
            OutboundInventoryState::Unsupported => (
                lang.tr("outbound_inventory_unsupported_title").into_owned(),
                lang.tr("outbound_inventory_unsupported_detail")
                    .into_owned(),
            ),
            OutboundInventoryState::Failed { failure } => (
                lang.tr("outbound_inventory_failed_title").into_owned(),
                failure.message.clone(),
            ),
            OutboundInventoryState::Ready { collections, .. } if collections.is_empty() => (
                lang.tr("outbound_inventory_no_collections_title")
                    .into_owned(),
                lang.tr("outbound_inventory_no_collections_detail")
                    .into_owned(),
            ),
            OutboundInventoryState::Ready { .. }
                if !state.runtime.proxy_filter.trim().is_empty() =>
            {
                (
                    lang.tr("outbound_inventory_search_empty").into_owned(),
                    String::new(),
                )
            }
            OutboundInventoryState::Ready { .. } => (
                lang.tr("outbound_inventory_no_selectable_title")
                    .into_owned(),
                String::new(),
            ),
        },
        (PageStatus::Ready, None) => (
            lang.tr("outbound_inventory_unavailable_title").into_owned(),
            lang.tr("outbound_inventory_missing_detail").into_owned(),
        ),
    }
}

fn outbound_group_status(group: &OutboundGroupView, lang: &Lang<'_>) -> String {
    if let OutboundObservation::Observed(kind) = &group.collection_kind {
        return match kind {
            OutboundCollectionKind::ManualSelection => {
                lang.tr("outbound_availability_selectable").into_owned()
            }
            OutboundCollectionKind::AutomaticSelection => {
                lang.tr("outbound_availability_automatic").into_owned()
            }
            OutboundCollectionKind::Failover => "Failover".to_owned(),
            OutboundCollectionKind::LoadBalancing => "Load balancing".to_owned(),
            OutboundCollectionKind::Relay => "Relay".to_owned(),
        };
    }
    let availability = match group.availability {
        SelectionAvailability::Selectable => lang.tr("outbound_availability_selectable"),
        SelectionAvailability::ReadOnly => lang.tr("outbound_availability_read_only"),
        SelectionAvailability::Automatic => lang.tr("outbound_availability_automatic"),
        SelectionAvailability::Unsupported => lang.tr("outbound_availability_unsupported"),
        SelectionAvailability::Unknown => lang.tr("outbound_availability_unknown"),
    };
    let selection_state = match &group.selection {
        OutboundSelectionViewState::Unknown => {
            Some(lang.tr("outbound_selection_unknown").into_owned())
        }
        OutboundSelectionViewState::Selected(_) => None,
        OutboundSelectionViewState::Empty => Some(lang.tr("outbound_selection_empty").into_owned()),
        OutboundSelectionViewState::Unsupported => {
            Some(lang.tr("outbound_selection_unsupported").into_owned())
        }
        OutboundSelectionViewState::Failed(failure) => Some(failure.message.clone()),
    };
    let availability = availability.into_owned();
    selection_state.map_or(availability.clone(), |selection| {
        format!("{availability} · {selection}")
    })
}

fn outbound_group_icon(group: &OutboundGroupView) -> Icon {
    use lurker_core_contract::outbound::details::{OutboundCollectionKind, OutboundObservation};
    match &group.collection_kind {
        OutboundObservation::Observed(OutboundCollectionKind::AutomaticSelection) => Icon::Zap,
        OutboundObservation::Observed(OutboundCollectionKind::Failover) => Icon::Shield,
        OutboundObservation::Observed(OutboundCollectionKind::LoadBalancing) => Icon::ListChecks,
        OutboundObservation::Observed(OutboundCollectionKind::ManualSelection)
        | OutboundObservation::Observed(OutboundCollectionKind::Relay)
        | OutboundObservation::Unknown
        | OutboundObservation::Unsupported => Icon::Globe,
    }
}

fn member_view<'a>(state: &'a AppState, member_name: &str) -> Option<&'a OutboundMemberView> {
    state
        .runtime
        .proxy_groups
        .iter()
        .flat_map(|group| &group.proxies)
        .find(|node| node.name == member_name)
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
