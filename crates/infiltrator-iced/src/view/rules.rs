//! Rules management page (分流规则与提供者管理):
//! - Rule Tracer sandbox for testing routing matches against domain/IP queries.
//! - Custom rules list with live filter, semantic badge coloring, enable toggle, reordering, and pagination.
//! - Providers management for proxy and rule providers (with diff inspect, unpack, and update).
//! - Geo databases updater for official MetaCubeX Geo data assets.
//! - Token-driven lazy JSON editors for rule providers, proxy providers, and sniffer.

use crate::state::AppState;
use crate::types::editor::EditorLazyState;
use crate::types::message::Message;
use crate::types::rules::RuleBadgeKind;
use crate::view::components::{
    BadgeKind, badge, card, chip, editor_frame_surface, empty_state, form_field_label,
    form_input_style, form_pick_style, icon_button, kbd_badge, modern_scrollable, row_card_surface,
    search_input, section_header, segmented_control, status_dot, style_accent, style_ghost,
    text_btn, toggle_switch,
};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, MONO, SP_LG, SP_MD, tokens};
use iced::widget::{
    Space, button, column, container, pick_list, row, text, text_editor, text_input,
};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};
use infiltrator_contract::rules_workspace::{RulesJsonSection, RulesTab};
use infiltrator_domain::rules::matrix::RuleTypeFamily;
use infiltrator_domain::runtime::{ProxyProvider, RuleProvider};
use infiltrator_shared::locales::{Lang, Localizer};
use std::collections::HashMap;

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

/// Format raw rule type into the shared semantic display label (`Domain`,
/// `DomainSuffix`, `IPCIDR`, `GeoIP`, `Match`, `RuleSet`, …) for all 33
/// catalogue spellings (DUAL-11-01). Unknown spellings keep their raw text.
pub fn display_rule_type(rule_type: &str) -> String {
    infiltrator_domain::rules::matrix::matrix_label(rule_type)
}

/// Map rule type and classifier to the shared badge palette (DUAL-11-01). The
/// family comes from the shared catalogue, so every known type is colored by
/// semantics rather than by a per-surface spelling list; `kind` is only the
/// fallback for a spelling the catalogue does not know.
pub fn semantic_badge_kind(rule_type: &str, kind: RuleBadgeKind) -> BadgeKind {
    match infiltrator_domain::rules::matrix::matrix_family(rule_type) {
        RuleTypeFamily::Host => BadgeKind::Accent,
        RuleTypeFamily::Address => BadgeKind::Warning,
        RuleTypeFamily::Unknown => match kind {
            RuleBadgeKind::Domain => BadgeKind::Accent,
            RuleBadgeKind::Ip => BadgeKind::Warning,
            RuleBadgeKind::Other => BadgeKind::Neutral,
        },
        _ => BadgeKind::Neutral,
    }
}

fn editor_lazy_placeholder<'a>(title: String, on_press: Message) -> Element<'a, Message> {
    card(
        None,
        column![
            empty_state(Icon::Code2, title.as_str(), "Editor will load on demand"),
            Space::new().height(theme::SP_SM),
            text_btn("Load Editor".to_string(), style_accent, Some(on_press))
        ]
        .align_x(Alignment::Center),
    )
}

fn json_editor_card<'a>(
    title: String,
    content: &'a text_editor::Content,
    on_action: fn(text_editor::Action) -> Message,
    save_btn: Element<'a, Message>,
) -> Element<'a, Message> {
    card(
        Some(title),
        column![
            section_header("JSON", Some(save_btn)),
            Space::new().height(theme::SP_SM),
            container(
                text_editor(content)
                    .on_action(on_action)
                    .font(MONO)
                    .padding(10)
                    .height(Length::Fixed(440.0))
            )
            .width(Length::Fill)
            .style(editor_frame_surface),
        ],
    )
}

#[allow(clippy::too_many_arguments)]
fn json_tab_card<'a>(
    title: String,
    state: EditorLazyState,
    content: &'a text_editor::Content,
    dirty: bool,
    saving: bool,
    on_load: Message,
    on_action: fn(text_editor::Action) -> Message,
    on_save: Message,
    saved_text: &str,
    save_text: &str,
) -> Element<'a, Message> {
    if state == EditorLazyState::Unloaded {
        editor_lazy_placeholder(title, on_load)
    } else {
        let save_btn = save_action(
            dirty,
            saving,
            save_text.to_string(),
            saved_text.to_string(),
            on_save,
        );
        json_editor_card(title, content, on_action, save_btn)
    }
}

