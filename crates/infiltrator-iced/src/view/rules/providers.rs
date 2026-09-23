//! Providers tab: proxy/rule provider rows, lifecycle facts and the lazy
//! JSON editor cards.

use super::rules_list::save_action;
use crate::state::AppState;
use crate::types::editor::EditorLazyState;
use crate::types::message::Message;
use crate::view::component_forms::{
    editor_frame_surface, row_card_surface, style_accent, style_ghost, text_btn,
};
use crate::view::components::{
    BadgeKind, badge, card, chip, empty_state, icon_button, section_header, segmented_control,
};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, MONO, SP_MD, tokens};
use iced::widget::{Space, button, column, container, row, text, text_editor};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};
use infiltrator_contract::rules_workspace::RulesJsonSection;
use infiltrator_domain::runtime::{ProxyProvider, RuleProvider};
use infiltrator_shared::locales::{Lang, Localizer};

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

pub(super) fn proxy_provider_row<'a>(
    provider: &ProxyProvider,
    lang: &Lang<'_>,
) -> Element<'a, Message> {
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

/// DUAL-11-05: the kernel's real `etag-support` capability as declared by the
/// active profile. mihomo reads a top-level `etag-support` boolean (default
/// `true`) to gate its `ETag`/`If-None-Match` cache; the client renders the
/// declaration and never the per-request `304` outcome, which it cannot see.
pub fn etag_support_line(
    snapshot: &infiltrator_contract::provider_cache::KernelEtagSupportSnapshot,
    lang: &Lang<'_>,
) -> String {
    use infiltrator_contract::provider_cache::KernelEtagSupportState;
    let state = match snapshot.state {
        KernelEtagSupportState::Enabled => lang.tr("rules_etag_support_enabled"),
        KernelEtagSupportState::Disabled => lang.tr("rules_etag_support_disabled"),
        KernelEtagSupportState::NotDeclared => lang.tr("rules_etag_support_not_declared"),
    };
    format!("{}: {state}", lang.tr("rules_etag_support_label"))
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

    // DUAL-11-05: the kernel's real `etag-support` capability, declared at the
    // top of the active profile. It gates the kernel's ETag/304 cache for
    // provider downloads; the per-request 304 outcome stays inside the kernel.
    content = content.push(
        text(etag_support_line(&state.editor.rule_etag_support, lang).to_string())
            .size(11)
            .font(MONO)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary),
            }),
    );

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

pub(super) fn json_editors_view<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
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
