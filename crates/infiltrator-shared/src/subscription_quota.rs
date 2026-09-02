use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct QuotaStatus {
    pub upload_bytes: u64,
    pub download_bytes: u64,
    pub total_bytes: u64,
    pub expire_timestamp_secs: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum QuotaWarningLevel {
    Normal,
    ExpiringSoon { days_left: u64 },
    LowData { percent_left: f64 },
    Exhausted,
    Expired,
}

pub struct SubscriptionQuota;

impl SubscriptionQuota {
    pub fn parse_userinfo_header(header_value: &str) -> Option<QuotaStatus> {
        let mut upload = None;
        let mut download = None;
        let mut total = None;
        let mut expire = None;

        for part in header_value.split(';') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            if let Some((key, val)) = part.split_once('=') {
                let key = key.trim();
                let val = val.trim();
                match key {
                    "upload" => upload = val.parse::<u64>().ok(),
                    "download" => download = val.parse::<u64>().ok(),
                    "total" => total = val.parse::<u64>().ok(),
                    "expire" => expire = val.parse::<u64>().ok(),
                    _ => {}
                }
            }
        }

        Some(QuotaStatus {
            upload_bytes: upload?,
            download_bytes: download?,
            total_bytes: total?,
            expire_timestamp_secs: expire,
        })
    }

    pub fn calculate_used_bytes(status: &QuotaStatus) -> u64 {
        status.upload_bytes.saturating_add(status.download_bytes)
    }

    pub fn calculate_remaining_bytes(status: &QuotaStatus) -> u64 {
        status
            .total_bytes
            .saturating_sub(Self::calculate_used_bytes(status))
    }

    pub fn calculate_remaining_percent(status: &QuotaStatus) -> f64 {
        if status.total_bytes == 0 {
            return 0.0;
        }
        let remaining = Self::calculate_remaining_bytes(status);
        (remaining as f64 / status.total_bytes as f64) * 100.0
    }

    pub fn evaluate_warning_level(status: &QuotaStatus, now_secs: u64) -> QuotaWarningLevel {
        if let Some(exp) = status.expire_timestamp_secs
            && now_secs >= exp
        {
            return QuotaWarningLevel::Expired;
        }

        let remaining = Self::calculate_remaining_bytes(status);
        if remaining == 0 {
            return QuotaWarningLevel::Exhausted;
        }

        if let Some(exp) = status.expire_timestamp_secs {
            let seconds_left = exp.saturating_sub(now_secs);
            let days_left = seconds_left / 86400;
            if days_left < 3 {
                return QuotaWarningLevel::ExpiringSoon { days_left };
            }
        }

        let percent = Self::calculate_remaining_percent(status);
        if percent < 10.0 {
            return QuotaWarningLevel::LowData {
                percent_left: percent,
            };
        }

        QuotaWarningLevel::Normal
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_userinfo_header() {
        let header = "upload=100; download=200; total=1000; expire=1700000000";
        let status = SubscriptionQuota::parse_userinfo_header(header).unwrap();
        assert_eq!(
            status,
            QuotaStatus {
                upload_bytes: 100,
                download_bytes: 200,
                total_bytes: 1000,
                expire_timestamp_secs: Some(1700000000),
            }
        );

        let header2 = "total=5000 ;  upload=10 ; download=20 ";
        let status2 = SubscriptionQuota::parse_userinfo_header(header2).unwrap();
        assert_eq!(
            status2,
            QuotaStatus {
                upload_bytes: 10,
                download_bytes: 20,
                total_bytes: 5000,
                expire_timestamp_secs: None,
            }
        );

        let header_invalid = "upload=abc; download=200; total=1000";
        assert_eq!(
            SubscriptionQuota::parse_userinfo_header(header_invalid),
            None
        );
    }

    #[test]
    fn test_calculations() {
        let status = QuotaStatus {
            upload_bytes: 100,
            download_bytes: 300,
            total_bytes: 1000,
            expire_timestamp_secs: None,
        };
        assert_eq!(SubscriptionQuota::calculate_used_bytes(&status), 400);
        assert_eq!(SubscriptionQuota::calculate_remaining_bytes(&status), 600);
        assert_eq!(
            SubscriptionQuota::calculate_remaining_percent(&status),
            60.0
        );
    }

    #[test]
    fn test_zero_total() {
        let status = QuotaStatus {
            upload_bytes: 100,
            download_bytes: 300,
            total_bytes: 0,
            expire_timestamp_secs: None,
        };
        assert_eq!(SubscriptionQuota::calculate_remaining_bytes(&status), 0);
        assert_eq!(SubscriptionQuota::calculate_remaining_percent(&status), 0.0);
    }

    #[test]
    fn test_warning_level() {
        // Expired
        let status = QuotaStatus {
            upload_bytes: 100,
            download_bytes: 200,
            total_bytes: 1000,
            expire_timestamp_secs: Some(1000),
        };
        assert_eq!(
            SubscriptionQuota::evaluate_warning_level(&status, 1000),
            QuotaWarningLevel::Expired
        );
        assert_eq!(
            SubscriptionQuota::evaluate_warning_level(&status, 1500),
            QuotaWarningLevel::Expired
        );

        // Exhausted
        let status2 = QuotaStatus {
            upload_bytes: 500,
            download_bytes: 500,
            total_bytes: 1000,
            expire_timestamp_secs: Some(2000),
        };
        assert_eq!(
            SubscriptionQuota::evaluate_warning_level(&status2, 1000),
            QuotaWarningLevel::Exhausted
        );

        // Expiring soon
        let status3 = QuotaStatus {
            upload_bytes: 100,
            download_bytes: 200,
            total_bytes: 1000,
            expire_timestamp_secs: Some(1000 + 2 * 86400 + 10), // ~2 days left
        };
        assert_eq!(
            SubscriptionQuota::evaluate_warning_level(&status3, 1000),
            QuotaWarningLevel::ExpiringSoon { days_left: 2 }
        );

        // Low data
        let status4 = QuotaStatus {
            upload_bytes: 400,
            download_bytes: 550,
            total_bytes: 1000, // remaining 50 = 5%
            expire_timestamp_secs: Some(1000 + 10 * 86400),
        };
        assert_eq!(
            SubscriptionQuota::evaluate_warning_level(&status4, 1000),
            QuotaWarningLevel::LowData { percent_left: 5.0 }
        );

        // Normal
        let status5 = QuotaStatus {
            upload_bytes: 100,
            download_bytes: 200,
            total_bytes: 1000, // remaining 700 = 70%
            expire_timestamp_secs: Some(1000 + 10 * 86400),
        };
        assert_eq!(
            SubscriptionQuota::evaluate_warning_level(&status5, 1000),
            QuotaWarningLevel::Normal
        );
    }
}
