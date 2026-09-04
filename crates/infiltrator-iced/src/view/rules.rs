//! Rules management page (分流规则与提供者管理):
//! - Rule Tracer sandbox for testing routing matches against domain/IP queries.
//! - Custom rules list with live filter, semantic badge coloring, enable toggle, reordering, and pagination.
//! - Providers management for proxy and rule providers (with diff inspect, unpack, and update).
//! - Geo databases updater for official MetaCubeX Geo data assets.
//! - Token-driven lazy JSON editors for rule providers, proxy providers, and sniffer.

use crate::state::AppState;
use crate::types::editor::EditorLazyState;
use crate::types::message::Message;
use crate::types::rules::{RuleBadgeKind, RulesJsonTab, RulesTab};
use crate::view::components::{
    BadgeKind, badge, card, chip, editor_frame_surface, empty_state,
    form_field_label, form_input_style, form_pick_style, icon_button, kbd_badge,
    modern_scrollable, row_card_surface, search_input, section_header, segmented_control,
    status_dot, style_accent, style_ghost, text_btn, toggle_switch,
};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, MONO, SP_LG, SP_MD, tokens};
use iced::widget::{
    Space, button, column, container, pick_list, row, text, text_editor, text_input,
};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};
use infiltrator_shared::locales::{Lang, Localizer};
use std::collections::HashMap;

/// Rule hit statistics and recency metadata.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuleHitStats {
    pub count: usize,
    pub is_recent: bool,
}

fn normalize_rule_name(rule_type: &str) -> String {
    rule_type.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_ascii_lowercase()
}

/// Compute live hit statistics and recency for rules from active connection snapshots.
pub fn compute_rule_hit_stats(state: &AppState) -> HashMap<String, RuleHitStats> {
    let mut stats: HashMap<String, RuleHitStats> = HashMap::new();
    let Some(snapshot) = &state.diag.connections else { return stats; };

    for conn in &snapshot.connections {
        let norm_rule = normalize_rule_name(&conn.rule);
        let norm_payload = conn.rule_payload.trim().to_ascii_lowercase();

        stats.entry(format!("{norm_rule}:{norm_payload}")).or_insert(RuleHitStats { count: 0, is_recent: true }).count += 1;
        if !norm_payload.is_empty() {
            stats.entry(format!("p:{norm_payload}")).or_insert(RuleHitStats { count: 0, is_recent: true }).count += 1;
        }
        let host = conn.metadata.host.trim().to_ascii_lowercase();
        if !host.is_empty() && host != norm_payload {
            stats.entry(format!("h:{host}")).or_insert(RuleHitStats { count: 0, is_recent: true }).count += 1;
        }
        if norm_rule == "match" {
            stats.entry("match:".to_string()).or_insert(RuleHitStats { count: 0, is_recent: true }).count += 1;
        }
    }
    stats
}

/// Look up hit count and recency metadata for a specific rule item.
pub fn lookup_hit_stats(stats: &HashMap<String, RuleHitStats>, rule_type: &str, payload: &str) -> RuleHitStats {
    let norm_rule = normalize_rule_name(rule_type);
    let norm_payload = payload.trim().to_ascii_lowercase();

    if norm_rule == "match"
        && let Some(s) = stats.get("match:") { return *s; }
    if let Some(s) = stats.get(&format!("{norm_rule}:{norm_payload}")) { return *s; }
    if !norm_payload.is_empty() {
        if let Some(s) = stats.get(&format!("p:{norm_payload}")) { return *s; }
        if let Some(s) = stats.get(&format!("h:{norm_payload}")) { return *s; }
    }
    RuleHitStats::default()
}

/// Save / Saving… / Saved action used across rules panels.
fn save_action(dirty: bool, saving: bool, label: String, saved: String, on_press: Message) -> Element<'static, Message> {
    if saving {
        text_btn("Saving...".to_string(), style_ghost, None)
    } else if dirty {
        text_btn(label, style_accent, Some(on_press))
    } else {
        text_btn(saved, style_ghost, None)
    }
}

