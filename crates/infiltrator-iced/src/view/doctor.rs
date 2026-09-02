//! 设置页的 Doctor 体检卡片：一键体检 / 一键修复 / 初始化引导，加状态
//! 徽章（pass/warn/fail/skip）+ summary + hint 的检查项列表。
//!
//! 约束：纯渲染投影，只读 `diag.doctor`；文案走双语内联回退（locales.rs
//! 不在本 wave 的文件所有权内）。

use crate::state::AppState;
use crate::types::doctor::{DoctorCheckResult, DoctorReport, DoctorStatus};
use crate::types::message::Message;
use crate::view::components::{BadgeKind, badge, card};
use crate::view::theme::{self, FONT_MEDIUM};
use iced::widget::{Space, button, column, row, text};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_shared::locales::Lang;

/// 双语标签：无 locale key 时的内联回退（与 overview.rs 的 stat_label 同型）。
fn label(lang: &Lang<'_>, zh: &str, en: &str) -> String {
    if lang.0.starts_with("en") {
        en.to_string()
    } else {
        zh.to_string()
    }
}

/// Doctor 检查状态 → 徽章配色。
pub(crate) fn status_badge_kind(status: DoctorStatus) -> BadgeKind {
    match status {
        DoctorStatus::Pass => BadgeKind::Success,
        DoctorStatus::Warn => BadgeKind::Warning,
        DoctorStatus::Fail => BadgeKind::Danger,
        DoctorStatus::Skip => BadgeKind::Neutral,
    }
}

/// 徽章文本（大写缩写，与 "ACTIVE"/"ERROR" 徽章风格一致）。
pub(crate) fn status_label(status: DoctorStatus) -> &'static str {
    match status {
        DoctorStatus::Pass => "PASS",
        DoctorStatus::Warn => "WARN",
        DoctorStatus::Fail => "FAIL",
        DoctorStatus::Skip => "SKIP",
    }
}

/// 设置页挂载的体检卡片。
pub fn section(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);
    let doctor = &state.diag.doctor;

    let busy = doctor.is_running || doctor.is_fixing || doctor.is_bootstrapping;

    let actions = row![
        button(
            text(label(&lang, "一键体检", "Run check"))
                .size(12)
                .font(FONT_MEDIUM)
        )
        .padding([7, 14])
        .style(button::primary)
        .on_press_maybe((!doctor.is_running && !busy).then_some(Message::RunDoctor)),
        Space::new().width(theme::SP_SM),
        button(
            text(label(&lang, "一键修复", "Fix"))
                .size(12)
                .font(FONT_MEDIUM)
        )
        .padding([7, 14])
        .style(button::secondary)
        .on_press_maybe((!doctor.is_fixing && !busy).then_some(Message::RunDoctorFix)),
        Space::new().width(theme::SP_SM),
        button(
            text(label(&lang, "初始化引导", "Bootstrap"))
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
        body = body.push(secondary_text(label(
            &lang,
            "体检运行中…",
            "Check run in progress…",
        )));
    }
    if doctor.is_fixing {
        body = body.push(secondary_text(label(&lang, "修复执行中…", "Fixing…")));
    }
    if doctor.is_bootstrapping {
        body = body.push(secondary_text(label(
            &lang,
            "初始化引导执行中…",
            "Bootstrap in progress…",
        )));
    }
    if let Some(error) = &doctor.error {
        body = body.push(error_text(error));
    }

    body = match &doctor.report {
        Some(report) => {
            let mut body = body.push(summary_row(report));
            for check in &report.checks {
                body = body.push(check_row(check));
            }
            body
        }
        None => body.push(secondary_text(label(
            &lang,
            "点击「一键体检」运行自检：目录、settings、内核、控制端口与 pid 文件。",
            "Run the self-check to inspect directories, settings, cores, the controller port and the pid file.",
        ))),
    };

    card(
        Some(label(&lang, "体检与修复", "Doctor")),
        column![actions, body].spacing(theme::SP_MD),
    )
}

/// pass/warn/fail/skip 计数徽章行。
fn summary_row(report: &DoctorReport) -> Element<'static, Message> {
    let counts = [
        (DoctorStatus::Pass, "通过", "pass"),
        (DoctorStatus::Warn, "警告", "warn"),
        (DoctorStatus::Fail, "失败", "fail"),
        (DoctorStatus::Skip, "跳过", "skip"),
    ];
    let mut summary = row![].spacing(theme::SP_SM);
    for (status, zh, en) in counts {
        let count = report.count_by_status(status);
        summary = summary.push(badge(
            format!("{en} {zh} {count}"),
            status_badge_kind(status),
        ));
    }
    summary.into()
}

/// 单条检查：状态徽章 + summary/detail + hint（可折叠进第二行小字）。
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
