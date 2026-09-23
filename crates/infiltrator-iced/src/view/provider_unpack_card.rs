//! Rule-Provider Unpacker and Local Extraction component.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::component_forms::{style_accent, style_ghost};
use crate::view::components::{BadgeKind, badge, card};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, FONT_MEDIUM, MONO, tokens};
use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_shared::locales::{Lang, Localizer};

pub fn provider_unpack_card<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let unp = &state.editor.provider_unpack;
    // DUAL-11-06: the unpack target is a real declaration from the loaded
    // profile (or a provider the running kernel reports); it is never a
    // hardcoded name, and with nothing declared the action stays disabled.
    let unpack_target = state
        .editor
        .rule_providers
        .first()
        .map(|provider| provider.name.clone());
    let unpack_label = unpack_target
        .as_ref()
        .map(|name| {
            lang.tr("provider_btn_unpack")
                .replace("{provider}", name)
                .to_string()
        })
        .unwrap_or_else(|| lang.tr("provider_btn_unpack_idle").to_string());

    let mut unpack_btn = button(
        row![
            svg_icons::icon_themed(Icon::Plus, 12.0, |t: &Theme| tokens(t).on_accent),
            Space::new().width(theme::SP_XS),
            text(unpack_label).size(11).font(FONT_MEDIUM),
        ]
        .align_y(Alignment::Center),
    )
    .padding([4, 12])
    .style(style_accent);
    if let Some(provider) = unpack_target {
        unpack_btn = unpack_btn.on_press(Message::UnpackRuleProviderToCustom(provider));
    }

    let purge_btn = button(
        row![
            svg_icons::icon_themed(Icon::Trash2, 12.0, |t: &Theme| tokens(t).text_secondary),
            Space::new().width(theme::SP_XS),
            text(lang.tr("provider_btn_purge_cache").to_string())
                .size(11)
                .font(FONT_MEDIUM),
        ]
        .align_y(Alignment::Center),
    )
    .padding([4, 10])
    .style(style_ghost)
    .on_press(Message::PurgeRuleProviderCache);

    // DUAL-11-07: the observed cache fact published by the shared read model;
    // the surface never invents a directory, count or size.
    let cache = &state.editor.rule_provider_cache;
    let cache_line = match cache.state {
        infiltrator_contract::provider_cache::RuleProviderCacheState::Ready
        | infiltrator_contract::provider_cache::RuleProviderCacheState::Empty => lang
            .tr("provider_cache_ready")
            .replace("{dir}", cache.directory.as_deref().unwrap_or_default())
            .replace("{count}", &cache.file_count.to_string())
            .replace("{bytes}", &cache.total_bytes.to_string()),
        infiltrator_contract::provider_cache::RuleProviderCacheState::Unsupported => {
            lang.tr("provider_cache_unsupported").to_string()
        }
        infiltrator_contract::provider_cache::RuleProviderCacheState::Failed => {
            lang.tr("provider_cache_failed").to_string()
        }
        infiltrator_contract::provider_cache::RuleProviderCacheState::Unknown => {
            lang.tr("provider_cache_unknown").to_string()
        }
    };

    let feedback: Element<'_, Message> = if let Some(msg) = &unp.status_message {
        let failed = unp.is_unpacking || unp.is_purging_cache;
        container(
            row![
                svg_icons::icon_themed(Icon::ListChecks, 14.0, move |t: &Theme| if failed {
                    tokens(t).warning
                } else {
                    tokens(t).success
                }),
                Space::new().width(theme::SP_XS),
                text(msg.clone())
                    .size(11)
                    .style(move |t: &Theme| text::Style {
                        color: Some(if failed {
                            tokens(t).warning
                        } else {
                            tokens(t).success
                        })
                    }),
            ]
            .align_y(Alignment::Center),
        )
        .into()
    } else {
        Element::from(Space::new().height(0))
    };

    card(
        Some(lang.tr("provider_unpack_title").to_string()),
        column![
            text(lang.tr("provider_unpack_desc").to_string())
                .size(12)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary)
                }),
            Space::new().height(theme::SP_XS),
            row![
                text(
                    lang.tr("provider_unpack_total")
                        .replace("{count}", &unp.unpacked_rules_count.to_string())
                )
                .size(12)
                .font(MONO)
                .width(Length::Fill),
                badge(
                    lang.tr("provider_unpack_active").to_string(),
                    BadgeKind::Neutral
                ),
            ]
            .align_y(Alignment::Center),
            feedback,
            text(cache_line)
                .size(11)
                .font(MONO)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary)
                }),
            Space::new().height(theme::SP_XS),
            row![
                Space::new().width(Length::Fill),
                purge_btn,
                Space::new().width(theme::SP_SM),
                unpack_btn,
            ]
            .align_y(Alignment::Center),
        ]
        .spacing(theme::SP_SM),
    )
}
