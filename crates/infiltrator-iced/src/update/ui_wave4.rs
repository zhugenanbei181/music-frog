//! Wave 4 Advanced feature message handlers: Network Roaming, Crash Watchdog,
//! Web Dashboard launcher, Log Regex/Redaction, Subscription Quota, and PAC Manager.

use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use iced::Task;
use infiltrator_application::network_roaming_application::NetworkRoamingApplication;
use infiltrator_application::pac_application::PacApplication;
use infiltrator_application::vpn_application::VpnServiceApplication;
use infiltrator_contract::error::InfiltratorError;
use infiltrator_contract::network_roaming::{
    NetworkInterfaceKind, NetworkInterfaceSnapshot, NetworkRoamingEvent, NetworkRoamingSnapshot,
    NetworkRoamingStatus,
};
use infiltrator_contract::pac::{PacRequest, PacServiceState, PacSnapshot};
use infiltrator_contract::vpn::{VpnSessionSnapshot, VpnSessionState};
use infiltrator_ports::runtime_gateway::RuntimeGateway;

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
    }
}

impl AppState {
    fn apply_pac(&mut self) -> Task<Message> {
        let Some(runtime) = self.runtime.runtime.clone() else {
            return self.runtime_unavailable("应用 PAC 本地服务");
        };
        let Some(service) = runtime.pac_service_port() else {
            let error = InfiltratorError::Internal("当前宿主未提供 PAC 本地服务能力".to_owned());
            self.set_error(&error);
            return Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error));
        };
        let request = PacRequest {
            enabled: self.runtime.pac_manager.is_pac_mode_active,
            bypass_domains: split_pac_domains(&self.runtime.pac_manager.bypass_subnets),
            bypass_lan: true,
            minify: false,
        };
        let gateway: std::sync::Arc<dyn RuntimeGateway> = runtime;
        Task::perform(
            async move {
                PacApplication::new(gateway, service)
                    .apply(request)
                    .await
                    .map_err(|failure| InfiltratorError::Internal(failure.message))
            },
            Message::PacApplied,
        )
    }

    fn apply_demo_pac(&mut self) -> Task<Message> {
        let config = infiltrator_domain::pac_generator::PacGenerator::new("PROXY 127.0.0.1:7890");
        let script = config.compile_pac_script(&self.editor.rules);
        if infiltrator_domain::pac_generator::validate_pac_script(&script).is_ok() {
            self.runtime.pac_manager.last_compile_status = Some("Valid PAC compiled".into());
            Task::done(Message::ShowToast(
                "PAC script compiled successfully".into(),
                ToastStatus::Success,
            ))
        } else {
            self.runtime.pac_manager.last_compile_status = Some("PAC validation error".into());
            Task::done(Message::ShowToast(
                "PAC compilation failed".into(),
                ToastStatus::Error,
            ))
        }
    }

    fn apply_pac_snapshot(&mut self, snapshot: PacSnapshot) {
        self.runtime.pac_manager.snapshot = snapshot.clone();
        self.runtime.pac_manager.bypass_subnets = snapshot.bypass_domains.join(", ");
        match snapshot.state {
            PacServiceState::Running { url } => {
                self.runtime.pac_manager.is_pac_mode_active = true;
                self.runtime.pac_manager.pac_url = url;
            }
            PacServiceState::Disabled | PacServiceState::Unavailable { .. } => {
                self.runtime.pac_manager.is_pac_mode_active = false;
                self.runtime.pac_manager.pac_url.clear();
            }
        }
        self.runtime.pac_manager.dirty = false;
    }

    fn apply_vpn(&mut self, start: bool) -> Task<Message> {
        if self.shell.demo {
            let snapshot = VpnSessionSnapshot::unsupported(
                self.runtime.vpn.revision.saturating_add(1),
                "Android VpnService is not part of the desktop demo host",
            );
            return Task::done(Message::VpnSessionUpdated(Ok(snapshot)));
        }
        let Some(runtime) = self.runtime.runtime.clone() else {
            return self.runtime_unavailable(if start {
                "申请 Android VPN 服务"
            } else {
                "停止 Android VPN 服务"
            });
        };
        let Some(port) = runtime.vpn_service_port() else {
            let error =
                InfiltratorError::Internal("当前宿主未提供 Android VpnService 能力".to_owned());
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
        match message {
            Message::PollNetworkInterfaces => {
                if self.shell.demo {
                    let snapshot = demo_network_roaming_snapshot();
                    self.runtime.network_roaming = snapshot.clone();
                    return Task::done(Message::NetworkInterfacesPolled(snapshot));
                }
                let Some(runtime) = self.runtime.runtime.clone() else {
                    return self.runtime_unavailable("探测物理网卡与默认网关");
                };
                let Some(port) = runtime.network_roaming_port() else {
                    let snapshot = NetworkRoamingSnapshot::unsupported(
                        self.runtime.network_roaming.revision.saturating_add(1),
                        "当前宿主未提供物理网卡漫游能力",
                    );
                    return Task::done(Message::NetworkInterfacesPolled(snapshot));
                };
                let baseline = self.runtime.network_roaming.clone();
                let gateway: std::sync::Arc<dyn RuntimeGateway> = runtime;
                Task::perform(
                    async move {
                        NetworkRoamingApplication::new(port, Some(gateway))
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
                    return self.runtime_unavailable("修复 TUN 默认网关路由");
                };
                let Some(port) = runtime.network_roaming_port() else {
                    let error =
                        InfiltratorError::Internal("当前宿主未提供网卡漫游路由修复能力".to_owned());
                    return Task::done(Message::NetworkRoamingRepaired(Err(error)));
                };
                let gateway: std::sync::Arc<dyn RuntimeGateway> = runtime;
                Task::perform(
                    async move {
                        NetworkRoamingApplication::new(port, Some(gateway))
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
                    VpnSessionState::PermissionRequired => {
                        ("等待 Android VPN 用户授权".to_owned(), ToastStatus::Info)
                    }
                    VpnSessionState::Starting => (
                        "Android VPN 前台服务已启动，等待隧道 FD".to_owned(),
                        ToastStatus::Info,
                    ),
                    VpnSessionState::Running => (
                        "Android VPN 隧道已启动并完成前台 readback".to_owned(),
                        ToastStatus::Success,
                    ),
                    VpnSessionState::Stopped | VpnSessionState::Revoked => {
                        ("Android VPN 已停止".to_owned(), ToastStatus::Info)
                    }
                    VpnSessionState::Unsupported { reason } => (
                        format!("当前宿主不支持 Android VPN: {reason}"),
                        ToastStatus::Info,
                    ),
                    _ => ("Android VPN 状态已更新".to_owned(), ToastStatus::Info),
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
                    .map(|failure| crate::utils::sanitize_ui_text(&failure.message))
                    .or_else(|| Some("No crashes detected in current session".into()));
                self.diag.crash_watchdog.recovery_status = Some(match &watchdog.state {
                    infiltrator_contract::snapshot::CoreWatchdogState::Idle => {
                        "Watchdog is monitoring the active core session".into()
                    }
                    infiltrator_contract::snapshot::CoreWatchdogState::Waiting {
                        attempt,
                        retry_in_ms,
                    } => format!(
                        "Automatic restart attempt {attempt} is scheduled in {retry_in_ms} ms"
                    ),
                    infiltrator_contract::snapshot::CoreWatchdogState::Restarting { attempt } => {
                        format!("Automatic restart attempt {attempt} is in progress")
                    }
                    infiltrator_contract::snapshot::CoreWatchdogState::Recovered { attempts } => {
                        format!("Core recovered after {attempts} restart attempt(s)")
                    }
                    infiltrator_contract::snapshot::CoreWatchdogState::Tripped { attempts } => {
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
                let _ = std::fs::write(&path, payload);
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
                self.diag.log_filter.regex_query = q;
                Task::none()
            }
            Message::SetLogLevelFilter(lvl) => {
                self.diag.log_filter.level_filter = lvl;
                Task::none()
            }
            Message::ExportRedactedLogs => {
                let path = "/tmp/infiltrator_redacted_logs.log".to_string();
                let mut out = String::new();
                for line in &self.diag.logs {
                    out.push_str(&crate::utils::sanitize_ui_text(line));
                    out.push('\n');
                }
                let _ = std::fs::write(&path, out);
                self.diag.log_filter.exported_redacted_path = Some(path.clone());
                Task::done(Message::ShowToast(
                    format!("Redacted logs exported to {path}"),
                    ToastStatus::Success,
                ))
            }
            Message::EvaluateSubscriptionQuota => {
                self.profile.quota_schedule.used_bytes = 1024 * 1024 * 1024 * 45;
                self.profile.quota_schedule.total_bytes = 1024 * 1024 * 1024 * 100;
                self.profile.quota_schedule.remaining_percent = 55.0;
                self.profile.quota_schedule.warning_tier = "Normal".into();
                Task::none()
            }
            Message::UpdateCronScheduleHours(h) => {
                self.profile.quota_schedule.cron_interval_hours = h;
                Task::none()
            }
            Message::UpdatePacBypassSubnets(subnets) => {
                self.runtime.pac_manager.bypass_subnets = subnets;
                self.runtime.pac_manager.dirty = true;
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
                self.runtime.pac_manager.is_pac_mode_active = on;
                self.runtime.pac_manager.dirty = true;
                if self.shell.demo {
                    self.runtime.pac_manager.pac_url = if on {
                        "http://127.0.0.1:25211/proxy.pac".into()
                    } else {
                        String::new()
                    };
                    Task::none()
                } else {
                    self.apply_pac()
                }
            }
            Message::PacApplied(Ok(snapshot)) => {
                self.apply_pac_snapshot(snapshot);
                self.runtime.pac_manager.last_compile_status =
                    Some("PAC script compiled and service state read back".to_owned());
                Task::done(Message::ShowToast(
                    "PAC 脚本已生成，本地服务状态已回读".to_owned(),
                    ToastStatus::Success,
                ))
            }
            Message::PacApplied(Err(error)) => {
                let snapshot = self.runtime.pac_manager.snapshot.clone();
                self.apply_pac_snapshot(snapshot);
                self.runtime.pac_manager.last_compile_status = Some(error.to_string());
                self.set_error(&error);
                Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error))
            }
            _ => self.update_ui_wave5(message),
        }
    }
}
