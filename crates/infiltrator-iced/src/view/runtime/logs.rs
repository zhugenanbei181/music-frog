//! Runtime page system-logs section: log-level picker, clear button and the
//! scrollable, badge-annotated log line list.

use crate::locales::{Lang, Localizer};
use crate::view::components::{BadgeKind, icon_button, section_header};
use crate::view::runtime::styles::pick_style;
use crate::view::svg_icons::Icon;
use crate::view::theme::{self, MONO, R_CONTROL, tokens};
use crate::{AppState, Message};
use iced::widget::{Scrollable, Space, column, container, pick_list, row, text};
use iced::{Alignment, Border, Element, Length, Theme, border};

/// Fixed right padding so log text does not sit under the scrollbar.
const SCROLL_PAD: f32 = 16.0;

pub(super) fn logs_section<'a>(state: &'a AppState, lang: Lang<'a>) -> Element<'a, Message> {
    let logs_trailing = row![
        pick_list(
            &["debug", "info", "warning", "error"][..],
            Some(state.diag.log_level.as_str()),
            |l| Message::SetLogLevel(l.to_string())
        )
        .text_size(12)
        .style(pick_style),
        Space::new().width(theme::SP_SM),
        icon_button(Icon::Trash2, 14.0, Message::ClearRuntimeLogs),
    ]
    .align_y(Alignment::Center);

    let log_lines: Vec<Element<'_, Message>> = state
        .diag
        .logs
        .iter()
        .map(|l| {
            row![
                match log_kind(l) {
                    Some(kind) => badge_for_kind(kind),
                    None => Space::new().width(0).height(0).into(),
                },
                Space::new().width(theme::SP_SM),
                text(l.clone())
                    .size(11)
                    .font(MONO)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_secondary),
                    }),
            ]
            .align_y(Alignment::Center)
            .into()
        })
        .collect();

    column![
        section_header(
            lang.tr("runtime_system_logs").as_ref(),
            Some(logs_trailing.into())
        ),
        Space::new().height(theme::SP_MD),
        container(
            Scrollable::new(column(log_lines).spacing(2).padding(iced::Padding {
                top: theme::SP_SM,
                right: SCROLL_PAD,
                bottom: theme::SP_SM,
                left: theme::SP_SM,
            }))
            .id(iced::widget::Id::new("log_scroller"))
            // Definite height: `snap_to("log_scroller", ...)` in the update
            // path needs a real scrolling viewport, and a Fill height would
            // collapse inside the auto-height card.
            .height(Length::Fixed(240.0))
        )
        .style(|t: &Theme| container::Style {
            background: Some(tokens(t).control_bg.into()),
            border: Border {
                radius: border::Radius::from(R_CONTROL),
                ..Default::default()
            },
            ..Default::default()
        })
        .width(Length::Fill)
        .height(Length::Fill),
    ]
    .into()
}

/// Classify a raw log line into a badge kind (info→Neutral, warn→Warning,
/// error→Danger). Lines without a recognizable level get `None`.
fn log_kind(line: &str) -> Option<BadgeKind> {
    let upper = line.to_uppercase();
    if upper.contains("ERROR") || upper.contains("ERR") || upper.contains("FATAL") {
        Some(BadgeKind::Danger)
    } else if upper.contains("WARN") {
        Some(BadgeKind::Warning)
    } else if upper.contains("INFO")
        || upper.contains("INF")
        || upper.contains("DEBUG")
        || upper.contains("DBG")
    {
        Some(BadgeKind::Neutral)
    } else {
        None
    }
}

/// Small tinted pill for a log level.
fn badge_for_kind(kind: BadgeKind) -> Element<'static, Message> {
    crate::view::components::badge(level_label(kind), kind)
}

fn level_label(kind: BadgeKind) -> &'static str {
    match kind {
        BadgeKind::Danger => "ERR",
        BadgeKind::Warning => "WARN",
        _ => "INFO",
    }
}
