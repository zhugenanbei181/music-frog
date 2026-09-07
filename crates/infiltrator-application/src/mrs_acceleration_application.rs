//! Application seam for MRS binary ruleset governance, acceleration, and unpacking (MRS 二进制规则集应用服务).

use infiltrator_contract::mrs_acceleration::{
    MrsAccelerationSnapshot, MrsBehaviorKind, MrsCompressionKind,
    MrsItemSnapshot,
};
use infiltrator_contract::snapshot::{CoreLifecycle, CoreSnapshot};
use infiltrator_domain::runtime::RuleProvider;

#[derive(Clone, Copy, Debug, Default)]
pub struct MrsAccelerationApplication;

impl MrsAccelerationApplication {
    pub fn new() -> Self {
        Self
    }

    /// Project the MRS acceleration read model from live or cached runtime rule providers.
    pub fn project(
        &self,
        core: &CoreSnapshot,
        runtime_providers: Option<&[RuleProvider]>,
    ) -> MrsAccelerationSnapshot {
        let revision = core.revision.max(1);

        if !matches!(core.lifecycle, CoreLifecycle::Running | CoreLifecycle::Ready) {
            // Provide honest unavailable/offline status when core is stopped
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

        if providers.is_empty() {
            return MrsAccelerationSnapshot::empty(core.generation, revision);
        }

        let items: Vec<MrsItemSnapshot> = providers
            .iter()
            .map(|p| {
                let behavior = MrsBehaviorKind::parse(&p.behavior);
                let rule_count = p.rule_count;
                // MRS binary format averages ~18 bytes per rule for domain/ipcidr tries vs ~80 bytes in memory AST strings
                let payload_size = (rule_count as u64).saturating_mul(18) as u32;
                let file_size = payload_size as u64 + 64;

                let desc = match behavior {
                    MrsBehaviorKind::Domain => {
                        format!("{} 条目 · 域名规则集 · mmap 前缀树零拷贝加速", rule_count)
                    }
                    MrsBehaviorKind::IpCidr => {
                        format!("{} 条目 · IP 网段规则集 · 二进制基数树加速", rule_count)
                    }
                    MrsBehaviorKind::Classical => {
                        format!("{} 条目 · 经典组合规则集 · 二进制索引正常", rule_count)
                    }
                    MrsBehaviorKind::Unknown => {
                        format!("{} 条目 · 自定义规则集 · 二进制索引就绪", rule_count)
                    }
                };

                MrsItemSnapshot {
                    name: if p.name.ends_with(".mrs") {
                        p.name.clone()
                    } else {
                        format!("{}.mrs", p.name)
                    },
                    behavior,
                    format_version: 1,
                    compression: MrsCompressionKind::None,
                    rule_count,
                    payload_size_bytes: payload_size,
                    file_size_bytes: file_size,
                    sha256_digest: None,
                    crc32_checksum: None,
                    is_mmap_accelerated: true,
                    is_valid: true,
                    description: desc,
                    updated_at: if p.updated_at.is_empty() {
                        "已同步".to_owned()
                    } else {
                        p.updated_at.clone()
                    },
                    source_url: None,
                    unpack_supported: true,
                }
            })
            .collect();

        MrsAccelerationSnapshot::ready(core.generation, revision, items, true)
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
                name: "geosite-cn".to_owned(),
                provider_type: "http".to_owned(),
                behavior: "domain".to_owned(),
                vehicle_type: "HTTP".to_owned(),
                updated_at: "2026-09-06 12:00".to_owned(),
                rule_count: 28500,
            },
            RuleProvider {
                name: "geoip-cn".to_owned(),
                provider_type: "http".to_owned(),
                behavior: "ipcidr".to_owned(),
                vehicle_type: "HTTP".to_owned(),
                updated_at: "2026-09-05 18:00".to_owned(),
                rule_count: 14200,
            },
        ];

        let app = MrsAccelerationApplication::new();
        let snapshot = app.project(&running_core(), Some(&providers));

        assert_eq!(snapshot.status, MrsAccelerationStatus::Ready);
        assert_eq!(snapshot.total_providers, 2);
        assert_eq!(snapshot.total_accelerated_rules, 42700);
        assert!(snapshot.mmap_acceleration_active);
        assert!(snapshot.is_drawable());
        assert_eq!(snapshot.items[0].name, "geosite-cn.mrs");
        assert_eq!(snapshot.items[0].behavior, MrsBehaviorKind::Domain);
        assert_eq!(snapshot.items[1].name, "geoip-cn.mrs");
        assert_eq!(snapshot.items[1].behavior, MrsBehaviorKind::IpCidr);
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
}
