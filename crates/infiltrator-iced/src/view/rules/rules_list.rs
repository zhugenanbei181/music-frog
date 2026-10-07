//! Custom rules list: hit statistics, the tracer/add panels, the virtualised
//! list window and the publish-truncation note.

use super::{display_rule_type, semantic_badge_kind};
use crate::state::AppState;
use crate::types::message::Message;
use crate::types::rule_list::RuleListAction;
use crate::view::component_card::card;
use crate::view::component_forms::{
    form_field_label, form_input_style, form_pick_style, row_card_surface, search_input,
    style_accent, style_ghost, text_btn,
};
use crate::view::components::{
    badge, empty_state, icon_button, kbd_badge, modern_scrollable, toggle_switch,
};
use crate::view::rule_hit_card::rule_hit_card;
use crate::view::rules_window::{
    RULES_LIST_SCROLL_ID, rules_window_page, rules_window_range, rules_window_spacers,
    visible_rule_items,
};
use crate::view::subrules_builder::subrules_panel;
use crate::view::svg_icons::Icon;
use crate::view::theme::{FONT_MEDIUM, FONT_SEMIBOLD, MONO, SP_MD, tokens};
use crate::view::{svg_icons, theme};
use iced::widget::scrollable::Viewport;
use iced::widget::{Id, Space, button, column, container, pick_list, row, text, text_input};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};
use infiltrator_application::rule_row_projection::{row_detail, row_hits_copy};
use infiltrator_contract::rule_document::RuleRowId;
use infiltrator_domain::rules::edit::CUSTOM_RULE_TYPE_CHOICES;
use infiltrator_domain::rules::view::{RULE_ROW_HEIGHT_PX, page_count};
use infiltrator_shared::i18n_interpolator::interpolate;
use infiltrator_shared::locales::{Lang, Localizer};

/// Rule hit statistics and recency metadata.
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

pub(crate) fn row_region_id(id: RuleRowId) -> Id {
    Id::from(format!("rule-list-row-{}", id.0))
}
fn add_rule_panel<'a>(
    state: &'a AppState,
    lang: &Lang<'_>,
    available_targets: Vec<String>,
) -> Element<'a, Message> {
    let rule_types = CUSTOM_RULE_TYPE_CHOICES
        .iter()
        .map(|choice| (*choice).to_string())
        .collect::<Vec<String>>();
    let add_rule_btn_style = if state.editor.is_adding_rule {
        style_ghost
    } else {
        style_accent
    };

    let fields = row![
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
                .on_press_maybe(
                    state
                        .editor
                        .rule_form_binding
                        .current(&state.editor.rule_list)
                        .then_some(Message::AddCustomRule)
                ),
                Space::new().width(theme::SP_SM),
                text_btn(
                    lang.tr("rules_inject_game_presets").to_string(),
                    style_ghost,
                    state
                        .editor
                        .rule_form_binding
                        .current(&state.editor.rule_list)
                        .then_some(Message::ApplyGameRoutingPresets)
                ),
            ]
            .align_y(Alignment::Center),
        ],
    ]
    .align_y(Alignment::Center);
    let reset = text_btn(
        lang.tr("rules_form_discard").to_string(),
        style_ghost,
        (state.editor.rule_list.pending.is_none() && !state.editor.rule_list.awaiting_read)
            .then_some(Message::RuleList(RuleListAction::DiscardForm)),
    );
    card(
        Some(lang.tr("rules_add_custom").to_string()),
        column![
            fields,
            text(
                state
                    .editor
                    .rule_form_binding
                    .status(&state.editor.rule_list, &state.shell.lang)
            )
            .size(12),
            reset,
        ]
        .spacing(theme::SP_SM),
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
    Some(interpolate(
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
    let add_rule_form = add_rule_panel(state, lang, available_targets);

    let search_bar = search_input(
        lang.tr("rules_filter_placeholder").as_ref(),
        &state.editor.rules_filter,
        Message::FilterRules,
        Message::FilterRules(String::new()),
    );

    let page_size = state.editor.rules_page_size.max(1);
    let total_count = state.editor.rules_filtered_indices.len();
    let total_pages = page_count(total_count, page_size);
    // DUAL-11-08: the rendered band is the shared virtual window of the
    // filtered list — fixed-height rows, visible band + overscan, spacers for
    // the rest. The list length never enters the row count.
    let visible = visible_rule_items(state);
    let (top_spacer, bottom_spacer) = rules_window_spacers(state);
    let (first_shown, last_shown) = rules_window_range(state);
    let current_page = rules_window_page(state);

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
            let Some(row_id) = state.editor.rule_list.row_id(source_index) else {
                continue;
            };
            let Some(entry) = state.editor.rule_list.draft.get(source_index) else {
                continue;
            };
            let is_enabled = entry.enabled;
            let bkind = semantic_badge_kind(&item.rule_type, item.badge);
            let display_type = display_rule_type(&item.rule_type);

            let up_button = if source_index > 0 {
                icon_button(Icon::ArrowUp, 13.0, Message::MoveRuleUp(row_id))
            } else {
                Space::new().width(26).height(26).into()
            };
            let down_button = if source_index + 1 < state.editor.rule_list.draft.len() {
                icon_button(Icon::ArrowDown, 13.0, Message::MoveRuleDown(row_id))
            } else {
                Space::new().width(26).height(26).into()
            };

            let target_label = if item.target.is_empty() {
                "—"
            } else {
                &item.target
            };
            let detail = row_detail(
                item.source_ip,
                item.no_resolve,
                item.failure.as_ref(),
                &state.shell.lang,
            );

            rules_list = rules_list.push(
                container(
                    row![
                        toggle_switch(is_enabled, move |_| Message::ToggleRuleEnabled(row_id)),
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
                            text(detail).size(11),
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
                        text(row_hits_copy(
                            item.hit_count,
                            is_enabled,
                            item.is_shadowed,
                            &state.shell.lang
                        ))
                        .size(11),
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
                .id(row_region_id(row_id))
                .width(Length::Fill)
                // DUAL-11-08: fixed-height rows are what makes the window
                // arithmetic exact; content that would overflow is clipped
                // instead of pushing the row taller.
                .height(Length::Fixed(RULE_ROW_HEIGHT_PX))
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
            text(interpolate(
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
        .id(Id::new(RULES_LIST_SCROLL_ID))
        .on_scroll(|viewport: Viewport| {
            let offset = viewport.absolute_offset();
            Message::RulesListScrolled {
                offset_px: offset.y,
                viewport_px: viewport.bounds().height,
            }
        })
        .height(Length::Fixed(
            state.editor.rules_viewport_px.max(RULE_ROW_HEIGHT_PX),
        ));

    column![
        search_bar,
        Space::new().height(theme::SP_SM),
        pager,
        Space::new().height(theme::SP_SM),
        window_scroller,
        Space::new().height(theme::SP_MD),
        add_rule_form,
        Space::new().height(theme::SP_MD),
        subrules_panel(state, lang),
        Space::new().height(theme::SP_MD),
        rule_hit_card(state, lang),
    ]
    .spacing(theme::SP_SM)
    .into()
}
