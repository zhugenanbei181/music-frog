//! Pure domain scheduling policy, retry backoff calculation, format detection,
//! and quota warning evaluation for subscription lifecycles.

use chrono::{DateTime, Datelike, Timelike, Utc};
use infiltrator_contract::subscription_import::SubscriptionFormat;
use std::time::Duration;
use thiserror::Error;

use crate::subscription::SubscriptionUserInfo;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CronParseError {
    #[error("Cron 表达式不能为空")]
    Empty,
    #[error("Cron 表达式字段数量错误: 期望 5 个字段 (分 时 日 月 周), 实际 {0} 个")]
    InvalidFieldCount(usize),
    #[error("字段 `{field}` 值无效: `{value}` ({reason})")]
    InvalidFieldValue {
        field: &'static str,
        value: String,
        reason: String,
    },
    #[error("未知宏表达式: `{0}`")]
    UnknownMacro(String),
}

/// 5-field UTC Cron expression schedule parser and next-run evaluator.
/// Fields: minute (0-59), hour (0-23), day of month (1-31), month (1-12), day of week (0-6, 0=Sun).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CronSchedule {
    pub raw: String,
    minute_mask: u64,
    hour_mask: u32,
    day_mask: u32,
    month_mask: u16,
    dow_mask: u8,
}

impl CronSchedule {
    /// Parse a 5-part cron expression (or standard macros like `@daily`, `@hourly`, `@every 6h`).
    pub fn parse(expr: &str) -> Result<Self, CronParseError> {
        let trimmed = expr.trim();
        if trimmed.is_empty() {
            return Err(CronParseError::Empty);
        }

        // Support predefined macros
        if let Some(macro_str) = trimmed.strip_prefix('@') {
            return match macro_str.to_ascii_lowercase().as_str() {
                "hourly" => Self::parse("0 * * * *"),
                "daily" | "midnight" => Self::parse("0 0 * * *"),
                "weekly" => Self::parse("0 0 * * 0"),
                "monthly" => Self::parse("0 0 1 * *"),
                "every 6h" => Self::parse("0 */6 * * *"),
                "every 12h" => Self::parse("0 */12 * * *"),
                "every 24h" => Self::parse("0 0 * * *"),
                other => Err(CronParseError::UnknownMacro(format!("@{other}"))),
            };
        }

        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() != 5 {
            return Err(CronParseError::InvalidFieldCount(parts.len()));
        }

        let minute_mask = parse_field(parts[0], 0, 59, "minute")?;
        let hour_mask = parse_field(parts[1], 0, 23, "hour")? as u32;
        let day_mask = parse_field(parts[2], 1, 31, "day_of_month")? as u32;
        let month_mask = parse_field(parts[3], 1, 12, "month")? as u16;
        let dow_mask = parse_dow_field(parts[4])?;

        Ok(Self {
            raw: trimmed.to_string(),
            minute_mask,
            hour_mask,
            day_mask,
            month_mask,
            dow_mask,
        })
    }

    /// Calculate the next scheduled occurrence strictly after `after`.
    pub fn next_occurrence(&self, after: DateTime<Utc>) -> Option<DateTime<Utc>> {
        // Start search at the next minute boundary (seconds & nanoseconds zeroed).
        let mut cur = after + chrono::Duration::minutes(1);
        cur = cur.with_second(0)?.with_nanosecond(0)?;

        let max_horizon = after + chrono::Duration::days(365 * 5); // 5-year safety cap

        while cur <= max_horizon {
            let month = cur.month();
            if (self.month_mask & (1 << month)) == 0 {
                // Advance to 1st of next month at 00:00
                let (y, m) = if month == 12 {
                    (cur.year() + 1, 1)
                } else {
                    (cur.year(), month + 1)
                };
                cur = chrono::NaiveDate::from_ymd_opt(y, m, 1)?
                    .and_hms_opt(0, 0, 0)?
                    .and_utc();
                continue;
            }

            let day = cur.day();
            let dow = cur.weekday().num_days_from_sunday() as u8; // 0=Sun..6=Sat
            let day_matches = (self.day_mask & (1 << day)) != 0;
            let dow_matches = (self.dow_mask & (1 << dow)) != 0;

            if !day_matches || !dow_matches {
                // Advance to start of next day at 00:00
                cur = (cur.date_naive() + chrono::Duration::days(1))
                    .and_hms_opt(0, 0, 0)?
                    .and_utc();
                continue;
            }

            let hour = cur.hour();
            if (self.hour_mask & (1 << hour)) == 0 {
                // Advance to start of next hour at 00 minutes
                cur = (cur + chrono::Duration::hours(1))
                    .with_minute(0)?
                    .with_second(0)?;
                continue;
            }

            let minute = cur.minute();
            if (self.minute_mask & (1u64 << minute)) == 0 {
                cur += chrono::Duration::minutes(1);
                continue;
            }

            return Some(cur);
        }

        None
    }
}

