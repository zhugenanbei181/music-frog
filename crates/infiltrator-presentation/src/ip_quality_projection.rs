//! IP quality, fraud risk, and usage classification badge projections (FLPARITY-001).

use infiltrator_contract::ip_quality::{IpQualityRecord, IpQualitySnapshot, IpRiskLevel};

/// User-facing projection for IP quality and fraud assessment badges.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IpQualityBadgeProjection {
    /// User-facing usage type label (e.g. "住宅宽带", "数据中心", "移动蜂窝").
    pub usage_type_label: &'static str,
    /// Semantic badge color role for usage type ("success", "accent", "info", "warning", "dim").
    pub usage_badge_role: &'static str,
    /// User-facing risk level label (e.g. "安全洁净", "低风险", "中度风险", "高度风险").
    pub risk_level_label: &'static str,
    /// Semantic badge color role for risk level ("success", "accent", "warning", "danger").
    pub risk_badge_role: &'static str,
    /// Formatted fraud score text (e.g. "欺诈分: 5" or "欺诈分: --").
    pub fraud_score_text: String,
    /// Formatted ASN and ISP description (e.g. "AS13335 (Cloudflare)").
    pub isp_asn_text: String,
    /// One-line security summary (e.g. "纯净住宅 · 无滥用记录").
    pub security_summary_text: String,
    /// Optional cache age note (e.g. "已缓存 1小时前").
    pub cache_note: Option<String>,
    /// Detailed tooltip text explaining the assessment facts.
    pub tooltip: String,
    /// Whether this IP is residential broadband.
    pub is_residential: bool,
    /// Whether this IP is clean and free of detected proxy/VPN/abuser flags.
    pub is_clean: bool,
}

impl IpQualityBadgeProjection {
    pub fn from_record(
        record: &IpQualityRecord,
        is_cached: bool,
        cache_age_seconds: Option<u64>,
    ) -> Self {
        let usage_type_label = record.usage_type.label_zh();
        let usage_badge_role = record.usage_type.badge_color_role();
        let risk_level_label = record.risk_level.label_zh();
        let risk_badge_role = record.risk_level.badge_color_role();

        let fraud_score_text = match record.fraud_score {
            Some(score) => format!("欺诈分: {score}"),
            None => "欺诈分: --".to_owned(),
        };

        let isp_asn_text = match (record.asn, record.as_organization.as_deref()) {
            (Some(asn), Some(org)) => format!("AS{asn} ({org})"),
            (Some(asn), None) => format!("AS{asn}"),
            (None, Some(org)) => org.to_owned(),
            (None, None) => "未知网络组织".to_owned(),
        };

        let is_clean = !record.is_proxy
            && !record.is_vpn
            && !record.is_tor
            && !record.is_abuser
            && record.risk_level == IpRiskLevel::Clean;

        let security_summary_text = if is_clean && record.usage_type.is_residential() {
            "原生住宅 · 洁净无滥用".to_owned()
        } else if record.is_abuser {
            "高危网络 · 存在滥用记录".to_owned()
        } else if record.is_tor {
            "Tor 节点 · 高度受限".to_owned()
        } else if record.is_vpn || record.is_proxy {
            "数据中心/代理 · 商业出口".to_owned()
        } else {
            format!(
                "{} · {}",
                record.usage_type.label_zh(),
                record.risk_level.label_zh()
            )
        };

        let cache_note = if is_cached {
            cache_age_seconds.map(|secs| {
                if secs < 60 {
                    "刚刚更新".to_owned()
                } else if secs < 3600 {
                    format!("缓存 {}分钟前", secs / 60)
                } else {
                    format!("缓存 {}小时前", secs / 3600)
                }
            })
        } else {
            None
        };

        let mut tooltip_parts = vec![
            format!("IP: {}", record.ip),
            format!("类型: {}", record.usage_type.label_zh()),
            format!("风险: {}", record.risk_level.label_zh()),
        ];
        if let Some(score) = record.fraud_score {
            tooltip_parts.push(format!("欺诈分: {score}/100"));
        }
        if record.is_proxy {
            tooltip_parts.push("标记: Proxy".to_owned());
        }
        if record.is_vpn {
            tooltip_parts.push("标记: VPN".to_owned());
        }
        if record.is_tor {
            tooltip_parts.push("标记: Tor".to_owned());
        }
        if record.is_abuser {
            tooltip_parts.push("警告: 存在滥用历史".to_owned());
        }
        if let Some(note) = &cache_note {
            tooltip_parts.push(note.clone());
        }

        Self {
            usage_type_label,
            usage_badge_role,
            risk_level_label,
            risk_badge_role,
            fraud_score_text,
            isp_asn_text,
            security_summary_text,
            cache_note,
            tooltip: tooltip_parts.join(" · "),
            is_residential: record.usage_type.is_residential(),
            is_clean,
        }
    }
}

