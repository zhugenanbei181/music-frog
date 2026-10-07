//! Runtime page system-logs section: log-level picker, stream badge, log count,
//! scroll pin/freeze button, clear logs button, and structured log lines.

use crate::state::AppState;
use crate::types::message::Message;
use crate::types::runtime::RuntimeStreamState;
use crate::view::component_forms::form_pick_style;
use crate::view::components::{BadgeKind, badge, icon_button, section_header};
use crate::view::runtime::logs_controls;
use crate::view::svg_icons::Icon;
use crate::view::theme;
use crate::view::theme::{MONO, R_CONTROL, tokens};
use crate::view_root::interaction_regions::InteractionRegion;
use iced::advanced::text::Wrapping;
use iced::widget::{
    Id, Scrollable, Space, column, container, pick_list, rich_text, row, span, text,
};
use iced::{Alignment, Border, Element, Length, Renderer, Theme, border};
use infiltrator_application::log_search::LogSearchRow;
use infiltrator_contract::logs::LogLevel;
use infiltrator_contract::search_text::SearchTextRun;
use infiltrator_contract::surface_snapshot::PageStatus;
use infiltrator_shared::i18n_interpolator::interpolate;
use infiltrator_shared::locales::{Lang, Localizer};
use std::fmt;
use std::fmt::{Display, Formatter};
use std::iter::once;

#[derive(Clone, PartialEq)]
struct LogChoice {
    level: Option<LogLevel>,
    label: String,
}
impl Display for LogChoice {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.label)
    }
}

/// Fixed right padding so log text does not sit under the scrollbar.
const SCROLL_PAD: f32 = 16.0;

pub(crate) fn logs_section<'a>(state: &'a AppState, lang: Lang<'a>) -> Element<'a, Message> {
    let log_count = state
        .surface
        .latest()
        .and_then(|snapshot| snapshot.pages.logs.data.as_ref())
        .map_or(state.diag.logs.len(), |logs| logs.total_entries);
    let failure = state.diag.log_command_failure.as_ref().or_else(|| {
        state
            .surface
            .latest()
            .and_then(|snapshot| match &snapshot.pages.logs.status {
                PageStatus::Failed { failure } | PageStatus::Unavailable { failure } => {
                    Some(failure)
                }
                _ => None,
            })
    });
    let failure_copy: Element<'_, Message> = match failure {
        Some(failure) => text(failure.message.clone()).size(12).into(),
        None => Space::new().height(0).into(),
    };

    let choices: Vec<_> = once(LogChoice {
        level: None,
        label: lang.tr("logs_level_all").into_owned(),
    })
    .chain(LogLevel::ALL.into_iter().map(|level| LogChoice {
        level: Some(level),
        label: level.label().into(),
    }))
    .collect();
    let selected_level = (!state.diag.log_filter.level_filter.is_empty())
        .then(|| LogLevel::from_identifier(&state.diag.log_filter.level_filter));
    let selected = choices
        .iter()
        .find(|choice| choice.level == selected_level)
        .cloned();
    let status_controls = row![
        pick_list(choices, selected, |choice: LogChoice| {
            Message::SetLogLevelFilter(
                choice
                    .level
                    .map(|level| level.label().into())
                    .unwrap_or_default(),
            )
        })
        .text_size(12)
        .style(form_pick_style),
        Space::new().width(theme::SP_SM),
        stream_badge(&state.diag.logs_stream_state, &lang),
        Space::new().width(theme::SP_SM),
        badge(
            interpolate(
                &lang.tr("logs_count_unit"),
                &[("log_count", &log_count.to_string())]
            ),
            BadgeKind::Neutral
        ),
        Space::new().width(theme::SP_SM),
    ]
    .align_y(Alignment::Center);
    let actions = row![
        logs_controls::export(state),
        Space::new().width(theme::SP_XS),
        logs_controls::follow(state),
        Space::new().width(theme::SP_XS),
        icon_button(Icon::Trash2, 14.0, Message::ClearRuntimeLogs),
    ]
    .align_y(Alignment::Center);

    let logs_trailing: Element<'_, Message> = if state.shell.viewport.tier.is_narrow() {
        column![status_controls, actions].spacing(4).into()
    } else {
        row![status_controls, Space::new().width(Length::Fill), actions]
            .align_y(Alignment::Center)
            .into()
    };

    let log_lines: Vec<Element<'_, Message>> = state
        .diag
        .log_search
        .rows()
        .iter()
        .filter(|row| row.visible)
        .map(|row| render_log_line(row, state))
        .collect();
    column![
        section_header(lang.tr("runtime_system_logs").as_ref(), None),
        logs_trailing,
        Space::new().height(theme::SP_XS),
        logs_controls::search(state),
        failure_copy,
        Space::new().height(theme::SP_SM),
        container(
            Scrollable::new(column(log_lines).spacing(4).padding(iced::Padding {
                top: theme::SP_SM,
                right: SCROLL_PAD,
                bottom: theme::SP_SM,
                left: theme::SP_SM,
            }))
            .id(Id::new("log_scroller"))
            .on_scroll(|viewport| Message::LogsScrolled {
                offset: viewport.absolute_offset().y,
                content: viewport.content_bounds().height,
                viewport: viewport.bounds().height
            })
            .height(Length::Fixed(260.0))
        )
        .style(|t: &Theme| container::Style {
            background: Some(tokens(t).control_bg.into()),
            border: Border {
                radius: border::Radius::from(R_CONTROL),
                ..Default::default()
            },
            ..Default::default()
        })
        .id(InteractionRegion::LogsResults.id())
        .width(Length::Fill)
        .height(Length::Fill),
    ]
    .into()
}