fn provider_icon_chip<'a>(icon: Icon, size: f32) -> Element<'a, Message> {
    container(svg_icons::icon_themed(icon, size, |t: &Theme| {
        tokens(t).accent
    }))
    .width(32)
    .height(32)
    .align_x(Alignment::Center)
    .align_y(Alignment::Center)
    .style(|t: &Theme| {
        let tk = tokens(t);
        container::Style {
            background: Some(tk.accent_soft.into()),
            border: Border {
                radius: border::Radius::from(theme::R_CONTROL),
                width: theme::HAIRLINE,
                color: Color {
                    a: 0.20,
                    ..tk.accent
                },
            },
            ..Default::default()
        }
    })
    .into()
}

fn proxy_provider_row<'a>(provider: &ProxyProvider, lang: &Lang<'_>) -> Element<'a, Message> {
    let update_btn = button(
        row![
            svg_icons::icon_themed(Icon::RefreshCw, 12.0, |t: &Theme| tokens(t).text_secondary),
            Space::new().width(4.0),
            text(lang.tr("btn_update").to_string())
                .size(11)
                .font(FONT_MEDIUM),
        ]
        .align_y(Alignment::Center),
    )
    .padding([4, 10])
    .style(style_ghost)
    .on_press(Message::UpdateProxyProvider(provider.name.clone()));

    let updated_text = if provider.updated_at.is_empty() {
        "—".to_string()
    } else {
        format!("Updated: {}", provider.updated_at)
    };
    let vehicle = if provider.vehicle_type.is_empty() {
        "HTTP"
    } else {
        &provider.vehicle_type
    };

    container(
        row![
            provider_icon_chip(Icon::Server, 16.0),
            Space::new().width(theme::SP_MD),
            column![
                text(provider.name.clone())
                    .size(13)
                    .font(FONT_SEMIBOLD)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_primary)
                    }),
                text(updated_text)
                    .size(11)
                    .font(MONO)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_secondary)
                    }),
            ]
            .width(Length::Fill),
            chip(vehicle),
            Space::new().width(theme::SP_SM),
            update_btn,
        ]
        .align_y(Alignment::Center),
    )
    .padding([theme::SP_SM, SP_MD])
    .width(Length::Fill)
    .style(row_card_surface)
    .into()
}

/// Format the behavior badge text for rule providers (`Domain`, `IPCIDR`, `Classical`).
pub fn format_provider_behavior(behavior: &str) -> &'static str {
    match behavior.to_ascii_lowercase().as_str() {
        "domain" => "Domain",
        "ipcidr" | "ip-cidr" => "IPCIDR",
        "classical" => "Classical",
        _ => "Domain",
    }
}

/// Sniff or normalize rule provider payload format (e.g. `MRS`, `YAML`, `TEXT`, `FILE`, `HTTP`).
pub fn format_rule_provider_format(provider: &RuleProvider) -> &'static str {
    let lower_name = provider.name.to_ascii_lowercase();
    let lower_type = provider.provider_type.to_ascii_lowercase();
    let lower_vehicle = provider.vehicle_type.to_ascii_lowercase();

    if lower_name.ends_with(".mrs") || lower_type == "mrs" || lower_vehicle == "mrs" {
        "MRS"
    } else if lower_name.ends_with(".txt") || lower_type == "text" {
        "TEXT"
    } else if lower_name.ends_with(".yaml") || lower_name.ends_with(".yml") || lower_type == "yaml"
    {
        "YAML"
    } else if lower_type == "file" || lower_vehicle == "file" {
        "FILE"
    } else if lower_type == "http" || lower_vehicle == "http" {
        "HTTP"
    } else {
        "YAML"
    }
}

/// Compute total external rules loaded across all active rule providers.
pub fn total_external_rules(rule_providers: &[RuleProvider]) -> u32 {
    rule_providers.iter().map(|rp| rp.rule_count).sum()
}

/// DUAL-11-04/11-05: provider lifecycle line combining the last update time
/// with the declared source URL (or an honest "not declared" for runtime-only
/// providers) and the declared automatic-refresh interval. The kernel executes
/// the schedule and owns the `ETag`/`304` conditional cache, so no cache
/// hit/miss state is invented here.
pub fn provider_lifecycle_line(
    updated_at: &str,
    source_url: Option<&str>,
    refresh_interval_secs: Option<u64>,
) -> String {
    let mut parts = Vec::new();
    if updated_at.is_empty() {
        parts.push("Updated: —".to_string());
    } else {
        parts.push(format!("Updated: {updated_at}"));
    }
    match source_url {
        Some(url) if !url.is_empty() => parts.push(format!("Source: {url}")),
        _ => parts.push("Source: not declared".to_string()),
    }
    match refresh_interval_secs {
        Some(secs) => parts.push(format!(
            "Auto: {} (kernel-scheduled)",
            infiltrator_domain::rules::view::format_refresh_interval(secs)
        )),
        None => parts.push("Auto: not declared".to_string()),
    }
    parts.join(" · ")
}

