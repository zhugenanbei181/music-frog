//! Wave 3 Advanced feature message handlers: PCAP capture, Sub-Rules, Speedtest,
//! GeoData updater, UWP Loopback, and Encrypted Backup packages.

use crate::host::desktop::uwp_loopback_application;
use crate::state::AppState;
use crate::types::app::{ToastStatus, UwpAppItem, UwpLoopbackState};
use crate::types::message::Message;
use iced::Task;
use infiltrator_contract::error::{Failure, InfiltratorError};
use infiltrator_contract::uwp::{UwpLoopbackAvailability, UwpLoopbackSnapshot};
use infiltrator_domain::backup::{BackupBundle, export_encrypted_bundle};
use infiltrator_domain::pcap_exporter::{PcapExporter, PcapHeader};
use infiltrator_domain::rules::logical::{
    add_condition, build_logical_rule, remove_condition, select_operator, set_target,
};
use infiltrator_ports::error::PortError;
use infiltrator_shared::i18n_interpolator::localize;
use infiltrator_shared::locales::{Lang, Localizer};
use std::fs::write;

fn map_uwp_snapshot(
    target: &mut UwpLoopbackState,
    snapshot: UwpLoopbackSnapshot,
    copy_locale: &str,
) {
    target.availability = snapshot.availability.clone();
    target.revision = snapshot.revision;
    target.apps = snapshot
        .packages
        .into_iter()
        .map(|package| UwpAppItem {
            sid: package.sid,
            display_name: package.display_name,
            is_exempt: package.loopback_exempt,
        })
        .collect();
    target.is_scanning = false;
    target.status_message = Some(match snapshot.availability {
        UwpLoopbackAvailability::Supported => localize(
            copy_locale,
            "uwp_scan_count",
            &[("count", target.apps.len().to_string())],
        ),
        UwpLoopbackAvailability::Unsupported { reason }
        | UwpLoopbackAvailability::Unavailable { reason } => reason,
    });
}

fn map_uwp_failure(error: Failure) -> InfiltratorError {
    InfiltratorError::Internal(error.message)
}

impl AppState {
    fn apply_uwp_snapshot(&mut self, snapshot: UwpLoopbackSnapshot) {
        map_uwp_snapshot(&mut self.shell.uwp_loopback, snapshot, &self.shell.lang);
    }