/// Native presentation replays neutral spans without parsing or filtering log facts.
fn render_log_line<'a>(record: &'a LogSearchRow, state: &'a AppState) -> Element<'a, Message> {
    container(
        column![
            row![
                log_text(&record.timestamp, state, Length::Shrink, None),
                log_text(
                    &record.level_label,
                    state,
                    Length::Shrink,
                    Some(record.level)
                ),
                log_text(&record.tag, state, Length::Shrink, None)
            ]
            .spacing(theme::SP_SM),
            log_text(&record.message, state, Length::Fill, None),
        ]
        .spacing(2),
    )
    .id(Id::from(format!("log-row-{}", record.id)))
    .padding([4, 8])
    .width(Length::Fill)
    .into()
}
fn log_text<'a>(
    runs: &'a [SearchTextRun],
    state: &'a AppState,
    width: Length,
    level: Option<LogLevel>,
) -> Element<'a, Message> {
    let palette = tokens(&state.shell.theme);
    let plain = match level {
        Some(LogLevel::Error) => palette.danger,
        Some(LogLevel::Warn) => palette.warning,
        Some(LogLevel::Debug) => palette.text_tertiary,
        _ => palette.text_primary,
    };
    let spans: Vec<_> = runs
        .iter()
        .map(|run| {
            span(run.text.as_str()).color(if run.highlighted {
                palette.accent
            } else {
                plain
            })
        })
        .collect();
    rich_text::<(), Message, Theme, Renderer>(spans)
        .size(11)
        .font(MONO)
        .width(width)
        .wrapping(Wrapping::WordOrGlyph)
        .into()
}

fn stream_badge<'a>(state: &RuntimeStreamState, lang: &Lang<'_>) -> Element<'a, Message> {
    let (key, kind) = match state {
        RuntimeStreamState::Idle => ("conn_state_disconnected", BadgeKind::Neutral),
        RuntimeStreamState::Connecting => ("conn_state_connecting", BadgeKind::Neutral),
        RuntimeStreamState::Connected => ("conn_state_live", BadgeKind::Success),
        RuntimeStreamState::Reconnecting => ("conn_state_reconnecting", BadgeKind::Warning),
        RuntimeStreamState::Failed(_) => ("conn_state_unavailable", BadgeKind::Danger),
    };
    badge(lang.tr(key).to_string(), kind)
}

#[cfg(test)]
#[path = "../../../tests/gui/view_runtime_logs_tests.rs"]
mod tests;
