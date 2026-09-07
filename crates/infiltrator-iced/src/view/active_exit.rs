//! Iced adapter for the shared active-exit-node read model.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::{BadgeKind, badge, card_surface, chip};
use crate::view::svg_icons::{Icon, icon_themed};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, MONO, tokens};
use iced::widget::{Space, column, container, row, text};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_contract::active_exit::{ActiveExitSnapshot, ActiveExitStatus};
use infiltrator_shared::country_flags::match_region;
use infiltrator_shared::locales::{Lang, Localizer};

/// High-fidelity selected outbound card shared by the Iced Overview surface.
pub fn active_exit_card<'a>(state: &'a AppState, lang: &Lang<'a>) -> Element<'a, Message> {
    let snapshot = &state.runtime.active_exit;
    let delay = snapshot
        .delay_ms
        .map(|value| format!("{value} ms"))
        .unwrap_or_else(|| "—".to_owned());
    let delay_kind = if snapshot.delay_ms.is_some() {
        BadgeKind::Success
    } else {
        BadgeKind::Neutral
    };
    let name = snapshot.name.as_deref().unwrap_or("—");
    let protocol = snapshot.protocol.as_deref().unwrap_or("—");
    let group = snapshot.group.as_deref().unwrap_or("—");
    let flag = country_flag(snapshot);

    let card_header = row![
        row![
            icon_themed(Icon::Globe, 16.0, |t: &Theme| tokens(t).accent),
            Space::new().width(theme::SP_SM),
            text(lang.tr("overview_active_exit").to_string())
                .size(14)
                .font(FONT_SEMIBOLD)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_primary),
                }),
        ]
        .align_y(Alignment::Center),
        Space::new().width(Length::Fill),
        badge(delay, delay_kind),
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill);

    let node_row = row![
        text(flag).size(22),
        Space::new().width(theme::SP_SM),
        column![
            text(name)
                .size(15)
                .font(FONT_SEMIBOLD)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_primary),
                }),
            text(format!(
                "{} {}",
                lang.tr("overview_active_exit_group"),
                group
            ))
            .size(10)
            .font(MONO)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary),
            }),
        ]
        .spacing(2),
        Space::new().width(Length::Fill),
        chip(protocol.to_owned()),
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill);

    let status = status_text(snapshot, lang);
    container(
        column![
            card_header,
            Space::new().height(theme::SP_MD),
            node_row,
            Space::new().height(theme::SP_SM),
            text(status)
                .size(10)
                .font(FONT_MEDIUM)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary),
                }),
        ]
        .spacing(theme::SP_XS),
    )
    .width(Length::FillPortion(1))
    .padding(theme::SP_XXL)
    .style(card_surface)
    .into()
}

fn country_flag(snapshot: &ActiveExitSnapshot) -> String {
    snapshot
        .country_code
        .as_deref()
        .and_then(match_region)
        .map(|region| region.emoji().to_owned())
        .unwrap_or_else(|| "🌐".to_owned())
}

fn status_text(snapshot: &ActiveExitSnapshot, lang: &Lang<'_>) -> String {
    match snapshot.status {
        ActiveExitStatus::Ready => match snapshot.alive {
            Some(true) => lang.tr("overview_active_exit_alive").to_string(),
            Some(false) => lang.tr("overview_active_exit_offline").to_string(),
            None => lang.tr("overview_active_exit_liveness_unknown").to_string(),
        },
        ActiveExitStatus::Empty => lang.tr("overview_active_exit_empty").to_string(),
        ActiveExitStatus::Unknown => lang.tr("overview_active_exit_unknown").to_string(),
        ActiveExitStatus::Unsupported | ActiveExitStatus::Failed => snapshot
            .failure
            .clone()
            .unwrap_or_else(|| lang.tr("overview_active_exit_unavailable").to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_text_tracks_shared_liveness_without_guessing() {
        let lang = Lang("en-US");
        let mut snapshot = ActiveExitSnapshot::demo_fixture();
        assert_eq!(status_text(&snapshot, &lang), "Alive");
        snapshot.alive = None;
        assert_eq!(status_text(&snapshot, &lang), "Liveness unknown");
        snapshot.status = ActiveExitStatus::Empty;
        assert_eq!(status_text(&snapshot, &lang), "No active exit");
    }

    #[test]
    fn country_flag_comes_from_the_shared_region_code() {
        let snapshot = ActiveExitSnapshot::demo_fixture();
        assert_eq!(country_flag(&snapshot), "🇭🇰");
        let mut unknown = snapshot;
        unknown.country_code = None;
        assert_eq!(country_flag(&unknown), "🌐");
    }
}
