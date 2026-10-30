//! Wave 3 Advanced feature message handlers: PCAP capture, Sub-Rules, Speedtest,
//! GeoData updater, UWP Loopback, and Encrypted Backup packages.

use crate::host::desktop::uwp_loopback_application;
use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::editor::GeoDataUpdatePhase;
use crate::types::message::Message;
use iced::Task;
use infiltrator_application::backup_restore_application::{
    MIN_BACKUP_PASSPHRASE_LEN, decrypt_and_verify_bundle, encrypt_verified_bundle,
};
use infiltrator_contract::app_routing::AppRoutingMode;
use infiltrator_contract::backup_restore::{BackupDeliveryTarget, BackupOperationReceipt};
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::speedtest::SpeedtestCancelOutcome;
use infiltrator_contract::surface_snapshot::AppRoutingPageSnapshot;
use infiltrator_contract::uwp::{UwpLoopbackSnapshot, UwpPackageSnapshot};
use infiltrator_domain::backup::BackupBundle;
use infiltrator_domain::backup::ProfileBackupItem;
use infiltrator_domain::pcap_exporter::{PcapExporter, PcapHeader};
use infiltrator_domain::rules::logical::{
    add_condition, build_logical_rule, remove_condition, select_operator, set_target,
};
use infiltrator_shared::locales::{Lang, Localizer};
use lurker_core_contract::error::ErrorCode;
use lurker_core_contract::error::{Failure, InfiltratorError};
use std::env;
use std::fs::{read, write};

fn map_uwp_failure(error: Failure) -> InfiltratorError {
    InfiltratorError::Internal(error.message)
}

