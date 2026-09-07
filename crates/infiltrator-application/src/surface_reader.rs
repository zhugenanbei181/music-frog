//! Default application-level assembler for the complete UI surface model.
//!
//! This type contains no concrete Mihomo, filesystem, or platform code. A
//! host composition supplies application facades and ports; this assembler
//! turns their results into the one contract read model consumed by Iced and
//! Bevy.

use crate::configuration_application::ConfigurationApplication;
use crate::core_application::CoreApplication;
use crate::doctor_application::DoctorApplication;
use crate::profile_application::ProfileApplication;
use crate::port_conflict_application::PortConflictApplication;
use crate::resource_application::ResourceApplication;
use crate::routing_application::RoutingApplication;
use crate::settings_application::SettingsApplication;
use crate::snapshot_application::SnapshotApplication;
use crate::service_mode_application::ServiceModeApplication;
use crate::version_application::VersionApplication;
use crate::uwp_loopback_application::UwpLoopbackApplication;
use crate::pac_application::PacApplication;
use crate::offline_startup_application::OfflineStartupApplication;
use crate::mtu_application::MtuApplication;
use crate::network_roaming_application::NetworkRoamingApplication;
use crate::vpn_application::VpnServiceApplication;
use crate::privileged_network_application::PrivilegedNetworkApplication;
use crate::traffic_waveform_application::TrafficWaveformApplication;
use crate::traffic_scale_application::TrafficScaleApplication;
use crate::system_proxy_application::SystemProxyApplication;
use infiltrator_contract::capability::CapabilitySnapshot;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot;
use infiltrator_contract::version::{CoreChannelStatus, CoreVersionSnapshot};
use infiltrator_domain::app_routing::{AppRoutingMode, AppRoutingRule};
use infiltrator_domain::proxy::Proxy;
use infiltrator_domain::rules::RuleEntry;
use infiltrator_ports::error::PortError;
use infiltrator_ports::endpoint::EndpointSource;
use infiltrator_ports::runtime_gateway::RuntimeGateway;
use infiltrator_ports::surface::SurfaceReader;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[path = "surface_reader_services.rs"]
mod surface_reader_services;

/// Application facades required to expose the page read model.
#[derive(Clone)]
pub struct ApplicationSurfaceReader {
    core: Arc<CoreApplication>,
    gateway: Option<Arc<dyn RuntimeGateway>>,
    profiles: Option<ProfileApplication>,
    configuration: Option<ConfigurationApplication>,
    doctor: Option<DoctorApplication>,
    routing: Option<RoutingApplication>,
    settings: Option<SettingsApplication>,
    snapshots: Option<SnapshotApplication>,
    versions: Option<VersionApplication>,
    endpoint_source: Option<Arc<dyn EndpointSource>>,
    service_mode: Option<ServiceModeApplication>,
    port_conflicts: Option<PortConflictApplication>,
    resources: Option<ResourceApplication>,
    offline_startup: Option<OfflineStartupApplication>,
    mtu: Option<MtuApplication>,
    system_proxy: Option<SystemProxyApplication>,
    uwp_loopback: Option<UwpLoopbackApplication>,
    pac: Option<PacApplication>,
    network_roaming: Option<NetworkRoamingApplication>,
    vpn: Option<VpnServiceApplication>,
    privileged_network: Option<PrivilegedNetworkApplication>,
    traffic_waveform: TrafficWaveformApplication,
    traffic_scale: TrafficScaleApplication,
    version_cache: Arc<Mutex<Option<(Instant, CoreVersionSnapshot)>>>,
    capabilities: CapabilitySnapshot,
    surface: SurfaceKind,
}

impl ApplicationSurfaceReader {
    pub fn new(core: Arc<CoreApplication>, surface: SurfaceKind, host: HostKind) -> Self {
        Self {
            core,
            gateway: None,
            profiles: None,
            configuration: None,
            doctor: None,
            routing: None,
            settings: None,
            snapshots: None,
            versions: None,
            endpoint_source: None,
            service_mode: None,
            port_conflicts: None,
            resources: None,
            offline_startup: None,
            mtu: None,
            system_proxy: None,
            uwp_loopback: None,
            pac: None,
            network_roaming: None,
            vpn: None,
            privileged_network: None,
            traffic_waveform: TrafficWaveformApplication::new(),
            traffic_scale: TrafficScaleApplication,
            version_cache: Arc::new(Mutex::new(None)),
            capabilities: CapabilitySnapshot::new(host, 0, Vec::new()),
            surface,
        }
    }

