//! Wave 4 Advanced feature message handlers: Network Roaming, Crash Watchdog,
//! Web Dashboard launcher, Log Regex/Redaction, Subscription Quota, and PAC Manager.

use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use crate::utils::sanitize_ui_text;
use iced::Task;
use infiltrator_application::network_roaming_application::NetworkRoamingApplication;
use infiltrator_application::pac_application::PacApplication;
use infiltrator_application::vpn_application::VpnServiceApplication;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::network_roaming::{
    NetworkInterfaceKind, NetworkInterfaceSnapshot, NetworkRoamingEvent, NetworkRoamingSnapshot,
    NetworkRoamingStatus,
};
use infiltrator_contract::pac::PacSnapshot;
use infiltrator_contract::snapshot::CoreWatchdogState;
use infiltrator_contract::vpn::{VpnSessionSnapshot, VpnSessionState};
use infiltrator_domain::pac_generator::{PacGenerator, validate_pac_script};
use infiltrator_presentation::log_page_model::LogLevelFilter;
use infiltrator_shared::i18n_interpolator::localize;
use infiltrator_shared::locales::{Lang, Localizer};
use lurker_core_contract::error::InfiltratorError;
use std::fs::write;

fn split_pac_domains(value: &str) -> Vec<String> {
    value
        .split([',', ';', '\n'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect()
}

fn demo_network_roaming_snapshot() -> NetworkRoamingSnapshot {
    NetworkRoamingSnapshot {
        status: NetworkRoamingStatus::Stable,
        interfaces: vec![
            NetworkInterfaceSnapshot {
                name: "eth0".to_owned(),
                kind: NetworkInterfaceKind::Ethernet,
                is_up: true,
                is_default_gateway: true,
                gateway_ip: Some("192.168.1.1".to_owned()),
                ip_addresses: vec!["192.168.1.10/24".to_owned()],
                mtu: Some(1500),
                metric: Some(100),
                dns_servers: vec!["192.168.1.1".to_owned()],
            },
            NetworkInterfaceSnapshot {
                name: "wlan0".to_owned(),
                kind: NetworkInterfaceKind::Wifi,
                is_up: false,
                is_default_gateway: false,
                gateway_ip: Some("192.168.2.1".to_owned()),
                ip_addresses: Vec::new(),
                mtu: Some(1500),
                metric: Some(200),
                dns_servers: Vec::new(),
            },
        ],
        active_interface: Some("eth0".to_owned()),
        default_gateway: Some("192.168.1.1".to_owned()),
        previous_interface: None,
        tun_interface: Some("Meta".to_owned()),
        physical_mtu: Some(1500),
        recommended_tun_mtu: Some(1420),
        tcp_mss: Some(1380),
        route_repair_count: 0,
        last_event: Some(NetworkRoamingEvent::InitialObservation {
            interface: Some("eth0".to_owned()),
            gateway_ip: Some("192.168.1.1".to_owned()),
        }),
        observed_at_epoch_ms: None,
        revision: 1,
        ..Default::default()
    }
}

impl AppState {
    fn apply_pac(&mut self) -> Task<Message> {
        let copy_locale = self.shell.lang.clone();
        let Some(runtime) = self.runtime.runtime.clone() else {
            return self.runtime_unavailable(Lang(&copy_locale).tr("runtime_action_pac").as_ref());
        };
        let Some(service) = runtime.pac_service_port() else {
            let error = InfiltratorError::Internal(
                Lang(&copy_locale)
                    .tr("pac_service_unavailable")
                    .into_owned(),
            );
            self.set_error(&error);
            return Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error));
        };
        // The request is the shared draft's own conversion: the LAN bypass
        // and the minifier are the owner's readback values, so the write
        // never hardcodes a policy the owner never applied.
        let request = self.runtime.settings_model.pac().to_request();
        self.runtime.settings_model.pac_write_phase_mut().begin();
        let scope_source = runtime.core_scope_source();
        let configuration_ports = runtime.runtime_configuration_ports();
        Task::perform(
            async move {
                PacApplication::new(scope_source, configuration_ports, service)
                    .apply(request)
                    .await
                    .map_err(|failure| InfiltratorError::Internal(failure.message))
            },
            Message::PacApplied,
        )
    }

    fn apply_demo_pac(&mut self) -> Task<Message> {
        let config = PacGenerator::new("PROXY 127.0.0.1:7890");
        let script = config.compile_pac_script(&self.editor.rule_list.draft);
        if validate_pac_script(&script).is_ok() {
            self.runtime.pac_compile_status = Some("Valid PAC compiled".into());
            Task::done(Message::ShowToast(
                "PAC script compiled successfully".into(),
                ToastStatus::Success,
            ))
        } else {
            self.runtime.pac_compile_status = Some("PAC validation error".into());
            Task::done(Message::ShowToast(
                "PAC compilation failed".into(),
                ToastStatus::Error,
            ))
        }
    }

    pub(crate) fn apply_pac_snapshot(&mut self, snapshot: PacSnapshot) {
        let modified = self.runtime.settings_model.pac_modified();
        self.runtime.pac_snapshot = snapshot.clone();
        self.runtime.settings_model.observe_pac(&snapshot);
        if !modified {
            self.runtime.settings_text.pac_bypass_domains = snapshot.bypass_domains.join(", ");
        }
    }

    fn apply_vpn(&mut self, start: bool) -> Task<Message> {
        let copy_locale = self.shell.lang.clone();
        if self.shell.demo {
            let snapshot = VpnSessionSnapshot::unsupported(
                self.runtime.vpn.revision.saturating_add(1),
                "Android VpnService is not part of the desktop demo host",
            );
            return Task::done(Message::VpnSessionUpdated(Ok(snapshot)));
        }
        let Some(runtime) = self.runtime.runtime.clone() else {
            let operation = Lang(&copy_locale).tr(if start {
                "runtime_action_vpn_start"
            } else {
                "runtime_action_vpn_stop"
            });
            return self.runtime_unavailable(operation.as_ref());
        };
        let Some(port) = runtime.vpn_service_port() else {
            let error = InfiltratorError::Internal(
                Lang(&copy_locale)
                    .tr("vpn_service_unavailable")
                    .into_owned(),
            );
            return Task::done(Message::VpnSessionUpdated(Err(error)));
        };
        let application = VpnServiceApplication::new(port);
        Task::perform(
            async move {
                if start {
                    application
                        .request_start()
                        .await
                        .map_err(|failure| InfiltratorError::Internal(failure.message))
                } else {
                    application
                        .stop()
                        .await
                        .map_err(|failure| InfiltratorError::Internal(failure.message))
                }
            },
            Message::VpnSessionUpdated,
        )
    }

    pub(super) fn update_ui_wave4(&mut self, message: Message) -> Task<Message> {
        let copy_locale = self.shell.lang.clone();
        match message {
            Message::PollNetworkInterfaces => {
                if self.shell.demo {
                    let snapshot = demo_network_roaming_snapshot();
                    self.runtime.network_roaming = snapshot.clone();
                    return Task::done(Message::NetworkInterfacesPolled(snapshot));
                }
                let Some(runtime) = self.runtime.runtime.clone() else {
                    return self.runtime_unavailable(
                        Lang(&copy_locale)
                            .tr("runtime_action_network_probe")
                            .as_ref(),
                    );
                };
                let Some(port) = runtime.network_roaming_port() else {
                    let snapshot = NetworkRoamingSnapshot::unsupported(
                        self.runtime.network_roaming.revision.saturating_add(1),
                        Lang(&copy_locale)
                            .tr("network_roaming_unavailable")
                            .as_ref(),
                    );
                    return Task::done(Message::NetworkInterfacesPolled(snapshot));
                };
                let baseline = self.runtime.network_roaming.clone();
                let scope_source = runtime.core_scope_source();
                let configuration_ports = runtime.runtime_configuration_ports();
                Task::perform(
                    async move {
                        NetworkRoamingApplication::new(port)
                            .with_core_configuration(scope_source, configuration_ports)
                            .refresh_from(Some(baseline))
                            .await
                    },
                    Message::NetworkInterfacesPolled,
                )
            }
            Message::NetworkInterfacesPolled(snapshot) => {
                self.runtime.network_roaming = snapshot.clone();
                if let NetworkRoamingStatus::Failed { failure } = &snapshot.status {
                    let error = InfiltratorError::Internal(failure.message.clone());
                    self.set_error(&error);
                }
                Task::none()
            }
            Message::ForceGatewayReconnect => {
                if self.shell.demo {
                    let mut snapshot = demo_network_roaming_snapshot();
                    snapshot.route_repair_count = snapshot.route_repair_count.saturating_add(1);
                    snapshot.last_event = Some(NetworkRoamingEvent::RoutesRepaired {
                        physical_interface: "eth0".to_owned(),
                        tun_interface: "Meta".to_owned(),
                        detail: "demo route readback matched".to_owned(),
                    });
                    self.runtime.network_roaming = snapshot.clone();
                    return Task::done(Message::NetworkRoamingRepaired(Ok(snapshot)));
                }
                let Some(runtime) = self.runtime.runtime.clone() else {
                    return self.runtime_unavailable(
                        Lang(&copy_locale)
                            .tr("runtime_action_gateway_repair")
                            .as_ref(),
                    );
                };
                let Some(port) = runtime.network_roaming_port() else {
                    let error = InfiltratorError::Internal(
                        Lang(&copy_locale)
                            .tr("network_route_repair_unavailable")
                            .into_owned(),
                    );
                    return Task::done(Message::NetworkRoamingRepaired(Err(error)));
                };
                let scope_source = runtime.core_scope_source();
                let configuration_ports = runtime.runtime_configuration_ports();
                Task::perform(
                    async move {
                        NetworkRoamingApplication::new(port)
                            .with_core_configuration(scope_source, configuration_ports)
                            .force_repair()
                            .await
                            .map_err(|failure| InfiltratorError::Internal(failure.message))
                    },
                    Message::NetworkRoamingRepaired,
                )
            }
            Message::NetworkRoamingRepaired(Ok(snapshot)) => {
                self.runtime.network_roaming = snapshot;
                Task::done(Message::ShowToast(
                    "Network gateway reconnected & routes healed".into(),
                    ToastStatus::Success,
                ))
            }
            Message::NetworkRoamingRepaired(Err(error)) => {
                self.set_error(&error);
                Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error))
            }
            Message::StartVpn => self.apply_vpn(true),
            Message::StopVpn => self.apply_vpn(false),
            Message::VpnSessionUpdated(Ok(snapshot)) => {
                let state = snapshot.state.clone();
                self.runtime.vpn = snapshot;
                let (message, status) = match state {
                    VpnSessionState::PermissionRequired => (
                        Lang(&copy_locale).tr("vpn_waiting_permission").into_owned(),
                        ToastStatus::Info,
                    ),
                    VpnSessionState::Starting => (
                        Lang(&copy_locale).tr("vpn_service_starting").into_owned(),
                        ToastStatus::Info,
                    ),
                    VpnSessionState::Running => (
                        Lang(&copy_locale).tr("vpn_running_verified").into_owned(),
                        ToastStatus::Success,
                    ),
                    VpnSessionState::Stopped | VpnSessionState::Revoked => (
                        Lang(&copy_locale).tr("vpn_stopped_notice").into_owned(),
                        ToastStatus::Info,
                    ),
                    VpnSessionState::Unsupported { reason } => (
                        localize(
                            &copy_locale,
                            "vpn_unsupported_reason",
                            &[("reason", reason)],
                        ),
                        ToastStatus::Info,
                    ),
                    _ => (
                        Lang(&copy_locale).tr("vpn_status_updated").into_owned(),
                        ToastStatus::Info,
                    ),
                };
                Task::done(Message::ShowToast(message, status))
            }
            Message::VpnSessionUpdated(Err(error)) => {
                self.set_error(&error);
                Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error))
            }
            Message::CheckCrashWatchdog => {
                let watchdog = &self.diag.crash_watchdog.shared;
                self.diag.crash_watchdog.last_crash_summary = watchdog
                    .last_error
                    .as_ref()
                    .map(|failure| sanitize_ui_text(&failure.message))
                    .or_else(|| Some("No crashes detected in current session".into()));
                self.diag.crash_watchdog.recovery_status = Some(match &watchdog.state {
                    CoreWatchdogState::Idle => {
                        "Watchdog is monitoring the active core session".into()
                    }
                    CoreWatchdogState::Waiting {
                        attempt,
                        retry_in_ms,
                    } => format!(
                        "Automatic restart attempt {attempt} is scheduled in {retry_in_ms} ms"
                    ),
                    CoreWatchdogState::Restarting { attempt } => {
                        format!("Automatic restart attempt {attempt} is in progress")
                    }
                    CoreWatchdogState::Recovered { attempts } => {
                        format!("Core recovered after {attempts} restart attempt(s)")
                    }
                    CoreWatchdogState::Tripped { attempts } => {
                        format!(
                            "Automatic recovery is suspended after {attempts} failed attempt(s)"
                        )
                    }
                });
                Task::none()
            }
            Message::RecoverOrphanedState => {
                self.diag.crash_watchdog.is_orphaned_detected = false;
                self.diag.crash_watchdog.recovery_status = Some("Orphaned states cleared".into());
                Task::done(Message::ShowToast(
                    "Orphaned state recovered successfully".into(),
                    ToastStatus::Success,
                ))
            }
            Message::ExportCrashDiagnostics => {
                let path = "/tmp/infiltrator_crash_diagnostics.json".to_string();
                let payload = serde_json::to_string_pretty(&self.diag.crash_watchdog.shared)
                    .unwrap_or_else(|_| "{}".to_string());
                let _ = write(&path, payload);
                self.diag.crash_watchdog.exported_log_path = Some(path.clone());
                Task::done(Message::ShowToast(
                    format!("Exported diagnostics: {path}"),
                    ToastStatus::Success,
                ))
            }
            Message::LaunchWebDashboard(dash) => {
                let port = 9090;
                let url = match dash {
                    "metacubexd" => format!("http://127.0.0.1:{port}/ui/"),
                    "yacd" => format!("http://127.0.0.1:{port}/ui/yacd/"),
                    _ => format!("http://127.0.0.1:{port}/ui/razord/"),
                };
                #[cfg(not(test))]
                if !self.shell.demo {
                    let _ = webbrowser::open(&url);
                }
                #[cfg(test)]
                let _ = &url;
                Task::done(Message::ShowToast(
                    format!("Opened {dash} dashboard"),
                    ToastStatus::Info,
                ))
            }
            Message::UpdateLogRegexFilter(q) => {
                self.diag.logs_page.edit_query(&q);
                Task::none()
            }
            Message::SetLogLevelFilter(level) => {
                if let Some(application) = self.command_ingress() {
                    let generation = self.runtime.core_lifecycle.generation;
                    let level = level.map(|level| level.label().to_string());
                    return Task::perform(
                        async move {
                            application
                                .execute(CommandIntent::SetLogLevelFilter { level })
                                .await
                                .into_unit()
                                .map(|_| ())
                        },
                        move |result| Message::LogsCommandFinished { generation, result },
                    );
                }

                self.diag.logs_page.select_level(match level {
                    Some(level) => LogLevelFilter::Level(level),
                    None => LogLevelFilter::All,
                });
                Task::none()
            }
            Message::UpdatePacBypassSubnets(subnets) => {
                self.runtime.settings_text.pac_bypass_domains = subnets.clone();
                let parsed = split_pac_domains(&subnets);
                self.runtime
                    .settings_model
                    .edit_pac(|draft| draft.bypass_domains = parsed);
                Task::none()
            }
            Message::CompileAndValidatePac => {
                if self.shell.demo {
                    self.apply_demo_pac()
                } else {
                    self.apply_pac()
                }
            }
            Message::TogglePacMode(on) => {
                self.runtime
                    .settings_model
                    .edit_pac(|draft| draft.enabled = on);
                if self.shell.demo {
                    Task::none()
                } else {
                    self.apply_pac()
                }
            }
            Message::PacApplied(Ok(snapshot)) => {
                // The owner's answer settles the draft: the value it confirms
                // becomes the draft the page shows.
                self.runtime.settings_model.discard_pac();
                self.apply_pac_snapshot(snapshot);
                self.runtime.settings_model.pac_write_phase_mut().reset();
                self.runtime.pac_compile_status =
                    Some("PAC script compiled and service state read back".to_owned());
                Task::done(Message::ShowToast(
                    Lang(&copy_locale).tr("pac_service_verified").into_owned(),
                    ToastStatus::Success,
                ))
            }
            Message::PacApplied(Err(error)) => {
                let snapshot = self.runtime.pac_snapshot.clone();
                self.apply_pac_snapshot(snapshot);
                self.runtime.settings_model.pac_write_phase_mut().reset();
                self.runtime.pac_compile_status = Some(error.to_string());
                self.set_error(&error);
                Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error))
            }
            _ => self.update_ui_wave5(message),
        }
    }
}
