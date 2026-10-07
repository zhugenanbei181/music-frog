//! Desktop composition for the complete shared UI surface.
//!
//! This module is intentionally outside both UI crates. It assembles concrete
//! desktop storage/controller adapters into the runtime-neutral
//! `ApplicationSurfaceReader` and `SurfacePump` consumed by Iced or Bevy.

use crate::mtu::DesktopMtuProbe;
use crate::offline_startup::offline_startup_port;
use crate::pac_service::DesktopPacServicePort;
use crate::rule_provider_cache::DesktopRuleProviderCache;
use crate::service_mode::DesktopServiceMode;
use crate::storage::{app_routing_store, endpoint_source, port_conflict, settings_store, version};
use crate::system_proxy::DesktopSystemProxy;
use crate::uwp_loopback_port::DesktopUwpLoopbackPort;
use infiltrator_application::configuration_application::ConfigurationApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::dns_cache_application::DnsCacheApplication;
use infiltrator_application::dns_latency_application::DnsLatencyApplication;
use infiltrator_application::dns_leak_application::DnsLeakApplication;
use infiltrator_application::dns_query_application::DnsQueryApplication;
use infiltrator_application::doctor_application::DoctorApplication;
use infiltrator_application::mtu_application::MtuApplication;
use infiltrator_application::network_roaming_application::NetworkRoamingApplication;
use infiltrator_application::offline_startup_application::OfflineStartupApplication;
use infiltrator_application::pac_application::PacApplication;
use infiltrator_application::port_conflict_application::PortConflictApplication;
use infiltrator_application::profile_application::ProfileApplication;
use infiltrator_application::proxy_preferences_application::ProxyPreferencesApplication;
use infiltrator_application::resource_application::ResourceApplication;
use infiltrator_application::routing_application::RoutingApplication;
use infiltrator_application::rule_list_application::RuleListApplication;
use infiltrator_application::rule_tracer_application::RuleTracerApplication;
use infiltrator_application::script_application::ScriptApplication;
use infiltrator_application::script_export_application::ScriptExportApplication;
use infiltrator_application::service_mode_application::ServiceModeApplication;
use infiltrator_application::settings_application::SettingsApplication;
use infiltrator_application::snapshot_application::SnapshotApplication;
use infiltrator_application::speedtest_application::SpeedtestApplication;
use infiltrator_application::stun_probe_application::StunProbeApplication;
use infiltrator_application::surface_application::SurfacePump;
use infiltrator_application::surface_reader::ApplicationSurfaceReader;
use infiltrator_application::system_proxy_application::SystemProxyApplication;
use infiltrator_application::uwp_loopback_application::UwpLoopbackApplication;
use infiltrator_application::version_application::VersionApplication;
use infiltrator_contract::capability::{
    Availability, Capability, CapabilitySnapshot, CapabilityStatus,
};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_core::settings_io::app_config_manager;
use infiltrator_ports::network_roaming::NetworkRoamingPort;
use infiltrator_ports::runtime_gateway::RuntimeGateway;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// Desktop capabilities exposed to either UI surface.
pub fn desktop_capabilities() -> CapabilitySnapshot {
    let entries = [
        Capability::CoreLifecycle,
        Capability::Profiles,
        Capability::ProxyMode,
        Capability::Connections,
        Capability::Logs,
        Capability::Dns,
        Capability::Tun,
        Capability::SystemProxy,
        Capability::LanAccessControl,
        Capability::Ipv6Routing,
        Capability::UwpLoopback,
        Capability::PacService,
        Capability::NetworkRoaming,
        Capability::VpnService,
        Capability::Autostart,
        Capability::CoreVersionInstall,
        Capability::WebDavSync,
        Capability::AppRouting,
    ]
    .into_iter()
    .map(|capability| CapabilityStatus {
        capability,
        availability: if capability == Capability::UwpLoopback && !cfg!(windows) {
            Availability::Unsupported {
                reason: "Windows CheckNetIsolation is unavailable on this host".to_owned(),
            }
        } else if capability == Capability::VpnService {
            Availability::Unsupported {
                reason: "Android VpnService is a mobile-host capability".to_owned(),
            }
        } else {
            Availability::Supported
        },
    })
    .collect();
    CapabilitySnapshot::new(HostKind::Desktop, 0, entries)
}

/// Shared application engines handed from the desktop runtime to the surface
/// reader, so UI intents and surface projections observe one instance each.
pub struct SurfaceEngines {
    pub profiles: ProfileApplication,
    pub configuration: ConfigurationApplication,
    pub snapshots: SnapshotApplication,
    pub scripts: ScriptApplication,
    pub script_exports: ScriptExportApplication,
    pub doctor: DoctorApplication,
    pub proxy_preferences: ProxyPreferencesApplication,
    pub speedtest: SpeedtestApplication,
    pub rule_tracer: RuleTracerApplication,
    pub rule_list: RuleListApplication,
    pub dns_cache: DnsCacheApplication,
    pub dns_query: DnsQueryApplication,
    /// DUAL-14-10/13: the shared per-nameserver prober whose last report
    /// drives the latency row and the DNS self-heal snapshot.
    pub dns_latency: DnsLatencyApplication,
    /// DUAL-14-08: the shared cross-source leak prober whose last report the
    /// DNS page publishes.
    pub dns_leak: DnsLeakApplication,
    /// DUAL-14-09 (re-scoped): the shared STUN UDP-egress prober whose last
    /// report the DNS privacy area publishes.
    pub stun_probe: StunProbeApplication,
}

