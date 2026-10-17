//! Runtime page latency section: typed sort control, probe URL/timeout inputs,
//! outbound refresh button, test-all button, and the per-outbound delay list
//! with country flags, latency status dots, mini progress bars, monospace badges,
//! and individual test buttons.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::component_forms::{
    form_input_style, row_card_surface, style_accent, style_ghost, text_btn,
};
use crate::view::components::{chip, empty_state, icon_button, section_header, segmented_control};
use crate::view::proxy_latency::latency_badge;
use crate::view::svg_icons::Icon;
use crate::view::theme::{FONT_MEDIUM, FONT_SEMIBOLD, MONO, SP_MD, tokens};
use crate::view::{svg_icons, theme};
use iced::widget::{Space, button, column, container, row, text, text_input};
use iced::{Alignment, Border, Element, Length, Theme, border};
use infiltrator_contract::runtime_settings::RuntimeDelaySortOrder;
use infiltrator_shared::country_flags::node_flag_emoji;
use infiltrator_shared::locales::{Lang, Localizer};
use lurker_core_contract::outbound::details::OutboundObservation;
use lurker_core_contract::outbound::identity::OutboundId;
use lurker_core_contract::outbound::inventory::OutboundInventoryState;
use lurker_core_contract::outbound::probe::OutboundProbeBatchScope;
use std::cmp::Ordering;
use std::collections::HashSet;

/// Node delay representation: (name, proxy_type, delay_ms).
pub type DelayNodeItem = (String, String, Option<u32>);

/// Sort delay nodes according to user selection.
pub fn sort_delay_nodes(nodes: &mut [DelayNodeItem], sort_order: RuntimeDelaySortOrder) {
    nodes.sort_by(|(left_name, _, left_delay), (right_name, _, right_delay)| {
        let compare_delay = |desc: bool| match (left_delay, right_delay) {
            (None, None) => left_name.cmp(right_name),
            (None, Some(_)) => Ordering::Greater,
            (Some(_), None) => Ordering::Less,
            (Some(left), Some(right)) => {
                let base = if desc {
                    right.cmp(left)
                } else {
                    left.cmp(right)
                };
                if base == Ordering::Equal {
                    left_name.cmp(right_name)
                } else {
                    base
                }
            }
        };
        match sort_order {
            RuntimeDelaySortOrder::NameAscending => left_name.cmp(right_name),
            RuntimeDelaySortOrder::NameDescending => right_name.cmp(left_name),
            RuntimeDelaySortOrder::LatencyDescending => compare_delay(true),
            RuntimeDelaySortOrder::LatencyAscending => compare_delay(false),
        }
    });
}