/// Format raw rule type into semantic display label (`Domain`, `DomainSuffix`, `IPCIDR`, `GeoIP`, `Match`, `RuleSet`).
pub fn display_rule_type(rule_type: &str) -> String {
    match rule_type.to_ascii_uppercase().as_str() {
        "DOMAIN" => "Domain".to_string(),
        "DOMAIN-SUFFIX" | "DOMAINSUFFIX" => "DomainSuffix".to_string(),
        "DOMAIN-KEYWORD" | "DOMAINKEYWORD" => "DomainKeyword".to_string(),
        "IP-CIDR" | "IPCIDR" => "IPCIDR".to_string(),
        "IP-CIDR6" | "IPCIDR6" => "IPCIDR6".to_string(),
        "IP-ASN" | "IPASN" => "IPASN".to_string(),
        "GEOIP" => "GeoIP".to_string(),
        "GEOSITE" => "GeoSite".to_string(),
        "MATCH" => "Match".to_string(),
        "RULE-SET" | "RULESET" => "RuleSet".to_string(),
        "PROCESS-NAME" | "PROCESSNAME" => "ProcessName".to_string(),
        "AND" => "And".to_string(),
        "OR" => "Or".to_string(),
        "NOT" => "Not".to_string(),
        "SUB-RULE" | "SUBRULE" => "SubRule".to_string(),
        other => if other.is_empty() { "Rule".to_string() } else { rule_type.to_string() },
    }
}

/// Map rule type and classifier to the shared badge palette.
pub fn semantic_badge_kind(rule_type: &str, kind: RuleBadgeKind) -> BadgeKind {
    match rule_type.to_ascii_uppercase().replace(['-', '_'], "").as_str() {
        "DOMAIN" | "DOMAINSUFFIX" | "DOMAINKEYWORD" | "RULESET" => BadgeKind::Accent,
        "IPCIDR" | "IPCIDR6" | "IPASN" => BadgeKind::Warning,
        "GEOIP" | "GEOSITE" | "MATCH" => BadgeKind::Neutral,
        _ => match kind {
            RuleBadgeKind::Domain => BadgeKind::Accent,
            RuleBadgeKind::Ip => BadgeKind::Warning,
            RuleBadgeKind::Other => BadgeKind::Neutral,
        },
    }
}

fn editor_lazy_placeholder<'a>(title: String, on_press: Message) -> Element<'a, Message> {
    card(None, column![empty_state(Icon::Code2, title.as_str(), "Editor will load on demand"), Space::new().height(theme::SP_SM), text_btn("Load Editor".to_string(), style_accent, Some(on_press))].align_x(Alignment::Center))
}

fn json_editor_card<'a>(title: String, content: &'a text_editor::Content, on_action: fn(text_editor::Action) -> Message, save_btn: Element<'a, Message>) -> Element<'a, Message> {
    card(Some(title), column![
        section_header("JSON", Some(save_btn)), Space::new().height(theme::SP_SM),
        container(text_editor(content).on_action(on_action).font(MONO).padding(10).height(Length::Fixed(440.0))).width(Length::Fill).style(editor_frame_surface),
    ])
}

#[allow(clippy::too_many_arguments)]
fn json_tab_card<'a>(
    title: String, state: EditorLazyState, content: &'a text_editor::Content,
    dirty: bool, saving: bool, on_load: Message, on_action: fn(text_editor::Action) -> Message,
    on_save: Message, saved_text: &str, save_text: &str,
) -> Element<'a, Message> {
    if state == EditorLazyState::Unloaded {
        editor_lazy_placeholder(title, on_load)
    } else {
        let save_btn = save_action(dirty, saving, save_text.to_string(), saved_text.to_string(), on_save);
        json_editor_card(title, content, on_action, save_btn)
    }
}

fn provider_icon_chip<'a>(icon: Icon, size: f32) -> Element<'a, Message> {
    container(svg_icons::icon_themed(icon, size, |t: &Theme| tokens(t).accent))
        .width(32).height(32).align_x(Alignment::Center).align_y(Alignment::Center)
        .style(|t: &Theme| {
            let tk = tokens(t);
            container::Style {
                background: Some(tk.accent_soft.into()),
                border: Border { radius: border::Radius::from(theme::R_CONTROL), width: 1.0, color: Color { a: 0.20, ..tk.accent } },
                ..Default::default()
            }
        }).into()
}