/// DUAL-11-05: the local cache-content fingerprint line. The body is the file
/// size, digest and last-modified time this client actually read; the trailing
/// label compares that local read with the previous one. It is explicitly
/// *not* an HTTP validator, and never claims the kernel skipped a download.
pub fn provider_fingerprint_line(
    observation: &infiltrator_contract::provider_cache::ProviderCacheFingerprint,
    lang: &Lang<'_>,
) -> String {
    use infiltrator_contract::provider_cache::ProviderFingerprintChange;
    let change = match observation.change {
        ProviderFingerprintChange::FirstSeen => lang.tr("rules_provider_fingerprint_first_seen"),
        ProviderFingerprintChange::Unchanged => lang.tr("rules_provider_fingerprint_unchanged"),
        ProviderFingerprintChange::Changed => lang.tr("rules_provider_fingerprint_changed"),
    };
    let body = infiltrator_domain::rules::view::format_content_fingerprint(
        &observation.current.sha256,
        observation.current.size_bytes,
        observation.current.modified_unix_secs,
    );
    format!(
        "{}: {body} · {change}",
        lang.tr("rules_provider_fingerprint_label")
    )
}

pub fn rule_provider_row<'a>(
    provider: &RuleProvider,
    source_url: Option<&str>,
    refresh_interval_secs: Option<u64>,
    fingerprint: Option<&infiltrator_contract::provider_cache::ProviderCacheFingerprint>,
    lang: &Lang<'_>,
) -> Element<'a, Message> {
    let behavior_badge_text = format_provider_behavior(&provider.behavior);
    let rule_count_str = crate::view::mrs_panel::format_rule_count(provider.rule_count);
    let format_str = format_rule_provider_format(provider);
    let updated_text =
        provider_lifecycle_line(&provider.updated_at, source_url, refresh_interval_secs);
    let fingerprint_text =
        fingerprint.map(|observation| provider_fingerprint_line(observation, lang));

    let actions = row![
        button(
            row![
                svg_icons::icon_themed(Icon::Code2, 12.0, |t: &Theme| tokens(t).text_secondary),
                Space::new().width(4.0),
                text("Diff").size(11).font(FONT_MEDIUM)
            ]
            .align_y(Alignment::Center)
        )
        .padding([4, 10])
        .style(style_ghost)
        .on_press(Message::InspectRuleProviderDiff(Some(
            provider.name.clone()
        ))),
        Space::new().width(theme::SP_XS),
        button(
            row![
                svg_icons::icon_themed(Icon::Zap, 12.0, |t: &Theme| tokens(t).text_secondary),
                Space::new().width(4.0),
                text("Unpack").size(11).font(FONT_MEDIUM)
            ]
            .align_y(Alignment::Center)
        )
        .padding([4, 10])
        .style(style_ghost)
        .on_press(Message::UnpackRuleProvider(provider.name.clone())),
        Space::new().width(theme::SP_XS),
        icon_button(
            Icon::RefreshCw,
            13.0,
            Message::UpdateRuleProvider(provider.name.clone())
        ),
    ]
    .align_y(Alignment::Center);

    container(
        row![
            provider_icon_chip(Icon::ListChecks, 16.0),
            Space::new().width(theme::SP_MD),
            {
                let mut details = column![
                    row![
                        text(provider.name.clone())
                            .size(13)
                            .font(FONT_SEMIBOLD)
                            .style(|t: &Theme| text::Style {
                                color: Some(tokens(t).text_primary)
                            }),
                        Space::new().width(theme::SP_SM),
                        badge(behavior_badge_text, BadgeKind::Neutral),
                        Space::new().width(theme::SP_XS),
                        badge(rule_count_str, BadgeKind::Accent),
                    ]
                    .align_y(Alignment::Center),
                    text(updated_text)
                        .size(11)
                        .font(MONO)
                        .style(|t: &Theme| text::Style {
                            color: Some(tokens(t).text_secondary)
                        }),
                ];
                if let Some(fingerprint_text) = fingerprint_text {
                    details = details.push(text(fingerprint_text).size(11).font(MONO).style(
                        |t: &Theme| text::Style {
                            color: Some(tokens(t).text_tertiary),
                        },
                    ));
                }
                details.width(Length::Fill)
            },
            chip(format_str),
            Space::new().width(theme::SP_SM),
            actions,
        ]
        .align_y(Alignment::Center),
    )
    .padding([theme::SP_SM, SP_MD])
    .width(Length::Fill)
    .style(row_card_surface)
    .into()
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

