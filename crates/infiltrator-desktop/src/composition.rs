//! Desktop composition root helpers.
//!
//! This module is the only place in the desktop host that assembles a
//! Tokio-backed application service with a concrete process and controller
//! adapter. UI crates receive the resulting `CoreApplication` handle.

use crate::certificate_authority::DesktopCertificateAuthority;
use crate::log_export::DesktopLogExportPort;
use crate::mtu::DesktopMtuProbe;
use crate::network_roaming::DesktopNetworkRoamingPort;
use crate::pac_service::DesktopPacServicePort;
use crate::rule_provider_cache::DesktopRuleProviderCache;
use crate::service::ServiceManager;
use crate::service_mode::DesktopServiceMode;
use crate::storage::{port_conflict, version};
use crate::subscription_import_port::DesktopSubscriptionImportPort;
use crate::subscription_notification_port::DesktopSubscriptionNotificationPort;
use crate::surface::SurfaceEngines;
use crate::system_proxy::DesktopSystemProxy;
use crate::uwp_loopback_port::DesktopUwpLoopbackPort;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::log_export_application::LogExportApplication;
use infiltrator_application::mtu_application::MtuApplication;
use infiltrator_application::network_roaming_application::NetworkRoamingApplication;
use infiltrator_application::pac_application::PacApplication;
use infiltrator_application::port_conflict_application::PortConflictApplication;
use infiltrator_application::service_mode_application::ServiceModeApplication;
use infiltrator_application::system_proxy_application::SystemProxyApplication;
use infiltrator_application::uwp_loopback_application::UwpLoopbackApplication;
use infiltrator_application::version_application::VersionApplication;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::subscription_source::SubscriptionSource;
use mihomo_api::client::MihomoClient;
use mihomo_api::overview::ControllerOverviewReader;
use mihomo_api::readiness::ControllerReadiness;
use std::path::PathBuf;
use std::sync::Arc;

/// Build the 0.30 lifecycle application over the desktop process host and
/// Mihomo controller readiness adapter.
///
/// `speedtest` is the same instance the surface reader publishes, so a
/// `RunSpeedtest` command and the Overview/Proxies telemetry read model share
/// one engine state instead of two divergent copies.
pub fn core_application(
    service: &ServiceManager,
    controller_url: impl Into<String>,
    secret: Option<String>,
    engines: SurfaceEngines,
    profile_store: Arc<dyn ProfileStore>,
    subscription_source: Arc<dyn SubscriptionSource>,
) -> anyhow::Result<CoreApplication> {
    let controller_url = controller_url.into();
    let exports = Arc::new(DesktopLogExportPort::new(profile_store.config_dir()));
    let export_secrets = secret.iter().cloned().collect();
    let client = MihomoClient::new(&controller_url, secret.clone())?;
    let runtime = infiltrator_composition::tokio_application_runtime()
        .map_err(|error| anyhow::anyhow!(error))?;
    let application = CoreApplication::new_with_overview(
        service.core_process(),
        Arc::new(ControllerReadiness::new(controller_url, secret)),
        Arc::new(ControllerOverviewReader::new(client.clone())),
        runtime.clone(),
    );
    application
        .install_log_gateway(Arc::new(client.clone()))
        .map_err(|failure| anyhow::anyhow!(failure.message))?;
    application.install_command_handler(Arc::new(
        desktop_command_application(DesktopCommandServices {
            binary_path: service.binary_path().to_path_buf(),
            client,
            engines,
            profile_store,
            subscription_source,
        })?
        .with_logs(application.log_application())
        .with_log_export(LogExportApplication::new(
            application.log_application(),
            Some(exports),
            export_secrets,
        )),
    ));
    Ok(application)
}

/// Ports and engines used by either desktop peer's command service.
pub struct DesktopCommandServices {
    pub binary_path: PathBuf,
    pub client: MihomoClient,
    pub engines: SurfaceEngines,
    pub profile_store: Arc<dyn ProfileStore>,
    pub subscription_source: Arc<dyn SubscriptionSource>,
}

pub fn desktop_command_application(
    services: DesktopCommandServices,
) -> anyhow::Result<CommandApplication> {
    let DesktopCommandServices {
        binary_path,
        client,
        engines,
        profile_store,
        subscription_source,
    } = services;
    let SurfaceEngines {
        profiles,
        configuration,
        snapshots,
        scripts,
        script_exports,
        doctor,
        proxy_preferences,
        speedtest,
        rule_tracer,
        rule_list,
        dns_cache,
        dns_query,
        dns_latency,
        dns_leak,
        stun_probe,
    } = engines;
    let runtime = infiltrator_composition::tokio_application_runtime()
        .map_err(|error| anyhow::anyhow!(error))?;
    // Keep the Bevy command seam live in the desktop composition: version
    // rollback is an application use-case, not a UI-local file operation.
    let versions = VersionApplication::new(Arc::new(version()?));
    let service_mode =
        ServiceModeApplication::new(Arc::new(DesktopServiceMode::new(binary_path.clone())));
    let port_conflicts = PortConflictApplication::new(Arc::new(port_conflict()?));
    let pac = PacApplication::new(
        Arc::new(client.clone()),
        Arc::new(DesktopPacServicePort::shared()),
    );
    let network_roaming = NetworkRoamingApplication::new(
        Arc::new(DesktopNetworkRoamingPort::shared()),
        Some(Arc::new(client.clone())),
    );
    // DUAL-11-06/07: the kernel's provider cache lives next to the profiles
    // (`-d <config dir>`), so the command handler purges exactly that folder.
    let rule_provider_cache = Arc::new(DesktopRuleProviderCache::new(profile_store.config_dir()));
    Ok(CommandApplication::new()
        .with_scripts(scripts, script_exports)
        .with_doctor(doctor)
        .with_runtime(Arc::new(client.clone()))
        .with_application_runtime(runtime)
        .with_profile(profiles)
        .with_proxy_preferences(proxy_preferences)
        .with_configuration(configuration)
        .with_subscription_source(subscription_source)
        .with_import_source(Arc::new(DesktopSubscriptionImportPort))
        .with_subscription_notifier(Arc::new(DesktopSubscriptionNotificationPort))
        .with_certificate_authority(Arc::new(DesktopCertificateAuthority::new()))
        .with_mtu(MtuApplication::new(Arc::new(DesktopMtuProbe::new())))
        .with_system_proxy(SystemProxyApplication::new(Arc::new(
            DesktopSystemProxy::new(),
        )))
        .with_uwp_loopback(UwpLoopbackApplication::new(Arc::new(
            DesktopUwpLoopbackPort,
        )))
        .with_pac(pac)
        .with_network_roaming(network_roaming)
        .with_versions(versions)
        .with_service_mode(service_mode)
        .with_port_conflicts(port_conflicts)
        .with_snapshots(snapshots)
        .with_speedtest(speedtest)
        .with_rule_tracer(rule_tracer)
        .with_rule_list(rule_list)
        .with_dns_query(dns_query)
        .with_dns_cache(dns_cache)
        .with_dns_latency(dns_latency)
        .with_dns_leak(dns_leak)
        .with_stun_probe(stun_probe)
        .with_rule_provider_cache(rule_provider_cache))
}
