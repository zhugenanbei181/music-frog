use crate::state::AppState;
use crate::types::editor::EditorLazyState;
use crate::types::message::Message;
use crate::types::rules::{RuleBadgeKind, RulesJsonTab, RulesTab};
use crate::view::components::{
    BadgeKind, card, empty_state, icon_button, modern_scrollable, section_header,
    segmented_control, toggle_switch,
};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, MONO, R_CONTROL, SP_LG, SP_MD, tokens};
use iced::widget::{
    Space, button, column, container, pick_list, row, text, text_editor, text_input,
};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};
use infiltrator_shared::locales::{Lang, Localizer};

// ---------------------------------------------------------------------------
// Token-driven control styles (ui-wave2-r)
// ---------------------------------------------------------------------------

fn style_accent(t: &Theme, status: button::Status) -> button::Style {
    let tk = tokens(t);
    let (bg, fg) = match status {
        button::Status::Disabled => (tk.accent_soft, tk.accent),
        button::Status::Hovered | button::Status::Pressed => (
            Color {
                a: 0.85,
                ..tk.accent
            },
            tk.on_accent,
        ),
        _ => (tk.accent, tk.on_accent),
    };
    button::Style {
        background: Some(bg.into()),
        border: Border {
            radius: border::Radius::from(R_CONTROL),
            ..Default::default()
        },
        text_color: fg,
        ..Default::default()
    }
}

fn style_ghost(t: &Theme, status: button::Status) -> button::Style {
    let tk = tokens(t);
    button::Style {
        background: match status {
            button::Status::Hovered | button::Status::Pressed => Some(tk.control_bg.into()),
            _ => None,
        },
        border: Border {
            radius: border::Radius::from(R_CONTROL),
            width: 1.0,
            color: tk.card_border,
        },
        text_color: match status {
            button::Status::Disabled => tk.text_tertiary,
            button::Status::Hovered | button::Status::Pressed => tk.text_primary,
            _ => tk.text_secondary,
        },
        ..Default::default()
    }
}

/// Text push button: `on_press == None` renders the disabled state.
fn text_btn<'a>(
    label: String,
    style: fn(&Theme, button::Status) -> button::Style,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    button(text(label).size(12).font(FONT_MEDIUM))
        .padding([7, 14])
        .style(style)
        .on_press_maybe(on_press)
        .into()
}