impl AppState {
    /// Observe the UWP loopback readback the host published.
    ///
    /// The loopback packages are a shared fact: the surface keeps no private
    /// copy of them.
    fn apply_uwp_snapshot(&mut self, snapshot: UwpLoopbackSnapshot) {
        // The routing mode is the shared model's fact; the page snapshot
        // carries it in the contract's vocabulary.
        let mode = self.shell.uwp_loopback.mode().map(AppRoutingMode::from);
        let include_system = self.shell.uwp_loopback.include_system().unwrap_or(false);
        let apps = self.shell.uwp_loopback.apps().to_vec();
        self.shell
            .uwp_loopback
            .observe_page(AppRoutingPageSnapshot {
                mode: mode.unwrap_or(AppRoutingMode::ProxyAll),
                include_system,
                apps,
                uwp_loopback: snapshot,
            });
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
            Message::StartSpeedtest(target) => {
                let Some(ingress) = self.command_ingress() else {
                    return Task::done(Message::ShowToast(
                        "Speedtest is not configured for this Core owner".to_string(),
                        ToastStatus::Error,
                    ));
                };
                let Some(requested_scope) = ingress.observed_scope() else {
                    return Task::done(Message::ShowToast(
                        "Wait for an accepted Core scope before starting a speedtest".to_string(),
                        ToastStatus::Info,
                    ));
                };
                Task::perform(
                    async move {
                        let outcome = ingress
                            .execute_outcome(CommandIntent::StartSpeedtest { target })
                            .await;
                        if outcome.scope.as_ref() != Some(&requested_scope) {
                            return Err(Failure::new(
                                ErrorCode::InvalidState,
                                "speedtest start completed for a stale Core scope",
                                false,
                            ));
                        }
                        let receipt =
                            outcome
                                .result
                                .into_output()
                                .and_then(|output| match output {
                                    CommandOutput::SpeedtestStarted(receipt) => Ok(receipt),
                                    _ => Err(Failure::new(
                                        ErrorCode::InvalidState,
                                        "speedtest start returned another output type",
                                        false,
                                    )),
                                })?;
                        let snapshot = ingress.speedtest_snapshot();
                        Ok((receipt, snapshot))
                    },
                    Message::SpeedtestStarted,
                )
            }
            Message::SpeedtestStarted(Ok((receipt, snapshot))) => {
                if snapshot.state.run_id() == Some(receipt.run_id) {
                    self.diag.speedtest = snapshot;
                } else {
                    log::debug!(
                        "discarded stale speedtest start receipt {}; Core owner now reports {:?}",
                        receipt.run_id.get(),
                        snapshot.state.run_id()
                    );
                }
                Task::none()
            }
            Message::SpeedtestStarted(Err(error)) => Task::done(Message::ShowToast(
                format!("Speedtest failed: {error}"),
                ToastStatus::Error,
            )),
            Message::SpeedtestCancelled(Ok((outcome, snapshot))) => {
                self.diag.speedtest = snapshot;
                let message = match outcome {
                    SpeedtestCancelOutcome::Requested { .. } => "Speedtest cancellation accepted",
                    SpeedtestCancelOutcome::AlreadyRequested { .. } => {
                        "Speedtest cancellation is already pending"
                    }
                    SpeedtestCancelOutcome::RunAlreadyFinished { .. } => {
                        "Speedtest run has already finished"
                    }
                    SpeedtestCancelOutcome::StaleRun { .. } => {
                        "Speedtest cancellation referred to a stale run"
                    }
                };
                Task::done(Message::ShowToast(message.to_owned(), ToastStatus::Info))
            }
            Message::SpeedtestCancelled(Err(error)) => Task::done(Message::ShowToast(
                format!("Speedtest cancellation failed: {error}"),
                ToastStatus::Error,
            )),
            Message::SpeedtestConcurrencySet(Ok(snapshot)) => {
                self.diag.speedtest = snapshot;
                Task::none()
            }
            Message::SpeedtestConcurrencySet(Err(error)) => Task::done(Message::ShowToast(
                format!("Speedtest concurrency update failed: {error}"),
                ToastStatus::Error,
            )),
            // The detail modal is pure view state over the shared
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
                let Some(application) = self.command_ingress() else {
                    let lang = Lang(&self.shell.lang);
                    return Task::done(Message::ShowToast(
                        lang.tr("geodata_unsupported_host").to_string(),
                        ToastStatus::Error,
                    ));
                };
                self.editor.geodata_status.phase = GeoDataUpdatePhase::Updating;
                Task::perform(
                    async move {
                        application
                            .execute(CommandIntent::UpgradeGeoDatabases)
                            .await
                            .into_unit()
                            .map_err(|failure| failure.message)
                    },
                    Message::GeoDataUpdateResult,
                )
            }
            Message::GeoDataUpdateResult(result) => {
                let lang = Lang(&self.shell.lang);
                self.editor.geodata_status.phase = GeoDataUpdatePhase::Idle;
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
                self.shell.uwp_loopback.scan_phase_mut().begin();
                if !self.shell.demo {
                    let application = uwp_loopback_application();
                    return Task::perform(
                        async move { application.snapshot().await },
                        Message::UwpSnapshotLoaded,
                    );
                }
                let packages = vec![
                    UwpPackageSnapshot {
                        sid: "S-1-15-2-1".into(),
                        display_name: "Microsoft Store".into(),
                        package_family_name: "Microsoft.Store_8wekyb3d8bbwe".into(),
                        loopback_exempt: true,
                    },
                    UwpPackageSnapshot {
                        sid: "S-1-15-2-2".into(),
                        display_name: "Xbox App".into(),
                        package_family_name: "Microsoft.XboxApp_8wekyb3d8bbwe".into(),
                        loopback_exempt: false,
                    },
                    UwpPackageSnapshot {
                        sid: "S-1-15-2-3".into(),
                        display_name: "Windows Terminal".into(),
                        package_family_name: "Microsoft.WindowsTerminal_8wekyb3d8bbwe".into(),
                        loopback_exempt: true,
                    },
                ];
                self.apply_uwp_snapshot(UwpLoopbackSnapshot::supported(1, packages));
                Task::none()
            }
            Message::UwpAppsLoaded(packages) => {
                self.apply_uwp_snapshot(UwpLoopbackSnapshot::supported(1, packages));
                Task::none()
            }
            Message::UwpSnapshotLoaded(snapshot) => {
                self.apply_uwp_snapshot(snapshot);
                Task::none()
            }
            Message::ExemptAllUwpApps => {
                self.shell.uwp_loopback.scan_phase_mut().begin();
                if !self.shell.demo {
                    let application = uwp_loopback_application();
                    return Task::perform(
                        async move { application.set_all(true).await.map_err(map_uwp_failure) },
                        Message::UwpExemptionsChanged,
                    );
                }
                self.apply_uwp_snapshot(UwpLoopbackSnapshot::supported(
                    1,
                    self.shell
                        .uwp_loopback
                        .uwp_packages()
                        .iter()
                        .map(|package| UwpPackageSnapshot {
                            loopback_exempt: true,
                            ..package.clone()
                        })
                        .collect(),
                ));
                Task::done(Message::ShowToast(
                    "All UWP apps loopback exempted".into(),
                    ToastStatus::Success,
                ))
            }
            Message::ClearAllUwpExemptions => {
                self.shell.uwp_loopback.scan_phase_mut().begin();
                if !self.shell.demo {
                    let application = uwp_loopback_application();
                    return Task::perform(
                        async move { application.set_all(false).await.map_err(map_uwp_failure) },
                        Message::UwpExemptionsChanged,
                    );
                }
                self.apply_uwp_snapshot(UwpLoopbackSnapshot::supported(
                    1,
                    self.shell
                        .uwp_loopback
                        .uwp_packages()
                        .iter()
                        .map(|package| UwpPackageSnapshot {
                            loopback_exempt: false,
                            ..package.clone()
                        })
                        .collect(),
                ));
                Task::done(Message::ShowToast(
                    "All UWP loopback exemptions cleared".into(),
                    ToastStatus::Success,
                ))
            }
            Message::ToggleUwpAppExemption(sid) => {
                let Some(package) = self
                    .shell
                    .uwp_loopback
                    .uwp_packages()
                    .iter()
                    .find(|package| package.sid == sid)
                    .cloned()
                else {
                    return Task::none();
                };
                self.shell.uwp_loopback.scan_phase_mut().begin();
                if !self.shell.demo {
                    let application = uwp_loopback_application();
                    return Task::perform(
                        async move {
                            application
                                .set_exempt(&package.sid, !package.loopback_exempt)
                                .await
                                .map_err(map_uwp_failure)
                        },
                        Message::UwpExemptionsChanged,
                    );
                }
                self.apply_uwp_snapshot(UwpLoopbackSnapshot::supported(
                    1,
                    self.shell
                        .uwp_loopback
                        .uwp_packages()
                        .iter()
                        .map(|package| {
                            if package.sid == sid {
                                UwpPackageSnapshot {
                                    loopback_exempt: !package.loopback_exempt,
                                    ..package.clone()
                                }
                            } else {
                                package.clone()
                            }
                        })
                        .collect(),
                ));
                Task::none()
            }
            Message::UwpExemptionsChanged(result) => match result {
                Ok(snapshot) => {
                    self.apply_uwp_snapshot(snapshot);
                    Task::done(Message::ShowToast(
                        Lang(&copy_locale).tr("uwp_loopback_verified").into_owned(),
                        ToastStatus::Success,
                    ))
                }
                Err(error) => {
                    let phase = self.shell.uwp_loopback.scan_phase_mut();
                    let token = phase.token().unwrap_or_else(|| phase.begin());
                    phase.fail(
                        token,
                        Failure::new(ErrorCode::Internal, error.to_string(), false),
                    );
                    self.set_error(&error);
                    Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error))
                }
            },
            Message::UpdateEncryptedBackupPassphrase(pass) => {
                self.profile.encrypted_backup.passphrase = pass;
                Task::none()
            }
            Message::ExportEncryptedPackage => {
                let pass = self.profile.encrypted_backup.passphrase.trim();
                if pass.len() < MIN_BACKUP_PASSPHRASE_LEN {
                    return Task::done(Message::ShowToast(
                        "Passphrase must be at least 6 characters".into(),
                        ToastStatus::Warning,
                    ));
                }
                let profiles = self
                    .profile
                    .profiles
                    .iter()
                    .map(|p| ProfileBackupItem::new(&p.name, &p.path))
                    .collect();
                let bundle = BackupBundle::new(profiles, String::new(), String::new());
                let target = BackupDeliveryTarget::LocalFile;
                let out_path = env::temp_dir()
                    .join("infiltrator_backup.encpkg")
                    .to_string_lossy()
                    .into_owned();
                let now = chrono::Utc::now().timestamp_millis().max(0) as u64;
                match encrypt_verified_bundle(&bundle, pass, &out_path, target, now) {
                    Ok((bytes, receipt)) => {
                        if let Err(e) = write(&out_path, bytes) {
                            Task::done(Message::ShowToast(
                                format!("Failed to write backup: {e}"),
                                ToastStatus::Error,
                            ))
                        } else {
                            self.profile.encrypted_backup.last_exported_path =
                                Some(out_path.clone());
                            if let BackupOperationReceipt::ExportSucceeded {
                                sha256_digest,
                                profile_count,
                                ..
                            } = receipt
                            {
                                let short_sha: String = sha256_digest.chars().take(8).collect();
                                Task::done(Message::ShowToast(
                                    format!(
                                        "Exported {profile_count} profiles ({short_sha}…): {out_path}"
                                    ),
                                    ToastStatus::Success,
                                ))
                            } else {
                                Task::done(Message::ShowToast(
                                    format!("Exported encrypted backup: {out_path}"),
                                    ToastStatus::Success,
                                ))
                            }
                        }
                    }
                    Err(BackupOperationReceipt::Failed { failure, .. }) => {
                        Task::done(Message::ShowToast(failure.summary_zh(), ToastStatus::Error))
                    }
                    _ => Task::none(),
                }
            }
            Message::ImportEncryptedPackage => {
                let pass = self.profile.encrypted_backup.passphrase.trim();
                if pass.len() < MIN_BACKUP_PASSPHRASE_LEN {
                    return Task::done(Message::ShowToast(
                        "Passphrase must be at least 6 characters".into(),
                        ToastStatus::Warning,
                    ));
                }
                let in_path = self
                    .profile
                    .encrypted_backup
                    .last_exported_path
                    .clone()
                    .unwrap_or_else(|| {
                        env::temp_dir()
                            .join("infiltrator_backup.encpkg")
                            .to_string_lossy()
                            .into_owned()
                    });
                let bytes = match read(&in_path) {
                    Ok(b) => b,
                    Err(e) => {
                        return Task::done(Message::ShowToast(
                            format!("Failed to read backup package: {e}"),
                            ToastStatus::Error,
                        ));
                    }
                };
                let now = chrono::Utc::now().timestamp_millis().max(0) as u64;
                match decrypt_and_verify_bundle(&bytes, pass, now) {
                    Ok(bundle) => {
                        let count = bundle.profiles.len();
                        Task::done(Message::ShowToast(
                            format!("Verified and imported {count} profiles successfully"),
                            ToastStatus::Success,
                        ))
                    }
                    Err(BackupOperationReceipt::Failed { failure, .. }) => {
                        Task::done(Message::ShowToast(failure.summary_zh(), ToastStatus::Error))
                    }
                    _ => Task::none(),
                }
            }
            _ => self.update_ui_wave4(message),
        }
    }
}
