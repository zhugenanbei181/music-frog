//! Dedicated Live Rule Tracer sandbox view for interactive rule matching and routing simulation.

use crate::state::AppState;
use crate::types::message::Message;
use crate::types::rules::RuleBadgeKind;
use crate::view::component_forms::{form_input_style, style_accent};
use crate::view::components::{BadgeKind, badge, card, icon_button, kbd_badge};
use crate::view::rules::{display_rule_type, semantic_badge_kind};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, MONO, tokens};
use iced::widget::{Space, button, column, container, row, text, text_input};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};
use infiltrator_shared::locales::{Lang, Localizer};

/// Quick preset test button for Rule Tracer.
fn quick_test_btn<'a>(sample: &'static str) -> Element<'a, Message> {
    button(text(sample).size(11).font(MONO))
        .padding([4, 8])
        .style(|t: &Theme, status| {
            let tk = tokens(t);
            let bg = match status {
                iced::widget::button::Status::Hovered => Color {
                    a: 0.15,
                    ..tk.accent
                },
                _ => Color {
                    a: 0.06,
                    ..tk.text_secondary
                },
            };
            button::Style {
                background: Some(bg.into()),
                border: Border {
                    radius: border::Radius::from(theme::R_CONTROL),
                    width: theme::HAIRLINE,
                    color: Color {
                        a: 0.20,
                        ..tk.card_border
                    },
                },
                ..Default::default()
            }
        })
        .on_press(Message::UpdateRulesTracerInput(sample.to_string()))
        .into()
}

/// Color for a decision-chain node status.
fn decision_status_color(
    status: infiltrator_contract::rule_tracer::DecisionNodeStatus,
) -> fn(&Theme) -> Color {
    match status {
        infiltrator_contract::rule_tracer::DecisionNodeStatus::Matched
        | infiltrator_contract::rule_tracer::DecisionNodeStatus::Passed => {
            |t: &Theme| tokens(t).success
        }
        infiltrator_contract::rule_tracer::DecisionNodeStatus::Failed => {
            |t: &Theme| tokens(t).danger
        }
        infiltrator_contract::rule_tracer::DecisionNodeStatus::Fallback => {
            |t: &Theme| tokens(t).warning
        }
        infiltrator_contract::rule_tracer::DecisionNodeStatus::Bypassed
        | infiltrator_contract::rule_tracer::DecisionNodeStatus::Neutral => {
            |t: &Theme| tokens(t).text_tertiary
        }
    }
}

/// Compact five-stage replay of the shared decision chain.
fn decision_chain_rows<'a>(
    chain: &'a infiltrator_contract::rule_tracer::DecisionChainSnapshot,
    lang: &Lang<'_>,
) -> Element<'a, Message> {
    let rows: Vec<Element<'a, Message>> = chain
        .nodes
        .iter()
        .map(|node| {
            let status_color = decision_status_color(node.status);
            row![
                text("●").size(10).style(move |t: &Theme| text::Style {
                    color: Some(status_color(t))
                }),
                Space::new().width(theme::SP_XS),
                text(node.title.clone())
                    .size(11)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_primary)
                    }),
                Space::new().width(Length::Fill),
                text(node.detail.clone())
                    .size(10)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_tertiary)
                    })
                    .width(Length::FillPortion(3)),
            ]
            .align_y(Alignment::Center)
            .spacing(theme::SP_XS)
            .width(Length::Fill)
            .into()
        })
        .collect();

    container(
        column![
            text(lang.tr("tracer_chain_title").to_string())
                .size(11)
                .font(FONT_SEMIBOLD)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary)
                }),
            Space::new().height(theme::SP_XS),
            column(rows).spacing(theme::SP_XS),
        ]
        .spacing(theme::SP_XS),
    )
    .padding([10, 14])
    .width(Length::Fill)
    .style(|t: &Theme| {
        let tk = tokens(t);
        container::Style {
            background: Some(tk.control_bg.into()),
            border: Border {
                radius: border::Radius::from(theme::R_CARD),
                width: theme::HAIRLINE,
                color: tk.divider,
            },
            ..Default::default()
        }
    })
    .into()
}

