//! Runtime page system-logs section: log-level picker, stream badge, log count,
//! scroll pin/freeze button, clear logs button, and structured log lines.

use iced::widget::{Scrollable, Space, button, column, container, pick_list, row, text, text_input};
use iced::{Alignment, Border, Element, Length, Theme, border};
use infiltrator_shared::country_flags::node_flag_emoji;
use infiltrator_shared::locales::{Lang, Localizer};

use crate::state::AppState;
use crate::types::message::Message;
use crate::types::runtime::RuntimeStreamState;
use crate::view::components::{
    form_input_style, style_accent, BadgeKind, badge, chip, form_pick_style, icon_button, section_header};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, MONO, R_CONTROL, SP_MD, SP_SM, SP_XS, tokens};

/// Fixed right padding so log text does not sit under the scrollbar.
const SCROLL_PAD: f32 = 16.0;

/// Severity classification for log entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Unknown,
}

/// Parsed structured log line representation.
#[derive(Debug, Clone, PartialEq)]
pub struct StructuredLogLine {
    pub level: LogLevel,
    pub timestamp: Option<String>,
    pub protocol: Option<String>,
    pub source: Option<String>,
    pub destination: Option<String>,
    pub rule: Option<String>,
    pub outbound_group: Option<String>,
    pub outbound_node: Option<String>,
    pub outbound_flag: Option<String>,
    pub message: String,
    pub is_connection: bool,
}

pub(super) fn logs_section<'a>(state: &'a AppState, lang: Lang<'a>) -> Element<'a, Message> {
    let log_count = state.diag.logs.len();
    let _is_zh = !lang.0.starts_with("en");

    // Header toolbar: log level picker, stream status badge, log count badge,
    // scroll pin/freeze button (Icon::Pin), and clear logs button (Icon::Trash2).
    let logs_trailing = row![
        pick_list(
            &["debug", "info", "warning", "error"][..],
            Some(state.diag.log_level.as_str()),
            |l| Message::SetLogLevel(l.to_string())
        )
        .text_size(12)
        .style(form_pick_style),
        Space::new().width(theme::SP_SM),
        stream_badge(&state.diag.logs_stream_state, &lang),
        Space::new().width(theme::SP_SM),
        badge(infiltrator_shared::i18n_interpolator::interpolate(&lang.tr("logs_count_unit"), &[("log_count", &log_count.to_string())]), BadgeKind::Neutral),
        Space::new().width(theme::SP_SM),
        button(
            row![
                svg_icons::icon_themed(Icon::FileText, 12.0, |t: &Theme| tokens(t).on_accent),
                Space::new().width(4.0),
                text(lang.tr("logs_btn_export_redacted").to_string()).size(11),
            ]
            .align_y(Alignment::Center)
        )
        .padding([4, 8])
        .style(style_accent)
        .on_press(Message::ExportRedactedLogs),
        Space::new().width(theme::SP_XS),
        icon_button(Icon::Pin, 14.0, Message::Noop),
        Space::new().width(theme::SP_XS),
        icon_button(Icon::Trash2, 14.0, Message::ClearRuntimeLogs),
    ]
    .align_y(Alignment::Center);

    let regex_filter = state.diag.log_filter.regex_query.trim().to_lowercase();
    let log_lines: Vec<Element<'_, Message>> = if state.diag.logs.is_empty() {
        vec![
            text(lang.tr("logs_no_realtime_records").to_string())
                .size(11)
                .font(MONO)
                .style(|t: &Theme| text::Style { color: Some(tokens(t).text_tertiary) })
                .into(),
        ]
    } else {
        state
            .diag
            .logs
            .iter()
            .filter(|l| {
                if regex_filter.is_empty() {
                    true
                } else {
                    l.to_lowercase().contains(&regex_filter)
                }
            })
            .map(|l| render_log_line(l, &state.shell.theme))
            .collect()
    };

    let regex_bar = text_input(
        lang.tr("logs_regex_placeholder").as_ref(),
        &state.diag.log_filter.regex_query,
    )
    .on_input(Message::UpdateLogRegexFilter)
    .padding([4, 8])
    .size(12)
    .font(MONO)
    .width(Length::Fill)
    .style(form_input_style);

    column![
        section_header(lang.tr("runtime_system_logs").as_ref(), Some(logs_trailing.into())),
        Space::new().height(theme::SP_XS),
        regex_bar,
        Space::new().height(theme::SP_SM),
        container(
            Scrollable::new(
                column(log_lines).spacing(4).padding(iced::Padding {
                    top: theme::SP_SM,
                    right: SCROLL_PAD,
                    bottom: theme::SP_SM,
                    left: theme::SP_SM,
                })
            )
            .id(iced::widget::Id::new("log_scroller"))
            .height(Length::Fixed(260.0))
        )
        .style(|t: &Theme| container::Style {
            background: Some(tokens(t).control_bg.into()),
            border: Border { radius: border::Radius::from(R_CONTROL), ..Default::default() },
            ..Default::default()
        })
        .width(Length::Fill)
        .height(Length::Fill),
    ]
    .into()
}