/// Assemble all currently available desktop application facades into one
/// surface reader. No UI toolkit appears in this function.
pub async fn application_surface_reader(
    core: Arc<CoreApplication>,
    gateway: Arc<dyn RuntimeGateway>,
    surface: SurfaceKind,
    binary_path: PathBuf,
    network_roaming_port: Arc<dyn NetworkRoamingPort>,
    engines: SurfaceEngines,
) -> anyhow::Result<ApplicationSurfaceReader> {
    let profile = engines.profiles;
    let config_dir = profile.config_dir();
    let configuration = engines.configuration;
    let settings_store = settings_store().await?;
    let settings = SettingsApplication::new(settings_store);
    let routing = RoutingApplication::new(Arc::new(app_routing_store()?));
    let versions = VersionApplication::new(Arc::new(version()?));
    let endpoint_source = Arc::new(endpoint_source().await?);
    let port_conflicts = PortConflictApplication::new(Arc::new(port_conflict()?));
    let resources = ResourceApplication::new(gateway.clone());
    let config_path = app_config_manager().await?.get_current_path().await?;
    let offline_startup =
        OfflineStartupApplication::new(Arc::new(offline_startup_port(&config_path, &binary_path)));
    let mtu = MtuApplication::new(Arc::new(DesktopMtuProbe::new()));
    let system_proxy = SystemProxyApplication::new(Arc::new(DesktopSystemProxy::new()));
    let uwp_loopback = UwpLoopbackApplication::new(Arc::new(DesktopUwpLoopbackPort));
    let pac = PacApplication::new(gateway.clone(), Arc::new(DesktopPacServicePort::shared()));
    let network_roaming =
        NetworkRoamingApplication::new(network_roaming_port, Some(gateway.clone()));
    let service_mode = ServiceModeApplication::new(Arc::new(DesktopServiceMode::new(binary_path)));
    // DUAL-11-07: the published cache fact comes from the same host directory
    // the desktop command handler purges.
    let rule_provider_cache = Arc::new(DesktopRuleProviderCache::new(config_dir.clone()));

    Ok(
        ApplicationSurfaceReader::new(core, surface, HostKind::Desktop)
            .with_capabilities(desktop_capabilities())
            .with_gateway(gateway)
            .with_resources(resources)
            .with_offline_startup(offline_startup)
            .with_mtu(mtu)
            .with_system_proxy(system_proxy)
            .with_uwp_loopback(uwp_loopback)
            .with_pac(pac)
            .with_network_roaming(network_roaming)
            .with_profiles(profile)
            .with_proxy_preferences(engines.proxy_preferences)
            .with_configuration(configuration)
            .with_doctor(engines.doctor)
            .with_routing(routing)
            .with_settings(settings)
            .with_snapshots(engines.snapshots)
            .with_versions(versions)
            .with_endpoint_source(endpoint_source)
            .with_service_mode(service_mode)
            .with_speedtest(engines.speedtest)
            .with_rule_tracer(engines.rule_tracer)
            .with_dns_query(engines.dns_query)
            .with_dns_cache(engines.dns_cache)
            .with_dns_latency(engines.dns_latency)
            .with_dns_leak(engines.dns_leak)
            .with_stun_probe(engines.stun_probe)
            .with_scripts(engines.scripts, engines.script_exports)
            .with_rule_provider_cache(rule_provider_cache)
            .with_port_conflicts(port_conflicts),
    )
}

/// Spawn the bounded desktop surface pump used by frame-driven UI hosts.
pub async fn surface_pump(
    core: Arc<CoreApplication>,
    gateway: Arc<dyn RuntimeGateway>,
    surface: SurfaceKind,
    sample_interval: Duration,
    binary_path: PathBuf,
    network_roaming_port: Arc<dyn NetworkRoamingPort>,
    engines: SurfaceEngines,
) -> anyhow::Result<SurfacePump> {
    let reader = application_surface_reader(
        Arc::clone(&core),
        gateway,
        surface,
        binary_path,
        network_roaming_port,
        engines,
    )
    .await?;
    let runtime = infiltrator_composition::tokio_application_runtime()
        .map_err(|error| anyhow::anyhow!(error))?;
    let initial = SurfaceSnapshot::unavailable(
        surface,
        HostKind::Desktop,
        Failure::new(
            ErrorCode::NotReady,
            "waiting for the first desktop surface snapshot",
            true,
        ),
    );
    Ok(SurfacePump::spawn(
        Arc::new(reader),
        sample_interval,
        runtime,
        initial,
    ))
}