pub(super) fn delay_section<'a>(state: &'a AppState, lang: Lang<'a>) -> Element<'a, Message> {
    // 1. Sort segmented control: Delay Asc, Delay Desc, Name Asc, Name Desc
    let delay_sort_labels: Vec<String> = vec![
        lang.tr("runtime_delay_sort_delay_asc").to_string(),
        lang.tr("runtime_delay_sort_delay_desc").to_string(),
        lang.tr("runtime_delay_sort_name_asc").to_string(),
        lang.tr("runtime_delay_sort_name_desc").to_string(),
    ];
    let delay_sort_index = match state.runtime.runtime_panel_delay_sort_order {
        RuntimeDelaySortOrder::LatencyAscending => 0,
        RuntimeDelaySortOrder::LatencyDescending => 1,
        RuntimeDelaySortOrder::NameAscending => 2,
        RuntimeDelaySortOrder::NameDescending => 3,
    };
    let delay_sort_control = segmented_control(&delay_sort_labels, delay_sort_index, |index| {
        let sort_order = *RuntimeDelaySortOrder::ALL
            .get(index)
            .expect("segmented control emits an index from its four labels");
        Message::SetRuntimePanelDelaySort(sort_order)
    });

    // 2. Test all delays button
    let delay_testing = state.runtime.outbound_probe_batch.is_running()
        || state.runtime.outbound_probe_pending.is_some();
    let delay_test_all_btn = text_btn(
        if delay_testing {
            lang.tr("runtime_delay_testing_all").to_string()
        } else {
            lang.tr("runtime_delay_test_all").to_string()
        },
        if delay_testing {
            style_ghost
        } else {
            style_accent
        },
        (!delay_testing)
            .then(|| state.outbound_probe_batch_message(OutboundProbeBatchScope::All))
            .flatten(),
    );

    // 3. Extract and sort non-group proxy nodes
    let collection_ids = state
        .runtime
        .outbound_inventory
        .data
        .as_ref()
        .and_then(|inventory| match inventory.inventory() {
            OutboundInventoryState::Ready { collections, .. } => Some(
                collections
                    .iter()
                    .map(|collection| collection.id.clone())
                    .collect::<HashSet<_>>(),
            ),
            _ => None,
        })
        .unwrap_or_default();
    let mut seen = HashSet::<OutboundId>::new();
    let mut delay_nodes: Vec<DelayNodeItem> = state
        .runtime
        .proxy_groups
        .iter()
        .flat_map(|group| &group.proxies)
        .filter_map(|member| {
            let id = member.id.as_ref()?;
            if collection_ids.contains(id)
                || !seen.insert(id.clone())
                || matches!(member.current_latency_ms, OutboundObservation::Unsupported)
            {
                return None;
            }
            Some((
                member.search_name().to_owned(),
                member
                    .protocol_display_name
                    .clone()
                    .unwrap_or_else(|| "Unknown".to_owned()),
                member.latency_ms().filter(|delay| *delay > 0),
            ))
        })
        .collect();

    sort_delay_nodes(
        &mut delay_nodes,
        state.runtime.runtime_panel_delay_sort_order,
    );

    // 4. Build node list
    let mut delay_list = column![].spacing(theme::SP_SM);
    if delay_nodes.is_empty() {
        delay_list = delay_list.push(empty_state(
            Icon::Activity,
            lang.tr("runtime_delay_empty").as_ref(),
            "",
        ));
    } else {
        for (name, proxy_type, delay) in delay_nodes {
            let outbound_id = state
                .runtime
                .proxy_groups
                .iter()
                .flat_map(|group| &group.proxies)
                .find(|member| member.search_name() == name || member.name == name)
                .and_then(|member| member.id.clone());
            let is_testing = state.runtime.outbound_probe_batch.is_running()
                || state.runtime.outbound_probe_pending.is_some()
                    && outbound_id.as_ref() == state.runtime.outbound_inspecting.as_ref();
            let probe_message = outbound_id
                .as_ref()
                .and_then(|id| state.outbound_probe_message(id));
            let test_button = compact_test_btn(
                if is_testing {
                    lang.tr("runtime_delay_testing_one").to_string()
                } else {
                    lang.tr("runtime_delay_test_one").to_string()
                },
                (!is_testing).then_some(probe_message).flatten(),
            );

            let flag = node_flag_emoji(&name);

            let card_content = row![
                text(flag).size(16),
                Space::new().width(theme::SP_SM),
                column![
                    text(name).size(13).font(FONT_SEMIBOLD).style(|t: &Theme| {
                        text::Style {
                            color: Some(tokens(t).text_primary),
                        }
                    }),
                    chip(proxy_type),
                ]
                .spacing(theme::SP_XS)
                .width(Length::Fill),
                row![
                    delay_status_dot(delay),
                    latency_bar(delay),
                    latency_badge(delay, &lang),
                ]
                .spacing(theme::SP_SM)
                .align_y(Alignment::Center),
                Space::new().width(theme::SP_MD),
                test_button,
            ]
            .align_y(Alignment::Center);

            delay_list = delay_list.push(
                container(card_content)
                    .padding([theme::SP_SM, SP_MD])
                    .width(Length::Fill)
                    .style(row_card_surface),
            );
        }
    }

    // 5. Header toolbar: sort control, refresh proxies button, test all button
    let header_trailing = row![
        delay_sort_control,
        Space::new().width(theme::SP_SM),
        icon_button(Icon::RefreshCw, 14.0, Message::RefreshOutboundInventory),
        Space::new().width(theme::SP_SM),
        delay_test_all_btn,
    ]
    .align_y(Alignment::Center);

    // 6. Config input row: delay test URL & timeout ms
    let inputs_row = row![
        svg_icons::icon_themed(Icon::Globe, 14.0, |t: &Theme| tokens(t).text_tertiary),
        Space::new().width(theme::SP_XS),
        text_input(
            lang.tr("runtime_probe_url_draft_placeholder").as_ref(),
            &state.runtime.runtime_probe_url_draft
        )
        .on_input(Message::UpdateRuntimeProbeUrlDraft)
        .padding([8, 12])
        .size(12)
        .width(Length::Fill)
        .style(form_input_style),
        Space::new().width(theme::SP_MD),
        svg_icons::icon_themed(Icon::Activity, 14.0, |t: &Theme| tokens(t).text_tertiary),
        Space::new().width(theme::SP_XS),
        text_input(
            lang.tr("runtime_probe_timeout_draft_placeholder").as_ref(),
            &state.runtime.runtime_probe_timeout_draft
        )
        .on_input(Message::UpdateRuntimeProbeTimeoutDraft)
        .padding([8, 12])
        .size(12)
        .font(MONO)
        .width(Length::Fixed(140.0))
        .style(form_input_style),
    ]
    .align_y(Alignment::Center);

    column![
        section_header(
            lang.tr("runtime_delay_title").as_ref(),
            Some(header_trailing.into()),
        ),
        Space::new().height(theme::SP_MD),
        inputs_row,
        Space::new().height(theme::SP_MD),
        delay_list,
    ]
    .into()
}