fn proxy_provider_row<'a>(provider: &mihomo_api::types::ProxyProvider, lang: &Lang<'_>) -> Element<'a, Message> {
    let update_btn = button(row![
        svg_icons::icon_themed(Icon::RefreshCw, 12.0, |t: &Theme| tokens(t).text_secondary), Space::new().width(4.0), text(lang.tr("btn_update").to_string()).size(11).font(FONT_MEDIUM),
    ].align_y(Alignment::Center)).padding([4, 10]).style(style_ghost).on_press(Message::UpdateProxyProvider(provider.name.clone()));

    let updated_text = if provider.updated_at.is_empty() { "—".to_string() } else { format!("Updated: {}", provider.updated_at) };
    let vehicle = if provider.vehicle_type.is_empty() { "HTTP" } else { &provider.vehicle_type };

    container(row![
        provider_icon_chip(Icon::Server, 16.0), Space::new().width(theme::SP_MD),
        column![
            text(provider.name.clone()).size(13).font(FONT_SEMIBOLD).style(|t: &Theme| text::Style { color: Some(tokens(t).text_primary) }),
            text(updated_text).size(11).font(MONO).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
        ].width(Length::Fill),
        chip(vehicle), Space::new().width(theme::SP_SM), update_btn,
    ].align_y(Alignment::Center)).padding([theme::SP_SM, SP_MD]).width(Length::Fill).style(row_card_surface).into()
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
pub fn format_rule_provider_format(provider: &mihomo_api::types::RuleProvider) -> &'static str {
    let lower_name = provider.name.to_ascii_lowercase();
    let lower_type = provider.provider_type.to_ascii_lowercase();
    let lower_vehicle = provider.vehicle_type.to_ascii_lowercase();

    if lower_name.ends_with(".mrs") || lower_type == "mrs" || lower_vehicle == "mrs" {
        "MRS"
    } else if lower_name.ends_with(".txt") || lower_type == "text" {
        "TEXT"
    } else if lower_name.ends_with(".yaml") || lower_name.ends_with(".yml") || lower_type == "yaml" {
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
pub fn total_external_rules(rule_providers: &[mihomo_api::types::RuleProvider]) -> u32 {
    rule_providers.iter().map(|rp| rp.rule_count).sum()
}

pub fn rule_provider_row<'a>(provider: &mihomo_api::types::RuleProvider, _lang: &Lang<'_>) -> Element<'a, Message> {
    let behavior_badge_text = format_provider_behavior(&provider.behavior);
    let rule_count_str = crate::view::mrs_panel::format_rule_count(provider.rule_count);
    let format_str = format_rule_provider_format(provider);
    let updated_text = if provider.updated_at.is_empty() { "—".to_string() } else { format!("Updated: {}", provider.updated_at) };

    let actions = row![
        button(row![svg_icons::icon_themed(Icon::Code2, 12.0, |t: &Theme| tokens(t).text_secondary), Space::new().width(4.0), text("Diff").size(11).font(FONT_MEDIUM)].align_y(Alignment::Center))
            .padding([4, 10]).style(style_ghost).on_press(Message::InspectRuleProviderDiff(Some(provider.name.clone()))),
        Space::new().width(theme::SP_XS),
        button(row![svg_icons::icon_themed(Icon::Zap, 12.0, |t: &Theme| tokens(t).text_secondary), Space::new().width(4.0), text("Unpack").size(11).font(FONT_MEDIUM)].align_y(Alignment::Center))
            .padding([4, 10]).style(style_ghost).on_press(Message::UnpackRuleProvider(provider.name.clone())),
        Space::new().width(theme::SP_XS),
        icon_button(Icon::RefreshCw, 13.0, Message::UpdateRuleProvider(provider.name.clone())),
    ].align_y(Alignment::Center);

    container(row![
        provider_icon_chip(Icon::ListChecks, 16.0), Space::new().width(theme::SP_MD),
        column![
            row![
                text(provider.name.clone()).size(13).font(FONT_SEMIBOLD).style(|t: &Theme| text::Style { color: Some(tokens(t).text_primary) }),
                Space::new().width(theme::SP_SM), badge(behavior_badge_text, BadgeKind::Neutral),
                Space::new().width(theme::SP_XS), badge(rule_count_str, BadgeKind::Accent),
            ].align_y(Alignment::Center),
            text(updated_text).size(11).font(MONO).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
        ].width(Length::Fill),
        chip(format_str), Space::new().width(theme::SP_SM), actions,
    ].align_y(Alignment::Center)).padding([theme::SP_SM, SP_MD]).width(Length::Fill).style(row_card_surface).into()
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

    container(row![
        svg_icons::icon_themed(icon, 12.0, move |t: &Theme| {
            let tk = tokens(t);
            if !is_enabled { tk.text_tertiary } else if is_reject { tk.danger } else if is_direct { tk.success } else { tk.accent }
        }),
        Space::new().width(4.0),
        text(target.to_string()).size(11).font(FONT_SEMIBOLD).style(move |t: &Theme| {
            let tk = tokens(t);
            text::Style { color: Some(if !is_enabled { tk.text_tertiary } else if is_reject { tk.danger } else if is_direct { tk.success } else { tk.text_primary }) }
        }),
    ].align_y(Alignment::Center)).padding([3, 10]).style(move |t: &Theme| {
        let tk = tokens(t);
        let border_color = if !is_enabled { tk.card_border } else if is_reject { Color { a: 0.25, ..tk.danger } } else if is_direct { Color { a: 0.25, ..tk.success } } else { Color { a: 0.25, ..tk.accent } };
        let bg_color = if !is_enabled { tk.chip_bg } else if is_reject { Color { a: 0.10, ..tk.danger } } else if is_direct { Color { a: 0.10, ..tk.success } } else { Color { a: 0.08, ..tk.accent } };
        container::Style { background: Some(bg_color.into()), border: Border { radius: border::Radius::from(theme::R_CHIP), width: 1.0, color: border_color }, ..Default::default() }
    }).into()
}

