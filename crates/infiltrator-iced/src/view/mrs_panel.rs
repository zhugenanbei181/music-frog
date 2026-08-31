//! MRS rule-provider detail panel for the Rules page providers tab: parsed
//! header metadata (behavior / rule count / version) per provider, with the
//! failure reason when a cache file is missing or not MRS.

use crate::state::AppState;
use crate::types::message::Message;
use crate::types::options::MrsProviderDetail;
use crate::view::components::{BadgeKind, card, badge};
use crate::view::theme::{self, FONT_SEMIBOLD, MONO, tokens};
use iced::widget::{column, row, text, Space};
use iced::{Element, Theme};
use infiltrator_shared::locales::{Lang, Localizer};

fn meta_text(value: String) -> text::Text<'static> {
    text(value)
        .size(11)
        .font(MONO)
        .style(|t: &Theme| text::Style {
            color: Some(tokens(t).text_secondary),
        })
}

fn detail_row<'a>(lang: &Lang<'a>, detail: &MrsProviderDetail) -> Element<'a, Message> {
    let (status_label, kind) = if detail.metadata.is_some() {
        (lang.tr("mrs_badge_mrs").into_owned(), BadgeKind::Success)
    } else {
        (lang.tr("mrs_badge_missing").into_owned(), BadgeKind::Neutral)
    };
    let behavior_line: Element<'_, Message> = if !detail.behavior.is_empty() {
        meta_text(format!(
            "{}: {}",
            lang.tr("mrs_behavior_label"),
            detail.behavior
        ))
        .into()
    } else {
        Space::new().width(0).into()
    };
    let mut lines = column![
        row![
            text(detail.name.clone())
                .size(13)
                .font(FONT_SEMIBOLD)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_primary),
                }),
            Space::new().width(theme::SP_SM),
            badge(status_label.to_string(), kind),
            Space::new().width(theme::SP_SM),
            behavior_line,
        ]
        .align_y(iced::Alignment::Center),
    ]
    .spacing(2);

    if let Some(meta) = &detail.metadata {
        lines = lines.push(meta_text(format!(
            "v{} · {} {} · {} {} · {}",
            meta.version,
            meta.rule_count,
            lang.tr("mrs_unit_rules"),
            meta.payload_size,
            lang.tr("mrs_unit_payload"),
            meta.description
        )));
        if let Some(file) = &detail.file {
            lines = lines.push(meta_text(file.display().to_string()));
        }
    } else if let Some(error) = detail.errors.first() {
        lines = lines.push(meta_text(error.clone()));
    }
    lines.into()
}

/// Rendered inside the providers tab, below the provider lists. Scan results
/// refresh whenever the live provider list reloads (`ProvidersLoaded`).
pub fn mrs_card(state: &AppState) -> Option<Element<'_, Message>> {
    if state.editor.mrs_details.is_empty() && !state.editor.is_scanning_mrs {
        return None;
    }
    let lang = Lang(&state.shell.lang);
    let mut body = column![].spacing(theme::SP_SM);
    if state.editor.is_scanning_mrs {
        body = body.push(
            text(lang.tr("mrs_scanning").to_string())
                .size(11)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_tertiary),
                }),
        );
    }
    for detail in &state.editor.mrs_details {
        body = body.push(detail_row(&lang, detail));
    }
    Some(card(Some(lang.tr("mrs_panel_title").to_string()), body))
}