/// Project the IP quality badge from a surface snapshot.
pub fn project_ip_quality_badge(snapshot: &IpQualitySnapshot) -> Option<IpQualityBadgeProjection> {
    let record = snapshot.record.as_ref()?;
    Some(IpQualityBadgeProjection::from_record(
        record,
        snapshot.is_cached,
        snapshot.cache_age_seconds,
    ))
}

/// Formats a concise one-line summary of IP quality facts.
pub fn summarize_ip_quality(record: &IpQualityRecord) -> String {
    let type_label = record.usage_type.label_zh();
    let risk_label = record.risk_level.label_zh();
    let score_str = record
        .fraud_score
        .map(|s| format!(" ({s}分)"))
        .unwrap_or_default();
    format!("{type_label} · {risk_label}{score_str}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::ip_quality::{IpQualityStatus, IpRiskLevel, IpUsageType};

    #[test]
    fn clean_residential_projection() {
        let mut record = IpQualityRecord::new(
            "203.0.113.7",
            IpUsageType::Residential,
            IpRiskLevel::Clean,
            1741300000000,
        );
        record.fraud_score = Some(0);
        record.asn = Some(13335);
        record.as_organization = Some("Cloudflare".to_owned());

        let proj = IpQualityBadgeProjection::from_record(&record, true, Some(7200));
        assert_eq!(proj.usage_type_label, "住宅宽带");
        assert_eq!(proj.usage_badge_role, "success");
        assert_eq!(proj.risk_level_label, "安全洁净");
        assert_eq!(proj.risk_badge_role, "success");
        assert_eq!(proj.fraud_score_text, "欺诈分: 0");
        assert_eq!(proj.isp_asn_text, "AS13335 (Cloudflare)");
        assert_eq!(proj.security_summary_text, "原生住宅 · 洁净无滥用");
        assert_eq!(proj.cache_note.as_deref(), Some("缓存 2小时前"));
        assert!(proj.is_residential);
        assert!(proj.is_clean);
    }

    #[test]
    fn abuser_hosting_projection() {
        let mut record = IpQualityRecord::new(
            "198.51.100.5",
            IpUsageType::Hosting,
            IpRiskLevel::High,
            1741300000000,
        );
        record.fraud_score = Some(92);
        record.is_proxy = true;
        record.is_abuser = true;

        let proj = IpQualityBadgeProjection::from_record(&record, false, None);
        assert_eq!(proj.usage_type_label, "数据中心");
        assert_eq!(proj.usage_badge_role, "warning");
        assert_eq!(proj.risk_level_label, "高度风险");
        assert_eq!(proj.risk_badge_role, "danger");
        assert_eq!(proj.security_summary_text, "高危网络 · 存在滥用记录");
        assert!(!proj.is_residential);
        assert!(!proj.is_clean);
        assert!(proj.cache_note.is_none());
    }

    #[test]
    fn snapshot_projection_returns_none_when_unready() {
        let snap = IpQualitySnapshot {
            generation: 1,
            revision: 1,
            status: IpQualityStatus::Probing,
            record: None,
            failure: None,
            is_cached: false,
            cache_age_seconds: None,
        };
        assert!(project_ip_quality_badge(&snap).is_none());
    }

    #[test]
    fn summary_formatting() {
        let mut record = IpQualityRecord::new(
            "1.1.1.1",
            IpUsageType::Residential,
            IpRiskLevel::Low,
            1741300000000,
        );
        record.fraud_score = Some(20);
        assert_eq!(summarize_ip_quality(&record), "住宅宽带 · 低风险 (20分)");
    }
}