    pub fn with_capabilities(mut self, capabilities: CapabilitySnapshot) -> Self {
        self.capabilities = capabilities;
        self
    }

    pub fn with_gateway(mut self, gateway: Arc<dyn RuntimeGateway>) -> Self {
        self.gateway = Some(gateway);
        self
    }

    pub fn with_profiles(mut self, profiles: ProfileApplication) -> Self {
        self.profiles = Some(profiles);
        self
    }

    pub fn with_configuration(mut self, configuration: ConfigurationApplication) -> Self {
        self.configuration = Some(configuration);
        self
    }

    pub fn with_doctor(mut self, doctor: DoctorApplication) -> Self {
        self.doctor = Some(doctor);
        self
    }

    pub fn with_routing(mut self, routing: RoutingApplication) -> Self {
        self.routing = Some(routing);
        self
    }

    pub fn with_settings(mut self, settings: SettingsApplication) -> Self {
        self.settings = Some(settings);
        self
    }

    pub fn with_snapshots(mut self, snapshots: SnapshotApplication) -> Self {
        self.snapshots = Some(snapshots);
        self
    }

    pub fn with_versions(mut self, versions: VersionApplication) -> Self {
        self.versions = Some(versions);
        self
    }

    pub fn with_endpoint_source(mut self, source: Arc<dyn EndpointSource>) -> Self {
        self.endpoint_source = Some(source);
        self
    }

    pub fn with_service_mode(mut self, service_mode: ServiceModeApplication) -> Self {
        self.service_mode = Some(service_mode);
        self
    }

    pub fn with_port_conflicts(mut self, port_conflicts: PortConflictApplication) -> Self {
        self.port_conflicts = Some(port_conflicts);
        self
    }

    pub fn with_resources(mut self, resources: ResourceApplication) -> Self {
        self.resources = Some(resources);
        self
    }

    pub fn with_offline_startup(mut self, startup: OfflineStartupApplication) -> Self {
        self.offline_startup = Some(startup);
        self
    }

    pub fn with_mtu(mut self, mtu: MtuApplication) -> Self {
        self.mtu = Some(mtu);
        self
    }

    pub fn with_system_proxy(mut self, system_proxy: SystemProxyApplication) -> Self {
        self.system_proxy = Some(system_proxy);
        self
    }

    pub fn with_uwp_loopback(mut self, uwp_loopback: UwpLoopbackApplication) -> Self {
        self.uwp_loopback = Some(uwp_loopback);
        self
    }

    pub fn with_pac(mut self, pac: PacApplication) -> Self {
        self.pac = Some(pac);
        self
    }

    pub fn with_network_roaming(mut self, application: NetworkRoamingApplication) -> Self {
        self.network_roaming = Some(application);
        self
    }

    pub fn with_vpn(mut self, application: VpnServiceApplication) -> Self {
        self.vpn = Some(application);
        self
    }

    pub fn with_privileged_network(
        mut self,
        application: PrivilegedNetworkApplication,
    ) -> Self {
        self.privileged_network = Some(application);
        self
    }

    pub fn core(&self) -> &Arc<CoreApplication> {
        &self.core
    }

    async fn read_versions(&self) -> CoreVersionSnapshot {
        let Some(versions) = &self.versions else {
            return CoreVersionSnapshot::default();
        };
        if let Some((probed_at, snapshot)) = self
            .version_cache
            .lock()
            .expect("version cache lock")
            .as_ref()
            .cloned()
        {
            let has_ready_channel = snapshot
                .channels
                .iter()
                .any(|channel| matches!(channel.status, CoreChannelStatus::Ready { .. }));
            let ttl = if has_ready_channel {
                Duration::from_secs(600)
            } else {
                Duration::from_secs(30)
            };
            if probed_at.elapsed() < ttl {
                return snapshot;
            }
        }

        let next_revision = self
            .version_cache
            .lock()
            .expect("version cache lock")
            .as_ref()
            .map_or(1, |(_, snapshot)| snapshot.revision.saturating_add(1));
        let mut snapshot = versions.probe_channels().await;
        snapshot.revision = next_revision;
        *self.version_cache.lock().expect("version cache lock") =
            Some((Instant::now(), snapshot.clone()));
        snapshot
    }

}