/// Hit counter badge and recent hit indicator.
fn hit_stats_badge<'a>(stats: RuleHitStats, lang: &Lang<'_>) -> Element<'a, Message> {
    if stats.count > 0 {
        let hits_text = infiltrator_shared::i18n_interpolator::interpolate(
            &lang.tr("rule_tracer_hits_count"),
            &[("count", &stats.count.to_string())],
        );
        row![
            status_dot(true), Space::new().width(theme::SP_XS),
            text(hits_text).size(11).font(FONT_MEDIUM).style(|t: &Theme| text::Style { color: Some(tokens(t).success) }),
            Space::new().width(theme::SP_XS), badge(lang.tr("rule_tracer_recent_hits").to_string(), BadgeKind::Success),
        ].align_y(Alignment::Center).into()
    } else {
        row![text(lang.tr("rule_tracer_zero_hits").to_string()).size(11).font(MONO).style(|t: &Theme| text::Style { color: Some(tokens(t).text_tertiary) })].align_y(Alignment::Center).into()
    }
}



fn tracer_panel<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    crate::view::rules_tracer::inline_tracer_panel(state, lang)
}

fn add_rule_panel<'a>(state: &'a AppState, lang: &Lang<'_>, available_targets: Vec<String>) -> Element<'a, Message> {
    let rule_types = vec![
        "DOMAIN".to_string(), "DOMAIN-SUFFIX".to_string(), "DOMAIN-KEYWORD".to_string(),
        "IP-CIDR".to_string(), "IP-CIDR6".to_string(), "GEOIP".to_string(),
        "MATCH".to_string(), "RULE-SET".to_string(), "AND".to_string(),
        "OR".to_string(), "NOT".to_string(), "SUB-RULE".to_string(),
    ];
    let add_rule_btn_style = if state.editor.is_adding_rule { style_ghost } else { style_accent };

    card(Some(lang.tr("rules_add_custom").to_string()), row![
        column![
            form_field_label(lang.tr("rules_type").to_string()), Space::new().height(theme::SP_XS),
            pick_list(rule_types, Some(&state.editor.new_rule_type), Message::UpdateNewRuleType).width(Length::Fill).style(form_pick_style),
        ].width(Length::FillPortion(1)),
        Space::new().width(theme::SP_LG),
        column![
            form_field_label(lang.tr("rules_payload").to_string()), Space::new().height(theme::SP_XS),
            text_input("e.g. google.com", &state.editor.new_rule_payload).on_input(Message::UpdateNewRulePayload).padding([8, 12]).size(12).font(MONO).style(form_input_style),
        ].width(Length::FillPortion(2)),
        Space::new().width(theme::SP_LG),
        column![
            form_field_label(lang.tr("rules_target").to_string()), Space::new().height(theme::SP_XS),
            pick_list(available_targets, Some(&state.editor.new_rule_target), Message::UpdateNewRuleTarget).width(Length::Fill).style(form_pick_style),
        ].width(Length::FillPortion(1)),
        Space::new().width(theme::SP_LG),
        column![
            Space::new().height(18.0),
            row![
                button(row![
                    svg_icons::icon_themed(Icon::Plus, 14.0, |t: &Theme| if state.editor.is_adding_rule { tokens(t).text_secondary } else { tokens(t).on_accent }),
                    text(lang.tr("rules_add_btn").to_string()).size(12).font(FONT_MEDIUM),
                ].spacing(theme::SP_SM)).padding([8, 16]).style(add_rule_btn_style).on_press(Message::AddCustomRule),
                Space::new().width(theme::SP_SM),
                text_btn(lang.tr("rules_inject_game_presets").to_string(), style_ghost, Some(Message::ApplyGameRoutingPresets)),
            ].align_y(Alignment::Center),
        ],
    ].align_y(Alignment::Center))
}