/// Render a single structured log line.
fn render_log_line<'a>(raw_line: &str, theme: &Theme) -> Element<'a, Message> {
    let parsed = parse_structured_log(raw_line);
    let tk = tokens(theme);
    let level_badge = badge_for_level(parsed.level);

    let ts_elem: Element<'a, Message> = if let Some(ts) = parsed.timestamp {
        text(format!("[{ts}]"))
            .size(11)
            .font(MONO)
            .style(move |_t: &Theme| text::Style { color: Some(tk.text_tertiary) })
            .into()
    } else {
        Space::new().width(0).into()
    };

    let proto_elem: Element<'a, Message> = if let Some(proto) = parsed.protocol {
        chip(proto)
    } else {
        Space::new().width(0).into()
    };

    if parsed.is_connection {
        let flow_elem: Element<'a, Message> = match (parsed.source, parsed.destination) {
            (Some(src), Some(dst)) => row![
                text(src).size(11).font(MONO).style(move |_t: &Theme| text::Style { color: Some(tk.text_secondary) }),
                text(" → ").size(11).font(MONO).style(move |_t: &Theme| text::Style { color: Some(tk.text_tertiary) }),
                text(dst).size(11).font(MONO).style(move |_t: &Theme| text::Style { color: Some(tk.text_primary) }),
            ]
            .align_y(Alignment::Center)
            .into(),
            _ => Space::new().width(0).into(),
        };

        let rule_elem: Element<'a, Message> = if let Some(rule) = parsed.rule {
            text(rule).size(11).font(MONO).style(move |_t: &Theme| text::Style { color: Some(tk.accent) }).into()
        } else {
            Space::new().width(0).into()
        };

        let outbound_elem: Element<'a, Message> = match (parsed.outbound_flag, parsed.outbound_node) {
            (Some(flag), Some(node)) => {
                let label = if let Some(grp) = parsed.outbound_group {
                    format!("{flag} {grp}[{node}]")
                } else {
                    format!("{flag} {node}")
                };
                container(
                    text(label).size(11).font(MONO).style(move |_t: &Theme| text::Style { color: Some(tk.text_secondary) })
                )
                .padding([2, 6])
                .style(move |_t: &Theme| container::Style {
                    background: Some(tk.chip_bg.into()),
                    border: Border { radius: border::Radius::from(4.0), ..Default::default() },
                    ..Default::default()
                })
                .into()
            }
            _ => Space::new().width(0).into(),
        };

        row![
            level_badge,
            Space::new().width(SP_XS),
            ts_elem,
            Space::new().width(SP_XS),
            proto_elem,
            Space::new().width(SP_SM),
            flow_elem,
            Space::new().width(SP_MD),
            rule_elem,
            Space::new().width(Length::Fill),
            outbound_elem,
        ]
        .align_y(Alignment::Center)
        .into()
    } else {
        let level_color = log_line_color(parsed.level, theme);
        row![
            level_badge,
            Space::new().width(SP_XS),
            ts_elem,
            Space::new().width(SP_XS),
            proto_elem,
            Space::new().width(SP_SM),
            text(parsed.message)
                .size(11)
                .font(MONO)
                .style(move |_t: &Theme| text::Style { color: Some(level_color) }),
        ]
        .align_y(Alignment::Center)
        .into()
    }
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