/// Mini circular status dot colored according to latency quality.
pub fn delay_status_dot<'a, Message: 'a>(delay: Option<u32>) -> Element<'a, Message> {
    container(Space::new().width(8).height(8))
        .style(move |t: &Theme| {
            let color = theme::latency_color(tokens(t), delay);
            container::Style {
                background: Some(color.into()),
                border: Border {
                    radius: border::Radius::from(theme::R_XS),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .into()
}

/// Mini horizontal latency/progress bar visualizing relative response time.
pub fn latency_bar<'a, Message: 'a>(delay: Option<u32>) -> Element<'a, Message> {
    const TOTAL_WIDTH: f32 = 56.0;
    const BAR_HEIGHT: f32 = 4.0;
    let fill_width = match delay {
        Some(ms) => (TOTAL_WIDTH * (ms as f32 / 800.0).clamp(0.08, 1.0)).max(4.0),
        None => 0.0,
    };

    let filled: Element<'a, Message> = if fill_width > 0.0 {
        container(
            Space::new()
                .width(Length::Fixed(fill_width))
                .height(Length::Fixed(BAR_HEIGHT)),
        )
        .style(move |t: &Theme| {
            let color = theme::latency_color(tokens(t), delay);
            container::Style {
                background: Some(color.into()),
                border: Border {
                    radius: border::Radius::from(theme::R_PILL),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .into()
    } else {
        Space::new().width(0).height(0).into()
    };

    container(
        row![filled, Space::new().width(Length::Fill)]
            .width(Length::Fixed(TOTAL_WIDTH))
            .height(Length::Fixed(BAR_HEIGHT)),
    )
    .width(Length::Fixed(TOTAL_WIDTH))
    .height(Length::Fixed(BAR_HEIGHT))
    .style(|t: &Theme| {
        let tk = tokens(t);
        container::Style {
            background: Some(tk.chip_bg.into()),
            border: Border {
                radius: border::Radius::from(theme::R_PILL),
                ..Default::default()
            },
            ..Default::default()
        }
    })
    .into()
}

/// Compact button suited for per-row action items.
fn compact_test_btn<'a, Message: 'a + Clone>(
    label: impl Into<String>,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    button(text(label.into()).size(11).font(FONT_MEDIUM))
        .padding([4, 10])
        .style(style_ghost)
        .on_press_maybe(on_press)
        .into()
}

#[cfg(test)]
#[path = "../../../tests/gui/view_runtime_delay_tests.rs"]
mod tests;
