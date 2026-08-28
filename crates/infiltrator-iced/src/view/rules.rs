use crate::locales::{Lang, Localizer};
use crate::types::{EditorLazyState, RuleBadgeKind, RulesJsonTab, RulesTab};
use crate::view::components::{card, modern_scrollable};
use crate::view::icons;
use crate::{AppState, Message};
use iced::widget::{
    Space, button, checkbox, column, container, pick_list, row, text, text_editor, text_input,
};
use iced::{Alignment, Border, Color, Element, Font, Length, Theme, border};

fn tab_button<'a>(
    label: String,
    active: bool,
    on_press: Message,
) -> iced::widget::Button<'a, Message> {
    button(text(label).size(12))
        .padding([6, 12])
        .style(if active {
            button::primary
        } else {
            button::secondary
        })
        .on_press(on_press)
}

fn save_action<'a>(
    dirty: bool,
    saving: bool,
    icon: &'a str,
    label: String,
    on_press: Message,
) -> Element<'a, Message> {
    if saving {
        button(text("Saving...").size(12))
            .padding([6, 12])
            .style(button::secondary)
            .into()
    } else if dirty {
        button(row![text(icon).size(12), text(label).size(12)].spacing(8))
            .padding([6, 12])
            .style(button::primary)
            .on_press(on_press)
            .into()
    } else {
        button(text("Saved").size(12))
            .padding([6, 12])
            .style(button::secondary)
            .into()
    }
}