/// Standalone Full-Page / Tab Live Rule Tracer Sandbox.
pub fn tracer_view<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let tracer_result_view: Element<'_, Message> = match &state.editor.rules_tracer_chain {
        Some(chain) if !chain.is_fallback => {
            let index_label = chain
                .hit_rule_index
                .map(|index| format!("#{}", index + 1))
                .unwrap_or_else(|| "—".to_owned());
            let (rule_type_part, payload_part) = chain
                .matched_rule_raw
                .split_once(',')
                .map(|(t, p)| (t.trim(), p.trim()))
                .unwrap_or((chain.matched_rule_raw.as_str(), ""));
            let bkind = semantic_badge_kind(rule_type_part, RuleBadgeKind::Other);
            let norm_type = display_rule_type(rule_type_part);

            // DUAL-12-08: one-click reverse-apply, gated on the shared
            // `can_reverse_apply` fact. A matched, concrete rule can be
            // rewritten; a fallback replay renders no chooser.
            let override_block: Element<'_, Message> = match (
                state.editor.rules_tracer_can_reverse_apply,
                chain.hit_rule_index,
            ) {
                (true, Some(rule_index)) => {
                    let chip = |label: &'static str| -> Element<'_, Message> {
                        button(text(label).size(11).font(MONO))
                            .padding([4, 8])
                            .style(|t: &Theme, status| {
                                let tk = tokens(t);
                                let bg = match status {
                                    iced::widget::button::Status::Hovered => Color {
                                        a: 0.15,
                                        ..tk.accent
                                    },
                                    _ => Color {
                                        a: 0.06,
                                        ..tk.text_secondary
                                    },
                                };
                                button::Style {
                                    background: Some(bg.into()),
                                    border: Border {
                                        radius: border::Radius::from(theme::R_CONTROL),
                                        width: theme::HAIRLINE,
                                        color: Color {
                                            a: 0.20,
                                            ..tk.card_border
                                        },
                                    },
                                    ..Default::default()
                                }
                            })
                            .on_press(Message::UpdateTracerOverrideTarget(label.to_string()))
                            .into()
                    };
                    let apply_btn = button(
                        row![
                            svg_icons::icon_themed(Icon::Target, 13.0, |t: &Theme| {
                                tokens(t).on_accent
                            }),
                            text(lang.tr("tracer_override_apply").to_string())
                                .size(12)
                                .font(FONT_MEDIUM),
                        ]
                        .spacing(theme::SP_XS)
                        .align_y(Alignment::Center),
                    )
                    .padding([6, 12])
                    .style(style_accent)
                    .on_press(Message::ApplyTracerRuleOverride { rule_index });

                    column![
                        text(lang.tr("tracer_override_label").to_string())
                            .size(11)
                            .style(|t: &Theme| text::Style {
                                color: Some(tokens(t).text_secondary)
                            }),
                        row![
                            chip("PROXY"),
                            Space::new().width(theme::SP_XS),
                            chip("DIRECT"),
                            Space::new().width(theme::SP_XS),
                            chip("REJECT"),
                            Space::new().width(theme::SP_SM),
                            text_input(
                                lang.tr("tracer_override_placeholder").as_ref(),
                                &state.editor.rules_tracer_override_target,
                            )
                            .on_input(Message::UpdateTracerOverrideTarget)
                            .on_submit(Message::ApplyTracerRuleOverride { rule_index })
                            .padding([6, 10])
                            .size(12)
                            .font(MONO)
                            .width(Length::Fill)
                            .style(form_input_style),
                            Space::new().width(theme::SP_SM),
                            apply_btn,
                        ]
                        .align_y(Alignment::Center),
                    ]
                    .spacing(theme::SP_XS)
                    .into()
                }
                _ => Space::new().width(0).into(),
            };

            container(
                column![
                    row![
                        svg_icons::icon_themed(Icon::Activity, 16.0, |t: &Theme| tokens(t).success),
                        Space::new().width(theme::SP_SM),
                        text(lang.tr("tracer_result_matched").to_string())
                            .size(14)
                            .font(FONT_SEMIBOLD)
                            .style(|t: &Theme| text::Style {
                                color: Some(tokens(t).success)
                            }),
                        Space::new().width(theme::SP_SM),
                        badge(index_label, BadgeKind::Success),
                        Space::new().width(Length::Fill),
                        kbd_badge(chain.target_proxy.clone()),
                    ]
                    .align_y(Alignment::Center),
                    Space::new().height(theme::SP_SM),
                    row![
                        text(format!("{}:", lang.tr("tracer_hit_pattern")))
                            .size(12)
                            .style(|t: &Theme| text::Style {
                                color: Some(tokens(t).text_secondary)
                            }),
                        Space::new().width(theme::SP_XS),
                        badge(norm_type, bkind),
                        Space::new().width(theme::SP_SM),
                        text(if payload_part.is_empty() {
                            chain.matched_rule_raw.as_str()
                        } else {
                            payload_part
                        })
                        .size(13)
                        .font(MONO)
                        .style(|t: &Theme| text::Style {
                            color: Some(tokens(t).text_primary)
                        }),
                    ]
                    .align_y(Alignment::Center),
                    Space::new().height(theme::SP_XS),
                    row![
                        text(format!("{}:", lang.tr("tracer_hit_target")))
                            .size(12)
                            .style(|t: &Theme| text::Style {
                                color: Some(tokens(t).text_secondary)
                            }),
                        Space::new().width(theme::SP_XS),
                        text(chain.target_proxy.clone())
                            .size(13)
                            .font(FONT_SEMIBOLD)
                            .style(|t: &Theme| text::Style {
                                color: Some(tokens(t).accent)
                            }),
                    ]
                    .align_y(Alignment::Center),
                    Space::new().height(theme::SP_SM),
                    decision_chain_rows(chain, lang),
                    Space::new().height(theme::SP_SM),
                    override_block,
                ]
                .spacing(theme::SP_XS),
            )
            .padding([14, 18])
            .width(Length::Fill)
            .style(|t: &Theme| {
                let tk = tokens(t);
                container::Style {
                    background: Some(
                        Color {
                            a: 0.08,
                            ..tk.success
                        }
                        .into(),
                    ),
                    border: Border {
                        radius: border::Radius::from(theme::R_CARD),
                        width: theme::HAIRLINE,
                        color: Color {
                            a: 0.35,
                            ..tk.success
                        },
                    },
                    ..Default::default()
                }
            })
            .into()
        }
        Some(chain) => container(
            column![
                row![
                    svg_icons::icon_themed(Icon::Shield, 16.0, |t: &Theme| tokens(t).warning),
                    Space::new().width(theme::SP_MD),
                    column![
                        text(lang.tr("tracer_result_fallback").to_string())
                            .size(13)
                            .font(FONT_SEMIBOLD)
                            .style(|t: &Theme| text::Style {
                                color: Some(tokens(t).warning)
                            }),
                        text(lang.tr("rule_tracer_fallback_desc").to_string())
                            .size(11)
                            .style(|t: &Theme| text::Style {
                                color: Some(tokens(t).text_secondary)
                            }),
                    ]
                    .width(Length::Fill),
                    badge(
                        lang.tr("rule_tracer_fallback_badge").to_string(),
                        BadgeKind::Warning
                    ),
                ]
                .align_y(Alignment::Center),
                Space::new().height(theme::SP_SM),
                decision_chain_rows(chain, lang),
            ]
            .spacing(theme::SP_XS),
        )
        .padding([12, 16])
        .width(Length::Fill)
        .style(|t: &Theme| {
            let tk = tokens(t);
            container::Style {
                background: Some(
                    Color {
                        a: 0.08,
                        ..tk.warning
                    }
                    .into(),
                ),
                border: Border {
                    radius: border::Radius::from(theme::R_CARD),
                    width: theme::HAIRLINE,
                    color: Color {
                        a: 0.25,
                        ..tk.warning
                    },
                },
                ..Default::default()
            }
        })
        .into(),
        None => container(
            row![
                svg_icons::icon_themed(Icon::Target, 16.0, |t: &Theme| tokens(t).text_tertiary),
                Space::new().width(theme::SP_SM),
                text(lang.tr("tracer_subtitle").to_string())
                    .size(12)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_tertiary)
                    }),
            ]
            .align_y(Alignment::Center),
        )
        .padding([12, 16])
        .width(Length::Fill)
        .style(|t: &Theme| {
            let tk = tokens(t);
            container::Style {
                background: Some(tk.control_bg.into()),
                border: Border {
                    radius: border::Radius::from(theme::R_CARD),
                    width: theme::HAIRLINE,
                    color: tk.card_border,
                },
                ..Default::default()
            }
        })
        .into(),
    };

    let clear_btn = if state.editor.rules_tracer_input.is_empty() {
        Element::from(Space::new().width(0))
    } else {
        icon_button(
            Icon::X,
            12.0,
            Message::UpdateRulesTracerInput(String::new()),
        )
    };

    let trace_btn = button(
        row![
            svg_icons::icon_themed(Icon::Search, 14.0, |t: &Theme| tokens(t).on_accent),
            text(lang.tr("tracer_btn_trace").to_string())
                .size(12)
                .font(FONT_MEDIUM),
            kbd_badge("↵")
        ]
        .spacing(theme::SP_SM)
        .align_y(Alignment::Center),
    )
    .padding([8, 16])
    .style(style_accent)
    .on_press(Message::RunRulesTracer);

    let quick_presets = row![
        text(lang.tr("rule_tracer_presets").to_string())
            .size(11)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_tertiary)
            }),
        Space::new().width(theme::SP_XS),
        quick_test_btn("google.com"),
        Space::new().width(theme::SP_XS),
        quick_test_btn("1.1.1.1:443"),
        Space::new().width(theme::SP_XS),
        quick_test_btn("steamcommunity.com"),
        Space::new().width(theme::SP_XS),
        quick_test_btn("netflix.com"),
        Space::new().width(theme::SP_XS),
        quick_test_btn("github.com"),
    ]
    .align_y(Alignment::Center);

    let main_card = card(
        Some(lang.tr("tracer_title").to_string()),
        column![
            text(lang.tr("tracer_subtitle").to_string())
                .size(12)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary)
                }),
            Space::new().height(theme::SP_XS),
            row![
                text_input(
                    lang.tr("tracer_query_placeholder").as_ref(),
                    &state.editor.rules_tracer_input
                )
                .on_input(Message::UpdateRulesTracerInput)
                .on_submit(Message::RunRulesTracer)
                .padding([8, 12])
                .size(12)
                .font(MONO)
                .width(Length::Fill)
                .style(form_input_style),
                clear_btn,
                Space::new().width(theme::SP_SM),
                trace_btn,
            ]
            .align_y(Alignment::Center),
            row![
                text(lang.tr("tracer_src_ip_label").to_string())
                    .size(11)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_tertiary)
                    }),
                Space::new().width(theme::SP_XS),
                text_input(
                    lang.tr("tracer_src_ip_placeholder").as_ref(),
                    &state.editor.rules_tracer_src_ip
                )
                .on_input(Message::UpdateTracerSourceIp)
                .on_submit(Message::RunRulesTracer)
                .padding([8, 12])
                .size(12)
                .font(MONO)
                .width(Length::Fill)
                .style(form_input_style),
            ]
            .align_y(Alignment::Center),
            quick_presets,
            Space::new().height(theme::SP_SM),
            tracer_result_view,
        ]
        .spacing(theme::SP_SM),
    );

    column![main_card].spacing(theme::SP_MD).into()
}

/// Compact inline tracer panel used at the head of the rules list.
pub fn inline_tracer_panel<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    tracer_view(state, lang)
}