/// Parse a raw log line into structured log components.
pub fn parse_structured_log(raw: &str) -> StructuredLogLine {
    let raw_trimmed = raw.trim();

    // 1. Check if raw string is JSON format: {"type":"info","payload":"..."}
    let (level_from_json, payload_text) = if raw_trimmed.starts_with('{') && raw_trimmed.ends_with('}') {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(raw_trimmed) {
            let lvl = value.get("type").and_then(|v| v.as_str()).map(parse_level_str);
            let payload = value.get("payload").and_then(|v| v.as_str()).unwrap_or(raw_trimmed).to_string();
            (lvl, payload)
        } else {
            (None, raw_trimmed.to_string())
        }
    } else {
        (None, raw_trimmed.to_string())
    };

    let text_to_parse = payload_text.trim();

    // 2. Parse Level and Timestamp prefix
    let mut level = level_from_json.unwrap_or(LogLevel::Unknown);
    let mut timestamp = None;
    let mut rest = text_to_parse;

    // Check pattern: "LEVEL[TIMESTAMP] ..."
    if let Some(bracket_idx) = rest.find('[') {
        let prefix = rest[..bracket_idx].trim().to_uppercase();
        if let Some(end_bracket) = rest[bracket_idx..].find(']') {
            let close_idx = bracket_idx + end_bracket;
            let bracket_content = &rest[bracket_idx + 1..close_idx];

            let detected_level = match prefix.as_str() {
                "INFO" | "INF" => Some(LogLevel::Info),
                "WARN" | "WARNING" | "WRN" => Some(LogLevel::Warn),
                "ERROR" | "ERR" | "FATAL" => Some(LogLevel::Error),
                "DEBUG" | "DBG" => Some(LogLevel::Debug),
                _ => None,
            };

            if let Some(lvl) = detected_level {
                if level == LogLevel::Unknown {
                    level = lvl;
                }
                timestamp = Some(bracket_content.to_string());
                rest = rest[close_idx + 1..].trim();
            }
        }
    }

    if level == LogLevel::Unknown {
        level = parse_log_level(rest);
    }

    let mut protocol = None;
    if rest.starts_with('[')
        && let Some(close_bracket) = rest.find(']') {
            let tag = &rest[1..close_bracket];
            let tag_upper = tag.to_uppercase();
            if matches!(tag_upper.as_str(), "TCP" | "UDP" | "HTTP" | "HTTPS" | "TLS" | "QUIC" | "DNS" | "SOCKS5" | "ICMP") {
                protocol = Some(tag_upper);
                rest = rest[close_bracket + 1..].trim();
            }
        }

    // Check connection routing line: "... --> ... match ... using ..."
    let arrow_delimiter = if rest.contains("-->") {
        Some("-->")
    } else if rest.contains("->") {
        Some("->")
    } else if rest.contains('→') {
        Some("→")
    } else {
        None
    };

    if let Some(arrow) = arrow_delimiter {
        let parts: Vec<&str> = rest.splitn(2, arrow).collect();
        if parts.len() == 2 {
            let source_part = parts[0].trim().to_string();
            let remainder = parts[1].trim();

            let match_idx = remainder.find(" match ").or_else(|| remainder.find(" matched "));
            if let Some(m_idx) = match_idx {
                let dest_part = remainder[..m_idx].trim().to_string();
                let after_match = if remainder[m_idx..].starts_with(" match ") {
                    &remainder[m_idx + 7..]
                } else {
                    &remainder[m_idx + 9..]
                };

                let using_idx = after_match.find(" using ").or_else(|| after_match.find(" via "));
                let (rule_part, target_part) = if let Some(u_idx) = using_idx {
                    let r = after_match[..u_idx].trim().to_string();
                    let t = if after_match[u_idx..].starts_with(" using ") {
                        after_match[u_idx + 7..].trim()
                    } else {
                        after_match[u_idx + 5..].trim()
                    };
                    (Some(r), Some(t))
                } else {
                    (Some(after_match.trim().to_string()), None)
                };

                let mut outbound_group = None;
                let mut outbound_node = None;
                let mut outbound_flag = None;

                if let Some(target) = target_part {
                    if let Some(open_b) = target.find('[') {
                        if let Some(close_b) = target[open_b..].find(']') {
                            let grp = target[..open_b].trim();
                            let node = &target[open_b + 1..open_b + close_b];
                            if !grp.is_empty() {
                                outbound_group = Some(grp.to_string());
                            }
                            outbound_node = Some(node.to_string());
                            outbound_flag = Some(node_flag_emoji(node).to_string());
                        }
                    } else {
                        outbound_node = Some(target.to_string());
                        outbound_flag = Some(node_flag_emoji(target).to_string());
                    }
                }

                return StructuredLogLine {
                    level,
                    timestamp,
                    protocol,
                    source: Some(source_part),
                    destination: Some(dest_part),
                    rule: rule_part,
                    outbound_group,
                    outbound_node,
                    outbound_flag,
                    message: text_to_parse.to_string(),
                    is_connection: true,
                };
            }
        }
    }

    StructuredLogLine {
        level,
        timestamp,
        protocol,
        source: None,
        destination: None,
        rule: None,
        outbound_group: None,
        outbound_node: None,
        outbound_flag: None,
        message: rest.to_string(),
        is_connection: false,
    }
}

