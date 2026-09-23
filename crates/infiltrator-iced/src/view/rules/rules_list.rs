//! Custom rules list: hit statistics, the tracer/add panels, the virtualised
//! list window and the publish-truncation note.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::{
    BadgeKind, badge, card, empty_state, form_field_label, form_input_style, form_pick_style,
    icon_button, kbd_badge, modern_scrollable, row_card_surface, search_input, status_dot,
    style_accent, style_ghost, text_btn, toggle_switch,
};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, MONO, SP_MD, tokens};
use iced::widget::{Space, button, column, container, pick_list, row, text, text_input};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};
use infiltrator_shared::locales::{Lang, Localizer};
use std::collections::HashMap;

use super::{display_rule_type, semantic_badge_kind};

/// Rule hit statistics and recency metadata.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuleHitStats {
    pub count: usize,
    pub is_recent: bool,
}

fn normalize_rule_name(rule_type: &str) -> String {
    rule_type
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase()
}

/// Compute live hit statistics and recency for rules from active connection snapshots.
pub fn compute_rule_hit_stats(state: &AppState) -> HashMap<String, RuleHitStats> {
    let mut stats: HashMap<String, RuleHitStats> = HashMap::new();
    let Some(snapshot) = &state.diag.connections else {
        return stats;
    };

    for conn in &snapshot.connections {
        let norm_rule = normalize_rule_name(&conn.rule);
        let norm_payload = conn.rule_payload.trim().to_ascii_lowercase();

        stats
            .entry(format!("{norm_rule}:{norm_payload}"))
            .or_insert(RuleHitStats {
                count: 0,
                is_recent: true,
            })
            .count += 1;
        if !norm_payload.is_empty() {
            stats
                .entry(format!("p:{norm_payload}"))
                .or_insert(RuleHitStats {
                    count: 0,
                    is_recent: true,
                })
                .count += 1;
        }
        let host = conn.metadata.host.trim().to_ascii_lowercase();
        if !host.is_empty() && host != norm_payload {
            stats
                .entry(format!("h:{host}"))
                .or_insert(RuleHitStats {
                    count: 0,
                    is_recent: true,
                })
                .count += 1;
        }
        if norm_rule == "match" {
            stats
                .entry("match:".to_string())
                .or_insert(RuleHitStats {
                    count: 0,
                    is_recent: true,
                })
                .count += 1;
        }
    }
    stats
}

/// Look up hit count and recency metadata for a specific rule item.
pub fn lookup_hit_stats(
    stats: &HashMap<String, RuleHitStats>,
    rule_type: &str,
    payload: &str,
) -> RuleHitStats {
    let norm_rule = normalize_rule_name(rule_type);
    let norm_payload = payload.trim().to_ascii_lowercase();

    if norm_rule == "match"
        && let Some(s) = stats.get("match:")
    {
        return *s;
    }
    if let Some(s) = stats.get(&format!("{norm_rule}:{norm_payload}")) {
        return *s;
    }
    if !norm_payload.is_empty() {
        if let Some(s) = stats.get(&format!("p:{norm_payload}")) {
            return *s;
        }
        if let Some(s) = stats.get(&format!("h:{norm_payload}")) {
            return *s;
        }
    }
    RuleHitStats::default()
}