fn rule_badge_style(kind: RuleBadgeKind) -> container::Style {
    let color = match kind {
        RuleBadgeKind::Domain => Color::from_rgb(0.2, 0.5, 0.8),
        RuleBadgeKind::Ip => Color::from_rgb(0.8, 0.5, 0.2),
        RuleBadgeKind::Other => Color::from_rgb(0.5, 0.5, 0.5),
    };
    container::Style {
        background: Some(color.into()),
        border: Border {
            radius: 4.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn editor_lazy_placeholder<'a>(title: String, on_press: Message) -> Element<'a, Message> {
    card(
        column![
            text(title).size(14),
            text("Editor will load on demand").size(12),
            button(text("Load Editor").size(12))
                .padding([6, 12])
                .style(button::secondary)
                .on_press(on_press)
        ]
        .spacing(10),
    )
}

pub fn view(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.lang);
    let bold_font = Font {
        weight: iced::font::Weight::Bold,
        ..Default::default()
    };

    let filtered_count = state.rules_filtered_indices.len();
    let save_rules_action = save_action(
        state.rules_dirty,
        state.is_saving_rules,
        icons::SAVE,
        lang.tr("rules_save_btn").to_string(),
        Message::SaveRules,
    );

    let header = row![
        text(lang.tr("rules_title")).size(24).font(bold_font),
        Space::new().width(10),
        text(format!("({} / {})", filtered_count, state.rules.len()))
            .size(14)
            .style(|_| text::Style {
                color: Some(Color::from_rgb(0.5, 0.5, 0.5))
            }),
        Space::new().width(Length::Fill),
        if state.is_loading_rules || state.is_loading_providers {
            Element::from(text("..."))
        } else {
            button(
                row![
                    text(icons::REFRESH).size(12),
                    text(lang.tr("refresh")).size(12)
                ]
                .spacing(8),
            )
            .on_press(Message::LoadRules)
            .padding([6, 12])
            .style(button::secondary)
            .into()
        },
        Space::new().width(8),
        save_rules_action
    ]
    .align_y(Alignment::Center);

    let tabs = row![
        tab_button(
            "Rules List".to_string(),
            state.rules_tab == RulesTab::RulesList,
            Message::SetRulesTab(RulesTab::RulesList)
        ),
        tab_button(
            "Providers".to_string(),
            state.rules_tab == RulesTab::Providers,
            Message::SetRulesTab(RulesTab::Providers)
        ),
        tab_button(
            "JSON Editors".to_string(),
            state.rules_tab == RulesTab::JsonEditors,
            Message::SetRulesTab(RulesTab::JsonEditors)
        ),
    ]
    .spacing(8);

    if !state.rules_heavy_ready {
        return column![
            header,
            Space::new().height(14),
            tabs,
            Space::new().height(16),
            card(
                column![
                    text("Preparing Rules panels...").font(bold_font),
                    text("Heavy widgets mount asynchronously to keep first paint responsive.")
                        .size(12)
                ]
                .spacing(8)
            )
        ]
        .spacing(10)
        .into();
    }

    let mut available_targets: Vec<String> = state
        .proxies
        .iter()
        .filter(|(_, p): &(&String, &mihomo_api::Proxy)| p.is_group())
        .map(|(name, _)| name.clone())
        .collect();
    available_targets.sort();
    if !available_targets.contains(&"DIRECT".to_string()) {
        available_targets.push("DIRECT".to_string());
    }
    if !available_targets.contains(&"REJECT".to_string()) {
        available_targets.push("REJECT".to_string());
    }

    let rule_types = vec![
        "DOMAIN".to_string(),
        "DOMAIN-SUFFIX".to_string(),
        "DOMAIN-KEYWORD".to_string(),
        "IP-CIDR".to_string(),
        "IP-CIDR6".to_string(),
        "GEOIP".to_string(),
        "MATCH".to_string(),
    ];

    let add_rule_form = card(column![
        text(lang.tr("rules_add_custom")).font(bold_font),
        Space::new().height(15),
        row![
            column![
                text(lang.tr("rules_type")).size(12),
                pick_list(
                    rule_types,
                    Some(&state.new_rule_type),
                    Message::UpdateNewRuleType
                )
                .width(Length::Fill),
            ]
            .width(Length::FillPortion(1))
            .spacing(5),
            Space::new().width(15),
            column![
                text(lang.tr("rules_payload")).size(12),
                text_input("e.g. google.com", &state.new_rule_payload)
                    .on_input(Message::UpdateNewRulePayload)
                    .padding(8),
            ]
            .width(Length::FillPortion(2))
            .spacing(5),
            Space::new().width(15),
            column![
                text(lang.tr("rules_target")).size(12),
                pick_list(available_targets, Some(&state.new_rule_target), |t| {
                    Message::UpdateNewRuleTarget(t)
                })
                .width(Length::Fill),
            ]
            .width(Length::FillPortion(1))
            .spacing(5),
            Space::new().width(15),
            column![
                text(" ").size(12),
                button(
                    row![
                        text(icons::ADD).size(12),
                        text(lang.tr("rules_add_btn")).size(12)
                    ]
                    .spacing(8)
                )
                .on_press(Message::AddCustomRule)
                .padding([8, 16])
                .style(if state.is_adding_rule {
                    button::secondary
                } else {
                    button::primary
                }),
            ]
            .spacing(5),
        ]
        .align_y(Alignment::Center)
    ]);

    let rules_list_view = {
        let search_bar = text_input(&lang.tr("rules_filter_placeholder"), &state.rules_filter)
            .on_input(Message::FilterRules)
            .padding(10)
            .size(16);
        let page_size = state.rules_page_size.max(1);
        let total_pages = if state.rules_filtered_indices.is_empty() {
            1
        } else {
            (state.rules_filtered_indices.len() - 1) / page_size + 1
        };
        let current_page = state.rules_page.min(total_pages.saturating_sub(1));
        let start = current_page * page_size;
        let end = (start + page_size).min(state.rules_filtered_indices.len());
        let visible = &state.rules_filtered_indices[start..end];

        let mut rules_list = column![].spacing(6);
        if visible.is_empty() {
            rules_list = rules_list.push(text(lang.tr("rules_empty")));
        } else {
            for cache_index in visible {
                let Some(item) = state.rules_render_cache.get(*cache_index) else {
                    continue;
                };
                let source_index = item.source_index;
                let Some(entry) = state.rules.get(source_index) else {
                    continue;
                };
                let up_button = if source_index > 0 {
                    button(text("↑").size(12))
                        .on_press(Message::MoveRuleUp(source_index))
                        .padding([4, 8])
                        .style(button::secondary)
                } else {
                    button(text("↑").size(12))
                        .padding([4, 8])
                        .style(button::secondary)
                };
                let down_button = if source_index + 1 < state.rules.len() {
                    button(text("↓").size(12))
                        .on_press(Message::MoveRuleDown(source_index))
                        .padding([4, 8])
                        .style(button::secondary)
                } else {
                    button(text("↓").size(12))
                        .padding([4, 8])
                        .style(button::secondary)
                };
                rules_list = rules_list.push(
                    container(
                        row![
                            checkbox(entry.enabled)
                                .on_toggle(move |_| Message::ToggleRuleEnabled(source_index))
                                .size(16),
                            container(text(item.rule_type.clone()).size(10).font(bold_font))
                                .padding([2, 6])
                                .style(move |_| rule_badge_style(item.badge)),
                            Space::new().width(10),
                            column![
                                text(item.payload.clone())
                                    .size(13)
                                    .style(move |_| text::Style {
                                        color: Some(if entry.enabled {
                                            Color::from_rgb(0.9, 0.9, 0.9)
                                        } else {
                                            Color::from_rgb(0.5, 0.5, 0.5)
                                        })
                                    }),
                                text(item.target.clone())
                                    .size(11)
                                    .style(move |_| text::Style {
                                        color: Some(if entry.enabled {
                                            Color::from_rgb(0.4, 0.7, 0.4)
                                        } else {
                                            Color::from_rgb(0.4, 0.4, 0.4)
                                        })
                                    }),
                            ]
                            .width(Length::Fill),
                            up_button,
                            Space::new().width(6),
                            down_button
                        ]
                        .align_y(Alignment::Center),
                    )
                    .padding(8)
                    .style(|_: &Theme| container::Style {
                        background: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.02).into()),
                        ..Default::default()
                    }),
                );
            }
        }

        let pager = row![
            text(format!("Page {}/{}", current_page + 1, total_pages)).size(12),
            Space::new().width(Length::Fill),
            button(text("Prev").size(12))
                .padding([4, 10])
                .style(button::secondary)
                .on_press_maybe((current_page > 0).then_some(Message::RulesPrevPage)),
            Space::new().width(8),
            button(text("Next").size(12))
                .padding([4, 10])
                .style(button::secondary)
                .on_press_maybe((current_page + 1 < total_pages).then_some(Message::RulesNextPage)),
        ]
        .align_y(Alignment::Center);

        column![
            add_rule_form,
            Space::new().height(12),
            search_bar,
            Space::new().height(8),
            pager,
            Space::new().height(8),
            modern_scrollable(rules_list).height(Length::Fill)
        ]
        .spacing(8)
    };

    let providers_view = {
        let mut content = column![
            row![
                text("Providers").font(bold_font),
                Space::new().width(Length::Fill),
                button(
                    text(if state.rules_providers_expanded {
                        "Collapse"
                    } else {
                        "Expand"
                    })
                    .size(12)
                )
                .padding([6, 12])
                .style(button::secondary)
                .on_press(Message::ToggleRulesProvidersExpanded)
            ]
            .align_y(Alignment::Center),
            text(format!(
                "Proxy Providers: {} | Rule Providers: {}",
                state.proxy_providers.len(),
                state.rule_providers.len()
            ))
            .size(12)
        ]
        .spacing(10);

        if state.rules_providers_expanded {
            let mut proxy_list = column![
                text(lang.tr("rules_proxy_providers")).font(bold_font),
                Space::new().height(8)
            ]
            .spacing(8);
            if state.proxy_providers.is_empty() {
                proxy_list = proxy_list.push(text(lang.tr("rules_no_providers")).size(12));
            } else {
                for provider in &state.proxy_providers {
                    proxy_list = proxy_list.push(
                        container(
                            row![
                                column![
                                    text(&provider.name).size(14).font(bold_font),
                                    text(format!(
                                        "{} - Updated: {}",
                                        provider.vehicle_type, provider.updated_at
                                    ))
                                    .size(10)
                                    .style(|_| text::Style {
                                        color: Some(Color::from_rgb(0.5, 0.5, 0.5))
                                    }),
                                ]
                                .width(Length::Fill),
                                button(
                                    row![
                                        text(icons::UPDATE).size(10),
                                        text(lang.tr("btn_update")).size(10)
                                    ]
                                    .spacing(6)
                                )
                                .on_press(Message::UpdateProxyProvider(provider.name.clone()))
                                .padding([4, 8])
                                .style(button::secondary)
                            ]
                            .align_y(Alignment::Center),
                        )
                        .padding(8)
                        .style(|_| container::Style {
                            background: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.03).into()),
                            border: Border {
                                radius: border::Radius::from(6.0),
                                ..Default::default()
                            },
                            ..Default::default()
                        }),
                    );
                }
            }

            let mut rule_list = column![
                text(lang.tr("rules_rule_providers")).font(bold_font),
                Space::new().height(8)
            ]
            .spacing(8);
            if state.rule_providers.is_empty() {
                rule_list = rule_list.push(text(lang.tr("rules_no_providers")).size(12));
            } else {
                for provider in &state.rule_providers {
                    rule_list = rule_list.push(
                        container(
                            row![
                                column![
                                    text(&provider.name).size(14).font(bold_font),
                                    text(format!(
                                        "{} rules - Updated: {}",
                                        provider.rule_count, provider.updated_at
                                    ))
                                    .size(10)
                                    .style(|_| text::Style {
                                        color: Some(Color::from_rgb(0.5, 0.5, 0.5))
                                    }),
                                ]
                                .width(Length::Fill),
                                button(
                                    row![
                                        text(icons::UPDATE).size(10),
                                        text(lang.tr("btn_update")).size(10)
                                    ]
                                    .spacing(6)
                                )
                                .on_press(Message::UpdateRuleProvider(provider.name.clone()))
                                .padding([4, 8])
                                .style(button::secondary)
                            ]
                            .align_y(Alignment::Center),
                        )
                        .padding(8)
                        .style(|_| container::Style {
                            background: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.03).into()),
                            border: Border {
                                radius: border::Radius::from(6.0),
                                ..Default::default()
                            },
                            ..Default::default()
                        }),
                    );
                }
            }

            content = content.push(row![
                container(card(proxy_list)).width(Length::FillPortion(1)),
                Space::new().width(20),
                container(card(rule_list)).width(Length::FillPortion(1)),
            ]);
        }
        content
    };

    let json_tab_buttons = row![
        tab_button(
            "Rule Providers".to_string(),
            state.rules_json_tab == RulesJsonTab::RuleProviders,
            Message::SetRulesJsonTab(RulesJsonTab::RuleProviders)
        ),
        tab_button(
            "Proxy Providers".to_string(),
            state.rules_json_tab == RulesJsonTab::ProxyProviders,
            Message::SetRulesJsonTab(RulesJsonTab::ProxyProviders)
        ),
        tab_button(
            "Sniffer".to_string(),
            state.rules_json_tab == RulesJsonTab::Sniffer,
            Message::SetRulesJsonTab(RulesJsonTab::Sniffer)
        ),
    ]
    .spacing(8);

    let json_view = match state.rules_json_tab {
        RulesJsonTab::RuleProviders => {
            if state.rule_providers_editor_state == EditorLazyState::Unloaded {
                editor_lazy_placeholder(
                    lang.tr("rules_rule_providers_json").to_string(),
                    Message::EnsureRuleProvidersEditorLoaded,
                )
            } else {
                let save_btn = save_action(
                    state.rule_providers_json_dirty,
                    state.is_saving_rule_providers_json,
                    icons::SAVE,
                    lang.tr("rules_save_rule_providers_btn").to_string(),
                    Message::SaveRuleProvidersJson,
                );
                card(column![
                    row![
                        text(lang.tr("rules_rule_providers_json")).font(bold_font),
                        Space::new().width(Length::Fill),
                        save_btn
                    ]
                    .align_y(Alignment::Center),
                    Space::new().height(10),
                    text_editor(&state.rule_providers_json_content)
                        .on_action(Message::RuleProvidersEditorAction)
                        .padding(10)
                        .height(Length::Fixed(420.0))
                ])
            }
        }
        RulesJsonTab::ProxyProviders => {
            if state.proxy_providers_editor_state == EditorLazyState::Unloaded {
                editor_lazy_placeholder(
                    "Proxy Providers JSON".to_string(),
                    Message::EnsureProxyProvidersEditorLoaded,
                )
            } else {
                let save_btn = save_action(
                    state.proxy_providers_json_dirty,
                    state.is_saving_proxy_providers_json,
                    icons::SAVE,
                    "Save Proxy Providers".to_string(),
                    Message::SaveProxyProvidersJson,
                );
                card(column![
                    row![
                        text("Proxy Providers JSON").font(bold_font),
                        Space::new().width(Length::Fill),
                        save_btn
                    ]
                    .align_y(Alignment::Center),
                    Space::new().height(10),
                    text_editor(&state.proxy_providers_json_content)
                        .on_action(Message::ProxyProvidersEditorAction)
                        .padding(10)
                        .height(Length::Fixed(420.0))
                ])
            }
        }
        RulesJsonTab::Sniffer => {
            if state.sniffer_editor_state == EditorLazyState::Unloaded {
                editor_lazy_placeholder(
                    lang.tr("rules_sniffer_json").to_string(),
                    Message::EnsureSnifferEditorLoaded,
                )
            } else {
                let save_btn = save_action(
                    state.sniffer_json_dirty,
                    state.is_saving_sniffer_json,
                    icons::SAVE,
                    lang.tr("rules_save_sniffer_btn").to_string(),
                    Message::SaveSnifferJson,
                );
                card(column![
                    row![
                        text(lang.tr("rules_sniffer_json")).font(bold_font),
                        Space::new().width(Length::Fill),
                        save_btn
                    ]
                    .align_y(Alignment::Center),
                    Space::new().height(10),
                    text_editor(&state.sniffer_json_content)
                        .on_action(Message::SnifferEditorAction)
                        .padding(10)
                        .height(Length::Fixed(420.0))
                ])
            }
        }
    };

    let tab_content: Element<'_, Message> = match state.rules_tab {
        RulesTab::RulesList => rules_list_view.into(),
        RulesTab::Providers => providers_view.into(),
        RulesTab::JsonEditors => column![json_tab_buttons, Space::new().height(12), json_view]
            .spacing(8)
            .into(),
    };

    column![
        header,
        Space::new().height(12),
        tabs,
        Space::new().height(12),
        tab_content
    ]
    .spacing(8)
    .into()
}
