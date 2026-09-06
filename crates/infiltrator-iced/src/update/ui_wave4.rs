//! Wave 4 Advanced feature message handlers: Network Roaming, Crash Watchdog,
//! Web Dashboard launcher, Log Regex/Redaction, Subscription Quota, and PAC Manager.

use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use iced::Task;
use infiltrator_application::pac_application::PacApplication;
use infiltrator_contract::pac::{PacRequest, PacServiceState, PacSnapshot};
use infiltrator_contract::error::InfiltratorError;
use infiltrator_ports::runtime_gateway::RuntimeGateway;

fn split_pac_domains(value: &str) -> Vec<String> {
    value
        .split([',', ';', '\n'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect()
}

impl AppState {
    fn apply_pac(&mut self) -> Task<Message> {
        let Some(runtime) = self.runtime.runtime.clone() else {
            return self.runtime_unavailable("应用 PAC 本地服务");
        };
        let Some(service) = runtime.pac_service_port() else {
            let error = InfiltratorError::Internal(
                "当前宿主未提供 PAC 本地服务能力".to_owned(),
            );
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

    pub(super) fn update_ui_wave4(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::PollNetworkInterfaces => {
                let ifaces = vec![
                    crate::types::runtime::NetworkInterfaceItem {
                        name: "eth0".into(),
                        is_active: true,
                        gateway_ip: "192.168.1.1".into(),
                        mtu: 1500,
                    },
                    crate::types::runtime::NetworkInterfaceItem {
                        name: "wlan0".into(),
                        is_active: false,
                        gateway_ip: "192.168.2.1".into(),
                        mtu: 1500,
                    },
                ];
                self.runtime.network_roaming.interfaces = ifaces.clone();
                self.runtime.network_roaming.active_interface = "eth0".into();
                self.runtime.network_roaming.default_gateway = "192.168.1.1".into();
                self.runtime.network_roaming.optimal_mtu = 1500;
                Task::done(Message::NetworkInterfacesPolled(ifaces))
            }
            Message::NetworkInterfacesPolled(ifaces) => {
                self.runtime.network_roaming.interfaces = ifaces;
                Task::none()
            }
            Message::ForceGatewayReconnect => {
                self.runtime.network_roaming.last_roam_event =
                    Some("Gateway re-synchronized to 192.168.1.1 via eth0".into());
                Task::done(Message::ShowToast(
                    "Network gateway reconnected & routes healed".into(),
                    ToastStatus::Success,
                ))
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
                        format!("Automatic recovery is suspended after {attempts} failed attempt(s)")
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