/// Helper to parse level from string representation.
fn parse_level_str(s: &str) -> LogLevel {
    match s.trim().to_ascii_lowercase().as_str() {
        "error" | "err" | "fatal" => LogLevel::Error,
        "warn" | "warning" | "wrn" => LogLevel::Warn,
        "info" | "inf" => LogLevel::Info,
        "debug" | "dbg" => LogLevel::Debug,
        _ => LogLevel::Unknown,
    }
}

/// Classify a raw log line into a severity level.
fn parse_log_level(line: &str) -> LogLevel {
    let trimmed = line.trim();
    let upper = trimmed.to_uppercase();

    if upper.starts_with("ERROR")
        || upper.starts_with("FATAL")
        || upper.starts_with("ERR")
        || upper.starts_with("[ERROR")
        || upper.starts_with("[FATAL")
        || upper.starts_with("[ERR")
    {
        return LogLevel::Error;
    }
    if upper.starts_with("WARN") || upper.starts_with("[WARN") || upper.starts_with("[WRN") {
        return LogLevel::Warn;
    }
    if upper.starts_with("DEBUG") || upper.starts_with("DBG") || upper.starts_with("[DEBUG") || upper.starts_with("[DBG") {
        return LogLevel::Debug;
    }
    if upper.starts_with("INFO") || upper.starts_with("INF") || upper.starts_with("[INFO") || upper.starts_with("[INF") {
        return LogLevel::Info;
    }

    if upper.contains("LEVEL=ERROR")
        || upper.contains("LEVEL=FATAL")
        || upper.contains("[ERROR]")
        || upper.contains("[FATAL]")
        || upper.contains("[ERR]")
        || upper.contains(" ERROR ")
        || upper.contains(" FATAL ")
    {
        LogLevel::Error
    } else if upper.contains("LEVEL=WARN")
        || upper.contains("LEVEL=WARNING")
        || upper.contains("[WARN]")
        || upper.contains("[WARNING]")
        || upper.contains("[WRN]")
        || upper.contains(" WARN ")
        || upper.contains(" WARNING ")
    {
        LogLevel::Warn
    } else if upper.contains("LEVEL=DEBUG")
        || upper.contains("[DEBUG]")
        || upper.contains("[DBG]")
        || upper.contains(" DEBUG ")
        || upper.contains(" DBG ")
    {
        LogLevel::Debug
    } else if upper.contains("LEVEL=INFO")
        || upper.contains("[INFO]")
        || upper.contains("[INF]")
        || upper.contains(" INFO ")
    {
        LogLevel::Info
    } else if upper.contains("ERROR") || upper.contains("FATAL") {
        LogLevel::Error
    } else if upper.contains("WARN") {
        LogLevel::Warn
    } else if upper.contains("DEBUG") {
        LogLevel::Debug
    } else if upper.contains("INFO") {
        LogLevel::Info
    } else {
        LogLevel::Unknown
    }
}

/// Small tinted pill for a log level.
fn badge_for_level(level: LogLevel) -> Element<'static, Message> {
    let (label, kind) = match level {
        LogLevel::Error => ("ERR", BadgeKind::Danger),
        LogLevel::Warn => ("WARN", BadgeKind::Warning),
        LogLevel::Info => ("INFO", BadgeKind::Accent),
        LogLevel::Debug => ("DEBUG", BadgeKind::Neutral),
        LogLevel::Unknown => ("", BadgeKind::Neutral),
    };
    badge(label, kind)
}

/// Line text color mapped by severity level.
fn log_line_color(level: LogLevel, t: &Theme) -> iced::Color {
    let tk = tokens(t);
    match level {
        LogLevel::Error => tk.danger,
        LogLevel::Warn => tk.warning,
        LogLevel::Info => tk.text_secondary,
        LogLevel::Debug => tk.text_tertiary,
        LogLevel::Unknown => tk.text_secondary,
    }
}

#[cfg(test)]
#[path = "../../../tests/gui/view_runtime_logs_tests.rs"]
mod tests;
