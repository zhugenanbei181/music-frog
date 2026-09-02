//! Runtime page delay-test section: sort control, test URL/timeout inputs,
//! refresh proxies button, test all delays button, and the per-node delay list
//! with country flags, latency status dots, mini progress bars, monospace badges,
//! and individual test buttons.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::{
    chip, empty_state, form_input_style, icon_button, latency_badge, row_card_surface,
    section_header, segmented_control, style_accent, style_ghost, text_btn,
};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, MONO, SP_MD, tokens};
use iced::widget::{Space, button, column, container, row, text, text_input};
use iced::{Alignment, Border, Element, Length, Theme, border};
use infiltrator_shared::country_flags::node_flag_emoji;
use infiltrator_shared::locales::{Lang, Localizer};

/// Node delay representation: (name, proxy_type, delay_ms).
pub type DelayNodeItem = (String, String, Option<u32>);

/// Sort delay nodes according to user selection.
pub fn sort_delay_nodes(nodes: &mut [DelayNodeItem], sort_key: &str) {
    nodes.sort_by(|(left_name, _, left_delay), (right_name, _, right_delay)| {
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
        match sort_key {
            "name_asc" => left_name.cmp(right_name),
            "name_desc" => right_name.cmp(left_name),
            "delay_desc" => compare_delay(true),
            _ => compare_delay(false),
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

    // 2. Test all delays button
    let delay_testing = state.runtime.runtime_testing_all_delays
        || !state.runtime.runtime_testing_delay_proxy.is_empty();
    let delay_test_all_btn = text_btn(
        if delay_testing {
            lang.tr("runtime_delay_testing_all").to_string()
        } else {
            lang.tr("runtime_delay_test_all").to_string()
        },
        if delay_testing { style_ghost } else { style_accent },
        (!delay_testing).then_some(Message::TestAllProxyDelays),
    );

    // 3. Extract and sort non-group proxy nodes
    let mut delay_nodes: Vec<DelayNodeItem> = state
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

    sort_delay_nodes(&mut delay_nodes, state.runtime.proxy_delay_sort.as_str());

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
            let is_testing = state.runtime.runtime_testing_all_delays
                || state.runtime.runtime_testing_delay_proxy == name;
            let test_button = compact_test_btn(
                if is_testing {
                    lang.tr("runtime_delay_testing_one").to_string()
                } else {
                    lang.tr("runtime_delay_test_one").to_string()
                },
                (!is_testing).then_some(Message::TestProxyDelay(name.clone())),
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
                    latency_badge(delay),
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
        icon_button(Icon::RefreshCw, 14.0, Message::LoadProxies),
        Space::new().width(theme::SP_SM),
        delay_test_all_btn,
    ]
    .align_y(Alignment::Center);

    // 6. Config input row: delay test URL & timeout ms
    let inputs_row = row![
        svg_icons::icon_themed(Icon::Globe, 14.0, |t: &Theme| tokens(t).text_tertiary),
        Space::new().width(theme::SP_XS),
        text_input(
            lang.tr("runtime_delay_test_url_placeholder").as_ref(),
            &state.runtime.runtime_delay_test_url
        )
        .on_input(Message::UpdateDelayTestUrl)
        .padding([8, 12])
        .size(12)
        .width(Length::Fill)
        .style(form_input_style),
        Space::new().width(theme::SP_MD),
        svg_icons::icon_themed(Icon::Activity, 14.0, |t: &Theme| tokens(t).text_tertiary),
        Space::new().width(theme::SP_XS),
        text_input(
            lang.tr("runtime_delay_timeout_ms_placeholder").as_ref(),
            &state.runtime.runtime_delay_timeout_ms
        )
        .on_input(Message::UpdateDelayTimeoutMs)
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
                    radius: border::Radius::from(4.0),
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
                    radius: border::Radius::from(2.0),
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
                radius: border::Radius::from(2.0),
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
mod tests {
    use super::*;

    #[test]
    fn test_sort_delay_nodes() {
        let mut nodes: Vec<DelayNodeItem> = vec![
            ("Node B".to_string(), "Shadowsocks".to_string(), Some(150)),
            ("Node A".to_string(), "VMess".to_string(), Some(50)),
            ("Node C".to_string(), "Trojan".to_string(), None),
            ("Node D".to_string(), "Hysteria2".to_string(), Some(300)),
        ];

        sort_delay_nodes(&mut nodes, "delay_asc");
        assert_eq!(nodes[0].0, "Node A");
        assert_eq!(nodes[1].0, "Node B");
        assert_eq!(nodes[2].0, "Node D");
        assert_eq!(nodes[3].0, "Node C");

        sort_delay_nodes(&mut nodes, "delay_desc");
        assert_eq!(nodes[0].0, "Node D");
        assert_eq!(nodes[1].0, "Node B");
        assert_eq!(nodes[2].0, "Node A");
        assert_eq!(nodes[3].0, "Node C");

        sort_delay_nodes(&mut nodes, "name_asc");
        assert_eq!(nodes[0].0, "Node A");
        assert_eq!(nodes[1].0, "Node B");
        assert_eq!(nodes[2].0, "Node C");
        assert_eq!(nodes[3].0, "Node D");

        sort_delay_nodes(&mut nodes, "name_desc");
        assert_eq!(nodes[0].0, "Node D");
        assert_eq!(nodes[1].0, "Node C");
        assert_eq!(nodes[2].0, "Node B");
        assert_eq!(nodes[3].0, "Node A");
    }

    #[test]
    fn test_latency_bar_and_status_dot() {
        let _dot_some: Element<'_, Message> = delay_status_dot(Some(100));
        let _dot_none: Element<'_, Message> = delay_status_dot(None);

        let _bar_some: Element<'_, Message> = latency_bar(Some(200));
        let _bar_none: Element<'_, Message> = latency_bar(None);
    }
}
