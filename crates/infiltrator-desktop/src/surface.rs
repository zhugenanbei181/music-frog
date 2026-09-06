//! Desktop composition for the complete shared UI surface.
//!
//! This module is intentionally outside both UI crates. It assembles concrete
//! desktop storage/controller adapters into the runtime-neutral
//! `ApplicationSurfaceReader` and `SurfacePump` consumed by Iced or Bevy.

use infiltrator_application::configuration_application::ConfigurationApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::port_conflict_application::PortConflictApplication;
use infiltrator_application::resource_application::ResourceApplication;
use infiltrator_application::doctor_application::DoctorApplication;
use infiltrator_application::profile_application::ProfileApplication;
use infiltrator_application::routing_application::RoutingApplication;
use infiltrator_application::settings_application::SettingsApplication;
use infiltrator_application::service_mode_application::ServiceModeApplication;
use infiltrator_application::snapshot_application::SnapshotApplication;
use infiltrator_application::surface_application::SurfacePump;
use infiltrator_application::offline_startup_application::OfflineStartupApplication;
use infiltrator_application::mtu_application::MtuApplication;
use infiltrator_application::network_roaming_application::NetworkRoamingApplication;
use infiltrator_application::pac_application::PacApplication;
use infiltrator_application::system_proxy_application::SystemProxyApplication;
use infiltrator_application::uwp_loopback_application::UwpLoopbackApplication;
use infiltrator_application::surface_reader::ApplicationSurfaceReader;
use infiltrator_application::version_application::VersionApplication;
use infiltrator_contract::capability::{
    Availability, Capability, CapabilitySnapshot, CapabilityStatus,
};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_ports::runtime_gateway::RuntimeGateway;
use infiltrator_ports::network_roaming::NetworkRoamingPort;
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
        } else {
            Availability::Supported
        },
    })
    .collect();
    CapabilitySnapshot::new(HostKind::Desktop, 0, entries)
}

/// Assemble all currently available desktop application facades into one
/// surface reader. No UI toolkit appears in this function.
pub async fn application_surface_reader(
    core: Arc<CoreApplication>,
    gateway: Arc<dyn RuntimeGateway>,
    surface: SurfaceKind,
    binary_path: std::path::PathBuf,
    network_roaming_port: Arc<dyn NetworkRoamingPort>,
) -> anyhow::Result<ApplicationSurfaceReader> {
    let profile_store = crate::storage::profile_store().await?;
    let configuration_store = Arc::clone(&profile_store);
    let snapshot_profile_store = Arc::clone(&profile_store);
    let profile = ProfileApplication::new(profile_store);
    let configuration = ConfigurationApplication::new(configuration_store);
    let settings_store = crate::storage::settings_store().await?;
    let settings = SettingsApplication::new(settings_store);
    let doctor = DoctorApplication::new(Arc::new(crate::storage::doctor()?));
    let routing = RoutingApplication::new(Arc::new(crate::storage::app_routing_store()?));
    let snapshots = SnapshotApplication::new(
        snapshot_profile_store,
        Arc::new(crate::storage::snapshot_store().await?),
    );
    let versions = VersionApplication::new(Arc::new(crate::storage::version()?));
    let endpoint_source = Arc::new(crate::storage::endpoint_source().await?);
    let port_conflicts =
        PortConflictApplication::new(Arc::new(crate::storage::port_conflict()?));
    let resources = ResourceApplication::new(gateway.clone());
    let config_path = infiltrator_core::settings_io::app_config_manager()
        .await?
        .get_current_path()
        .await?;
    let offline_startup = OfflineStartupApplication::new(Arc::new(
        crate::offline_startup::offline_startup_port(&config_path, &binary_path),
    ));
    let mtu = MtuApplication::new(Arc::new(crate::mtu::DesktopMtuProbe::new()));
    let system_proxy = SystemProxyApplication::new(Arc::new(
        crate::system_proxy::DesktopSystemProxy::new(),
    ));
    let uwp_loopback = UwpLoopbackApplication::new(Arc::new(
        crate::uwp_loopback_port::DesktopUwpLoopbackPort,
    ));
    let pac = PacApplication::new(
        gateway.clone(),
        Arc::new(crate::pac_service::DesktopPacServicePort::shared()),
    );
    let network_roaming = NetworkRoamingApplication::new(
        network_roaming_port,
        Some(gateway.clone()),
    );
    let service_mode =
        ServiceModeApplication::new(Arc::new(crate::service_mode::DesktopServiceMode::new(
            binary_path,
        )));

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
            .with_configuration(configuration)
            .with_doctor(doctor)
            .with_routing(routing)
            .with_settings(settings)
            .with_snapshots(snapshots)
            .with_versions(versions)
            .with_endpoint_source(endpoint_source)
            .with_service_mode(service_mode)
            .with_port_conflicts(port_conflicts),
    )
}

/// Spawn the bounded desktop surface pump used by frame-driven UI hosts.
pub async fn surface_pump(
    core: Arc<CoreApplication>,
    gateway: Arc<dyn RuntimeGateway>,
    surface: SurfaceKind,
    sample_interval: Duration,
    binary_path: std::path::PathBuf,
    network_roaming_port: Arc<dyn NetworkRoamingPort>,
) -> anyhow::Result<SurfacePump> {
    let reader =
        application_surface_reader(
            Arc::clone(&core),
            gateway,
            surface,
            binary_path,
            network_roaming_port,
        )
        .await?;
    let runtime = infiltrator_composition::tokio_application_runtime()
        .map_err(|error| anyhow::anyhow!(error))?;
    let initial = infiltrator_contract::surface_snapshot::SurfaceSnapshot::unavailable(
        surface,
        HostKind::Desktop,
        infiltrator_contract::error::Failure::new(
            infiltrator_contract::error::ErrorCode::NotReady,
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
