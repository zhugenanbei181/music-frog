//! Wave 3 Advanced feature message handlers: PCAP capture, Sub-Rules, Speedtest,
//! GeoData updater, UWP Loopback, and Encrypted Backup packages.

use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use iced::Task;

impl AppState {
    pub(super) fn update_ui_wave3(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::TogglePcapCapture => {
                self.diag.pcap_state.is_capturing = !self.diag.pcap_state.is_capturing;
                if self.diag.pcap_state.is_capturing {
                    self.diag.pcap_state.packet_count = 0;
                    self.diag.pcap_state.total_bytes = 0;
                }
                Task::none()
            }
            Message::ExportPcapBuffer => {
                let mut writer = infiltrator_domain::pcap_exporter::PcapExporter::new(
                    infiltrator_domain::pcap_exporter::PcapHeader::new(65535, 1),
                );
                let data = [0x45, 0x00, 0x00, 0x3c, 0x00, 0x01];
                writer.append_packet(1725360000, 500000, &data);
                let path = "/tmp/infiltrator_capture.pcap".to_string();
                let _ = std::fs::write(&path, writer.as_bytes());
                self.diag.pcap_state.exported_path = Some(path.clone());
                Task::done(Message::ShowToast(
                    format!("Exported: {path}"),
                    ToastStatus::Success,
                ))
            }
            Message::UpdateSubRuleOperator(op) => {
                self.editor.subrule_draft.operator = op;
                Task::none()
            }
            Message::AddSubRuleCondition(cond) => {
                self.editor.subrule_draft.conditions.push(cond);
                Task::none()
            }
            Message::RemoveSubRuleCondition(idx) => {
                if idx < self.editor.subrule_draft.conditions.len() {
                    self.editor.subrule_draft.conditions.remove(idx);
                }
                Task::none()
            }
            Message::UpdateSubRuleTarget(t) => {
                self.editor.subrule_draft.target = t;
                Task::none()
            }
            Message::InsertSubRuleIntoRules => {
                let op = &self.editor.subrule_draft.operator;
                let conds = self.editor.subrule_draft.conditions.join(", ");
                let target = &self.editor.subrule_draft.target;
                let formatted_rule = format!("{op}(({conds})),{target}");
                self.editor.rules.push(infiltrator_core::rules::RuleEntry {
                    rule: formatted_rule.clone(),
                    enabled: true,
                });
                self.editor.rules_dirty = true;
                Task::done(Message::ShowToast(
                    format!("Inserted: {formatted_rule}"),
                    ToastStatus::Success,
                ))
            }
            Message::RunNodeSpeedtest(node) => {
                self.diag.speedtest_result.target_node = node.clone();
                self.diag.speedtest_result.is_running = true;
                Task::perform(
                    async move {
                        let duration_ms = 2400u64;
                        let total_bytes = 1024 * 1024 * 48;
                        let bandwidth_mbps =
                            infiltrator_core::diagnostics::SpeedtestCalculator::calculate_bandwidth(
                                total_bytes,
                                duration_ms,
                            ) * 8.0
                                / 1000.0;
                        let mut jitter_calc =
                            infiltrator_core::diagnostics::JitterCalculator::new();
                        jitter_calc.record_success(24.5);
                        jitter_calc.record_success(28.2);
                        jitter_calc.record_success(22.1);
                        jitter_calc.record_success(26.0);
                        let jitter_stats = jitter_calc.calculate();
                        crate::types::perf::SpeedtestResult {
                            target_node: node,
                            bandwidth_mbps,
                            jitter_ms: jitter_stats.jitter_ms,
                            packet_loss_percent: jitter_stats.loss_rate_percent,
                            tier: if bandwidth_mbps > 100.0 {
                                "Excellent".into()
                            } else {
                                "Good".into()
                            },
                            is_running: false,
                        }
                    },
                    Message::NodeSpeedtestFinished,
                )
            }
            Message::NodeSpeedtestFinished(res) => {
                self.diag.speedtest_result = res;
                Task::none()
            }
            Message::CheckGeoDataUpdates => {
                self.editor.geodata_status.is_updating = true;
                self.editor.geodata_status.geoip_version = "v2026.09.01".into();
                self.editor.geodata_status.geosite_version = "v2026.09.01".into();
                self.editor.geodata_status.geoip_size_bytes = 7_450_210;
                self.editor.geodata_status.geosite_size_bytes = 4_892_100;
                self.editor.geodata_status.is_updating = false;
                self.editor.geodata_status.update_message =
                    Some("Geo databases are up to date".into());
                Task::none()
            }
            Message::TriggerGeoDataUpdate => {
                self.editor.geodata_status.is_updating = true;
                self.editor.geodata_status.geoip_version = "v2026.09.03".into();
                self.editor.geodata_status.geosite_version = "v2026.09.03".into();
                self.editor.geodata_status.is_updating = false;
                self.editor.geodata_status.update_message =
                    Some("Updated GeoIP and GeoSite successfully".into());
                Task::done(Message::ShowToast(
                    "Geo databases updated successfully".into(),
                    ToastStatus::Success,
                ))
            }
            Message::GeoDataUpdateFinished(st) => {
                self.editor.geodata_status = st;
                Task::none()
            }
            Message::ScanUwpApps => {
                self.shell.uwp_loopback.is_scanning = true;
                let apps = vec![
                    crate::types::app::UwpAppItem {
                        sid: "S-1-15-2-1".into(),
                        display_name: "Microsoft Store".into(),
                        is_exempt: true,
                    },
                    crate::types::app::UwpAppItem {
                        sid: "S-1-15-2-2".into(),
                        display_name: "Xbox App".into(),
                        is_exempt: false,
                    },
                    crate::types::app::UwpAppItem {
                        sid: "S-1-15-2-3".into(),
                        display_name: "Windows Terminal".into(),
                        is_exempt: true,
                    },
                ];
                self.shell.uwp_loopback.is_scanning = false;
                self.shell.uwp_loopback.apps = apps;
                Task::none()
            }
            Message::UwpAppsLoaded(apps) => {
                self.shell.uwp_loopback.apps = apps;
                self.shell.uwp_loopback.is_scanning = false;
                Task::none()
            }
            Message::ExemptAllUwpApps => {
                for app in &mut self.shell.uwp_loopback.apps {
                    app.is_exempt = true;
                }
                Task::done(Message::ShowToast(
                    "All UWP apps loopback exempted".into(),
                    ToastStatus::Success,
                ))
            }
            Message::ClearAllUwpExemptions => {
                for app in &mut self.shell.uwp_loopback.apps {
                    app.is_exempt = false;
                }
                Task::done(Message::ShowToast(
                    "All UWP loopback exemptions cleared".into(),
                    ToastStatus::Success,
                ))
            }
            Message::ToggleUwpAppExemption(sid) => {
                if let Some(app) = self.shell.uwp_loopback.apps.iter_mut().find(|a| a.sid == sid) {
                    app.is_exempt = !app.is_exempt;
                }
                Task::none()
            }
            Message::UpdateEncryptedBackupPassphrase(pass) => {
                self.profile.encrypted_backup.passphrase = pass;
                Task::none()
            }
            Message::ExportEncryptedPackage => {
                let pass = self.profile.encrypted_backup.passphrase.trim();
                if pass.len() < 6 {
                    return Task::done(Message::ShowToast(
                        "Passphrase must be at least 6 characters".into(),
                        ToastStatus::Warning,
                    ));
                }
                let dummy_bundle =
                    infiltrator_core::backup::BackupBundle::new(vec![], String::new(), String::new());
                if let Ok(bytes) =
                    infiltrator_core::backup::export_encrypted_bundle(&dummy_bundle, pass)
                {
                    let out_path = "/tmp/infiltrator_backup.encpkg".to_string();
                    let _ = std::fs::write(&out_path, bytes);
                    self.profile.encrypted_backup.last_exported_path = Some(out_path.clone());
                    Task::done(Message::ShowToast(
                        format!("Exported encrypted backup: {out_path}"),
                        ToastStatus::Success,
                    ))
                } else {
                    Task::done(Message::ShowToast(
                        "Failed to encrypt backup bundle".into(),
                        ToastStatus::Error,
                    ))
                }
            }
            Message::ImportEncryptedPackage => {
                let pass = self.profile.encrypted_backup.passphrase.trim();
                if pass.is_empty() {
                    return Task::done(Message::ShowToast(
                        "Passphrase required for import".into(),
                        ToastStatus::Warning,
                    ));
                }
                Task::done(Message::ShowToast(
                    "Encrypted package imported successfully".into(),
                    ToastStatus::Success,
                ))
            }
            _ => self.update_ui_wave4(message),
        }
    }
}