#[async_trait::async_trait]
impl SurfaceReader for ApplicationSurfaceReader {
    async fn read(
        &self,
    ) -> Result<surface_snapshot::SurfaceSnapshot, infiltrator_ports::error::PortError> {
        let core = self.core.snapshot();
        let revision = core.revision.max(1);
        let traffic_waveform = self.traffic_waveform.record(&core);
        let traffic_scale = self.traffic_scale.compute(&traffic_waveform);
        let versions = self.read_versions().await;
        let controller_auth = self.read_controller_auth().await;
        let service_mode = self.read_service_mode().await;
        let port_conflicts = self.read_port_conflicts().await;
        let resources = self.read_resources().await;
        let offline_startup = self.read_offline_startup().await;
        let system_proxy = self.read_system_proxy().await;
        let system_proxy_recovery = self.read_system_proxy_recovery();
        let pac = self.read_pac().await;
        let network_roaming = self.read_network_roaming().await;
        let vpn = self.read_vpn().await;
        let privileged_network = self.read_privileged_network().await;
        let mut pages = surface_snapshot::SurfacePages::unavailable(missing("surface reader"));

        pages.overview =
            surface_snapshot::PageData::ready(surface_snapshot::OverviewPageSnapshot {
                proxy_mode: core.proxy_mode,
                upload_bps: core.upload_bps,
                download_bps: core.download_bps,
                active_connections: core.active_connections,
                memory_bytes: core.memory_bytes,
                core_version: core.core_version.clone(),
            });

        let runtime_config = match &self.gateway {
            Some(gateway) => Some(gateway.get_config().await),
            None => None,
        };
        let mtu = merge_applied_mtu(self.read_mtu().await, runtime_config.as_ref());
        let runtime_proxies = match &self.gateway {
            Some(gateway) => Some(gateway.get_proxies().await),
            None => None,
        };
        let runtime_connections = match &self.gateway {
            Some(gateway) => Some(gateway.get_connections().await),
            None => None,
        };
        let runtime_rule_providers = match &self.gateway {
            Some(gateway) => Some(gateway.get_rule_providers().await),
            None => None,
        };

        pages.proxies = page_from_result(
            runtime_proxies,
            |proxies| surface_snapshot::ProxiesPageSnapshot {
                groups: proxy_groups(&proxies),
                testing: false,
                active_exit: active_exit(&proxies),
            },
            "Mihomo proxy gateway",
        );

        if let Some(profiles) = &self.profiles {
            pages.profiles = match profiles.list_profiles().await {
                Ok(items) if items.is_empty() => {
                    surface_snapshot::PageData::empty(surface_snapshot::ProfilesPageSnapshot {
                        profiles: Vec::new(),
                        auto_update_interval_hours: 0,
                        updating: false,
                    })
                }
                Ok(items) => {
                    surface_snapshot::PageData::ready(surface_snapshot::ProfilesPageSnapshot {
                        profiles: items
                            .into_iter()
                            .map(|item| surface_snapshot::ProfileSnapshot {
                                id: item.name.clone(),
                                name: item.name,
                                url: item.subscription_url.unwrap_or_default(),
                                updated_at: item
                                    .last_updated
                                    .map(|value| value.to_rfc3339())
                                    .unwrap_or_default(),
                                upload_bytes: item.traffic_upload.unwrap_or_default(),
                                download_bytes: item.traffic_download.unwrap_or_default(),
                                total_bytes: item.traffic_total.unwrap_or_default(),
                                is_active: item.active,
                            })
                            .collect(),
                        auto_update_interval_hours: 0,
                        updating: false,
                    })
                }
                Err(failure) => surface_snapshot::PageData::failed(failure),
            };
        }

        pages.connections = page_from_result(
            runtime_connections,
            |connections| surface_snapshot::ConnectionsPageSnapshot {
                total_connections: connections.connections.len(),
                total_upload_bytes: connections.upload_total,
                total_download_bytes: connections.download_total,
                connections: connections
                    .connections
                    .into_iter()
                    .map(|connection| surface_snapshot::ConnectionSnapshot {
                        id: connection.id,
                        host: if connection.metadata.destination_port.is_empty() {
                            connection.metadata.host
                        } else {
                            format!(
                                "{}:{}",
                                connection.metadata.host, connection.metadata.destination_port
                            )
                        },
                        process: if connection.metadata.process_path.is_empty() {
                            "unknown".to_owned()
                        } else {
                            connection.metadata.process_path
                        },
                        rule: connection.rule,
                        chain: connection.chains.join(" -> "),
                        upload_bps: 0.0,
                        download_bps: 0.0,
                        upload_total: connection.upload,
                        download_total: connection.download,
                    })
                    .collect(),
            },
            "Mihomo connections gateway",
        );

        pages.rules = build_rules_page(self.configuration.as_ref(), runtime_rule_providers).await;

        pages.dns = build_dns_page(self.configuration.as_ref(), runtime_config.as_ref()).await;

        if self.gateway.is_some() {
            // Logs are an event stream, not an HTTP snapshot. Keep the page
            // explicitly loading until the host attaches the stream reader.
            pages.logs = surface_snapshot::PageData::loading();
        }

        if self.doctor.is_some() {
            // Diagnostics are user-triggered and must not be rerun every
            // telemetry tick. The command result later replaces this page.
            pages.doctor = surface_snapshot::PageData::loading();
        }

        let uwp_loopback = self.read_uwp_loopback().await;
        if let Some(routing) = &self.routing {
            pages.app_routing = match routing.load() {
                Ok(config) => surface_snapshot::PageData::ready(app_routing_page(
                    config,
                    uwp_loopback.clone(),
                )),
                Err(failure) => surface_snapshot::PageData::failed(failure),
            };
        }

        let hydrated_settings = match &self.settings {
            Some(settings) => Some(settings.load_hydrated().await),
            None => None,
        };
        pages.sync = build_sync_page(
            hydrated_settings.as_ref(),
            self.profiles.as_ref(),
            self.snapshots.as_ref(),
        )
        .await;
        pages.settings = build_settings_page(
            hydrated_settings.as_ref(),
            runtime_config.as_ref(),
            &system_proxy,
            pac,
        );

        Ok(surface_snapshot::SurfaceSnapshot {
            surface: self.surface,
            origin: surface_snapshot::SurfaceOrigin::Live,
            generation: core.generation,
            revision,
            core,
            capabilities: self.capabilities.clone(),
            failure: None,
            pages,
            versions,
            controller_auth,
            service_mode,
            port_conflicts,
            resources,
            offline_startup,
            mtu,
            system_proxy,
            system_proxy_recovery,
            network_roaming,
            vpn,
            privileged_network,
            traffic_waveform,
            traffic_scale,
        })
    }
}