fn rules_list_view<'a>(state: &'a AppState, lang: &Lang<'_>, available_targets: Vec<String>) -> Element<'a, Message> {
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
    let total_pages = if total_count == 0 { 1 } else { (total_count - 1) / page_size + 1 };
    let current_page = state.editor.rules_page.min(total_pages.saturating_sub(1));
    let start = current_page * page_size;
    let end = (start + page_size).min(total_count);
    let visible = &state.editor.rules_filtered_indices[start..end];

    let mut rules_list = column![].spacing(theme::SP_SM);
    if visible.is_empty() {
        rules_list = rules_list.push(empty_state(Icon::ListChecks, lang.tr("rules_empty").as_ref(), ""));
    } else {
        for cache_index in visible {
            let Some(item) = state.editor.rules_render_cache.get(*cache_index) else { continue; };
            let source_index = item.source_index;
            let Some(entry) = state.editor.rules.get(source_index) else { continue; };
            let is_enabled = entry.enabled;
            let bkind = semantic_badge_kind(&item.rule_type, item.badge);
            let display_type = display_rule_type(&item.rule_type);
            let hit_stats = lookup_hit_stats(&hit_stats_map, &item.rule_type, &item.payload);

            let up_button = if source_index > 0 { icon_button(Icon::ArrowUp, 13.0, Message::MoveRuleUp(source_index)) } else { Space::new().width(26).height(26).into() };
            let down_button = if source_index + 1 < state.editor.rules.len() { icon_button(Icon::ArrowDown, 13.0, Message::MoveRuleDown(source_index)) } else { Space::new().width(26).height(26).into() };

            let target_label = if item.target.is_empty() {
                if item.rule_type.eq_ignore_ascii_case("MATCH") {
                    if !item.payload.is_empty() { &item.payload } else { "DIRECT" }
                } else {
                    "—"
                }
            } else {
                &item.target
            };

            rules_list = rules_list.push(
                container(row![
                    toggle_switch(is_enabled, move |_| Message::ToggleRuleEnabled(source_index)),
                    Space::new().width(theme::SP_SM), badge(display_type, bkind), Space::new().width(theme::SP_MD),
                    column![
                        text(if item.payload.is_empty() {
                            if item.rule_type.eq_ignore_ascii_case("MATCH") { lang.tr("rule_match_all").to_string() } else { "—".to_string() }
                        } else {
                            item.payload.clone()
                        }).size(13).font(MONO).style(move |t: &Theme| text::Style {
                            color: Some(if is_enabled { tokens(t).text_primary } else { tokens(t).text_tertiary }),
                        }),
                        if !item.target.is_empty() && !item.rule_type.eq_ignore_ascii_case("MATCH") {
                            Element::from(row![
                                text("➔ ").size(11).style(|t: &Theme| text::Style { color: Some(tokens(t).text_tertiary) }),
                                text(item.target.clone()).size(11).font(FONT_MEDIUM).style(move |t: &Theme| text::Style {
                                    color: Some(if is_enabled { tokens(t).text_secondary } else { tokens(t).text_tertiary }),
                                }),
                            ].align_y(Alignment::Center))
                        } else {
                            Element::from(Space::new().width(0).height(0))
                        },
                    ].width(Length::Fill),
                    hit_stats_badge(hit_stats, lang), Space::new().width(theme::SP_MD),
                    target_group_pill(target_label, is_enabled), Space::new().width(theme::SP_SM),
                    row![up_button, down_button].spacing(2).align_y(Alignment::Center),
                ].align_y(Alignment::Center)).padding([theme::SP_SM, SP_MD]).width(Length::Fill).style(row_card_surface),
            );
        }
    }

    let pager = row![
        row![
            text(infiltrator_shared::i18n_interpolator::interpolate(
                &lang.tr("rule_rules_showing"),
                &[
                    ("start", &(if total_count == 0 { 0 } else { start + 1 }).to_string()),
                    ("end", &end.to_string()),
                    ("total", &total_count.to_string()),
                ],
            )).size(12).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
            Space::new().width(theme::SP_SM), kbd_badge(format!("{}/{}", current_page + 1, total_pages)),
        ].align_y(Alignment::Center),
        Space::new().width(Length::Fill),
        text_btn("Prev".to_string(), style_ghost, (current_page > 0).then_some(Message::RulesPrevPage)),
        Space::new().width(theme::SP_SM),
        text_btn("Next".to_string(), style_ghost, (current_page + 1 < total_pages).then_some(Message::RulesNextPage)),
    ].align_y(Alignment::Center);

    column![
        tracer_card, Space::new().height(theme::SP_MD),
        crate::view::rule_hit_card::rule_hit_card(state, lang), Space::new().height(theme::SP_MD),
        crate::view::subrules_builder::subrules_panel(state, lang), Space::new().height(theme::SP_MD),
        add_rule_form, Space::new().height(theme::SP_MD),
        search_bar, Space::new().height(theme::SP_SM),
        pager, Space::new().height(theme::SP_SM),
        modern_scrollable(rules_list).height(Length::Fill),
    ].spacing(theme::SP_SM).into()
}