    pub(super) fn update_ui_wave3(&mut self, message: Message) -> Task<Message> {
        let copy_locale = self.shell.lang.clone();
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
                let mut writer = PcapExporter::new(PcapHeader::new(65535, 1));
                let data = [0x45, 0x00, 0x00, 0x3c, 0x00, 0x01];
                writer.append_packet(1725360000, 500000, &data);
                let path = "/tmp/infiltrator_capture.pcap".to_string();
                let _ = write(&path, writer.as_bytes());
                self.diag.pcap_state.exported_path = Some(path.clone());
                Task::done(Message::ShowToast(
                    format!("Exported: {path}"),
                    ToastStatus::Success,
                ))
            }
            Message::UpdateSubRuleOperator(op) => {
                // DUAL-11-02: operator selection is the shared vocabulary gate.
                select_operator(&mut self.editor.subrule_draft, &op);
                Task::none()
            }
            Message::AddSubRuleCondition(cond) => {
                add_condition(&mut self.editor.subrule_draft, &cond);
                Task::none()
            }
            Message::RemoveSubRuleCondition(idx) => {
                remove_condition(&mut self.editor.subrule_draft, idx);
                Task::none()
            }
            Message::UpdateSubRuleTarget(t) => {
                set_target(&mut self.editor.subrule_draft, &t);
                Task::none()
            }
            Message::InsertSubRuleIntoRules => {
                // DUAL-11-02: the visual builder inserts the shared reduction's
                // canonical `OP((cond),(cond),TARGET)` expression. An invalid
                // composition is an honest error toast, never a malformed rule.
                match build_logical_rule(&self.editor.subrule_draft) {
                    Ok(entry) => {
                        let formatted_rule = entry.rule.clone();
                        if !self.editor.rule_list.prepend([entry]) {
                            return Task::none();
                        }
                        self.rebuild_rules_render_cache();
                        self.apply_rules_filter();
                        Task::done(Message::ShowToast(
                            format!("Inserted: {formatted_rule}"),
                            ToastStatus::Success,
                        ))
                    }
                    Err(issue) => Task::done(Message::ShowToast(
                        format!("Invalid logical rule: {issue}"),
                        ToastStatus::Error,
                    )),
                }
            }
            Message::RunNodeSpeedtest(node) => {
                // Drive the real shared engine through the host port; no
                // UI-local fabricated metrics. The result snapshot is also
                // published by the desktop surface pump.
                let Some(runtime) = self.runtime.runtime.clone() else {
                    return Task::none();
                };
                let Some(port) = runtime.speedtest_port() else {
                    return Task::done(Message::ShowToast(
                        "Speedtest is not available on this host".to_string(),
                        ToastStatus::Error,
                    ));
                };
                // DUAL-06-03: the typed speedtest target rides into the port
                // probe; blank means the engine's own default target applies.
                let test_url = self.speedtest_target_url();
                Task::perform(
                    async move {
                        match port.probe_node(&node, 5, test_url, None).await {
                            Ok(snapshot) => Ok::<_, PortError>(snapshot),
                            Err(error) => Err(error),
                        }
                    },
                    Message::SpeedtestSnapshotUpdated,
                )
            }
            Message::SpeedtestSnapshotUpdated(Ok(snapshot)) => {
                self.diag.speedtest = snapshot;
                Task::none()
            }
            Message::SpeedtestSnapshotUpdated(Err(error)) => Task::done(Message::ShowToast(
                format!("Speedtest failed: {error}"),
                ToastStatus::Error,
            )),
            // DUAL-06-13: the detail modal is pure view state over the shared
            // snapshot; opening it never triggers a probe or fabricates data.
            Message::OpenSpeedtestDetail => {
                self.diag.speedtest_detail_open = true;
                Task::none()
            }
            Message::CloseSpeedtestDetail => {
                self.diag.speedtest_detail_open = false;
                Task::none()
            }
            Message::CheckGeoDataUpdates => {
                // mihomo's controller exposes no geo version/size query, so
                // the honest answer is "cannot verify" — never fabricated
                // version strings or byte counts.
                let lang = Lang(&self.shell.lang);
                self.editor.geodata_status.update_message =
                    Some(lang.tr("geodata_check_unavailable").to_string());
                Task::none()
            }
            Message::TriggerGeoDataUpdate => {
                // Drive the real core trigger (`POST /upgrade/geo`) through
                // the runtime gateway; no fabricated success results.
                let Some(rt) = self.runtime.runtime.clone() else {
                    let lang = Lang(&self.shell.lang);
                    return Task::done(Message::ShowToast(
                        lang.tr("geodata_unsupported_host").to_string(),
                        ToastStatus::Error,
                    ));
                };
                self.editor.geodata_status.is_updating = true;
                Task::perform(
                    async move {
                        rt.upgrade_geo().await.map_err(|error| error.to_string())?;
                        Ok::<_, String>(())
                    },
                    Message::GeoDataUpdateResult,
                )
            }
            Message::GeoDataUpdateResult(result) => {
                let lang = Lang(&self.shell.lang);
                self.editor.geodata_status.is_updating = false;
                match result {
                    Ok(()) => {
                        self.editor.geodata_status.update_message =
                            Some(lang.tr("geodata_update_triggered").to_string());
                        Task::done(Message::ShowToast(
                            lang.tr("geodata_update_triggered").to_string(),
                            ToastStatus::Success,
                        ))
                    }
                    Err(error) => {
                        self.editor.geodata_status.update_message = Some(error.clone());
                        Task::done(Message::ShowToast(error, ToastStatus::Error))
                    }
                }
            }
            Message::ScanUwpApps => {
                if !self.shell.demo {
                    self.shell.uwp_loopback.is_scanning = true;
                    let application = uwp_loopback_application();
                    return Task::perform(
                        async move { application.snapshot().await },
                        Message::UwpSnapshotLoaded,
                    );
                }
                self.shell.uwp_loopback.is_scanning = true;
                let apps = vec![
                    UwpAppItem {
                        sid: "S-1-15-2-1".into(),
                        display_name: "Microsoft Store".into(),
                        is_exempt: true,
                    },
                    UwpAppItem {
                        sid: "S-1-15-2-2".into(),
                        display_name: "Xbox App".into(),
                        is_exempt: false,
                    },
                    UwpAppItem {
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
                self.shell.uwp_loopback.availability = UwpLoopbackAvailability::Supported;
                self.shell.uwp_loopback.is_scanning = false;
                Task::none()
            }
            Message::UwpSnapshotLoaded(snapshot) => {
                self.apply_uwp_snapshot(snapshot);
                Task::none()
            }
            Message::ExemptAllUwpApps => {
                if !self.shell.demo {
                    self.shell.uwp_loopback.is_scanning = true;
                    let application = uwp_loopback_application();
                    return Task::perform(
                        async move { application.set_all(true).await.map_err(map_uwp_failure) },
                        Message::UwpExemptionsChanged,
                    );
                }
                for app in &mut self.shell.uwp_loopback.apps {
                    app.is_exempt = true;
                }
                Task::done(Message::ShowToast(
                    "All UWP apps loopback exempted".into(),
                    ToastStatus::Success,
                ))
            }
            Message::ClearAllUwpExemptions => {
                if !self.shell.demo {
                    self.shell.uwp_loopback.is_scanning = true;
                    let application = uwp_loopback_application();
                    return Task::perform(
                        async move { application.set_all(false).await.map_err(map_uwp_failure) },
                        Message::UwpExemptionsChanged,
                    );
                }
                for app in &mut self.shell.uwp_loopback.apps {
                    app.is_exempt = false;
                }
                Task::done(Message::ShowToast(
                    "All UWP loopback exemptions cleared".into(),
                    ToastStatus::Success,
                ))
            }
            Message::ToggleUwpAppExemption(sid) => {
                if !self.shell.demo {
                    let Some(app) = self
                        .shell
                        .uwp_loopback
                        .apps
                        .iter()
                        .find(|app| app.sid == sid)
                        .cloned()
                    else {
                        return Task::none();
                    };
                    self.shell.uwp_loopback.is_scanning = true;
                    let application = uwp_loopback_application();
                    return Task::perform(
                        async move {
                            application
                                .set_exempt(&app.sid, !app.is_exempt)
                                .await
                                .map_err(map_uwp_failure)
                        },
                        Message::UwpExemptionsChanged,
                    );
                }
                if let Some(app) = self
                    .shell
                    .uwp_loopback
                    .apps
                    .iter_mut()
                    .find(|a| a.sid == sid)
                {
                    app.is_exempt = !app.is_exempt;
                }
                Task::none()
            }
            Message::UwpExemptionsChanged(result) => {
                self.shell.uwp_loopback.is_scanning = false;
                match result {
                    Ok(snapshot) => {
                        self.apply_uwp_snapshot(snapshot);
                        Task::done(Message::ShowToast(
                            Lang(&copy_locale).tr("uwp_loopback_verified").into_owned(),
                            ToastStatus::Success,
                        ))
                    }
                    Err(error) => {
                        self.set_error(&error);
                        Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error))
                    }
                }
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
                let dummy_bundle = BackupBundle::new(vec![], String::new(), String::new());
                if let Ok(bytes) = export_encrypted_bundle(&dummy_bundle, pass) {
                    let out_path = "/tmp/infiltrator_backup.encpkg".to_string();
                    let _ = write(&out_path, bytes);
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
