//! Doctor diagnostics card: self-healing check / repair / bootstrap actions.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::component_card::card;
use crate::view::components::{BadgeKind, badge};
use crate::view::crash_watchdog_card::crash_watchdog_card;
use crate::view::theme;
use crate::view::theme::{FONT_MEDIUM, FONT_SEMIBOLD};
use crate::view_root::interaction_regions::InteractionRegion;
use iced::border::Radius;
use iced::widget::{Space, button, column, container, row, scrollable, text};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_application::doctor_projection::{check_detail, check_name, status_key, summary};
use infiltrator_contract::doctor::{DoctorCheckResult, DoctorReport, DoctorStatus};
use infiltrator_contract::surface_snapshot::{DoctorCheckSnapshot, PageStatus};
use infiltrator_shared::i18n_interpolator::localize;
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

/// Doctor section in settings / diagnostics.
pub fn section(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);
    let doctor = &state.diag.doctor;

    let page = state
        .surface
        .latest()
        .map(|snapshot| &snapshot.pages.doctor);
    let busy = doctor.action.pending.is_some()
        || doctor.is_running
        || doctor.is_fixing
        || doctor.is_bootstrapping;
    let repairable = !busy
        && page.is_some_and(|page| {
            matches!(page.status, PageStatus::Ready)
                && page
                    .data
                    .as_ref()
                    .is_some_and(|data| data.checks.iter().any(|check| check.fix_available))
        });

    let actions: Element<'_, Message> = if state.shell.viewport.tier.is_compact() {
        column![
            button(
                text(lang.tr("doctor_btn_check").to_string())
                    .size(12)
                    .font(FONT_MEDIUM),
            )
            .width(Length::Fill)
            .padding([7, 14])
            .style(button::primary)
            .on_press_maybe((!doctor.is_running && !busy).then_some(Message::RunDoctor)),
            button(
                text(lang.tr("doctor_btn_fix").to_string())
                    .size(12)
                    .font(FONT_MEDIUM),
            )
            .width(Length::Fill)
            .padding([7, 14])
            .style(button::secondary)
            .on_press_maybe(repairable.then_some(Message::RunDoctorFix)),
            button(
                text(lang.tr("doctor_btn_bootstrap").to_string())
                    .size(12)
                    .font(FONT_MEDIUM),
            )
            .width(Length::Fill)
            .padding([7, 14])
            .style(button::secondary)
            .on_press_maybe((!doctor.is_bootstrapping && !busy).then_some(Message::RunBootstrap)),
        ]
        .spacing(theme::SP_SM)
        .into()
    } else {
        row![
            button(
                text(lang.tr("doctor_btn_check").to_string())
                    .size(12)
                    .font(FONT_MEDIUM),
            )
            .padding([7, 14])
            .style(button::primary)
            .on_press_maybe((!doctor.is_running && !busy).then_some(Message::RunDoctor)),
            Space::new().width(theme::SP_SM),
            button(
                text(lang.tr("doctor_btn_fix").to_string())
                    .size(12)
                    .font(FONT_MEDIUM),
            )
            .padding([7, 14])
            .style(button::secondary)
            .on_press_maybe(repairable.then_some(Message::RunDoctorFix)),
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
        .align_y(Alignment::Center)
        .into()
    };

    let mut body = column![].spacing(theme::SP_SM);

    if doctor.action.pending.is_some() {
        body = body.push(secondary_text(
            lang.tr("doctor_operation_pending").into_owned(),
        ));
    }
    if doctor.is_running {
        body = body.push(secondary_text(lang.tr("doctor_running_check").to_string()));
    }
    if doctor.is_fixing {
        body = body.push(secondary_text(lang.tr("doctor_running_fix").to_string()));
    }
    if doctor.is_bootstrapping {
        body = body.push(secondary_text(
            lang.tr("doctor_running_bootstrap").to_string(),
        ));
    }
    if let Some(error) = &doctor.error {
        body = body.push(error_text(error));
    }

    if doctor.action.can_retry() {
        body = body.push(
            container(
                button(text(lang.tr("doctor_retry_action").into_owned()))
                    .on_press_maybe((!busy).then_some(Message::RetryDoctorCommand)),
            )
            .id(InteractionRegion::DoctorRetry.id()),
        );
    }
    if let Some(page) = page {
        if let PageStatus::Failed { failure } = &page.status
            && doctor.error.as_deref() != Some(&failure.message)
        {
            body = body.push(error_text(&failure.message));
        }
        if let Some(data) = &page.data {
            body = body.push(secondary_text(summary(
                data.checks.iter().map(|check| check.state),
                data.report_finished_at,
                &state.shell.lang,
            )));
            if !data.last_run.is_empty() {
                body = body.push(secondary_text(localize(
                    &state.shell.lang,
                    "doctor_last_run_value",
                    &[("time", data.last_run.clone())],
                )));
            }
            for check in &data.checks {
                body = body.push(snapshot_check_row(check, &state.shell.lang, repairable));
            }
        }
    } else if let Some(report) = &doctor.report {
        body = body.push(summary_row(report, &lang));
        for check in &report.checks {
            body = body.push(check_row(check, &state.shell.lang));
        }
    } else {
        body = body.push(secondary_text(lang.tr("doctor_not_observed").into_owned()));
    }

    let watchdog = crash_watchdog_card(state, &lang);

    column![
        card(
            Some(lang.tr("doctor_section_title").to_string()),
            column![
                actions,
                container(body).id(InteractionRegion::DoctorReport.id())
            ]
            .spacing(theme::SP_MD),
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

fn check_row(check: &DoctorCheckResult, code: &str) -> Element<'static, Message> {
    let mut lines = column![
        row![
            badge(
                Lang(code).tr(status_key(check.status)).into_owned(),
                status_badge_kind(check.status)
            ),
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

    container(row)
        .width(Length::Fill)
        .padding(theme::SP_SM)
        .style(|t: &Theme| container::Style {
            background: Some(theme::tokens(t).control_bg.into()),
            border: iced::Border {
                radius: Radius::from(theme::R_CONTROL),
                width: theme::HAIRLINE,
                color: theme::tokens(t).card_border,
            },
            ..Default::default()
        })
        .into()
}

fn snapshot_check_row(
    check: &DoctorCheckSnapshot,
    code: &str,
    repairable: bool,
) -> Element<'static, Message> {
    let lang = Lang(code);
    let mut content = column![
        text(check_name(check.kind, &check.name, &check.category, code)).size(13),
        text(check_detail(
            check.detail_copy_key.as_deref(),
            &check.detail,
            code
        ))
        .size(12),
        row![
            badge(
                lang.tr(status_key(check.state)).into_owned(),
                status_badge_kind(check.state)
            ),
            button(text(lang.tr("doctor_repair_row").into_owned()).size(12)).on_press_maybe(
                (repairable && check.fix_available)
                    .then(|| Message::RepairDoctorIssue(check.id.clone()))
            )
        ]
        .spacing(theme::SP_SM)
        .align_y(Alignment::Center)
    ]
    .spacing(4);
    if let Some(hint) = &check.hint {
        content = content.push(text(hint.clone()).size(12));
    }
    container(content)
        .width(Length::Fill)
        .padding(theme::SP_SM)
        .style(|t: &Theme| container::Style {
            background: Some(theme::tokens(t).control_bg.into()),
            border: iced::Border {
                radius: Radius::from(theme::R_CONTROL),
                width: theme::HAIRLINE,
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

    let content = column![title, Space::new().height(theme::SP_MD), section(state),]
        .spacing(theme::SP_MD)
        .width(Length::Fill);

    scrollable(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
