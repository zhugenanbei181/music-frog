//! Application seam for MRS binary ruleset governance, acceleration, and unpacking (MRS 二进制规则集应用服务).

use infiltrator_contract::mrs_acceleration::{
    MrsAccelerationSnapshot, MrsBehaviorKind, MrsCompressionKind, MrsItemSnapshot,
};
use infiltrator_contract::snapshot::{CoreLifecycle, CoreSnapshot};
use infiltrator_domain::mrs::{Behavior, CompressionType, validate_mrs_bytes};
use infiltrator_domain::runtime::RuleProvider;

#[derive(Clone, Copy, Debug, Default)]
pub struct MrsAccelerationApplication;

fn is_mrs_provider(p: &RuleProvider) -> bool {
    p.name.ends_with(".mrs")
        || p.vehicle_type.eq_ignore_ascii_case("mrs")
        || p.provider_type.eq_ignore_ascii_case("mrs")
}

impl MrsAccelerationApplication {
    pub fn new() -> Self {
        Self
    }

    /// Project the MRS acceleration read model from verified MRS items.
    pub fn project_with_items(
        &self,
        core: &CoreSnapshot,
        items: Vec<MrsItemSnapshot>,
    ) -> MrsAccelerationSnapshot {
        let revision = core.revision.max(1);

        if !matches!(
            core.lifecycle,
            CoreLifecycle::Running | CoreLifecycle::Ready
        ) {
            return MrsAccelerationSnapshot::unavailable(
                core.generation,
                revision,
                "内核未运行：MRS 二进制加速状态不可用",
            );
        }

        let mmap_active = items.iter().any(|item| item.is_mmap_accelerated);
        MrsAccelerationSnapshot::ready(core.generation, revision, items, mmap_active)
    }

    /// Parse and validate real MRS bytes from disk, extracting exact binary header metadata.
    pub fn inspect_mrs_bytes(
        &self,
        name: &str,
        bytes: &[u8],
        updated_at: &str,
        source_url: Option<String>,
    ) -> Result<MrsItemSnapshot, String> {
        let report = validate_mrs_bytes(bytes).map_err(|e| format!("MRS parse error: {e}"))?;

        if !report.is_valid {
            return Err(report.errors.join("; "));
        }

        let header = report
            .header
            .ok_or_else(|| "missing MRS header".to_string())?;

        let behavior = match header.behavior {
            Behavior::Domain => MrsBehaviorKind::Domain,
            Behavior::IpCidr => MrsBehaviorKind::IpCidr,
            Behavior::Classical => MrsBehaviorKind::Classical,
            Behavior::Unknown(_) => MrsBehaviorKind::Unknown,
        };

        let compression = match header.compression {
            CompressionType::None => MrsCompressionKind::None,
            CompressionType::Zstd => MrsCompressionKind::Zstd,
            CompressionType::Gzip => MrsCompressionKind::Gzip,
            CompressionType::Unknown(_) => MrsCompressionKind::Unknown,
        };

        let desc = format!(
            "{} 条目 · MRS v{} · 真实二进制校验通过",
            header.rule_count, header.version
        );

        Ok(MrsItemSnapshot {
            name: name.to_owned(),
            behavior,
            format_version: header.version,
            compression,
            rule_count: header.rule_count,
            payload_size_bytes: header.payload_size,
            file_size_bytes: bytes.len() as u64,
            sha256_digest: report.sha256_digest.or(header.sha256),
            crc32_checksum: report.crc32_checksum.or(header.crc32),
            is_mmap_accelerated: true,
            is_valid: true,
            description: desc,
            updated_at: if updated_at.is_empty() {
                "未记录".to_owned()
            } else {
                updated_at.to_owned()
            },
            source_url,
            unpack_supported: true,
        })
    }

