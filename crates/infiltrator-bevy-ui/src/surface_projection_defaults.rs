use crate::pages::app_routing::AppRoutingProjection;
use crate::pages::connections::ConnectionsProjection;
use crate::pages::dns::DnsProjection;
use crate::pages::doctor::DoctorProjection;
use crate::pages::logs::LogsProjection;
use crate::pages::profiles::ProfilesProjection;
use crate::pages::rules::RulesProjection;
use crate::pages::sync::SyncProjection;
use infiltrator_contract::connection::ConnectionStreamPhase;
use infiltrator_contract::dns::{
    DnsCoreSwitches, DnsEnhancedMode, DnsFakeIpFilterMode, FakeIpMappingPool,
};
use infiltrator_contract::dns_cache::DnsCacheFlushReport;
use infiltrator_contract::dns_form::DnsWorkbenchForm;
use infiltrator_contract::dns_latency::DnsLatencyReport;
use infiltrator_contract::dns_leak::DnsLeakReport;
use infiltrator_contract::dns_self_heal::DnsSelfHealSnapshot;
use infiltrator_contract::stun_probe::StunProbeReport;
use infiltrator_contract::surface_snapshot;
use infiltrator_contract::sync::SyncStatus;
use infiltrator_domain::app_routing::AppRoutingMode;
use infiltrator_domain::rules::view::RULE_PUBLISH_LIMIT;

pub(super) fn empty_profiles() -> ProfilesProjection {
    ProfilesProjection {
        profiles: Vec::new(),
        auto_update_interval_hours: 0,
        updating: false,
        aggregation: None,
        aggregation_templates: Vec::new(),
        aggregation_templates_available: true,
        yaml_ast_diff: None,
        snapshot_history: None,
        apply_transaction: None,
        editor_read: Default::default(),
        profile_document: None,
        profile_options: None,
        script_sandbox: None,
        script_export: None,
    }
}

pub(super) fn empty_rules() -> RulesProjection {
    RulesProjection {
        total_rules: 0,
        default_action: "—".to_owned(),
        providers: Vec::new(),
        rules: Vec::new(),
        tracer: Default::default(),
        hit_audit: Default::default(),
        mrs_acceleration: Default::default(),
        truncated_rule_count: None,
        rule_publish_limit: RULE_PUBLISH_LIMIT,
        provider_cache: Default::default(),
        etag_support: Default::default(),
        json_documents: Vec::new(),
    }
}

pub(super) fn empty_app_routing() -> AppRoutingProjection {
    AppRoutingProjection {
        mode: AppRoutingMode::ProxyAll,
        include_system: false,
        apps: Vec::new(),
        uwp_loopback: Default::default(),
    }
}

pub(super) fn empty_connections() -> ConnectionsProjection {
    ConnectionsProjection {
        total_connections: 0,
        total_upload_bytes: 0,
        total_download_bytes: 0,
        stream_phase: ConnectionStreamPhase::Idle,
        connections: Vec::new(),
    }
}

pub(super) fn empty_logs() -> LogsProjection {
    LogsProjection {
        status: surface_snapshot::PageStatus::Loading,
        generation: 0,
        session_token: None,
        total_entries: 0,
        active_level: None,
        entries: Vec::new(),
    }
}

pub(super) fn empty_dns() -> DnsProjection {
    DnsProjection {
        mode: DnsEnhancedMode::Unmapped,
        cache_entries: 0,
        fake_ip_range: "—".to_owned(),
        servers: Vec::new(),
        switches: DnsCoreSwitches::default(),
        filter_mode: DnsFakeIpFilterMode::default(),
        form: DnsWorkbenchForm::default(),
        cache_flush: DnsCacheFlushReport::default(),
        fake_ip_pool: FakeIpMappingPool::default(),
        latency: DnsLatencyReport::default(),
        leak: DnsLeakReport::default(),
        stun: StunProbeReport::default(),
        self_heal: DnsSelfHealSnapshot::default(),
        hosts: Vec::new(),
    }
}

pub(super) fn empty_doctor() -> DoctorProjection {
    DoctorProjection {
        report_finished_at: None,
        overall_healthy: false,
        last_run: "—".to_owned(),
        checks: Vec::new(),
        watchdog: Default::default(),
    }
}

pub(super) fn empty_sync() -> SyncProjection {
    SyncProjection {
        status: SyncStatus::Unknown,
        history_status: surface_snapshot::PageStatus::Loading,
        server_url: String::new(),
        username: String::new(),
        last_sync: None,
        auto_sync: false,
        conflict: None,
        snapshots: Vec::new(),
    }
}