fn parse_field(
    raw: &str,
    min: u32,
    max: u32,
    field_name: &'static str,
) -> Result<u64, CronParseError> {
    let mut mask = 0u64;

    for item in raw.split(',') {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }

        if item == "*" {
            for v in min..=max {
                mask |= 1u64 << v;
            }
        } else if let Some(step_str) = item.strip_prefix("*/") {
            let step: u32 = step_str
                .parse()
                .map_err(|e| CronParseError::InvalidFieldValue {
                    field: field_name,
                    value: item.to_string(),
                    reason: format!("步长解析失败: {e}"),
                })?;
            if step == 0 {
                return Err(CronParseError::InvalidFieldValue {
                    field: field_name,
                    value: item.to_string(),
                    reason: "步长不能为0".to_string(),
                });
            }
            let mut v = min;
            while v <= max {
                mask |= 1u64 << v;
                v += step;
            }
        } else if let Some((start_s, end_s)) = item.split_once('-') {
            let start: u32 = start_s
                .parse()
                .map_err(|e| CronParseError::InvalidFieldValue {
                    field: field_name,
                    value: item.to_string(),
                    reason: format!("范围起始解析失败: {e}"),
                })?;
            let end: u32 = end_s
                .parse()
                .map_err(|e| CronParseError::InvalidFieldValue {
                    field: field_name,
                    value: item.to_string(),
                    reason: format!("范围终止解析失败: {e}"),
                })?;
            if start > end || start < min || end > max {
                return Err(CronParseError::InvalidFieldValue {
                    field: field_name,
                    value: item.to_string(),
                    reason: format!("范围 {start}-{end} 超出合法界限 {min}-{max}"),
                });
            }
            for v in start..=end {
                mask |= 1u64 << v;
            }
        } else {
            let val: u32 = item
                .parse()
                .map_err(|e| CronParseError::InvalidFieldValue {
                    field: field_name,
                    value: item.to_string(),
                    reason: format!("整数解析失败: {e}"),
                })?;
            if val < min || val > max {
                return Err(CronParseError::InvalidFieldValue {
                    field: field_name,
                    value: item.to_string(),
                    reason: format!("数值 {val} 超出合法界限 {min}-{max}"),
                });
            }
            mask |= 1u64 << val;
        }
    }

    if mask == 0 {
        return Err(CronParseError::InvalidFieldValue {
            field: field_name,
            value: raw.to_string(),
            reason: "未能匹配任何合法数值".to_string(),
        });
    }

    Ok(mask)
}

fn parse_dow_field(raw: &str) -> Result<u8, CronParseError> {
    let parsed_mask = parse_field(raw, 0, 7, "day_of_week")?;
    let mut dow_mask = 0u8;
    for v in 0..=7 {
        if (parsed_mask & (1u64 << v)) != 0 {
            if v == 7 {
                dow_mask |= 1 << 0; // 7 is also Sunday
            } else {
                dow_mask |= 1 << v;
            }
        }
    }
    Ok(dow_mask)
}

/// Unified scheduling cadence: fixed hours or Cron expression.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubscriptionSchedule {
    IntervalHours(u32),
    Cron(CronSchedule),
    ManualOnly,
}

