use crate::locales::{Lang, Localizer};
use crate::view::components::{card, modern_scrollable};
use crate::view::icons;
use crate::{AppState, Message};
use iced::widget::{Space, button, column, container, row, text, text_input};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};

pub fn view(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.lang);
    let bold_font = iced::Font {
        weight: iced::font::Weight::Bold,
        ..Default::default()
    };

    let search_bar = row![
        text_input(
            lang.tr("rules_filter_placeholder").as_ref(),
            &state.proxy_filter
        )
        .on_input(Message::FilterProxies)
        .padding(10)
        .size(14)
        .width(Length::Fixed(240.0)),
        if !state.proxy_filter.is_empty() {
            button(text(icons::CLOSE).size(12))
                .on_press(Message::FilterProxies(String::new()))
                .padding([10, 14])
                .style(button::secondary)
        } else {
            button(text(" ").size(12))
                .padding([10, 14])
                .style(button::secondary)
        }
    ]
    .spacing(5)
    .align_y(Alignment::Center);

    let header = row![
        text(lang.tr("proxies_title")).size(28).font(bold_font),
        Space::new().width(30),
        search_bar,
        Space::new().width(Length::Fill),
    ]
    .align_y(Alignment::Center);

    let sort_delay_asc_btn = if state.proxy_delay_sort == "delay_asc" {
        button(text(lang.tr("runtime_delay_sort_delay_asc")).size(11))
            .padding([6, 10])
            .style(button::primary)
    } else {
        button(text(lang.tr("runtime_delay_sort_delay_asc")).size(11))
            .on_press(Message::UpdateProxyDelaySort("delay_asc".to_string()))
            .padding([6, 10])
            .style(button::secondary)
    };
    let sort_delay_desc_btn = if state.proxy_delay_sort == "delay_desc" {
        button(text(lang.tr("runtime_delay_sort_delay_desc")).size(11))
            .padding([6, 10])
            .style(button::primary)
    } else {
        button(text(lang.tr("runtime_delay_sort_delay_desc")).size(11))
            .on_press(Message::UpdateProxyDelaySort("delay_desc".to_string()))
            .padding([6, 10])
            .style(button::secondary)
    };
    let sort_name_asc_btn = if state.proxy_delay_sort == "name_asc" {
        button(text(lang.tr("runtime_delay_sort_name_asc")).size(11))
            .padding([6, 10])
            .style(button::primary)
    } else {
        button(text(lang.tr("runtime_delay_sort_name_asc")).size(11))
            .on_press(Message::UpdateProxyDelaySort("name_asc".to_string()))
            .padding([6, 10])
            .style(button::secondary)
    };
    let sort_name_desc_btn = if state.proxy_delay_sort == "name_desc" {
        button(text(lang.tr("runtime_delay_sort_name_desc")).size(11))
            .padding([6, 10])
            .style(button::primary)
    } else {
        button(text(lang.tr("runtime_delay_sort_name_desc")).size(11))
            .on_press(Message::UpdateProxyDelaySort("name_desc".to_string()))
            .padding([6, 10])
            .style(button::secondary)
    };

    let test_all_btn: Element<'_, Message> = if state.runtime_testing_all_delays {
        button(text(lang.tr("runtime_delay_testing_all")).size(12))
            .padding([8, 14])
            .style(button::secondary)
            .into()
    } else {
        button(
            row![
                text(icons::SPEED).size(12),
                text(lang.tr("runtime_delay_test_all")).size(12),
            ]
            .spacing(8),
        )
        .on_press(Message::TestAllProxyDelays)
        .padding([8, 14])
        .style(button::secondary)
        .into()
    };

    let control_row = row![
        text(lang.tr("runtime_delay_sort")).size(12),
        Space::new().width(8),
        sort_delay_asc_btn,
        sort_delay_desc_btn,
        sort_name_asc_btn,
        sort_name_desc_btn,
        Space::new().width(Length::Fill),
        text_input(
            lang.tr("runtime_delay_test_url_placeholder").as_ref(),
            &state.runtime_delay_test_url,
        )
        .on_input(Message::UpdateDelayTestUrl)
        .padding([8, 10])
        .size(12)
        .width(Length::FillPortion(2)),
        Space::new().width(8),
        text_input(
            lang.tr("runtime_delay_timeout_ms_placeholder").as_ref(),
            &state.runtime_delay_timeout_ms,
        )
        .on_input(Message::UpdateDelayTimeoutMs)
        .padding([8, 10])
        .size(12)
        .width(Length::Fixed(120.0)),
        Space::new().width(8),
        button(
            row![
                text(icons::REFRESH).size(14),
                text(lang.tr("refresh")).size(14)
            ]
            .spacing(10)
        )
        .on_press(Message::LoadProxies)
        .padding([8, 14])
        .style(button::secondary),
        Space::new().width(8),
        test_all_btn,
    ]
    .align_y(Alignment::Center);

    if state.runtime.is_none() {
        return column![
            header,
            Space::new().height(12),
            control_row,
            Space::new().height(40),
            card(text(lang.tr("proxy_not_running")))
        ]
        .into();
    }

    let mut groups_col = column![].spacing(30);

    for (group_name, members) in &state.filtered_groups {
        let Some(group_info) = state.proxies.get(group_name) else {
            continue;
        };

        let group_test_btn: Element<'_, Message> = if state.runtime_testing_all_delays {
            button(text(icons::SPEED).size(14))
                .padding([6, 12])
                .style(button::secondary)
                .into()
        } else {
            button(text(icons::SPEED).size(14))
                .on_press(Message::TestGroupDelay(group_name.clone()))
                .padding([6, 12])
                .style(button::secondary)
                .into()
        };

        let group_header = row![
            text(group_name).font(bold_font).size(20),
            Space::new().width(12),
            container(text(group_info.proxy_type().to_string()).size(10))
                .padding([2, 8])
                .style(|_| container::Style {
                    background: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.05).into()),
                    border: Border {
                        radius: border::Radius::from(4.0),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
            Space::new().width(Length::Fill),
            text(format!("{} nodes", members.len()))
                .size(12)
                .style(|_| text::Style {
                    color: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.3))
                }),
            Space::new().width(15),
            group_test_btn,
        ]
        .align_y(Alignment::Center);

        let mut members_col = column![].spacing(10);
        let mut members_row = row![].spacing(10);

        let mut i = 0;
        for member_name in members {
            let is_active = group_info.now() == Some(member_name);
            let delay = state
                .proxies
                .get(member_name)
                .and_then(|p: &mihomo_api::Proxy| p.history().last().map(|h| h.delay));
            let m_name = member_name.clone();

            let mut btn = button(
                row![
                    text(member_name).size(14).width(Length::Fill),
                    if let Some(d) = delay {
                        let color = if d < 200 {
                            Color::from_rgb(0.4, 0.8, 0.4)
                        } else if d < 500 {
                            Color::from_rgb(0.8, 0.8, 0.4)
                        } else {
                            Color::from_rgb(0.8, 0.4, 0.4)
                        };
                        text(format!("{}ms", d))
                            .size(11)
                            .style(move |_: &Theme| text::Style { color: Some(color) })
                    } else {
                        text("").size(11)
                    }
                ]
                .align_y(Alignment::Center),
            )
            .width(Length::FillPortion(1))
            .padding(12);

            if is_active {
                btn = btn.style(button::primary);
            } else {
                btn = btn
                    .style(button::secondary)
                    .on_press(Message::SelectProxy(group_name.clone(), m_name));
            }

            members_row = members_row.push(btn);
            i += 1;
            if i % 3 == 0 {
                members_col = members_col.push(members_row);
                members_row = row![].spacing(10);
            }
        }

        if i % 3 != 0 {
            for _ in 0..(3 - (i % 3)) {
                members_row = members_row.push(Space::new().width(Length::FillPortion(1)));
            }
            members_col = members_col.push(members_row);
        }

        groups_col = groups_col.push(card(column![
            group_header,
            Space::new().height(15),
            members_col
        ]));
    }

    let content = column![
        header,
        Space::new().height(12),
        control_row,
        Space::new().height(20),
        modern_scrollable(groups_col).height(Length::Fill)
    ];

    content.into()
}