fn missing(what: &str) -> Failure {
    Failure::new(
        ErrorCode::Unsupported,
        format!("{what} is not composed for this host"),
        false,
    )
}

fn merge_applied_mtu(
    mut snapshot: infiltrator_contract::mtu::MtuNegotiationSnapshot,
    runtime_config: Option<&Result<infiltrator_domain::runtime::ConfigSnapshot, PortError>>,
) -> infiltrator_contract::mtu::MtuNegotiationSnapshot {
    if let Some(Ok(config)) = runtime_config
        && let Some(mtu) = config.tun.as_ref().and_then(|tun| tun.mtu)
    {
        snapshot.applied_tun_mtu = Some(mtu);
    }
    snapshot
}

fn page_from_result<T, U, E>(
    result: Option<Result<T, E>>,
    map: impl FnOnce(T) -> U,
    what: &str,
) -> surface_snapshot::PageData<U>
where
    E: std::fmt::Display,
{
    match result {
        Some(Ok(value)) => surface_snapshot::PageData::ready(map(value)),
        Some(Err(error)) => surface_snapshot::PageData::failed(Failure::new(
            ErrorCode::Network,
            format!("{what}: {error}"),
            true,
        )),
        None => surface_snapshot::PageData::unavailable(missing(what)),
    }
}

