use super::*;
use infiltrator_contract::app_routing::AppRoutingMode;
use infiltrator_contract::dns::parse_server_list;
use infiltrator_contract::logs::LogStreamState;
use infiltrator_contract::rule_provider_snapshot::RuleProviderSnapshot;
use infiltrator_contract::rule_snapshot::RuleSnapshot;

impl From<ProfilesProjection> for surface_snapshot::ProfilesPageSnapshot {
    fn from(value: ProfilesProjection) -> Self {
        Self {
            profiles: value
                .profiles
                .into_iter()
                .map(|profile| surface_snapshot::ProfileSnapshot {
                    id: profile.id,
                    name: profile.name,
                    url: profile.url,
                    updated_at: profile.updated_at,
                    upload_bytes: profile.upload_bytes,
                    download_bytes: profile.download_bytes,
                    total_bytes: profile.total_bytes,
                    is_active: profile.is_active,
                    user_agent: profile.user_agent,
                    insecure_skip_verify: profile.insecure_skip_verify,
                    etag: profile.etag,
                    last_modified: profile.last_modified,
                    has_backup: profile.has_backup,
                    cron_expression: profile.cron_expression,
                    auto_update_enabled: profile.auto_update_enabled,
                    update_interval_hours: profile.update_interval_hours,
                    next_update: profile.next_update,
                    auto_reload_core: profile.auto_reload_core,
                    filter: profile.filter,
                    filter_source: profile.filter_source,
                    write_protection: profile.write_protection,
                })
                .collect(),
            auto_update_interval_hours: value.auto_update_interval_hours,
            updating: value.updating,
            snapshot_history: value.snapshot_history,
            apply_transaction: value.apply_transaction,
            aggregation: value.aggregation,
            aggregation_templates: value.aggregation_templates,
            aggregation_templates_available: value.aggregation_templates_available,
            named_overrides: Default::default(),
        }
    }
}

impl From<RulesProjection> for surface_snapshot::RulesPageSnapshot {
    fn from(value: RulesProjection) -> Self {
        Self {
            document: None,
            total_rules: value.total_rules,
            default_action: value.default_action,
            providers: value
                .providers
                .into_iter()
                .map(|provider| RuleProviderSnapshot {
                    name: provider.name,
                    provider_type: provider.provider_type,
                    rule_count: provider.rule_count,
                    behavior: provider.behavior,
                    vehicle_type: provider.vehicle_type,
                    updated_at: provider.updated_at,
                    controller: None,
                    source_url: provider.source_url,
                    refresh_interval_secs: provider.refresh_interval_secs,
                    cache_fingerprint: provider.cache_fingerprint,
                })
                .collect(),
            rules: value
                .rules
                .into_iter()
                .map(|rule| RuleSnapshot {
                    raw: rule.raw,
                    source_ip: rule.source_ip,
                    no_resolve: rule.no_resolve,
                    failure: rule.failure,
                    id: rule.id,
                    rule_type: rule.rule_type,
                    payload: rule.payload,
                    proxy: rule.proxy,
                    hit_count: rule.hit_count,
                    ..Default::default()
                })
                .collect(),
            tracer: value.tracer,
            mrs_acceleration: value.mrs_acceleration,
            hit_audit: value.hit_audit,
            rule_publish_limit: value.rule_publish_limit,
            provider_cache: value.provider_cache,
            etag_support: value.etag_support,
            json_documents: value.json_documents,
        }
    }
}

impl From<ConnectionsProjection> for surface_snapshot::ConnectionsPageSnapshot {
    fn from(value: ConnectionsProjection) -> Self {
        Self {
            total_connections: value.total_connections,
            total_upload_bytes: value.total_upload_bytes,
            total_download_bytes: value.total_download_bytes,
            connections: value.connections,
        }
    }
}

impl From<LogsProjection> for surface_snapshot::LogsPageSnapshot {
    fn from(value: LogsProjection) -> Self {
        Self {
            stream: LogStreamState::Live,
            total_entries: value.total_entries,
            active_level: value
                .active_level
                .map(|level| level.label().to_ascii_lowercase()),
            entries: value
                .entries
                .into_iter()
                .map(|entry| surface_snapshot::LogSnapshot {
                    id: entry.id,
                    raw: None,
                    timestamp: entry.timestamp,
                    level: entry.level.label().to_ascii_lowercase(),
                    tag: entry.tag,
                    message: entry.message,
                })
                .collect(),
        }
    }
}

impl From<DnsProjection> for surface_snapshot::DnsPageSnapshot {
    fn from(value: DnsProjection) -> Self {
        Self {
            enhanced_mode: value.mode,
            cache_entries: value.cache_entries,
            fake_ip_range: value.fake_ip_range,
            switches: value.switches,
            filter_mode: value.filter_mode,
            servers: value
                .servers
                .into_iter()
                .map(|server| surface_snapshot::DnsServerSnapshot {
                    address: server.address,
                    protocol: server.protocol,
                    latency_ms: server.latency_ms,
                    is_fallback: server.is_fallback,
                    tags: server.tags,
                })
                .collect(),
            default_nameserver: parse_server_list(&value.form.bootstrap_nameserver),
            fallback_policy: value.form.fallback_policy.policy(),
            fake_ip_filter: parse_server_list(&value.form.fake_ip_filter),
            proxy_server_nameserver: parse_server_list(&value.form.proxy_server_nameserver),
            direct_nameserver: parse_server_list(&value.form.direct_nameserver),
            fake_ip_pool: value.fake_ip_pool,
            latency: value.latency,
            stun: value.stun,
            self_heal: value.self_heal,
        }
    }
}

impl From<DoctorProjection> for surface_snapshot::DoctorPageSnapshot {
    fn from(value: DoctorProjection) -> Self {
        Self {
            report_started_at: None,
            report_finished_at: value.report_finished_at,
            overall_healthy: value.overall_healthy,
            last_run: value.last_run,
            checks: value
                .checks
                .into_iter()
                .map(|check| surface_snapshot::DoctorCheckSnapshot {
                    kind: check.kind,
                    detail_copy_key: check.detail_copy_key.clone(),
                    id: check.id,
                    name: check.name,
                    category: check.category,
                    state: check.state,
                    detail: check.detail,
                    fix_available: check.fix_available,
                    hint: check.hint.clone(),
                })
                .collect(),
        }
    }
}

impl From<AppRoutingProjection> for surface_snapshot::AppRoutingPageSnapshot {
    fn from(value: AppRoutingProjection) -> Self {
        Self {
            // The demo owner publishes a routing mode; an unobserved one
            // would mean the demo has no owner at all.
            mode: value
                .mode
                .map(Into::into)
                .unwrap_or(AppRoutingMode::ProxyAll),
            include_system: value.include_system,
            uwp_loopback: value.uwp_loopback,
            apps: value
                .apps
                .into_iter()
                .map(|app| surface_snapshot::AppSnapshot {
                    id: app.id,
                    name: app.name,
                    process_name: app.process_name,
                    rule: app.rule.into(),
                    is_system: app.is_system,
                })
                .collect(),
        }
    }
}

impl From<SyncProjection> for SyncPageSnapshot {
    fn from(value: SyncProjection) -> Self {
        value.owner_page()
    }
}