fn rules_list_view<'a>(
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

pub fn providers_view<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let total_ext_rules = total_external_rules(&state.editor.rule_providers);
    let mut content = column![section_header(
        "Providers",
        Some(
            row![
                chip(format!("Proxy: {}", state.editor.proxy_providers.len())),
                Space::new().width(theme::SP_XS),
                chip(format!("Rule: {}", state.editor.rule_providers.len())),
                Space::new().width(theme::SP_XS),
                chip(format!("External Rules: {total_ext_rules}")),
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
            .into()
        ),
    )]
    .spacing(theme::SP_MD);

    content = content.push(crate::view::provider_unpack_card::provider_unpack_card(
        state, lang,
    ));
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
                proxy_list = proxy_list.push(proxy_provider_row(provider, lang));
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
                let source_url = state
                    .editor
                    .rule_provider_source_urls
                    .get(&provider.name)
                    .map(String::as_str);
                let refresh_interval_secs = state
                    .editor
                    .rule_provider_intervals
                    .get(&provider.name)
                    .copied();
                let fingerprint = state.editor.rule_provider_fingerprints.get(&provider.name);
                rule_list = rule_list.push(rule_provider_row(
                    provider,
                    source_url,
                    refresh_interval_secs,
                    fingerprint,
                    lang,
                ));
            }
        }

        let update_geo_btn = if state.editor.is_updating_geo_databases {
            button(
                row![
                    svg_icons::icon_themed(Icon::RefreshCw, 12.0, |t: &Theme| tokens(t)
                        .text_secondary),
                    Space::new().width(4.0),
                    text(lang.tr("rules_updating_geo").to_string())
                        .size(12)
                        .font(FONT_MEDIUM),
                ]
                .align_y(Alignment::Center),
            )
            .padding([7, 14])
            .style(style_ghost)
        } else {
            button(
                row![
                    svg_icons::icon_themed(Icon::RefreshCw, 12.0, |t: &Theme| tokens(t).on_accent),
                    Space::new().width(4.0),
                    text(lang.tr("rules_update_geo_btn").to_string())
                        .size(12)
                        .font(FONT_MEDIUM),
                ]
                .align_y(Alignment::Center),
            )
            .padding([7, 14])
            .style(style_accent)
            .on_press(Message::UpdateGeoDatabases)
        };

        let geo_card = card(
            Some(lang.tr("rules_geo_databases_title").to_string()),
            column![
                row![
                    provider_icon_chip(Icon::Globe, 18.0),
                    Space::new().width(theme::SP_MD),
                    column![
                        text("geoip.metadb / geosite.dat / Country.mmdb")
                            .size(13)
                            .font(MONO)
                            .style(|t: &Theme| text::Style {
                                color: Some(tokens(t).text_primary)
                            }),
                        text(lang.tr("rule_repo_official").to_string())
                            .size(11)
                            .style(|t: &Theme| text::Style {
                                color: Some(tokens(t).text_secondary)
                            }),
                    ]
                    .width(Length::Fill),
                    badge("MetaCubeX", BadgeKind::Accent),
                    Space::new().width(theme::SP_XS),
                    chip("db / dat"),
                    Space::new().width(theme::SP_SM),
                    update_geo_btn,
                ]
                .align_y(Alignment::Center)
            ]
            .spacing(theme::SP_SM),
        );

        let rule_card_header = row![
            text(lang.tr("rules_rule_providers").to_string())
                .size(14)
                .font(FONT_SEMIBOLD)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_primary)
                }),
            Space::new().width(theme::SP_SM),
            chip(format!("Total External Rules: {total_ext_rules}")),
        ]
        .align_y(Alignment::Center);

        let rule_card = card(
            None,
            column![rule_card_header, rule_list].spacing(theme::SP_MD),
        );

        content = content
            .push(
                column![
                    card(
                        Some(lang.tr("rules_proxy_providers").to_string()),
                        proxy_list
                    ),
                    Space::new().height(theme::SP_MD),
                    rule_card,
                ]
                .spacing(theme::SP_MD),
            )
            .push(Space::new().height(theme::SP_MD))
            .push(geo_card);

        if let Some(mrs_panel) = crate::view::mrs_panel::mrs_card(state) {
            content = content
                .push(Space::new().height(theme::SP_MD))
                .push(mrs_panel);
        }
        content = content
            .push(Space::new().height(theme::SP_MD))
            .push(crate::view::mrs_panel::mrs_acceleration_card(state));
    }
    content.into()
}