fn proxy_groups(proxies: &HashMap<String, Proxy>) -> Vec<surface_snapshot::ProxyGroupSnapshot> {
    let mut groups = proxies
        .iter()
        .filter_map(|(name, proxy)| {
            let members = proxy.all()?;
            let current = proxy.now().unwrap_or_default().to_owned();
            Some(surface_snapshot::ProxyGroupSnapshot {
                name: name.clone(),
                group_type: proxy.proxy_type().to_owned(),
                current: current.clone(),
                expanded: true,
                proxies: members
                    .iter()
                    .filter_map(|member| {
                        let proxy = proxies.get(member)?;
                        Some(surface_snapshot::ProxyNodeSnapshot {
                            name: member.clone(),
                            node_type: proxy.proxy_type().to_owned(),
                            delay_ms: proxy.delay(),
                            selected: member == &current,
                            favorite: false,
                            features: proxy.udp().then(|| "UDP".to_owned()).into_iter().collect(),
                        })
                    })
                    .collect(),
            })
        })
        .collect::<Vec<_>>();
    groups.sort_by(|left, right| left.name.cmp(&right.name));
    groups
}

fn active_exit(proxies: &HashMap<String, Proxy>) -> String {
    proxies
        .iter()
        .find(|(name, proxy)| name.eq_ignore_ascii_case("proxies") && proxy.is_group())
        .and_then(|(_, proxy)| proxy.now())
        .or_else(|| {
            proxies
                .values()
                .find(|proxy| proxy.is_group())
                .and_then(Proxy::now)
        })
        .unwrap_or("—")
        .to_owned()
}

async fn build_rules_page(
    configuration: Option<&ConfigurationApplication>,
    runtime_providers: Option<Result<Vec<infiltrator_domain::runtime::RuleProvider>, PortError>>,
) -> surface_snapshot::PageData<surface_snapshot::RulesPageSnapshot> {
    let Some(configuration) = configuration else {
        return surface_snapshot::PageData::unavailable(missing("configuration application"));
    };
    let rules = match configuration.load_rules().await {
        Ok(rules) => rules,
        Err(error) => return surface_snapshot::PageData::failed(error),
    };
    let providers = match runtime_providers {
        Some(Ok(providers)) => providers
            .into_iter()
            .map(|provider| surface_snapshot::RuleProviderSnapshot {
                name: provider.name,
                rule_count: provider.rule_count as usize,
                behavior: provider.behavior,
                updated_at: provider.updated_at,
            })
            .collect(),
        Some(Err(error)) => {
            return surface_snapshot::PageData::failed(Failure::new(
                ErrorCode::Network,
                error.to_string(),
                true,
            ));
        }
        None => Vec::new(),
    };
    let mut entries = rules
        .into_iter()
        .enumerate()
        .map(|(id, rule)| rule_snapshot(id + 1, rule))
        .collect::<Vec<_>>();
    let default_action = entries
        .last()
        .map(|entry| entry.proxy.clone())
        .unwrap_or_else(|| "—".to_owned());
    entries.truncate(entries.len().min(5000));
    let total_rules = entries.len();
    let data = surface_snapshot::RulesPageSnapshot {
        total_rules,
        default_action,
        providers,
        rules: entries,
    };
    if total_rules == 0 {
        surface_snapshot::PageData::empty(data)
    } else {
        surface_snapshot::PageData::ready(data)
    }
}

fn rule_snapshot(id: usize, rule: RuleEntry) -> surface_snapshot::RuleSnapshot {
    let mut parts = rule.rule.splitn(3, ',');
    let rule_type = parts.next().unwrap_or_default().to_owned();
    let payload = parts.next().unwrap_or_default().to_owned();
    let proxy = parts.next().unwrap_or_default().to_owned();
    surface_snapshot::RuleSnapshot {
        id,
        rule_type,
        payload,
        proxy,
        hit_count: 0,
    }
}

