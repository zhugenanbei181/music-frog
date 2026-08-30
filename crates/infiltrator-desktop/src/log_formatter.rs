use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
    Silent,
}

impl LogLevel {
    fn from_str_case_insensitive(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "debug" | "dbg" => LogLevel::Debug,
            "info" | "inf" => LogLevel::Info,
            "warning" | "warn" => LogLevel::Warning,
            "error" | "err" => LogLevel::Error,
            "silent" => LogLevel::Silent,
            _ => LogLevel::Info,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct FormattedLog {
    pub level: LogLevel,
    pub timestamp_str: String,
    pub tag: Option<String>,
    pub message: String,
    pub ansi_styled: String,
}

pub struct LogFormatter;

impl LogFormatter {
    pub fn parse_log_line(raw: &str) -> FormattedLog {
        let trimmed = raw.trim();

        // Check if it's key-value format e.g., time="2026-08-30" level=info msg="hello"
        if trimmed.contains("time=") && trimmed.contains("level=") && trimmed.contains("msg=") {
            let mut time_str = String::new();
            let mut level_str = String::new();
            let mut msg_str = String::new();

            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            for part in parts {
                if let Some(val) = part.strip_prefix("time=\"") {
                    if let Some(end) = val.find('"') {
                        time_str = val[..end].to_string();
                    }
                } else if let Some(val) = part.strip_prefix("level=") {
                    level_str = val.trim_matches('"').to_string();
                }
            }

            // Msg might have spaces, a better parsing for msg:
            if let Some(msg_start) = trimmed.find("msg=\"") {
                let after_msg = &trimmed[msg_start + 5..];
                if let Some(msg_end) = after_msg.find('"') {
                    msg_str = after_msg[..msg_end].to_string();
                } else {
                    msg_str = after_msg.to_string();
                }
            }

            let level = LogLevel::from_str_case_insensitive(&level_str);
            let ansi_styled = Self::format_ansi(level.clone(), &msg_str);

            return FormattedLog {
                level,
                timestamp_str: time_str,
                tag: None,
                message: msg_str,
                ansi_styled,
            };
        }

        // Check if it's bracket format e.g., [INFO] [DNS] Resolved example.com
        if trimmed.starts_with('[') {
            if let Some(level_end) = trimmed.find(']') {
                let level_str = &trimmed[1..level_end];
                let level = LogLevel::from_str_case_insensitive(level_str);
                
                let remainder = trimmed[level_end + 1..].trim();
                let mut tag = None;
                let mut message = remainder.to_string();

                if remainder.starts_with('[') {
                    if let Some(tag_end) = remainder.find(']') {
                        tag = Some(remainder[1..tag_end].to_string());
                        message = remainder[tag_end + 1..].trim().to_string();
                    }
                }

                let ansi_styled = Self::format_ansi(level.clone(), &message);

                return FormattedLog {
                    level,
                    timestamp_str: String::new(),
                    tag,
                    message,
                    ansi_styled,
                };
            }
        }

        // Fallback
        let ansi_styled = Self::format_ansi(LogLevel::Info, trimmed);
        FormattedLog {
            level: LogLevel::Info,
            timestamp_str: String::new(),
            tag: None,
            message: trimmed.to_string(),
            ansi_styled,
        }
    }

    pub fn format_ansi(level: LogLevel, message: &str) -> String {
        let (prefix, suffix) = match level {
            LogLevel::Error => ("\x1b[31m", "\x1b[0m"),   // Red
            LogLevel::Warning => ("\x1b[33m", "\x1b[0m"), // Yellow
            LogLevel::Info => ("\x1b[32m", "\x1b[0m"),    // Green
            LogLevel::Debug => ("\x1b[36m", "\x1b[0m"),   // Cyan
            LogLevel::Silent => ("", ""),
        };
        format!("{}{}{}", prefix, message, suffix)
    }

    pub fn format_html(level: LogLevel, message: &str) -> String {
        let class_name = match level {
            LogLevel::Error => "log-error",
            LogLevel::Warning => "log-warning",
            LogLevel::Info => "log-info",
            LogLevel::Debug => "log-debug",
            LogLevel::Silent => "log-silent",
        };
        
        let escaped = message
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&#39;");

        format!("<span class=\"{}\">{}</span>", class_name, escaped)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bracket_format() {
        let raw = "[INFO] [DNS] Resolved example.com";
        let parsed = LogFormatter::parse_log_line(raw);
        assert_eq!(parsed.level, LogLevel::Info);
        assert_eq!(parsed.tag, Some("DNS".to_string()));
        assert_eq!(parsed.message, "Resolved example.com");
    }

    #[test]
    fn test_parse_key_value_format() {
        let raw = "time=\"2026-08-30\" level=error msg=\"Connection failed\"";
        let parsed = LogFormatter::parse_log_line(raw);
        assert_eq!(parsed.level, LogLevel::Error);
        assert_eq!(parsed.timestamp_str, "2026-08-30");
        assert_eq!(parsed.tag, None);
        assert_eq!(parsed.message, "Connection failed");
    }

    #[test]
    fn test_format_ansi() {
        let ansi = LogFormatter::format_ansi(LogLevel::Error, "fatal error");
        assert_eq!(ansi, "\x1b[31mfatal error\x1b[0m");
    }

    #[test]
    fn test_format_html() {
        let html = LogFormatter::format_html(LogLevel::Warning, "use of <uninitialized> & 'var'");
        assert_eq!(html, "<span class=\"log-warning\">use of &lt;uninitialized&gt; &amp; &#39;var&#39;</span>");
    }
}