pub fn providers_view<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let total_ext_rules = total_external_rules(&state.editor.rule_providers);
    let mut content = column![section_header(
        "Providers",
        Some(row![
            chip(format!("Proxy: {}", state.editor.proxy_providers.len())),
            Space::new().width(theme::SP_XS),
            chip(format!("Rule: {}", state.editor.rule_providers.len())),
            Space::new().width(theme::SP_XS),
            chip(format!("External Rules: {total_ext_rules}")),
            Space::new().width(theme::SP_SM),
            text_btn(
                if state.editor.rules_providers_expanded { lang.tr("rules_collapse").to_string() } else { lang.tr("rules_expand").to_string() },
                style_ghost,
                Some(Message::ToggleRulesProvidersExpanded),
            ),
        ].align_y(Alignment::Center).into()),
    )].spacing(theme::SP_MD);

    content = content.push(crate::view::provider_unpack_card::provider_unpack_card(state, lang));
    if state.editor.rules_providers_expanded {
        let mut proxy_list = column![].spacing(theme::SP_SM);
        if state.editor.proxy_providers.is_empty() {
            proxy_list = proxy_list.push(empty_state(Icon::Server, lang.tr("rules_no_providers").as_ref(), ""));
        } else {
            for provider in &state.editor.proxy_providers {
                proxy_list = proxy_list.push(proxy_provider_row(provider, lang));
            }
        }

        let mut rule_list = column![].spacing(theme::SP_SM);
        if state.editor.rule_providers.is_empty() {
            rule_list = rule_list.push(empty_state(Icon::ListChecks, lang.tr("rules_no_providers").as_ref(), ""));
        } else {
            for provider in &state.editor.rule_providers {
                rule_list = rule_list.push(rule_provider_row(provider, lang));
            }
        }

        let update_geo_btn = if state.editor.is_updating_geo_databases {
            button(row![
                svg_icons::icon_themed(Icon::RefreshCw, 12.0, |t: &Theme| tokens(t).text_secondary),
                Space::new().width(4.0), text(lang.tr("rules_updating_geo").to_string()).size(12).font(FONT_MEDIUM),
            ].align_y(Alignment::Center)).padding([7, 14]).style(style_ghost)
        } else {
            button(row![
                svg_icons::icon_themed(Icon::RefreshCw, 12.0, |t: &Theme| tokens(t).on_accent),
                Space::new().width(4.0), text(lang.tr("rules_update_geo_btn").to_string()).size(12).font(FONT_MEDIUM),
            ].align_y(Alignment::Center)).padding([7, 14]).style(style_accent).on_press(Message::UpdateGeoDatabases)
        };

        let geo_card = card(
            Some(lang.tr("rules_geo_databases_title").to_string()),
            column![row![
                provider_icon_chip(Icon::Globe, 18.0), Space::new().width(theme::SP_MD),
                column![
                    text("geoip.metadb / geosite.dat / Country.mmdb").size(13).font(MONO).style(|t: &Theme| text::Style { color: Some(tokens(t).text_primary) }),
                    text(lang.tr("rule_repo_official").to_string()).size(11).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
                ].width(Length::Fill),
                badge("MetaCubeX", BadgeKind::Accent), Space::new().width(theme::SP_XS), chip("db / dat"),
                Space::new().width(theme::SP_SM), update_geo_btn,
            ].align_y(Alignment::Center)].spacing(theme::SP_SM),
        );

        let rule_card_header = row![
            text(lang.tr("rules_rule_providers").to_string()).size(14).font(FONT_SEMIBOLD).style(|t: &Theme| text::Style { color: Some(tokens(t).text_primary) }),
            Space::new().width(theme::SP_SM),
            chip(format!("Total External Rules: {total_ext_rules}")),
        ].align_y(Alignment::Center);

        let rule_card = card(
            None,
            column![rule_card_header, rule_list].spacing(theme::SP_MD),
        );

        content = content
            .push(column![
                card(Some(lang.tr("rules_proxy_providers").to_string()), proxy_list),
                Space::new().height(theme::SP_MD),
                rule_card,
            ].spacing(theme::SP_MD))
            .push(Space::new().height(theme::SP_MD))
            .push(geo_card);

        if let Some(mrs_panel) = crate::view::mrs_panel::mrs_card(state) {
            content = content.push(Space::new().height(theme::SP_MD)).push(mrs_panel);
        }
    }
    content.into()
}