async fn build_dns_page(
    configuration: Option<&ConfigurationApplication>,
    runtime_config: Option<&Result<infiltrator_domain::runtime::ConfigSnapshot, PortError>>,
) -> surface_snapshot::PageData<surface_snapshot::DnsPageSnapshot> {
    if let Some(configuration) = configuration {
        let dns = configuration.load_dns_config().await;
        let fake_ip = configuration.load_fake_ip_config().await;
        if let (Ok(dns), Ok(fake_ip)) = (dns, fake_ip) {
            let nameservers = dns.nameserver.clone().unwrap_or_default();
            let fallback = dns.fallback.clone().unwrap_or_default();
            let data = surface_snapshot::DnsPageSnapshot {
                mode: dns.enhanced_mode.unwrap_or_default(),
                cache_entries: 0,
                fake_ip_range: fake_ip.fake_ip_range.unwrap_or_default(),
                servers: dns_servers(nameservers, fallback),
            };
            return surface_snapshot::PageData::ready(data);
        }
    }
    match runtime_config {
        Some(Ok(config)) => match &config.dns {
            Some(dns) => surface_snapshot::PageData::ready(surface_snapshot::DnsPageSnapshot {
                mode: dns.enhanced_mode.clone(),
                cache_entries: 0,
                fake_ip_range: String::new(),
                servers: dns_servers(dns.nameserver.clone(), dns.fallback.clone()),
            }),
            None => surface_snapshot::PageData::empty(surface_snapshot::DnsPageSnapshot {
                mode: String::new(),
                cache_entries: 0,
                fake_ip_range: String::new(),
                servers: Vec::new(),
            }),
        },
        Some(Err(error)) => surface_snapshot::PageData::failed(Failure::new(
            ErrorCode::Network,
            error.to_string(),
            true,
        )),
        None => surface_snapshot::PageData::unavailable(missing("DNS reader")),
    }
}

fn dns_servers(
    nameservers: Vec<String>,
    fallback: Vec<String>,
) -> Vec<surface_snapshot::DnsServerSnapshot> {
    nameservers
        .into_iter()
        .map(|address| surface_snapshot::DnsServerSnapshot {
            protocol: dns_protocol(&address),
            address,
            latency_ms: None,
            is_fallback: false,
        })
        .chain(
            fallback
                .into_iter()
                .map(|address| surface_snapshot::DnsServerSnapshot {
                    protocol: dns_protocol(&address),
                    address,
                    latency_ms: None,
                    is_fallback: true,
                }),
        )
        .collect()
}

fn dns_protocol(address: &str) -> String {
    if address.starts_with("tls://") {
        "DoT".to_owned()
    } else if address.starts_with("quic://") {
        "DoQ".to_owned()
    } else if address.starts_with("http://") || address.starts_with("https://") {
        "DoH".to_owned()
    } else {
        "Plain".to_owned()
    }
}

fn app_routing_page(
    config: infiltrator_domain::app_routing::AppRoutingConfig,
    uwp_loopback: infiltrator_contract::uwp::UwpLoopbackSnapshot,
) -> surface_snapshot::AppRoutingPageSnapshot {
    let mode = match config.mode {
        AppRoutingMode::ProxyAll => "proxy_all",
        AppRoutingMode::ProxySelected => "proxy_selected",
        AppRoutingMode::BypassSelected => "bypass_selected",
    };
    let mut ids = config.packages.iter().cloned().collect::<Vec<_>>();
    ids.extend(config.rules.keys().cloned());
    ids.sort();
    ids.dedup();
    let apps = ids
        .into_iter()
        .map(|id| {
            let rule = config.rules.get(&id).copied().unwrap_or(match config.mode {
                AppRoutingMode::ProxyAll => AppRoutingRule::Proxy,
                AppRoutingMode::ProxySelected => AppRoutingRule::Direct,
                AppRoutingMode::BypassSelected => AppRoutingRule::Proxy,
            });
            surface_snapshot::AppSnapshot {
                id: id.clone(),
                name: id.clone(),
                process_name: id,
                rule: match rule {
                    AppRoutingRule::Proxy => "proxy",
                    AppRoutingRule::Direct => "direct",
                    AppRoutingRule::Block => "block",
                }
                .to_owned(),
                is_system: false,
            }
        })
        .collect();
    surface_snapshot::AppRoutingPageSnapshot {
        mode: mode.to_owned(),
        include_system: false,
        apps,
        uwp_loopback,
    }
}