/// Save / Saving… / Saved action used across rules panels.
pub(super) fn save_action(
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
/// Render an enhanced target group pill with an appropriate icon and status-aware styling.
fn target_group_pill<'a>(target: &str, is_enabled: bool) -> Element<'a, Message> {
    let target_upper = target.to_ascii_uppercase();
    let (icon, is_direct, is_reject) = if target_upper.starts_with("DIRECT") {
        (Icon::Zap, true, false)
    } else if target_upper.starts_with("REJECT") {
        (Icon::Shield, false, true)
    } else {
        (Icon::Target, false, false)
    };

    container(
        row![
            svg_icons::icon_themed(icon, 12.0, move |t: &Theme| {
                let tk = tokens(t);
                if !is_enabled {
                    tk.text_tertiary
                } else if is_reject {
                    tk.danger
                } else if is_direct {
                    tk.success
                } else {
                    tk.accent
                }
            }),
            Space::new().width(4.0),
            text(target.to_string())
                .size(11)
                .font(FONT_SEMIBOLD)
                .style(move |t: &Theme| {
                    let tk = tokens(t);
                    text::Style {
                        color: Some(if !is_enabled {
                            tk.text_tertiary
                        } else if is_reject {
                            tk.danger
                        } else if is_direct {
                            tk.success
                        } else {
                            tk.text_primary
                        }),
                    }
                }),
        ]
        .align_y(Alignment::Center),
    )
    .padding([3, 10])
    .style(move |t: &Theme| {
        let tk = tokens(t);
        let border_color = if !is_enabled {
            tk.card_border
        } else if is_reject {
            Color {
                a: 0.25,
                ..tk.danger
            }
        } else if is_direct {
            Color {
                a: 0.25,
                ..tk.success
            }
        } else {
            Color {
                a: 0.25,
                ..tk.accent
            }
        };
        let bg_color = if !is_enabled {
            tk.chip_bg
        } else if is_reject {
            Color {
                a: 0.10,
                ..tk.danger
            }
        } else if is_direct {
            Color {
                a: 0.10,
                ..tk.success
            }
        } else {
            Color {
                a: 0.08,
                ..tk.accent
            }
        };
        container::Style {
            background: Some(bg_color.into()),
            border: Border {
                radius: border::Radius::from(theme::R_CHIP),
                width: theme::HAIRLINE,
                color: border_color,
            },
            ..Default::default()
        }
    })
    .into()
}

/// Hit counter badge and recent hit indicator.
fn hit_stats_badge<'a>(stats: RuleHitStats, lang: &Lang<'_>) -> Element<'a, Message> {
    if stats.count > 0 {
        let hits_text = infiltrator_shared::i18n_interpolator::interpolate(
            &lang.tr("rule_tracer_hits_count"),
            &[("count", &stats.count.to_string())],
        );
        row![
            status_dot(true),
            Space::new().width(theme::SP_XS),
            text(hits_text)
                .size(11)
                .font(FONT_MEDIUM)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).success)
                }),
            Space::new().width(theme::SP_XS),
            badge(
                lang.tr("rule_tracer_recent_hits").to_string(),
                BadgeKind::Success
            ),
        ]
        .align_y(Alignment::Center)
        .into()
    } else {
        row![
            text(lang.tr("rule_tracer_zero_hits").to_string())
                .size(11)
                .font(MONO)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_tertiary)
                })
        ]
        .align_y(Alignment::Center)
        .into()
    }
}

fn tracer_panel<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    crate::view::rules_tracer::inline_tracer_panel(state, lang)
}

fn add_rule_panel<'a>(
    state: &'a AppState,
    lang: &Lang<'_>,
    available_targets: Vec<String>,
) -> Element<'a, Message> {
    let rule_types = infiltrator_domain::rules::edit::CUSTOM_RULE_TYPE_CHOICES
        .iter()
        .map(|choice| (*choice).to_string())
        .collect::<Vec<String>>();
    let add_rule_btn_style = if state.editor.is_adding_rule {
        style_ghost
    } else {
        style_accent
    };

    card(
        Some(lang.tr("rules_add_custom").to_string()),
        row![
            column![
                form_field_label(lang.tr("rules_type").to_string()),
                Space::new().height(theme::SP_XS),
                pick_list(
                    rule_types,
                    Some(&state.editor.new_rule_type),
                    Message::UpdateNewRuleType
                )
                .width(Length::Fill)
                .style(form_pick_style),
            ]
            .width(Length::FillPortion(1)),
            Space::new().width(theme::SP_LG),
            column![
                form_field_label(lang.tr("rules_payload").to_string()),
                Space::new().height(theme::SP_XS),
                text_input("e.g. google.com", &state.editor.new_rule_payload)
                    .on_input(Message::UpdateNewRulePayload)
                    .padding([8, 12])
                    .size(12)
                    .font(MONO)
                    .style(form_input_style),
            ]
            .width(Length::FillPortion(2)),
            Space::new().width(theme::SP_LG),
            column![
                form_field_label(lang.tr("rules_target").to_string()),
                Space::new().height(theme::SP_XS),
                pick_list(
                    available_targets,
                    Some(&state.editor.new_rule_target),
                    Message::UpdateNewRuleTarget
                )
                .width(Length::Fill)
                .style(form_pick_style),
            ]
            .width(Length::FillPortion(1)),
            Space::new().width(theme::SP_LG),
            column![
                Space::new().height(18.0),
                row![
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
                        .spacing(theme::SP_SM)
                    )
                    .padding([8, 16])
                    .style(add_rule_btn_style)
                    .on_press(Message::AddCustomRule),
                    Space::new().width(theme::SP_SM),
                    text_btn(
                        lang.tr("rules_inject_game_presets").to_string(),
                        style_ghost,
                        Some(Message::ApplyGameRoutingPresets)
                    ),
                ]
                .align_y(Alignment::Center),
            ],
        ]
        .align_y(Alignment::Center),
    )
}

