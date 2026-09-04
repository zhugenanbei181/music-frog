//! Doctor diagnostics card: self-healing check / repair / bootstrap actions.

use crate::state::AppState;
use crate::types::doctor::{DoctorCheckResult, DoctorReport, DoctorStatus};
use crate::types::message::Message;
use crate::view::components::{BadgeKind, badge, card};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD};
use iced::widget::{Space, button, column, row, scrollable, text};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_shared::locales::{Lang, Localizer};

/// Doctor check status badge coloring.
pub(crate) fn status_badge_kind(status: DoctorStatus) -> BadgeKind {
    match status {
        DoctorStatus::Pass => BadgeKind::Success,
        DoctorStatus::Warn => BadgeKind::Warning,
        DoctorStatus::Fail => BadgeKind::Danger,
        DoctorStatus::Skip => BadgeKind::Neutral,
    }
}

/// Status text badge.
pub(crate) fn status_label(status: DoctorStatus) -> &'static str {
    match status {
        DoctorStatus::Pass => "PASS",
        DoctorStatus::Warn => "WARN",
        DoctorStatus::Fail => "FAIL",
        DoctorStatus::Skip => "SKIP",
    }
}

/// Doctor section in settings / diagnostics.
pub fn section(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);
    let doctor = &state.diag.doctor;

    let busy = doctor.is_running || doctor.is_fixing || doctor.is_bootstrapping;

    let actions = row![
        button(
            text(lang.tr("doctor_btn_check").to_string())
                .size(12)
                .font(FONT_MEDIUM)
        )
        .padding([7, 14])
        .style(button::primary)
        .on_press_maybe((!doctor.is_running && !busy).then_some(Message::RunDoctor)),
        Space::new().width(theme::SP_SM),
        button(
            text(lang.tr("doctor_btn_fix").to_string())
                .size(12)
                .font(FONT_MEDIUM)
        )
        .padding([7, 14])
        .style(button::secondary)
        .on_press_maybe((!doctor.is_fixing && !busy).then_some(Message::RunDoctorFix)),
        Space::new().width(theme::SP_SM),
        button(
            text(lang.tr("doctor_btn_bootstrap").to_string())
                .size(12)
                .font(FONT_MEDIUM),
        )
        .padding([7, 14])
        .style(button::secondary)
        .on_press_maybe((!doctor.is_bootstrapping && !busy).then_some(Message::RunBootstrap)),
    ]
    .spacing(0)
    .align_y(Alignment::Center);

    let mut body = column![].spacing(theme::SP_SM);

    if doctor.is_running {
        body = body.push(secondary_text(lang.tr("doctor_running_check").to_string()));
    }
    if doctor.is_fixing {
        body = body.push(secondary_text(lang.tr("doctor_running_fix").to_string()));
    }
    if doctor.is_bootstrapping {
        body = body.push(secondary_text(lang.tr("doctor_running_bootstrap").to_string()));
    }
    if let Some(error) = &doctor.error {
        body = body.push(error_text(error));
    }

    body = match &doctor.report {
        Some(report) => {
            let mut body = body.push(summary_row(report, &lang));
            for check in &report.checks {
                body = body.push(check_row(check));
            }
            body
        }
        None => body.push(secondary_text(lang.tr("doctor_hint_desc").to_string())),
    };

    let watchdog = crate::view::crash_watchdog_card::crash_watchdog_card(state, &lang);

    column![
        card(
            Some(lang.tr("doctor_section_title").to_string()),
            column![actions, body].spacing(theme::SP_MD),
        ),
        Space::new().height(theme::SP_MD),
        watchdog,
    ]
    .spacing(theme::SP_SM)
    .into()
}

fn summary_row(report: &DoctorReport, lang: &Lang<'_>) -> Element<'static, Message> {
    let counts = [
        (DoctorStatus::Pass, "doctor_status_pass"),
        (DoctorStatus::Warn, "doctor_status_warn"),
        (DoctorStatus::Fail, "doctor_status_fail"),
        (DoctorStatus::Skip, "doctor_status_skip"),
    ];
    let mut summary = row![].spacing(theme::SP_SM);
    for (status, key) in counts {
        let count = report.count_by_status(status);
        summary = summary.push(badge(
            format!("{} {}", lang.tr(key), count),
            status_badge_kind(status),
        ));
    }
    summary.into()
}

fn check_row(check: &DoctorCheckResult) -> Element<'static, Message> {
    let mut lines = column![
        row![
            badge(status_label(check.status), status_badge_kind(check.status)),
            Space::new().width(theme::SP_SM),
            text(check.summary.clone())
                .size(13)
                .style(|t: &Theme| text::Style {
                    color: Some(theme::tokens(t).text_primary),
                }),
        ]
        .spacing(0)
        .align_y(Alignment::Center),
    ]
    .spacing(4);

    if let Some(detail) = &check.detail {
        lines = lines.push(
            text(detail.clone())
                .size(12)
                .style(|t: &Theme| text::Style {
                    color: Some(theme::tokens(t).text_secondary),
                }),
        );
    }
    if let Some(hint) = &check.hint {
        lines = lines.push(
            text(format!("→ {hint}"))
                .size(12)
                .style(|t: &Theme| text::Style {
                    color: Some(theme::tokens(t).text_tertiary),
                }),
        );
    }

    let row = row![
        lines.width(Length::Fill),
        text(check.id.clone())
            .size(11)
            .font(theme::MONO)
            .style(|t: &Theme| text::Style {
                color: Some(theme::tokens(t).text_tertiary),
            }),
    ]
    .align_y(Alignment::Center);

    iced::widget::container(row)
        .width(Length::Fill)
        .padding(theme::SP_SM)
        .style(|t: &Theme| iced::widget::container::Style {
            background: Some(theme::tokens(t).control_bg.into()),
            border: iced::Border {
                radius: iced::border::Radius::from(theme::R_CONTROL),
                width: 1.0,
                color: theme::tokens(t).card_border,
            },
            ..Default::default()
        })
        .into()
}

fn secondary_text(value: String) -> Element<'static, Message> {
    text(value)
        .size(12)
        .style(|t: &Theme| text::Style {
            color: Some(theme::tokens(t).text_secondary),
        })
        .into()
}

fn error_text(value: &str) -> Element<'_, Message> {
    text(value.to_string())
        .size(12)
        .style(|t: &Theme| text::Style {
            color: Some(theme::tokens(t).danger),
        })
        .into()
}

/// Standalone full-page Doctor view.
pub fn view(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);
    let title = row![
        text(lang.tr("nav_doctor").to_string())
            .size(24)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(theme::tokens(t).text_primary),
            }),
    ]
    .align_y(Alignment::Center);

    let content = column![
        title,
        Space::new().height(theme::SP_MD),
        section(state),
    ]
    .spacing(theme::SP_MD)
    .width(Length::Fill);

    scrollable(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