impl SubscriptionSchedule {
    pub fn from_metadata(
        interval_hours: Option<u32>,
        cron_expr: Option<&str>,
    ) -> Result<Self, CronParseError> {
        if let Some(expr) = cron_expr
            && !expr.trim().is_empty()
        {
            let cron = CronSchedule::parse(expr)?;
            return Ok(Self::Cron(cron));
        }
        if let Some(hours) = interval_hours
            && hours > 0
        {
            return Ok(Self::IntervalHours(hours));
        }
        Ok(Self::ManualOnly)
    }

    pub fn next_run(&self, after: DateTime<Utc>) -> Option<DateTime<Utc>> {
        match self {
            Self::IntervalHours(hours) => Some(after + chrono::Duration::hours(*hours as i64)),
            Self::Cron(cron) => cron.next_occurrence(after),
            Self::ManualOnly => None,
        }
    }
}

/// Step 5: Network retry and exponential backoff policy (30s, 1m, 5m).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetryBackoffPolicy {
    delays: Vec<Duration>,
}

impl Default for RetryBackoffPolicy {
    fn default() -> Self {
        Self {
            delays: vec![
                Duration::from_secs(30),
                Duration::from_secs(60),
                Duration::from_secs(300),
            ],
        }
    }
}

impl RetryBackoffPolicy {
    pub fn new(delays: Vec<Duration>) -> Self {
        Self { delays }
    }

    /// Fast policy with immediate retries for unit tests.
    pub fn test_immediate() -> Self {
        Self {
            delays: vec![Duration::ZERO, Duration::ZERO, Duration::ZERO],
        }
    }

    pub fn max_attempts(&self) -> usize {
        self.delays.len()
    }

    /// Returns the backoff delay for the given retry attempt (1-based index).
    /// Attempt 0 has zero delay. Attempts > max_attempts return None.
    pub fn delay_for_attempt(&self, attempt: usize) -> Option<Duration> {
        if attempt == 0 {
            Some(Duration::ZERO)
        } else if attempt <= self.delays.len() {
            Some(self.delays[attempt - 1])
        } else {
            None
        }
    }
}

/// Step 7: Subscription quota usage and expiration warning evaluator.
pub struct QuotaWarningPolicy;

impl QuotaWarningPolicy {
    /// Threshold: usage > 85% triggers warning badge.
    pub const USAGE_WARNING_THRESHOLD_PERCENT: f64 = 85.0;
    /// Threshold: expiry < 3 days triggers orange warning tag.
    pub const EXPIRING_SOON_DAYS: i64 = 3;

    /// Evaluates `SubscriptionUserInfo` against the standard thresholds.
    /// Returns `(usage_warning, expiry_warning)`.
    pub fn evaluate(info: &SubscriptionUserInfo, now_unix: i64) -> (bool, bool) {
        let usage_warning = info
            .usage_percentage()
            .is_some_and(|pct| pct >= Self::USAGE_WARNING_THRESHOLD_PERCENT);
        let expiry_warning = info
            .remaining_days(now_unix)
            .is_some_and(|days| (0..=Self::EXPIRING_SOON_DAYS).contains(&days));
        (usage_warning, expiry_warning)
    }
}

/// Helper for sniffing the format of imported subscription text.
pub struct FormatDetector;

impl FormatDetector {
    /// Count the `proxies` entries of a Clash-format document. Non-Clash or
    /// malformed payloads honestly report `0` rather than guessing a count.
    pub fn count_nodes(content: &str) -> usize {
        let Ok(value) = serde_yaml_ng::from_str::<serde_yaml_ng::Value>(content) else {
            return 0;
        };
        value
            .get("proxies")
            .and_then(|proxies| proxies.as_sequence())
            .map_or(0, Vec::len)
    }

    pub fn detect(content: &str) -> SubscriptionFormat {
        let trimmed = content.trim();
        if trimmed.is_empty() {
            return SubscriptionFormat::Unknown;
        }

        if (trimmed.contains("proxies:") || trimmed.contains("proxy-groups:"))
            && serde_yaml_ng::from_str::<serde_yaml_ng::Value>(trimmed).is_ok()
        {
            return SubscriptionFormat::ClashYaml;
        }

        if trimmed.starts_with('{') && trimmed.contains("\"outbounds\"") {
            return SubscriptionFormat::SingBoxJson;
        }

        if trimmed.starts_with("ss://") {
            return SubscriptionFormat::ShadowsocksUri;
        }

        if trimmed.starts_with("trojan://") {
            return SubscriptionFormat::TrojanUri;
        }

        if let Ok(decoded) = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            trimmed.as_bytes(),
        ) && let Ok(text) = std::str::from_utf8(&decoded)
            && (text.contains("vmess://")
                || text.contains("vless://")
                || text.contains("ss://")
                || text.contains("trojan://"))
        {
            return SubscriptionFormat::Base64VmessVless;
        }