fn json_editors_view<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    // DUAL-11-14: the section identities, order and labels are the shared
    // workspace vocabulary, not a per-surface list.
    let json_tab_labels: Vec<String> = RulesJsonSection::ALL
        .iter()
        .map(|section| lang.tr(section.i18n_key()).to_string())
        .collect();
    let json_tab_index = state.editor.rules_json_tab.index();
    let json_tab_buttons = segmented_control(&json_tab_labels, json_tab_index, |index| {
        Message::SetRulesJsonTab(RulesJsonSection::from_index(index))
    });

    let json_view = match state.editor.rules_json_tab {
        RulesJsonSection::RuleProviders => json_tab_card(
            lang.tr("rules_rule_providers_json").to_string(),
            state.editor.rule_providers_editor_state,
            &state.editor.rule_providers_json_content,
            state.editor.rule_providers_json_dirty,
            state.editor.is_saving_rule_providers_json,
            Message::EnsureRuleProvidersEditorLoaded,
            Message::RuleProvidersEditorAction,
            Message::SaveRuleProvidersJson,
            lang.tr("rules_saved").as_ref(),
            lang.tr("rules_save_rule_providers_btn").as_ref(),
        ),
        RulesJsonSection::ProxyProviders => json_tab_card(
            lang.tr("rules_proxy_providers_json").to_string(),
            state.editor.proxy_providers_editor_state,
            &state.editor.proxy_providers_json_content,
            state.editor.proxy_providers_json_dirty,
            state.editor.is_saving_proxy_providers_json,
            Message::EnsureProxyProvidersEditorLoaded,
            Message::ProxyProvidersEditorAction,
            Message::SaveProxyProvidersJson,
            lang.tr("rules_saved").as_ref(),
            lang.tr("rules_save_proxy_providers_btn").as_ref(),
        ),
        RulesJsonSection::Sniffer => json_tab_card(
            lang.tr("rules_sniffer_json").to_string(),
            state.editor.sniffer_editor_state,
            &state.editor.sniffer_json_content,
            state.editor.sniffer_json_dirty,
            state.editor.is_saving_sniffer_json,
            Message::EnsureSnifferEditorLoaded,
            Message::SnifferEditorAction,
            Message::SaveSnifferJson,
            lang.tr("rules_saved").as_ref(),
            lang.tr("rules_save_sniffer_btn").as_ref(),
        ),
    };

    column![
        json_tab_buttons,
        Space::new().height(theme::SP_MD),
        json_view
    ]
    .spacing(theme::SP_SM)
    .into()
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
                color: Some(tokens(t).text_primary)
            }),
        Space::new().width(theme::SP_MD),
        text(format!("{} / {}", filtered_count, state.editor.rules.len()))
            .size(13)
            .font(MONO)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_tertiary)
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

    // DUAL-11-14: the partition list, its order and its i18n keys are the
    // shared workspace vocabulary.
    let tab_labels: Vec<String> = RulesTab::ALL
        .iter()
        .map(|tab| lang.tr(tab.i18n_key()).to_string())
        .collect();
    let tab_index = state.editor.rules_tab.index();
    let tabs = segmented_control(&tab_labels, tab_index, |index| {
        Message::SetRulesTab(RulesTab::from_index(index))
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
                            color: Some(tokens(t).text_primary)
                        }),
                    text("Heavy widgets mount asynchronously to keep first paint responsive.")
                        .size(12)
                        .style(|t: &Theme| text::Style {
                            color: Some(tokens(t).text_secondary)
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
        .filter(|(_, p): &(&String, &infiltrator_domain::proxy::Proxy)| p.is_group())
        .map(|(name, _)| name.clone())
        .collect();
    available_targets.sort();
    if !available_targets.contains(&"DIRECT".to_string()) {
        available_targets.push("DIRECT".to_string());
    }
    if !available_targets.contains(&"REJECT".to_string()) {
        available_targets.push("REJECT".to_string());
    }

    let tab_content: Element<'_, Message> = match state.editor.rules_tab {
        RulesTab::List => rules_list_view(state, &lang, available_targets),
        RulesTab::Providers => providers_view(state, &lang),
        RulesTab::JsonEditors => json_editors_view(state, &lang),
        RulesTab::Tracer => crate::view::rules_tracer::tracer_view(state, &lang),
    };

    column![
        header,
        Space::new().height(theme::SP_MD),
        tabs,
        Space::new().height(theme::SP_MD),
        tab_content
    ]
    .spacing(SP_LG)
    .into()
}

#[cfg(test)]
#[path = "../../tests/gui/view_rules_tests.rs"]
mod tests;