fn json_editors_view<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let json_tab_labels: Vec<String> = vec!["Rule Providers".to_string(), "Proxy Providers".to_string(), "Sniffer".to_string()];
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
        RulesJsonTab::RuleProviders => json_tab_card(
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
        RulesJsonTab::ProxyProviders => json_tab_card(
            "Proxy Providers JSON".to_string(),
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
        RulesJsonTab::Sniffer => json_tab_card(
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

    column![json_tab_buttons, Space::new().height(theme::SP_MD), json_view].spacing(theme::SP_SM).into()
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
        text(lang.tr("rules_title").to_string()).size(24).font(FONT_SEMIBOLD).style(|t: &Theme| text::Style { color: Some(tokens(t).text_primary) }),
        Space::new().width(theme::SP_MD),
        text(format!("{} / {}", filtered_count, state.editor.rules.len())).size(13).font(MONO).style(|t: &Theme| text::Style { color: Some(tokens(t).text_tertiary) }),
        Space::new().width(Length::Fill),
        if state.editor.is_loading_rules || state.editor.is_loading_providers {
            Element::from(text("...").size(12).style(|t: &Theme| text::Style { color: Some(tokens(t).text_tertiary) }))
        } else {
            icon_button(Icon::RefreshCw, 16.0, Message::LoadRules)
        },
        Space::new().width(theme::SP_SM),
        save_rules_action,
    ].align_y(Alignment::Center);

    let tab_labels: Vec<String> = vec![
        lang.tr("rules_tab_list").to_string(),
        lang.tr("rules_tab_providers").to_string(),
        lang.tr("rules_tab_json").to_string(),
        lang.tr("rules_tab_tracer").to_string(),
    ];
    let tab_index = match state.editor.rules_tab {
        RulesTab::Providers => 1,
        RulesTab::JsonEditors => 2,
        RulesTab::Tracer => 3,
        RulesTab::RulesList => 0,
    };
    let tabs = segmented_control(&tab_labels, tab_index, |index| {
        Message::SetRulesTab(match index {
            1 => RulesTab::Providers,
            2 => RulesTab::JsonEditors,
            3 => RulesTab::Tracer,
            _ => RulesTab::RulesList,
        })
    });

    if !state.editor.rules_heavy_ready {
        return column![
            header, Space::new().height(theme::SP_MD), tabs, Space::new().height(SP_LG),
            card(None, column![
                text("Preparing Rules panels...").size(14).font(FONT_SEMIBOLD).style(|t: &Theme| text::Style { color: Some(tokens(t).text_primary) }),
                text("Heavy widgets mount asynchronously to keep first paint responsive.").size(12).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
            ].spacing(theme::SP_SM)),
        ].spacing(10).into();
    }

    let mut available_targets: Vec<String> = state.runtime.proxies.iter()
        .filter(|(_, p): &(&String, &mihomo_api::proxy::types::Proxy)| p.is_group())
        .map(|(name, _)| name.clone())
        .collect();
    available_targets.sort();
    if !available_targets.contains(&"DIRECT".to_string()) { available_targets.push("DIRECT".to_string()); }
    if !available_targets.contains(&"REJECT".to_string()) { available_targets.push("REJECT".to_string()); }

    let tab_content: Element<'_, Message> = match state.editor.rules_tab {
        RulesTab::RulesList => rules_list_view(state, &lang, available_targets),
        RulesTab::Providers => providers_view(state, &lang),
        RulesTab::JsonEditors => json_editors_view(state, &lang),
        RulesTab::Tracer => crate::view::rules_tracer::tracer_view(state, &lang),
    };

    column![header, Space::new().height(theme::SP_MD), tabs, Space::new().height(theme::SP_MD), tab_content].spacing(SP_LG).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semantic_badge_kind_mapping() {
        assert_eq!(semantic_badge_kind("DOMAIN", RuleBadgeKind::Domain), BadgeKind::Accent);
        assert_eq!(semantic_badge_kind("DOMAIN-SUFFIX", RuleBadgeKind::Domain), BadgeKind::Accent);
        assert_eq!(semantic_badge_kind("DOMAIN-KEYWORD", RuleBadgeKind::Domain), BadgeKind::Accent);
        assert_eq!(semantic_badge_kind("IP-CIDR", RuleBadgeKind::Ip), BadgeKind::Warning);
        assert_eq!(semantic_badge_kind("IP-CIDR6", RuleBadgeKind::Ip), BadgeKind::Warning);
        assert_eq!(semantic_badge_kind("IP-ASN", RuleBadgeKind::Ip), BadgeKind::Warning);
        assert_eq!(semantic_badge_kind("GEOIP", RuleBadgeKind::Ip), BadgeKind::Neutral);
        assert_eq!(semantic_badge_kind("GEOSITE", RuleBadgeKind::Other), BadgeKind::Neutral);
        assert_eq!(semantic_badge_kind("MATCH", RuleBadgeKind::Other), BadgeKind::Neutral);
        assert_eq!(semantic_badge_kind("CUSTOM", RuleBadgeKind::Domain), BadgeKind::Accent);
        assert_eq!(semantic_badge_kind("CUSTOM", RuleBadgeKind::Ip), BadgeKind::Warning);
        assert_eq!(semantic_badge_kind("CUSTOM", RuleBadgeKind::Other), BadgeKind::Neutral);
    }

    #[test]
    fn test_display_rule_type_formatting() {
        assert_eq!(display_rule_type("DOMAIN"), "Domain");
        assert_eq!(display_rule_type("DOMAIN-SUFFIX"), "DomainSuffix");
        assert_eq!(display_rule_type("IP-CIDR"), "IPCIDR");
        assert_eq!(display_rule_type("GEOIP"), "GeoIP");
        assert_eq!(display_rule_type("MATCH"), "Match");
        assert_eq!(display_rule_type("RULE-SET"), "RuleSet");
    }

    #[test]
    fn test_rule_hit_stats_matching() {
        let stats_map = {
            let mut map = HashMap::new();
            map.insert("domainsuffix:google.com".to_string(), RuleHitStats { count: 5, is_recent: true });
            map.insert("match:".to_string(), RuleHitStats { count: 12, is_recent: true });
            map
        };

        let hit = lookup_hit_stats(&stats_map, "DOMAIN-SUFFIX", "google.com");
        assert_eq!(hit.count, 5);
        assert!(hit.is_recent);

        let match_hit = lookup_hit_stats(&stats_map, "MATCH", "");
        assert_eq!(match_hit.count, 12);
        assert!(match_hit.is_recent);

        let unhit = lookup_hit_stats(&stats_map, "DOMAIN", "unknown.com");
        assert_eq!(unhit.count, 0);
        assert!(!unhit.is_recent);
    }

    #[test]
    fn test_proxy_and_rule_provider_row_render() {
        let lang = Lang("en");
        let proxy_p = mihomo_api::types::ProxyProvider {
            name: "DefaultProxies".into(),
            provider_type: "http".into(),
            vehicle_type: "HTTP".into(),
            updated_at: "2026-09-02 12:00:00".into(),
        };
        let _proxy_element = proxy_provider_row(&proxy_p, &lang);

        let rule_p = mihomo_api::types::RuleProvider {
            name: "RejectAds".into(),
            provider_type: "http".into(),
            behavior: "domain".into(),
            vehicle_type: "HTTP".into(),
            updated_at: "2026-09-02 12:00:00".into(),
            rule_count: 179,
        };
        let _rule_element = rule_provider_row(&rule_p, &lang);

        assert_eq!(format_provider_behavior(&rule_p.behavior), "Domain");
        assert_eq!(format_rule_provider_format(&rule_p), "HTTP");
        assert_eq!(total_external_rules(&[rule_p]), 179);
    }
}