        SubscriptionFormat::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_cron_macro_hourly() {
        let cron = CronSchedule::parse("@hourly").unwrap();
        let base = Utc.with_ymd_and_hms(2026, 9, 7, 10, 15, 30).unwrap();
        let next = cron.next_occurrence(base).unwrap();
        assert_eq!(next, Utc.with_ymd_and_hms(2026, 9, 7, 11, 0, 0).unwrap());
    }

    #[test]
    fn test_cron_macro_daily() {
        let cron = CronSchedule::parse("@daily").unwrap();
        let base = Utc.with_ymd_and_hms(2026, 9, 7, 10, 15, 30).unwrap();
        let next = cron.next_occurrence(base).unwrap();
        assert_eq!(next, Utc.with_ymd_and_hms(2026, 9, 8, 0, 0, 0).unwrap());
    }

    #[test]
    fn test_cron_step_every_6h() {
        let cron = CronSchedule::parse("0 */6 * * *").unwrap();
        let base = Utc.with_ymd_and_hms(2026, 9, 7, 7, 30, 0).unwrap();
        let next = cron.next_occurrence(base).unwrap();
        assert_eq!(next, Utc.with_ymd_and_hms(2026, 9, 7, 12, 0, 0).unwrap());
    }

    #[test]
    fn test_retry_backoff_policy_values() {
        let policy = RetryBackoffPolicy::default();
        assert_eq!(policy.delay_for_attempt(0), Some(Duration::ZERO));
        assert_eq!(policy.delay_for_attempt(1), Some(Duration::from_secs(30)));
        assert_eq!(policy.delay_for_attempt(2), Some(Duration::from_secs(60)));
        assert_eq!(policy.delay_for_attempt(3), Some(Duration::from_secs(300)));
        assert_eq!(policy.delay_for_attempt(4), None);
    }

    #[test]
    fn test_quota_warning_policy_thresholds() {
        let now = 1_700_000_000i64;
        let info_normal = SubscriptionUserInfo {
            upload: Some(10),
            download: Some(20),
            total: Some(100),
            expire: Some(now + 86400 * 10),
        };
        let (usage_warn, expiry_warn) = QuotaWarningPolicy::evaluate(&info_normal, now);
        assert!(!usage_warn);
        assert!(!expiry_warn);

        let info_warning = SubscriptionUserInfo {
            upload: Some(40),
            download: Some(50), // 90%
            total: Some(100),
            expire: Some(now + 86400 * 2), // 2 days left
        };
        let (usage_warn, expiry_warn) = QuotaWarningPolicy::evaluate(&info_warning, now);
        assert!(usage_warn);
        assert!(expiry_warn);
    }

    #[test]
    fn test_format_detector() {
        let yaml = "proxies:\n  - name: node1\n    type: ss\n    server: 1.1.1.1\n    port: 443\n";
        assert_eq!(FormatDetector::detect(yaml), SubscriptionFormat::ClashYaml);

        let ss = "ss://YWVzLTEyOC1nY206cGFzc3dvcmRAMS4yLjMuNDo4MzM4#Test";
        assert_eq!(
            FormatDetector::detect(ss),
            SubscriptionFormat::ShadowsocksUri
        );
    }

    #[test]
    fn test_format_detector_counts_nodes_honestly() {
        let yaml = "proxies:\n  - name: n1\n    type: ss\n  - name: n2\n    type: vmess\n";
        assert_eq!(FormatDetector::count_nodes(yaml), 2);
        assert_eq!(FormatDetector::count_nodes("ss://not-clash"), 0);
        assert_eq!(FormatDetector::count_nodes(""), 0);
    }
}