/// DUAL-11-08: honest note shown only while the shared read model reports a
/// truncated published view. The Iced editor list is loaded in full from the
/// profile, so this states the publish cap instead of pretending a windowed
/// O(1) render exists.
pub fn publish_truncation_line(state: &AppState, lang: &Lang<'_>) -> Option<String> {
    let omitted = state
        .editor
        .rule_publish_omitted
        .filter(|omitted| *omitted > 0)?;
    Some(infiltrator_shared::i18n_interpolator::interpolate(
        &lang.tr("rules_publish_truncated"),
        &[
            ("omitted", &omitted.to_string()),
            ("limit", &state.editor.rule_publish_limit.to_string()),
        ],
    ))
}

fn publish_truncation_note<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    match publish_truncation_line(state, lang) {
        Some(line) => text(line)
            .size(11)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).warning),
            })
            .into(),
        None => Space::new().width(0).height(0).into(),
    }
}
pub(super) fn rules_list_view<'a>(
    state: &'a AppState,
    lang: &Lang<'_>,
    available_targets: Vec<String>,
) -> Element<'a, Message> {
    let tracer_card = tracer_panel(state, lang);
    let add_rule_form = add_rule_panel(state, lang, available_targets);
    let hit_stats_map = compute_rule_hit_stats(state);

    let search_bar = search_input(
        lang.tr("rules_filter_placeholder").as_ref(),
        &state.editor.rules_filter,
        Message::FilterRules,
        Message::FilterRules(String::new()),
    );

    let page_size = state.editor.rules_page_size.max(1);
    let total_count = state.editor.rules_filtered_indices.len();
    let total_pages = infiltrator_domain::rules::view::page_count(total_count, page_size);
    // DUAL-11-08: the rendered band is the shared virtual window of the
    // filtered list — fixed-height rows, visible band + overscan, spacers for
    // the rest. The list length never enters the row count.
    let visible = crate::view::rules_window::visible_rule_items(state);
    let (top_spacer, bottom_spacer) = crate::view::rules_window::rules_window_spacers(state);
    let (first_shown, last_shown) = crate::view::rules_window::rules_window_range(state);
    let current_page = crate::view::rules_window::rules_window_page(state);

    let mut rules_list = column![].spacing(0.0);
    if total_count == 0 {
        rules_list = rules_list.push(empty_state(
            Icon::ListChecks,
            lang.tr("rules_empty").as_ref(),
            "",
        ));
    } else {
        if top_spacer > 0.0 {
            rules_list = rules_list.push(Space::new().height(Length::Fixed(top_spacer)));
        }
        for cache_index in &visible {
            let Some(item) = state.editor.rules_render_cache.get(*cache_index) else {
                continue;
            };
            let source_index = item.source_index;
            let Some(entry) = state.editor.rules.get(source_index) else {
                continue;
            };
            let is_enabled = entry.enabled;
            let bkind = semantic_badge_kind(&item.rule_type, item.badge);
            let display_type = display_rule_type(&item.rule_type);
            let hit_stats = lookup_hit_stats(&hit_stats_map, &item.rule_type, &item.payload);

            let up_button = if source_index > 0 {
                icon_button(Icon::ArrowUp, 13.0, Message::MoveRuleUp(source_index))
            } else {
                Space::new().width(26).height(26).into()
            };
            let down_button = if source_index + 1 < state.editor.rules.len() {
                icon_button(Icon::ArrowDown, 13.0, Message::MoveRuleDown(source_index))
            } else {
                Space::new().width(26).height(26).into()
            };

            let target_label = if item.target.is_empty() {
                if item.rule_type.eq_ignore_ascii_case("MATCH") {
                    if !item.payload.is_empty() {
                        &item.payload
                    } else {
                        "DIRECT"
                    }
                } else {
                    "—"
                }
            } else {
                &item.target
            };

            rules_list = rules_list.push(
                container(
                    row![
                        toggle_switch(is_enabled, move |_| Message::ToggleRuleEnabled(
                            source_index
                        )),
                        Space::new().width(theme::SP_SM),
                        badge(display_type, bkind),
                        Space::new().width(theme::SP_MD),
                        column![
                            text(if item.payload.is_empty() {
                                if item.rule_type.eq_ignore_ascii_case("MATCH") {
                                    lang.tr("rule_match_all").to_string()
                                } else {
                                    "—".to_string()
                                }
                            } else {
                                item.payload.clone()
                            })
                            .size(13)
                            .font(MONO)
                            .style(move |t: &Theme| text::Style {
                                color: Some(if is_enabled {
                                    tokens(t).text_primary
                                } else {
                                    tokens(t).text_tertiary
                                }),
                            }),
                            if !item.target.is_empty()
                                && !item.rule_type.eq_ignore_ascii_case("MATCH")
                            {
                                Element::from(
                                    row![
                                        text("➔ ").size(11).style(|t: &Theme| text::Style {
                                            color: Some(tokens(t).text_tertiary)
                                        }),
                                        text(item.target.clone()).size(11).font(FONT_MEDIUM).style(
                                            move |t: &Theme| text::Style {
                                                color: Some(if is_enabled {
                                                    tokens(t).text_secondary
                                                } else {
                                                    tokens(t).text_tertiary
                                                }),
                                            }
                                        ),
                                    ]
                                    .align_y(Alignment::Center),
                                )
                            } else {
                                Element::from(Space::new().width(0).height(0))
                            },
                        ]
                        .width(Length::Fill),
                        hit_stats_badge(hit_stats, lang),
                        Space::new().width(theme::SP_MD),
                        target_group_pill(target_label, is_enabled),
                        Space::new().width(theme::SP_SM),
                        row![up_button, down_button]
                            .spacing(2)
                            .align_y(Alignment::Center),
                    ]
                    .align_y(Alignment::Center),
                )
                .padding([theme::SP_SM, SP_MD])
                .width(Length::Fill)
                // DUAL-11-08: fixed-height rows are what makes the window
                // arithmetic exact; content that would overflow is clipped
                // instead of pushing the row taller.
                .height(Length::Fixed(
                    infiltrator_domain::rules::view::RULE_ROW_HEIGHT_PX,
                ))
                .clip(true)
                .style(row_card_surface),
            );
        }
        if bottom_spacer > 0.0 {
            rules_list = rules_list.push(Space::new().height(Length::Fixed(bottom_spacer)));
        }
    }

    let pager = row![
        row![
            text(infiltrator_shared::i18n_interpolator::interpolate(
                &lang.tr("rule_rules_showing"),
                &[
                    ("start", &first_shown.to_string()),
                    ("end", &last_shown.to_string()),
                    ("total", &total_count.to_string()),
                ],
            ))
            .size(12)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary)
            }),
            Space::new().width(theme::SP_SM),
            kbd_badge(format!("{}/{}", current_page + 1, total_pages)),
        ]
        .align_y(Alignment::Center),
        Space::new().width(Length::Fill),
        publish_truncation_note(state, lang),
        Space::new().width(theme::SP_MD),
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

    // DUAL-11-08: the viewport itself is the scroll driver — its measured
    // offset and height are published back as `RulesListScrolled`, which is
    // the only thing the render window depends on.
    let window_scroller = modern_scrollable(rules_list)
        .id(iced::widget::Id::new(
            crate::view::rules_window::RULES_LIST_SCROLL_ID,
        ))
        .on_scroll(|viewport: iced::widget::scrollable::Viewport| {
            let offset = viewport.absolute_offset();
            Message::RulesListScrolled {
                offset_px: offset.y,
                viewport_px: viewport.bounds().height,
            }
        })
        .height(Length::Fixed(
            state
                .editor
                .rules_viewport_px
                .max(infiltrator_domain::rules::view::RULE_ROW_HEIGHT_PX),
        ));

    column![
        tracer_card,
        Space::new().height(theme::SP_MD),
        crate::view::rule_hit_card::rule_hit_card(state, lang),
        Space::new().height(theme::SP_MD),
        crate::view::subrules_builder::subrules_panel(state, lang),
        Space::new().height(theme::SP_MD),
        add_rule_form,
        Space::new().height(theme::SP_MD),
        search_bar,
        Space::new().height(theme::SP_SM),
        pager,
        Space::new().height(theme::SP_SM),
        window_scroller,
    ]
    .spacing(theme::SP_SM)
    .into()
}