async fn build_sync_page(
    settings: Option<&Result<infiltrator_domain::settings::AppSettings, Failure>>,
    profiles: Option<&ProfileApplication>,
    snapshots: Option<&SnapshotApplication>,
) -> surface_snapshot::PageData<surface_snapshot::SyncPageSnapshot> {
    let Some(Ok(settings)) = settings else {
        return surface_snapshot::PageData::unavailable(missing("settings application"));
    };
    let mut snapshot_items = Vec::new();
    if let (Some(profiles), Some(snapshots)) = (profiles, snapshots)
        && let Ok(profile) = profiles.current_profile().await
        && let Ok(items) = snapshots.list(&profile).await
    {
        snapshot_items = items
            .into_iter()
            .map(|item| surface_snapshot::SnapshotItemSnapshot {
                id: item.path.to_string_lossy().to_string(),
                timestamp: item.timestamp.to_rfc3339(),
                device: "local".to_owned(),
                size_bytes: 0,
            })
            .collect();
    }
    surface_snapshot::PageData::ready(surface_snapshot::SyncPageSnapshot {
        status: if settings.webdav.enabled {
            "connected".to_owned()
        } else {
            "disconnected".to_owned()
        },
        server_url: settings.webdav.url.clone(),
        username: settings.webdav.username.clone(),
        last_sync: None,
        auto_sync: settings.webdav.sync_on_startup,
        conflict: None,
        snapshots: snapshot_items,
    })
}

fn build_settings_page(
    settings: Option<&Result<infiltrator_domain::settings::AppSettings, Failure>>,
    runtime_config: Option<&Result<infiltrator_domain::runtime::ConfigSnapshot, PortError>>,
    system_proxy: &infiltrator_contract::system_proxy::SystemProxySnapshot,
    pac: infiltrator_contract::pac::PacSnapshot,
) -> surface_snapshot::PageData<surface_snapshot::SettingsPageSnapshot> {
    let settings = match settings {
        Some(Ok(settings)) => settings,
        Some(Err(error)) => {
            return surface_snapshot::PageData::failed(Failure::new(
                ErrorCode::Storage,
                error.message.clone(),
                true,
            ));
        }
        None => return surface_snapshot::PageData::unavailable(missing("settings application")),
    };
    let config = runtime_config.and_then(|result| result.as_ref().ok());
    surface_snapshot::PageData::ready(surface_snapshot::SettingsPageSnapshot {
        autostart: false,
        system_proxy: system_proxy.is_enabled(),
        mixed_port: config.map_or(0, |value| value.mixed_port),
        allow_lan: config.is_some_and(|value| value.allow_lan),
        lan_bind_address: config.map_or_else(
            || infiltrator_contract::lan::DEFAULT_BIND_ADDRESS.to_owned(),
            |value| value.bind_address.clone(),
        ),
        lan_security: config.map_or_else(
            infiltrator_contract::lan::LanSecuritySnapshot::default,
            |value| infiltrator_contract::lan::LanSecuritySnapshot::new(
                0,
                value.lan_allowed_ips.clone(),
                value.lan_disallowed_ips.clone(),
                value.skip_auth_prefixes.clone(),
                value.authentication_enabled,
                value.authentication_user_count,
                value.authentication_username.clone(),
            ),
        ),
        ipv6_routing: config.map_or_else(
            infiltrator_contract::ipv6::Ipv6RoutingSnapshot::default,
            |value| infiltrator_contract::ipv6::Ipv6RoutingSnapshot::new(
                0,
                value.ipv6,
                value.tun.as_ref().is_some_and(|tun| tun.enable),
            ),
        ),
        pac,
        tun_enabled: config
            .and_then(|value| value.tun.as_ref())
            .is_some_and(|tun| tun.enable),
        tun_stack: config
            .and_then(|value| value.tun.as_ref())
            .map_or_else(String::new, |tun| tun.stack.clone()),
        tun_auto_route: config
            .and_then(|value| value.tun.as_ref())
            .is_some_and(|tun| tun.auto_route),
        tun_strict_route: config
            .and_then(|value| value.tun.as_ref())
            .is_some_and(|tun| tun.strict_route),
        controller_port: 0,
        log_level: config
            .map(|value| value.log_level.clone())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "info".to_owned()),
        core_channel: settings.core_channel.clone(),
    })
}

#[cfg(test)]
#[path = "surface_reader_test.rs"]
mod tests;
