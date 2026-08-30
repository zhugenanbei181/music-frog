//! Runtime page delay-test section: sort control, test URL/timeout inputs
//! and the per-node delay list with individual test buttons.

use crate::locales::{Lang, Localizer};
use crate::view::components::{
    chip, empty_state, icon_button, latency_badge, section_header, segmented_control,
};
use crate::view::runtime::styles::{input_style, row_card, style_ghost, text_btn};
use crate::view::svg_icons::Icon;
use crate::view::theme::{self, FONT_SEMIBOLD, MONO, SP_MD, tokens};
use crate::{AppState, Message};
use iced::widget::{Space, column, container, row, text, text_input};
use iced::{Alignment, Element, Length, Theme};

pub(super) fn delay_section<'a>(state: &'a AppState, lang: Lang<'a>) -> Element<'a, Message> {
    let delay_sort_labels: Vec<String> = vec![
        lang.tr("runtime_delay_sort_delay_asc").to_string(),
        lang.tr("runtime_delay_sort_delay_desc").to_string(),
        lang.tr("runtime_delay_sort_name_asc").to_string(),
        lang.tr("runtime_delay_sort_name_desc").to_string(),
    ];
    let delay_sort_index = match state.runtime.proxy_delay_sort.as_str() {
        "delay_desc" => 1,
        "name_asc" => 2,
        "name_desc" => 3,
        _ => 0,
    };
    let delay_sort_control = segmented_control(&delay_sort_labels, delay_sort_index, |index| {
        let key = match index {
            1 => "delay_desc",
            2 => "name_asc",
            3 => "name_desc",
            _ => "delay_asc",
        };
        Message::UpdateProxyDelaySort(key.to_string())
    });

    let delay_testing = state.runtime.runtime_testing_all_delays
        || !state.runtime.runtime_testing_delay_proxy.is_empty();
    let delay_test_all_btn = text_btn(
        if delay_testing {
            lang.tr("runtime_delay_testing_all").to_string()
        } else {
            lang.tr("runtime_delay_test_all").to_string()
        },
        style_ghost,
        (!delay_testing).then_some(Message::TestAllProxyDelays),
    );

    let mut delay_nodes: Vec<(String, String, Option<u32>)> = state
        .runtime
        .proxies
        .iter()
        .filter_map(|(name, proxy)| {
            if proxy.is_group() {
                None
            } else {
                Some((
                    name.clone(),
                    proxy.proxy_type().to_string(),
                    proxy
                        .history()
                        .last()
                        .map(|item| item.delay)
                        .filter(|delay| *delay > 0),
                ))
            }
        })
        .collect();
    delay_nodes.sort_by(|(left_name, _, left_delay), (right_name, _, right_delay)| {
        let compare_delay = |desc: bool| match (left_delay, right_delay) {
            (None, None) => left_name.cmp(right_name),
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (Some(_), None) => std::cmp::Ordering::Less,
            (Some(left), Some(right)) => {
                let base = if desc {
                    right.cmp(left)
                } else {
                    left.cmp(right)
                };
                if base == std::cmp::Ordering::Equal {
                    left_name.cmp(right_name)
                } else {
                    base
                }
            }
        };
        match state.runtime.proxy_delay_sort.as_str() {
            "name_asc" => left_name.cmp(right_name),
            "name_desc" => right_name.cmp(left_name),
            "delay_desc" => compare_delay(true),
            _ => compare_delay(false),
        }
    });

    let mut delay_list = column![].spacing(theme::SP_SM);
    if delay_nodes.is_empty() {
        delay_list = delay_list.push(empty_state(
            Icon::Activity,
            lang.tr("runtime_delay_empty").as_ref(),
            "",
        ));
    } else {
        for (name, proxy_type, delay) in delay_nodes {
            let is_testing = state.runtime.runtime_testing_all_delays
                || state.runtime.runtime_testing_delay_proxy == name;
            let test_button = text_btn(
                if is_testing {
                    lang.tr("runtime_delay_testing_one").to_string()
                } else {
                    lang.tr("runtime_delay_test_one").to_string()
                },
                style_ghost,
                (!is_testing).then_some(Message::TestProxyDelay(name.clone())),
            );

            delay_list = delay_list.push(
                container(
                    row![
                        column![
                            text(name).size(12).font(FONT_SEMIBOLD).style(|t: &Theme| {
                                text::Style {
                                    color: Some(tokens(t).text_primary),
                                }
                            }),
                            chip(proxy_type),
                        ]
                        .spacing(2)
                        .width(Length::Fill),
                        latency_badge(delay),
                        Space::new().width(theme::SP_MD),
                        test_button,
                    ]
                    .align_y(Alignment::Center),
                )
                .padding([theme::SP_SM, SP_MD])
                .width(Length::Fill)
                .style(row_card),
            );
        }
    }

    column![
        section_header(
            lang.tr("runtime_delay_title").as_ref(),
            Some(
                row![
                    delay_sort_control,
                    Space::new().width(theme::SP_SM),
                    icon_button(Icon::RefreshCw, 14.0, Message::LoadProxies),
                    Space::new().width(theme::SP_SM),
                    delay_test_all_btn,
                ]
                .align_y(Alignment::Center)
                .into(),
            ),
        ),
        Space::new().height(theme::SP_MD),
        row![
            text_input(
                lang.tr("runtime_delay_test_url_placeholder").as_ref(),
                &state.runtime.runtime_delay_test_url
            )
            .on_input(Message::UpdateDelayTestUrl)
            .padding([8, 12])
            .size(12)
            .width(Length::Fill)
            .style(input_style),
            Space::new().width(theme::SP_SM),
            text_input(
                lang.tr("runtime_delay_timeout_ms_placeholder").as_ref(),
                &state.runtime.runtime_delay_timeout_ms
            )
            .on_input(Message::UpdateDelayTimeoutMs)
            .padding([8, 12])
            .size(12)
            .font(MONO)
            .width(Length::Fixed(140.0))
            .style(input_style),
        ]
        .align_y(Alignment::Center),
        Space::new().height(theme::SP_MD),
        delay_list,
    ]
    .into()
}
