//! A single pure parser for original controller log records on all peers.
use infiltrator_contract::logs::{LogLevel, LogStreamState};
use infiltrator_contract::surface_snapshot::{LogSnapshot, LogsPageSnapshot, PageData};
use infiltrator_shared::country_flags::node_flag_emoji;

#[derive(Debug, Clone, PartialEq, Eq)]
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

pub fn project_log_record(id: u64, raw: &str) -> LogSnapshot {
    let parsed = parse_structured_log(raw);
    LogSnapshot {
        id,
        raw: Some(raw.into()),
        level: parsed.level.label().into(),
        timestamp: parsed.timestamp.unwrap_or_default(),
        tag: parsed.protocol.unwrap_or_default(),
        message: parsed.message,
    }
}
pub fn page_from_log_records<'a>(
    records: impl Iterator<Item = &'a str>,
) -> PageData<LogsPageSnapshot> {
    let entries: Vec<_> = records
        .enumerate()
        .map(|(index, raw)| project_log_record(index as u64 + 1, raw))
        .collect();
    let data = LogsPageSnapshot {
        total_entries: entries.len(),
        entries,
        active_level: None,
        stream: LogStreamState::Live,
    };
    if data.entries.is_empty() {
        PageData::empty(data)
    } else {
        PageData::ready(data)
    }
}

/// Parse a raw log line into structured log components.
pub fn parse_structured_log(raw: &str) -> StructuredLogLine {
    let raw_trimmed = raw.trim();

    // 1. Check if raw string is JSON format: {"type":"info","payload":"..."}
    let (level_from_json, payload_text) =
        if raw_trimmed.starts_with('{') && raw_trimmed.ends_with('}') {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(raw_trimmed) {
                let lvl = value
                    .get("type")
                    .and_then(|v| v.as_str())
                    .map(LogLevel::from_identifier);
                let payload = value
                    .get("payload")
                    .and_then(|v| v.as_str())
                    .unwrap_or(raw_trimmed)
                    .to_string();
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
                if level_from_json.is_none() && level == LogLevel::Unknown {
                    level = lvl;
                }
                timestamp = Some(bracket_content.to_string());
                rest = rest[close_idx + 1..].trim();
            }
        }
    }

    if level_from_json.is_none() && level == LogLevel::Unknown {
        level = parse_log_level(rest);
    }

    let mut protocol = None;
    if rest.starts_with('[')
        && let Some(close_bracket) = rest.find(']')
    {
        let tag = &rest[1..close_bracket];
        let tag_upper = tag.to_uppercase();
        if matches!(
            tag_upper.as_str(),
            "TCP" | "UDP" | "HTTP" | "HTTPS" | "TLS" | "QUIC" | "DNS" | "SOCKS5" | "ICMP"
        ) {
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

            let match_idx = remainder
                .find(" match ")
                .or_else(|| remainder.find(" matched "));
            if let Some(m_idx) = match_idx {
                let dest_part = remainder[..m_idx].trim().to_string();
                let after_match = if remainder[m_idx..].starts_with(" match ") {
                    &remainder[m_idx + 7..]
                } else {
                    &remainder[m_idx + 9..]
                };

                let using_idx = after_match
                    .find(" using ")
                    .or_else(|| after_match.find(" via "));
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
                    message: rest.to_string(),
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

/// Classify a raw log line into a severity level.
fn parse_log_level(line: &str) -> LogLevel {
    let prefix = line
        .trim()
        .trim_start_matches('[')
        .split(|ch: char| ch.is_whitespace() || ch == '[' || ch == ']' || ch == ':')
        .next()
        .unwrap_or_default();
    LogLevel::from_identifier(prefix)
}