    /// Project the MRS acceleration read model from live or cached runtime rule providers.
    /// Non-MRS providers are ignored, and unread MRS files report honest unobserved metrics.
    pub fn project(
        &self,
        core: &CoreSnapshot,
        runtime_providers: Option<&[RuleProvider]>,
    ) -> MrsAccelerationSnapshot {
        let revision = core.revision.max(1);

        if !matches!(
            core.lifecycle,
            CoreLifecycle::Running | CoreLifecycle::Ready
        ) {
            return MrsAccelerationSnapshot::unavailable(
                core.generation,
                revision,
                "内核未运行：MRS 二进制加速状态不可用",
            );
        }

        let Some(providers) = runtime_providers else {
            return MrsAccelerationSnapshot::unsupported(
                core.generation,
                revision,
                "内核未配置规则集提供者网关",
            );
        };

        let mrs_providers: Vec<&RuleProvider> =
            providers.iter().filter(|p| is_mrs_provider(p)).collect();

        if mrs_providers.is_empty() {
            return MrsAccelerationSnapshot::empty(core.generation, revision);
        }

        let items: Vec<MrsItemSnapshot> = mrs_providers
            .into_iter()
            .map(|p| {
                let behavior = MrsBehaviorKind::parse(&p.behavior);
                let rule_count = p.rule_count;
                let desc = format!("{} 条目 · 二进制规则集 · 元数据待校验", rule_count);

                MrsItemSnapshot {
                    name: p.name.clone(),
                    behavior,
                    format_version: 0,
                    compression: MrsCompressionKind::Unknown,
                    rule_count,
                    payload_size_bytes: 0,
                    file_size_bytes: 0,
                    sha256_digest: None,
                    crc32_checksum: None,
                    is_mmap_accelerated: false,
                    is_valid: false,
                    description: desc,
                    updated_at: if p.updated_at.is_empty() {
                        "未记录".to_owned()
                    } else {
                        p.updated_at.clone()
                    },
                    source_url: None,
                    unpack_supported: false,
                }
            })
            .collect();

        MrsAccelerationSnapshot::ready(core.generation, revision, items, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::mrs_acceleration::MrsAccelerationStatus;

    fn running_core() -> CoreSnapshot {
        CoreSnapshot {
            lifecycle: CoreLifecycle::Running,
            generation: 2,
            revision: 3,
            session_token: None,
            proxy_mode: None,
            core_version: None,
            sampled_at_epoch_ms: None,
            failure: None,
            upload_bps: 0.0,
            download_bps: 0.0,
            active_connections: 0,
            memory_bytes: None,
            watchdog: Default::default(),
        }
    }

    #[test]
    fn projects_ready_mrs_acceleration_from_providers() {
        let providers = vec![
            RuleProvider {
                name: "geosite-cn.mrs".to_owned(),
                provider_type: "mrs".to_owned(),
                behavior: "domain".to_owned(),
                vehicle_type: "File".to_owned(),
                updated_at: "2026-09-06 12:00".to_owned(),
                rule_count: 28500,
            },
            RuleProvider {
                name: "geoip-cn.mrs".to_owned(),
                provider_type: "mrs".to_owned(),
                behavior: "ipcidr".to_owned(),
                vehicle_type: "File".to_owned(),
                updated_at: "2026-09-05 18:00".to_owned(),
                rule_count: 14200,
            },
        ];

        let app = MrsAccelerationApplication::new();
        let snapshot = app.project(&running_core(), Some(&providers));

        assert_eq!(snapshot.status, MrsAccelerationStatus::Ready);
        assert_eq!(snapshot.total_providers, 2);
        assert_eq!(snapshot.total_accelerated_rules, 42700);
        assert!(!snapshot.mmap_acceleration_active);
        assert_eq!(snapshot.total_memory_saved_bytes, 0);
        assert!(snapshot.is_drawable());
        assert_eq!(snapshot.items[0].name, "geosite-cn.mrs");
        assert_eq!(snapshot.items[0].behavior, MrsBehaviorKind::Domain);
        assert_eq!(snapshot.items[0].format_version, 0);
        assert_eq!(snapshot.items[0].payload_size_bytes, 0);
        assert!(!snapshot.items[0].is_valid);
        assert_eq!(snapshot.items[1].name, "geoip-cn.mrs");
        assert_eq!(snapshot.items[1].behavior, MrsBehaviorKind::IpCidr);
    }

    #[test]
    fn ignores_non_mrs_providers_and_reports_empty_when_no_mrs() {
        let providers = vec![RuleProvider {
            name: "geosite-cn".to_owned(),
            provider_type: "http".to_owned(),
            behavior: "domain".to_owned(),
            vehicle_type: "HTTP".to_owned(),
            updated_at: "2026-09-06 12:00".to_owned(),
            rule_count: 28500,
        }];

        let app = MrsAccelerationApplication::new();
        let snapshot = app.project(&running_core(), Some(&providers));
        assert_eq!(snapshot.status, MrsAccelerationStatus::Empty);
        assert_eq!(snapshot.total_providers, 0);
    }

    #[test]
    fn stopped_core_reports_unavailable() {
        let mut core = running_core();
        core.lifecycle = CoreLifecycle::Stopped;
        let app = MrsAccelerationApplication::new();
        let snapshot = app.project(&core, None);
        assert_eq!(snapshot.status, MrsAccelerationStatus::Unknown);
        assert!(!snapshot.is_drawable());
    }

    #[test]
    fn empty_providers_reports_empty_status() {
        let app = MrsAccelerationApplication::new();
        let snapshot = app.project(&running_core(), Some(&[]));
        assert_eq!(snapshot.status, MrsAccelerationStatus::Empty);
        assert_eq!(snapshot.total_providers, 0);
    }

    #[test]
    fn inspects_real_binary_mrs_bytes_and_projects_ready_state() {
        use infiltrator_domain::mrs::{Behavior, build_mrs_bytes};

        let payload = b"google.com\nyoutube.com\n";
        let bytes = build_mrs_bytes(Behavior::Domain, 1, 2, "Test Domains", payload, None);

        let app = MrsAccelerationApplication::new();
        let item = app
            .inspect_mrs_bytes(
                "domains.mrs",
                &bytes,
                "2026-10-08 12:00",
                Some("https://example.com/domains.mrs".into()),
            )
            .expect("valid real MRS bytes");

        assert_eq!(item.name, "domains.mrs");
        assert_eq!(item.behavior, MrsBehaviorKind::Domain);
        assert_eq!(item.format_version, 1);
        assert_eq!(item.rule_count, 2);
        assert_eq!(item.payload_size_bytes, payload.len() as u32);
        assert!(item.is_valid);
        assert!(item.is_mmap_accelerated);
        assert!(item.unpack_supported);

        let snapshot = app.project_with_items(&running_core(), vec![item]);
        assert_eq!(snapshot.status, MrsAccelerationStatus::Ready);
        assert_eq!(snapshot.total_providers, 1);
        assert_eq!(snapshot.total_accelerated_rules, 2);
        assert!(snapshot.mmap_acceleration_active);
    }
}
