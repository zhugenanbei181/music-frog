//! Dedicated Live Rule Tracer sandbox view for interactive rule matching and routing simulation.

use crate::state::AppState;
use crate::types::message::Message;
use crate::types::rules::RuleBadgeKind;
use crate::view::components::{
    BadgeKind, badge, card, form_input_style, icon_button, kbd_badge, style_accent,
    };
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
                    width: 1.0,
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

/// Standalone Full-Page / Tab Live Rule Tracer Sandbox.
pub fn tracer_view<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let tracer_result_view: Element<'_, Message> = match &state.editor.rules_tracer_result {
        Some((index, matched_rule, target)) => {
            let (rule_type_part, payload_part) = matched_rule
                .split_once(',')
                .map(|(t, p)| (t.trim(), p.trim()))
                .unwrap_or((matched_rule.as_str(), ""));
            let bkind = semantic_badge_kind(rule_type_part, RuleBadgeKind::Other);
            let norm_type = display_rule_type(rule_type_part);

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
                        badge(format!("#{}", index + 1), BadgeKind::Success),
                        Space::new().width(Length::Fill),
                        kbd_badge(target.clone()),
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
                            matched_rule.as_str()
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
                        text(target.clone())
                            .size(13)
                            .font(FONT_SEMIBOLD)
                            .style(|t: &Theme| text::Style {
                                color: Some(tokens(t).accent)
                            }),
                    ]
                    .align_y(Alignment::Center),
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
                        width: 1.0,
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
        None => {
            if !state.editor.rules_tracer_input.trim().is_empty() {
                container(
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
                            width: 1.0,
                            color: Color {
                                a: 0.25,
                                ..tk.warning
                            },
                        },
                        ..Default::default()
                    }
                })
                .into()
            } else {
                container(
                    row![
                        svg_icons::icon_themed(Icon::Target, 16.0, |t: &Theme| tokens(t)
                            .text_tertiary),
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
                            width: 1.0,
                            color: tk.card_border,
                        },
                        ..Default::default()
                    }
                })
                .into()
            }
        }
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
