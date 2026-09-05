//! Crash Watchdog and Sanitized Forensics Reporter component.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::{badge, card, style_accent, style_ghost, BadgeKind};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, FONT_MEDIUM, MONO, tokens};
use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_contract::snapshot::CoreWatchdogState;
use infiltrator_shared::locales::{Lang, Localizer};

pub fn crash_watchdog_card<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let dog = &state.diag.crash_watchdog;

    let recover_btn = button(
        row![
            svg_icons::icon_themed(Icon::Shield, 12.0, |t: &Theme| tokens(t).on_accent),
            Space::new().width(theme::SP_XS),
            text(lang.tr("crash_watchdog_btn_recover").to_string())
                .size(11)
                .font(FONT_MEDIUM),
        ]
        .align_y(Alignment::Center),
    )
    .padding([4, 12])
    .style(style_accent)
    .on_press(Message::RecoverOrphanedState);

    let export_btn = button(
        row![
            svg_icons::icon_themed(Icon::FileText, 12.0, |t: &Theme| tokens(t).text_secondary),
            Space::new().width(theme::SP_XS),
            text(lang.tr("crash_watchdog_btn_export").to_string())
                .size(11)
                .font(FONT_MEDIUM),
        ]
        .align_y(Alignment::Center),
    )
    .padding([4, 10])
    .style(style_ghost)
    .on_press(Message::ExportCrashDiagnostics);

    let status_row = if dog.is_orphaned_detected {
        row![
            badge("Orphaned State".to_string(), BadgeKind::Danger),
            Space::new().width(theme::SP_SM),
            text("Abnormal termination detected in previous run; proxy settings require cleanup").size(12).style(|t: &Theme| text::Style { color: Some(tokens(t).danger) }),
        ]
        .align_y(Alignment::Center)
    } else {
        let (label, kind, detail) = match &dog.shared.state {
            CoreWatchdogState::Idle => (
                "Healthy",
                BadgeKind::Success,
                lang.tr("crash_watchdog_clean").to_string(),
            ),
            CoreWatchdogState::Waiting {
                attempt,
                retry_in_ms,
            } => (
                "Recovery pending",
                BadgeKind::Warning,
                format!("Automatic core restart attempt {attempt} in {retry_in_ms} ms"),
            ),
            CoreWatchdogState::Restarting { attempt } => (
                "Restarting",
                BadgeKind::Warning,
                format!("Automatic core restart attempt {attempt} is in progress"),
            ),
            CoreWatchdogState::Recovered { attempts } => (
                "Recovered",
                BadgeKind::Success,
                format!("Core recovered after {attempts} restart attempt(s)"),
            ),
            CoreWatchdogState::Tripped { attempts } => (
                "Circuit open",
                BadgeKind::Danger,
                format!("Automatic recovery stopped after {attempts} failed attempt(s)"),
            ),
        };
        row![
            badge(label.to_string(), kind),
            Space::new().width(theme::SP_SM),
            text(detail)
                .size(12)
                .style(move |t: &Theme| text::Style {
                    color: Some(match kind {
                        BadgeKind::Danger => tokens(t).danger,
                        BadgeKind::Warning => tokens(t).warning,
                        _ => tokens(t).text_secondary,
                    }),
                }),
        ]
        .align_y(Alignment::Center)
    };

    let summary_feedback: Element<'_, Message> = if let Some(path) = &dog.exported_log_path {
        container(
            row![
                svg_icons::icon_themed(Icon::ListChecks, 14.0, |t: &Theme| tokens(t).success),
                Space::new().width(theme::SP_XS),
                text(format!("Exported: {path}")).size(11).font(MONO).style(|t: &Theme| text::Style { color: Some(tokens(t).success) }),
            ]
            .align_y(Alignment::Center),
        )
        .into()
    } else {
        Element::from(Space::new().height(0))
    };

    card(
        Some(lang.tr("crash_watchdog_title").to_string()),
        column![
            text(lang.tr("crash_watchdog_desc").to_string()).size(12).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
            Space::new().height(theme::SP_XS),
            status_row,
            summary_feedback,
            Space::new().height(theme::SP_XS),
            row![
                Space::new().width(Length::Fill),
                export_btn,
                Space::new().width(theme::SP_SM),
                recover_btn,
            ]
            .align_y(Alignment::Center),
        ]
        .spacing(theme::SP_SM),
    )
}