fn input_style(t: &Theme, status: text_input::Status) -> text_input::Style {
    let tk = tokens(t);
    let (border_color, border_width) = match status {
        text_input::Status::Focused { .. } => (tk.accent, 1.5),
        _ => (tk.card_border, 1.0),
    };
    text_input::Style {
        background: tk.control_bg.into(),
        border: Border {
            radius: border::Radius::from(R_CONTROL),
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

fn pick_style(t: &Theme, _status: pick_list::Status) -> pick_list::Style {
    let tk = tokens(t);
    pick_list::Style {
        text_color: tk.text_primary,
        placeholder_color: tk.text_tertiary,
        handle_color: tk.text_secondary,
        background: tk.control_bg.into(),
        border: Border {
            radius: border::Radius::from(R_CONTROL),
            width: 1.0,
            color: tk.card_border,
        },
    }
}

/// Framed surface for embedded text editors (mono code area).
fn editor_frame(t: &Theme) -> container::Style {
    let tk = tokens(t);
    container::Style {
        background: Some(tk.control_bg.into()),
        border: Border {
            radius: border::Radius::from(R_CONTROL),
            width: 1.0,
            color: tk.card_border,
        },
        ..Default::default()
    }
}

/// White list-row surface used inside section cards.
fn row_card(t: &Theme) -> container::Style {
    let tk = tokens(t);
    container::Style {
        background: Some(tk.card_bg.into()),
        border: Border {
            radius: border::Radius::from(R_CONTROL),
            width: 1.0,
            color: tk.card_border,
        },
        ..Default::default()
    }
}

/// Save / Saving… / Saved action used across rules panels.
fn save_action(
    dirty: bool,
    saving: bool,
    label: String,
    saved: String,
    on_press: Message,
) -> Element<'static, Message> {
    if saving {
        text_btn("Saving...".to_string(), style_ghost, None)
    } else if dirty {
        text_btn(label, style_accent, Some(on_press))
    } else {
        text_btn(saved, style_ghost, None)
    }
}

/// Map the rule classifier to the shared badge palette:
/// DOMAIN→Accent, IP→Warning, everything else→Neutral.
fn badge_kind(kind: RuleBadgeKind) -> BadgeKind {
    match kind {
        RuleBadgeKind::Domain => BadgeKind::Accent,
        RuleBadgeKind::Ip => BadgeKind::Warning,
        RuleBadgeKind::Other => BadgeKind::Neutral,
    }
}

fn field_label(value: String) -> text::Text<'static> {
    text(value).size(11).style(|t: &Theme| text::Style {
        color: Some(tokens(t).text_secondary),
    })
}

fn editor_lazy_placeholder<'a>(title: String, on_press: Message) -> Element<'a, Message> {
    card(
        None,
        column![
            empty_state(Icon::Code2, title.as_str(), "Editor will load on demand"),
            Space::new().height(theme::SP_SM),
            text_btn("Load Editor".to_string(), style_accent, Some(on_press)),
        ]
        .align_x(Alignment::Center),
    )
}

pub fn view(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);

    let filtered_count = state.editor.rules_filtered_indices.len();
    let save_rules_action = save_action(
        state.editor.rules_dirty,
        state.editor.is_saving_rules,
        lang.tr("rules_save_btn").to_string(),
        lang.tr("rules_saved").to_string(),
        Message::SaveRules,
    );

    let header = row![
        text(lang.tr("rules_title").to_string())
            .size(24)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_primary),
            }),
        Space::new().width(theme::SP_MD),
        text(format!("{} / {}", filtered_count, state.editor.rules.len()))
            .size(13)
            .font(MONO)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_tertiary),
            }),
        Space::new().width(Length::Fill),
        if state.editor.is_loading_rules || state.editor.is_loading_providers {
            Element::from(text("...").size(12).style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_tertiary),
            }))
        } else {
            icon_button(Icon::RefreshCw, 16.0, Message::LoadRules)
        },
        Space::new().width(theme::SP_SM),
        save_rules_action,
    ]
    .align_y(Alignment::Center);

    let tab_labels: Vec<String> = vec![
        lang.tr("rules_tab_list").to_string(),
        lang.tr("rules_tab_providers").to_string(),
        lang.tr("rules_tab_json").to_string(),
    ];
    let tab_index = match state.editor.rules_tab {
        RulesTab::Providers => 1,
        RulesTab::JsonEditors => 2,
        RulesTab::RulesList => 0,
    };
    let tabs = segmented_control(&tab_labels, tab_index, |index| {
        Message::SetRulesTab(match index {
            1 => RulesTab::Providers,
            2 => RulesTab::JsonEditors,
            _ => RulesTab::RulesList,
        })
    });

    if !state.editor.rules_heavy_ready {
        return column![
            header,
            Space::new().height(theme::SP_MD),
            tabs,
            Space::new().height(SP_LG),
            card(
                None,
                column![
                    text("Preparing Rules panels...")
                        .size(14)
                        .font(FONT_SEMIBOLD)
                        .style(|t: &Theme| text::Style {
                            color: Some(tokens(t).text_primary),
                        }),
                    text("Heavy widgets mount asynchronously to keep first paint responsive.")
                        .size(12)
                        .style(|t: &Theme| text::Style {
                            color: Some(tokens(t).text_secondary),
                        }),
                ]
                .spacing(theme::SP_SM)
            ),
        ]
        .spacing(10)
        .into();
    }

    let mut available_targets: Vec<String> = state
        .runtime
        .proxies
        .iter()
        .filter(|(_, p): &(&String, &mihomo_api::proxy::types::Proxy)| p.is_group())
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
        "AND".to_string(),
        "OR".to_string(),
        "NOT".to_string(),
        "SUB-RULE".to_string(),
    ];

    let add_rule_btn_style = if state.editor.is_adding_rule {
        style_ghost
    } else {
        style_accent
    };
    let add_rule_form = card(
        Some(lang.tr("rules_add_custom").to_string()),
        row![
            column![
                field_label(lang.tr("rules_type").to_string()),
                Space::new().height(theme::SP_XS),
                pick_list(
                    rule_types,
                    Some(&state.editor.new_rule_type),
                    Message::UpdateNewRuleType
                )
                .width(Length::Fill)
                .style(pick_style),
            ]
            .width(Length::FillPortion(1))
            .spacing(theme::SP_XS),
            Space::new().width(theme::SP_LG),
            column![
                field_label(lang.tr("rules_payload").to_string()),
                Space::new().height(theme::SP_XS),
                text_input("e.g. google.com", &state.editor.new_rule_payload)
                    .on_input(Message::UpdateNewRulePayload)
                    .padding([8, 12])
                    .size(12)
                    .style(input_style),
            ]
            .width(Length::FillPortion(2))
            .spacing(theme::SP_XS),
            Space::new().width(theme::SP_LG),
            column![
                field_label(lang.tr("rules_target").to_string()),
                Space::new().height(theme::SP_XS),
                pick_list(
                    available_targets,
                    Some(&state.editor.new_rule_target),
                    |t| { Message::UpdateNewRuleTarget(t) }
                )
                .width(Length::Fill)
                .style(pick_style),
            ]
            .width(Length::FillPortion(1))
            .spacing(theme::SP_XS),
            Space::new().width(theme::SP_LG),
            column![
                Space::new().height(18.0),
                button(
                    row![
                        svg_icons::icon_themed(Icon::Plus, 14.0, |t: &Theme| {
                            if state.editor.is_adding_rule {
                                tokens(t).text_secondary
                            } else {
                                tokens(t).on_accent
                            }
                        }),
                        text(lang.tr("rules_add_btn").to_string())
                            .size(12)
                            .font(FONT_MEDIUM),
                    ]
                    .spacing(theme::SP_SM),
                )
                .padding([8, 16])
                .style(add_rule_btn_style)
                .on_press(Message::AddCustomRule),
            ]
            .spacing(theme::SP_XS),
        ]
        .align_y(Alignment::Center),
    );

    let rules_list_view = {
        let search_bar = row![
            svg_icons::icon_themed(Icon::Search, 14.0, |t: &Theme| tokens(t).text_tertiary),
            Space::new().width(theme::SP_SM),
            text_input(
                lang.tr("rules_filter_placeholder").as_ref(),
                &state.editor.rules_filter
            )
            .on_input(Message::FilterRules)
            .padding([8, 12])
            .size(13)
            .width(Length::Fill)
            .style(input_style),
        ]
        .align_y(Alignment::Center);

        let page_size = state.editor.rules_page_size.max(1);
        let total_pages = if state.editor.rules_filtered_indices.is_empty() {
            1
        } else {
            (state.editor.rules_filtered_indices.len() - 1) / page_size + 1
        };
        let current_page = state.editor.rules_page.min(total_pages.saturating_sub(1));
        let start = current_page * page_size;
        let end = (start + page_size).min(state.editor.rules_filtered_indices.len());
        let visible = &state.editor.rules_filtered_indices[start..end];

        let mut rules_list = column![].spacing(theme::SP_SM);
        if visible.is_empty() {
            rules_list = rules_list.push(empty_state(
                Icon::ListChecks,
                lang.tr("rules_empty").as_ref(),
                "",
            ));
        } else {
            for cache_index in visible {
                let Some(item) = state.editor.rules_render_cache.get(*cache_index) else {
                    continue;
                };
                let source_index = item.source_index;
                let Some(entry) = state.editor.rules.get(source_index) else {
                    continue;
                };
                let up_button = icon_button(Icon::ArrowUp, 14.0, Message::MoveRuleUp(source_index));
                let down_button =
                    icon_button(Icon::ArrowDown, 14.0, Message::MoveRuleDown(source_index));
                rules_list = rules_list.push(
                    container(
                        row![
                            toggle_switch(entry.enabled, move |_| {
                                Message::ToggleRuleEnabled(source_index)
                            }),
                            crate::view::components::badge(
                                item.rule_type.clone(),
                                badge_kind(item.badge),
                            ),
                            Space::new().width(theme::SP_MD),
                            column![
                                text(item.payload.clone()).size(13).style(move |t: &Theme| {
                                    text::Style {
                                        color: Some(if entry.enabled {
                                            tokens(t).text_primary
                                        } else {
                                            tokens(t).text_tertiary
                                        }),
                                    }
                                }),
                                text(item.target.clone()).size(11).style(move |t: &Theme| {
                                    text::Style {
                                        color: Some(if entry.enabled {
                                            tokens(t).text_secondary
                                        } else {
                                            tokens(t).text_tertiary
                                        }),
                                    }
                                }),
                            ]
                            .width(Length::Fill),
                            if source_index > 0 {
                                up_button
                            } else {
                                Space::new().width(28).height(14).into()
                            },
                            if source_index + 1 < state.editor.rules.len() {
                                down_button
                            } else {
                                Space::new().width(28).height(14).into()
                            },
                        ]
                        .align_y(Alignment::Center),
                    )
                    .padding([theme::SP_SM, SP_MD])
                    .width(Length::Fill)
                    .style(row_card),
                );
            }
        }

        let pager = row![
            text(format!("Page {}/{}", current_page + 1, total_pages))
                .size(12)
                .font(MONO)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary),
                }),
            Space::new().width(Length::Fill),
            text_btn(
                "Prev".to_string(),
                style_ghost,
                (current_page > 0).then_some(Message::RulesPrevPage)
            ),
            Space::new().width(theme::SP_SM),
            text_btn(
                "Next".to_string(),
                style_ghost,
                (current_page + 1 < total_pages).then_some(Message::RulesNextPage)
            ),
        ]
        .align_y(Alignment::Center);

        column![
            add_rule_form,
            Space::new().height(theme::SP_MD),
            search_bar,
            Space::new().height(theme::SP_SM),
            pager,
            Space::new().height(theme::SP_SM),
            modern_scrollable(rules_list).height(Length::Fill),
        ]
        .spacing(theme::SP_SM)
    };

    let providers_view = {
        let mut content = column![section_header(
            "Providers",
            Some(
                row![
                    crate::view::components::chip(format!(
                        "Proxy {} | Rule {}",
                        state.editor.proxy_providers.len(),
                        state.editor.rule_providers.len()
                    )),
                    Space::new().width(theme::SP_SM),
                    text_btn(
                        if state.editor.rules_providers_expanded {
                            lang.tr("rules_collapse").to_string()
                        } else {
                            lang.tr("rules_expand").to_string()
                        },
                        style_ghost,
                        Some(Message::ToggleRulesProvidersExpanded),
                    ),
                ]
                .align_y(Alignment::Center)
                .into(),
            ),
        ),]
        .spacing(theme::SP_MD);

        if state.editor.rules_providers_expanded {
            let mut proxy_list = column![].spacing(theme::SP_SM);
            if state.editor.proxy_providers.is_empty() {
                proxy_list = proxy_list.push(empty_state(
                    Icon::Server,
                    lang.tr("rules_no_providers").as_ref(),
                    "",
                ));
            } else {
                for provider in &state.editor.proxy_providers {
                    proxy_list = proxy_list.push(
                        container(
                            row![
                                column![
                                    text(&provider.name).size(13).font(FONT_SEMIBOLD).style(
                                        |t: &Theme| text::Style {
                                            color: Some(tokens(t).text_primary),
                                        }
                                    ),
                                    text(format!(
                                        "{} - Updated: {}",
                                        provider.vehicle_type, provider.updated_at
                                    ))
                                    .size(11)
                                    .font(MONO)
                                    .style(|t: &Theme| {
                                        text::Style {
                                            color: Some(tokens(t).text_secondary),
                                        }
                                    }),
                                ]
                                .width(Length::Fill),
                                text_btn(
                                    lang.tr("btn_update").to_string(),
                                    style_ghost,
                                    Some(Message::UpdateProxyProvider(provider.name.clone())),
                                ),
                            ]
                            .align_y(Alignment::Center),
                        )
                        .padding([theme::SP_SM, SP_MD])
                        .width(Length::Fill)
                        .style(row_card),
                    );
                }
            }

            let mut rule_list = column![].spacing(theme::SP_SM);
            if state.editor.rule_providers.is_empty() {
                rule_list = rule_list.push(empty_state(
                    Icon::ListChecks,
                    lang.tr("rules_no_providers").as_ref(),
                    "",
                ));
            } else {
                for provider in &state.editor.rule_providers {
                    rule_list = rule_list.push(
                        container(
                            row![
                                column![
                                    text(&provider.name).size(13).font(FONT_SEMIBOLD).style(
                                        |t: &Theme| text::Style {
                                            color: Some(tokens(t).text_primary),
                                        }
                                    ),
                                    text(format!(
                                        "{} rules - Updated: {}",
                                        provider.rule_count, provider.updated_at
                                    ))
                                    .size(11)
                                    .font(MONO)
                                    .style(|t: &Theme| {
                                        text::Style {
                                            color: Some(tokens(t).text_secondary),
                                        }
                                    }),
                                ]
                                .width(Length::Fill),
                                text_btn(
                                    lang.tr("btn_update").to_string(),
                                    style_ghost,
                                    Some(Message::UpdateRuleProvider(provider.name.clone())),
                                ),
                            ]
                            .align_y(Alignment::Center),
                        )
                        .padding([theme::SP_SM, SP_MD])
                        .width(Length::Fill)
                        .style(row_card),
                    );
                }
            }

            content = content.push(
                column![
                    card(
                        Some(lang.tr("rules_proxy_providers").to_string()),
                        proxy_list
                    ),
                    Space::new().height(theme::SP_MD),
                    card(Some(lang.tr("rules_rule_providers").to_string()), rule_list),
                ]
                .spacing(theme::SP_MD),
            );
            if let Some(mrs_panel) = crate::view::mrs_panel::mrs_card(state) {
                content = content
                    .push(Space::new().height(theme::SP_MD))
                    .push(mrs_panel);
            }
        }
        content
    };

    let json_tab_labels: Vec<String> = vec![
        "Rule Providers".to_string(),
        "Proxy Providers".to_string(),
        "Sniffer".to_string(),
    ];
    let json_tab_index = match state.editor.rules_json_tab {
        RulesJsonTab::ProxyProviders => 1,
        RulesJsonTab::Sniffer => 2,
        RulesJsonTab::RuleProviders => 0,
    };
    let json_tab_buttons = segmented_control(&json_tab_labels, json_tab_index, |index| {
        Message::SetRulesJsonTab(match index {
            1 => RulesJsonTab::ProxyProviders,
            2 => RulesJsonTab::Sniffer,
            _ => RulesJsonTab::RuleProviders,
        })
    });

    let json_view = match state.editor.rules_json_tab {
        RulesJsonTab::RuleProviders => {
            if state.editor.rule_providers_editor_state == EditorLazyState::Unloaded {
                editor_lazy_placeholder(
                    lang.tr("rules_rule_providers_json").to_string(),
                    Message::EnsureRuleProvidersEditorLoaded,
                )
            } else {
                let save_btn = save_action(
                    state.editor.rule_providers_json_dirty,
                    state.editor.is_saving_rule_providers_json,
                    lang.tr("rules_save_rule_providers_btn").to_string(),
                    lang.tr("rules_saved").to_string(),
                    Message::SaveRuleProvidersJson,
                );
                card(
                    Some(lang.tr("rules_rule_providers_json").to_string()),
                    column![
                        section_header("JSON", Some(save_btn)),
                        Space::new().height(theme::SP_SM),
                        container(
                            text_editor(&state.editor.rule_providers_json_content)
                                .on_action(Message::RuleProvidersEditorAction)
                                .font(MONO)
                                .padding(10)
                                .height(Length::Fixed(420.0))
                        )
                        .width(Length::Fill)
                        .style(editor_frame),
                    ],
                )
            }
        }
        RulesJsonTab::ProxyProviders => {
            if state.editor.proxy_providers_editor_state == EditorLazyState::Unloaded {
                editor_lazy_placeholder(
                    "Proxy Providers JSON".to_string(),
                    Message::EnsureProxyProvidersEditorLoaded,
                )
            } else {
                let save_btn = save_action(
                    state.editor.proxy_providers_json_dirty,
                    state.editor.is_saving_proxy_providers_json,
                    lang.tr("rules_save_proxy_providers_btn").to_string(),
                    lang.tr("rules_saved").to_string(),
                    Message::SaveProxyProvidersJson,
                );
                card(
                    Some("Proxy Providers JSON".to_string()),
                    column![
                        section_header("JSON", Some(save_btn)),
                        Space::new().height(theme::SP_SM),
                        container(
                            text_editor(&state.editor.proxy_providers_json_content)
                                .on_action(Message::ProxyProvidersEditorAction)
                                .font(MONO)
                                .padding(10)
                                .height(Length::Fixed(420.0))
                        )
                        .width(Length::Fill)
                        .style(editor_frame),
                    ],
                )
            }
        }
        RulesJsonTab::Sniffer => {
            if state.editor.sniffer_editor_state == EditorLazyState::Unloaded {
                editor_lazy_placeholder(
                    lang.tr("rules_sniffer_json").to_string(),
                    Message::EnsureSnifferEditorLoaded,
                )
            } else {
                let save_btn = save_action(
                    state.editor.sniffer_json_dirty,
                    state.editor.is_saving_sniffer_json,
                    lang.tr("rules_save_sniffer_btn").to_string(),
                    lang.tr("rules_saved").to_string(),
                    Message::SaveSnifferJson,
                );
                card(
                    Some(lang.tr("rules_sniffer_json").to_string()),
                    column![
                        section_header("JSON", Some(save_btn)),
                        Space::new().height(theme::SP_SM),
                        container(
                            text_editor(&state.editor.sniffer_json_content)
                                .on_action(Message::SnifferEditorAction)
                                .font(MONO)
                                .padding(10)
                                .height(Length::Fixed(420.0))
                        )
                        .width(Length::Fill)
                        .style(editor_frame),
                    ],
                )
            }
        }
    };

    let tab_content: Element<'_, Message> = match state.editor.rules_tab {
        RulesTab::RulesList => rules_list_view.into(),
        RulesTab::Providers => providers_view.into(),
        RulesTab::JsonEditors => column![
            json_tab_buttons,
            Space::new().height(theme::SP_MD),
            json_view,
        ]
        .spacing(theme::SP_SM)
        .into(),
    };

    column![
        header,
        Space::new().height(theme::SP_MD),
        tabs,
        Space::new().height(theme::SP_MD),
        tab_content,
    ]
    .spacing(SP_LG)
    .into()
}
